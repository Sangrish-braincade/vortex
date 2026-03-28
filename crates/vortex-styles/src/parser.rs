//! Style TOML schema and parser.

use serde::{Deserialize, Serialize};

/// A complete style template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Style {
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub cuts: CutSettings,
    pub velocity: VelocitySettings,
    pub effects: EffectSettings,
    pub color: ColorSettings,
    pub audio: AudioSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CutSettings {
    /// Target cuts per minute.
    pub cuts_per_minute: f64,
    /// Snap cuts to beat markers if within this many seconds.
    pub beat_snap_tolerance_secs: f64,
    /// Minimum clip duration in seconds.
    pub min_clip_secs: f64,
    /// Maximum clip duration in seconds.
    pub max_clip_secs: f64,
    /// Prefer cut on: "beat", "kill", "scene", "hybrid".
    pub cut_trigger: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VelocitySettings {
    /// Enable velocity ramp effects at kill moments.
    pub enabled: bool,
    /// Slowest speed during ramp (e.g. 0.15).
    pub min_speed: f64,
    /// Ramp-in duration in seconds.
    pub ramp_in_secs: f64,
    /// Ramp-out duration in seconds.
    pub ramp_out_secs: f64,
    /// Probability of applying velocity ramp per kill moment (0–1).
    pub probability: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectSettings {
    /// Enable zoom punch on kill moments.
    pub zoom_on_kill: bool,
    /// Zoom scale factor.
    pub zoom_scale: f64,
    /// Enable camera shake on impacts.
    pub shake_on_impact: bool,
    /// Shake intensity.
    pub shake_intensity: f64,
    /// Enable white flash on beat drops.
    pub flash_on_beat: bool,
    /// Flash opacity.
    pub flash_opacity: f64,
    /// Enable chromatic aberration.
    pub chromatic_aberration: bool,
    /// Chromatic aberration strength.
    pub chromatic_strength: f64,
    /// Add letterbox bars.
    pub letterbox: bool,
    /// Letterbox aspect ratio.
    pub letterbox_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorSettings {
    /// Optional LUT file path relative to styles/ directory.
    pub lut: Option<String>,
    /// LUT blend strength (0–1).
    pub lut_strength: f64,
    /// Saturation multiplier.
    pub saturation: f64,
    /// Contrast multiplier.
    pub contrast: f64,
    /// Brightness offset (-1 to +1).
    pub brightness: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSettings {
    /// Music volume (0–1).
    pub music_volume: f64,
    /// Gameplay audio volume (0–1).
    pub gameplay_volume: f64,
    /// Fade-in duration in seconds.
    pub fade_in_secs: f64,
    /// Fade-out duration in seconds.
    pub fade_out_secs: f64,
    /// Duck gameplay audio under music by this many dB.
    pub sidechain_db: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_aggressive_style() {
        let src = include_str!("../../../styles/aggressive.toml");
        let style: Style = toml::from_str(src).unwrap();
        assert_eq!(style.name, "aggressive");
        assert!(style.velocity.enabled);
    }

    #[test]
    fn parse_chill_style() {
        let src = include_str!("../../../styles/chill.toml");
        let style: Style = toml::from_str(src).unwrap();
        assert_eq!(style.name, "chill");
        assert!(!style.effects.shake_on_impact);
    }
}
