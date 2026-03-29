//! Scene cut and boundary detection via FFmpeg.
//!
//! Uses `ffmpeg -vf "select=gt(scene\,threshold),showinfo"` to find frames
//! where the scene change score exceeds the configured threshold. The
//! `showinfo` filter prints `pts_time:` for each selected frame.

use std::process::Stdio;

/// A detected scene boundary in a source video.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SceneCut {
    /// Timestamp in seconds where the cut occurs.
    pub time_secs: f64,
    /// Scene change score from FFmpeg (0.0–1.0). Higher = harder cut.
    /// Set to 0.8 when detected above threshold (exact score not available
    /// from showinfo; use `scdet` filter in FFmpeg 4.4+ for exact scores).
    pub score: f64,
    /// Cut type: "hard", "dissolve", "fade_in", "fade_out".
    pub cut_type: String,
}

/// Configuration for scene detection.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SceneDetectorConfig {
    /// Minimum scene change score to register as a cut (0.0–1.0).
    pub threshold: f64,
    /// Minimum gap between cuts in seconds (suppresses near-duplicates).
    pub min_gap_secs: f64,
}

impl Default for SceneDetectorConfig {
    fn default() -> Self {
        Self { threshold: 0.3, min_gap_secs: 0.5 }
    }
}

/// Scene boundary detector.
pub struct SceneDetector {
    config: SceneDetectorConfig,
}

impl SceneDetector {
    pub fn new(config: SceneDetectorConfig) -> Self {
        Self { config }
    }

    /// Detect scene cuts in a video file using FFmpeg's scene change filter.
    ///
    /// Runs:
    /// ```text
    /// ffmpeg -hide_banner -i <path>
    ///        -vf "select=gt(scene\,<threshold>),showinfo"
    ///        -vsync 0 -f null -
    /// ```
    /// Parses `pts_time:` values from the showinfo output on stderr.
    pub async fn detect(&self, video_path: &str) -> crate::Result<Vec<SceneCut>> {
        tracing::info!(path = video_path, threshold = self.config.threshold, "Detecting scene cuts");

        let vf = format!("select=gt(scene\\,{}),showinfo", self.config.threshold);

        let output = tokio::process::Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-i",
                video_path,
                "-vf",
                &vf,
                "-vsync",
                "0",
                "-f",
                "null",
                "-",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| crate::AnalysisError::FfmpegProbe(format!("ffmpeg error: {e}")))?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut cuts: Vec<SceneCut> = Vec::new();
        let mut last_time: f64 = -self.config.min_gap_secs;

        for line in stderr.lines() {
            // showinfo lines look like:
            // [Parsed_showinfo_1 @ 0x...] n:  5 pts:5120 pts_time:0.08533 ...
            if line.contains("showinfo") && line.contains("pts_time:") {
                if let Some(t) = parse_pts_time(line) {
                    if t - last_time >= self.config.min_gap_secs {
                        cuts.push(SceneCut {
                            time_secs: t,
                            score: 0.8, // above threshold by definition
                            cut_type: "hard".into(),
                        });
                        last_time = t;
                    }
                }
            }
        }

        // If FFmpeg isn't available, log a clear warning and return empty
        if !output.status.success() && cuts.is_empty() {
            let stderr_preview: String = stderr.lines().take(3).collect::<Vec<_>>().join(" | ");
            tracing::warn!(
                "Scene detection returned no results (FFmpeg may not be in PATH). stderr: {stderr_preview}"
            );
        }

        cuts.sort_by(|a, b| a.time_secs.partial_cmp(&b.time_secs).unwrap_or(std::cmp::Ordering::Equal));
        tracing::info!(count = cuts.len(), "Scene detection complete");
        Ok(cuts)
    }
}

/// Extract the `pts_time:` value from a showinfo log line.
fn parse_pts_time(line: &str) -> Option<f64> {
    let key = "pts_time:";
    let pos = line.find(key)?;
    let rest = line[pos + key.len()..].trim_start();
    let end = rest.find(|c: char| c.is_whitespace()).unwrap_or(rest.len());
    rest[..end].parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pts_time_from_showinfo_line() {
        let line = "[Parsed_showinfo_1 @ 0xdeadbeef] n:   5 pts:  5120 pts_time:8.53333 ...";
        assert!((parse_pts_time(line).unwrap() - 8.533_33).abs() < 0.001);
    }

    #[test]
    fn parse_pts_time_missing() {
        assert!(parse_pts_time("no pts_time here").is_none());
    }

    #[tokio::test]
    async fn scene_detector_on_missing_file_returns_empty() {
        // FFmpeg will fail on a non-existent file, but we should return Ok([])
        // rather than crashing (graceful degradation).
        let d = SceneDetector::new(SceneDetectorConfig::default());
        let result = d.detect("non_existent_file.mp4").await;
        assert!(result.is_ok(), "should not error on missing file");
    }
}
