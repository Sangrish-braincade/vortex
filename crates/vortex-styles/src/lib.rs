//! # vortex-styles
//!
//! Style template system. A style is a TOML file that describes a complete
//! visual and pacing personality for a montage — effect defaults, cut rhythm,
//! color grade, and audio settings.

pub mod parser;
pub use parser::*;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum StyleError {
    #[error("Style not found: {0}")]
    NotFound(String),

    #[error("TOML parse error: {0}")]
    ParseError(#[from] toml::de::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, StyleError>;

/// Registry of loaded styles.
pub struct StyleRegistry {
    styles: Vec<Style>,
}

impl StyleRegistry {
    /// Load the bundled default styles from `styles/`.
    ///
    /// # TODO (Phase 3)
    /// Load from `styles/` directory relative to binary or project root.
    /// Support user-defined styles from `~/.vortex/styles/`.
    pub fn load_default() -> Result<Self> {
        let aggressive = toml::from_str::<Style>(include_str!("../../../styles/aggressive.toml"))?;
        let chill = toml::from_str::<Style>(include_str!("../../../styles/chill.toml"))?;
        let cinematic = toml::from_str::<Style>(include_str!("../../../styles/cinematic.toml"))?;

        Ok(Self {
            styles: vec![aggressive, chill, cinematic],
        })
    }

    /// Return all loaded styles.
    pub fn styles(&self) -> &[Style] {
        &self.styles
    }

    /// Look up a style by name (case-insensitive).
    pub fn get(&self, name: &str) -> Result<&Style> {
        self.styles
            .iter()
            .find(|s| s.name.to_lowercase() == name.to_lowercase())
            .ok_or_else(|| StyleError::NotFound(name.to_string()))
    }
}
