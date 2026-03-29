//! Media probing via `ffprobe`.
//!
//! Lightweight wrapper that runs `ffprobe -v quiet -show_entries format=duration
//! -of default=noprint_wrappers=1:nokey=1` to read media duration without
//! decoding any frames.

use std::process::Stdio;

/// Probe a media file and return its duration in seconds.
///
/// Returns `None` if ffprobe is not available, the file doesn't exist,
/// or duration cannot be determined (e.g. some streaming formats).
pub async fn probe_duration(path: &str) -> Option<f64> {
    let output = tokio::process::Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            path,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.trim().parse::<f64>().ok()
}

/// Probe basic metadata for a media file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MediaInfo {
    pub duration_secs: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub codec: String,
}

/// Probe video metadata (duration, resolution, fps, codec) via ffprobe.
pub async fn probe_video(path: &str) -> crate::Result<MediaInfo> {
    let output = tokio::process::Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_streams",
            "-show_format",
            path,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .map_err(|e| crate::AnalysisError::FfmpegProbe(format!("ffprobe not found: {e}")))?;

    if !output.status.success() {
        return Err(crate::AnalysisError::FfmpegProbe(format!(
            "ffprobe failed on '{path}'"
        )));
    }

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| {
            crate::AnalysisError::FfmpegProbe(format!("ffprobe JSON parse error: {e}"))
        })?;

    // Duration from format
    let duration_secs = json["format"]["duration"]
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    // Find the first video stream
    let streams = json["streams"].as_array();
    let video = streams
        .and_then(|arr| arr.iter().find(|s| s["codec_type"] == "video"));

    let (width, height, fps, codec) = if let Some(v) = video {
        let w = v["width"].as_u64().unwrap_or(0) as u32;
        let h = v["height"].as_u64().unwrap_or(0) as u32;
        let fps = parse_rational(v["r_frame_rate"].as_str().unwrap_or("30/1"));
        let c = v["codec_name"].as_str().unwrap_or("unknown").to_string();
        (w, h, fps, c)
    } else {
        (0, 0, 0.0, "unknown".into())
    };

    Ok(MediaInfo { duration_secs, width, height, fps, codec })
}

/// Parse an FFprobe rational string like "30000/1001" or "60/1" → f64.
fn parse_rational(s: &str) -> f64 {
    if let Some((num, den)) = s.split_once('/') {
        let n: f64 = num.parse().unwrap_or(0.0);
        let d: f64 = den.parse().unwrap_or(1.0);
        if d != 0.0 { n / d } else { 0.0 }
    } else {
        s.parse().unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rational_values() {
        assert!((parse_rational("30000/1001") - 29.97).abs() < 0.01);
        assert!((parse_rational("60/1") - 60.0).abs() < 0.001);
        assert!((parse_rational("25") - 25.0).abs() < 0.001);
    }
}
