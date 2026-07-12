#![allow(clippy::field_reassign_with_default)]

use lw_core::{Config, EasingDirection, EasingStyle, WallpaperPosition};
use std::path::PathBuf;

#[test]
fn test_default_config() {
    let config = Config::default();
    assert!(!config.shuffle);
    assert_eq!(config.transition_default.effect_type, "fade");
    assert_eq!(config.transition_default.duration_secs, 1.0);
    assert_eq!(config.transition_default.easing_style, EasingStyle::Quad);
    assert_eq!(config.transition_default.easing_direction, EasingDirection::InOut);
    assert!(config.scheduler.enabled);
    assert_eq!(config.scheduler.interval_mins, 15);
    assert!(config.scheduler.change_on_startup);
    assert_eq!(config.position, WallpaperPosition::Fill);
    assert!(config.validate().is_ok());
}

#[test]
fn test_invalid_transition_duration() {
    let mut config = Config::default();

    // Too short (minimum is 0.1s)
    config.transition_default.duration_secs = 0.09;
    assert!(config.validate().is_err());

    // Too long (maximum is 10.0s)
    config.transition_default.duration_secs = 10.01;
    assert!(config.validate().is_err());

    // Valid boundary values
    config.transition_default.duration_secs = 0.1;
    assert!(config.validate().is_ok());

    config.transition_default.duration_secs = 10.0;
    assert!(config.validate().is_ok());
}

#[test]
fn test_invalid_scheduler_interval() {
    let mut config = Config::default();

    // Invalid (minimum is 1 min)
    config.scheduler.interval_mins = 0;
    assert!(config.validate().is_err());

    // Valid
    config.scheduler.interval_mins = 1;
    assert!(config.validate().is_ok());
}

#[test]
fn test_wallpaper_dir_validation() {
    let mut config = Config::default();

    // A directory that does not exist
    config.wallpaper_dir = PathBuf::from("Z:\\this\\path\\does\\not\\exist\\hopefully");
    assert!(config.validate().is_err());

    // A directory that exists (e.g. system temp directory)
    let temp_dir = std::env::temp_dir();
    config.wallpaper_dir = temp_dir;
    assert!(config.validate().is_ok());
}

#[test]
fn test_toml_serialization_deserialization() {
    let mut config = Config::default();
    config.wallpaper_dir = std::env::temp_dir();
    config.shuffle = true;
    config.transition_default.effect_type = "slide-left".to_string();
    config.transition_default.duration_secs = 2.0;
    config.transition_default.easing_style = EasingStyle::Linear;
    config.transition_default.easing_direction = EasingDirection::In;
    config.scheduler.enabled = false;
    config.scheduler.interval_mins = 30;
    config.scheduler.change_on_startup = false;
    config.position = WallpaperPosition::Fit;

    let toml_str = toml::to_string(&config).expect("Failed to serialize config");
    let deserialized: Config = toml::from_str(&toml_str).expect("Failed to deserialize config");

    assert_eq!(config, deserialized);
}

#[test]
fn test_save_and_load_file() {
    let mut config = Config::default();
    config.wallpaper_dir = std::env::temp_dir();
    config.transition_default.duration_secs = 1.5;

    let temp_file_path = std::env::temp_dir().join("liem_wallpaper_test_config.toml");

    // Save to file
    config.save_to_file(&temp_file_path).expect("Failed to save config to file");

    // Load from file
    let loaded = Config::load_from_file(&temp_file_path).expect("Failed to load config from file");

    assert_eq!(config, loaded);

    // Clean up
    let _ = std::fs::remove_file(temp_file_path);
}
