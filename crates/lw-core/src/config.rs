use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use crate::error::LwError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    pub wallpaper_dir: PathBuf,
    pub shuffle: bool,
    pub transition_default: TransitionConfig,
    pub scheduler: SchedulerConfig,
    pub position: WallpaperPosition,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransitionConfig {
    pub effect_type: String,
    pub duration_ms: u32,
    pub easing: EasingType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SchedulerConfig {
    pub enabled: bool,
    pub interval_mins: u32,
    pub change_on_startup: bool,
    pub run_on_startup: bool,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EasingType {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WallpaperPosition {
    Fill,
    Fit,
    Stretch,
    Tile,
    Center,
    Span,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            wallpaper_dir: PathBuf::new(),
            shuffle: false,
            transition_default: TransitionConfig::default(),
            scheduler: SchedulerConfig::default(),
            position: WallpaperPosition::Fill,
        }
    }
}

impl Default for TransitionConfig {
    fn default() -> Self {
        Self {
            effect_type: "fade".to_string(),
            duration_ms: 1000,
            easing: EasingType::EaseInOut,
        }
    }
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_mins: 15,
            change_on_startup: true,
            run_on_startup: false,
        }
    }
}

impl Config {
    pub fn validate(&self) -> Result<(), LwError> {
        // Validation check for wallpaper directory path
        // We only check if it is non-empty. If it's empty/default, we might allow it (e.g. before initial configuration is set)
        // but if it is set, it must exist.
        if !self.wallpaper_dir.as_os_str().is_empty() && !self.wallpaper_dir.exists() {
            return Err(LwError::Config(format!(
                "wallpaper_dir does not exist: {}",
                self.wallpaper_dir.display()
            )));
        }
        self.transition_default.validate()?;
        self.scheduler.validate()?;
        Ok(())
    }

    pub fn load_from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, LwError> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)
            .map_err(|e| LwError::Serialization(format!("Failed to deserialize config TOML: {e}")))?;
        config.validate()?;
        Ok(config)
    }

    pub fn save_to_file<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), LwError> {
        self.validate()?;
        let content = toml::to_string_pretty(self)
            .map_err(|e| LwError::Serialization(format!("Failed to serialize config TOML: {e}")))?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

impl TransitionConfig {
    pub fn validate(&self) -> Result<(), LwError> {
        if self.duration_ms < 100 || self.duration_ms > 10000 {
            return Err(LwError::Config(format!(
                "duration_ms must be between 100 and 10000, got {}",
                self.duration_ms
            )));
        }
        Ok(())
    }
}

impl SchedulerConfig {
    pub fn validate(&self) -> Result<(), LwError> {
        if self.interval_mins < 1 {
            return Err(LwError::Config(format!(
                "interval_mins must be greater than or equal to 1, got {}",
                self.interval_mins
            )));
        }
        Ok(())
    }
}
