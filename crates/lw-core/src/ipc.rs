use crate::config::Config;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum IpcRequest {
    SetWallpaper { path: PathBuf, transition: Option<TransitionParams> },
    NextWallpaper,
    PrevWallpaper,
    UpdateConfig { config: Config },
    GetStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransitionParams {
    pub effect_type: String,
    pub duration_ms: u32,
    pub easing_style: crate::config::EasingStyle,
    pub easing_direction: crate::config::EasingDirection,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status")]
pub enum IpcResponse {
    Success,
    StatusResponse {
        current_wallpaper: Option<PathBuf>,
        scheduler_active: bool,
        next_change_in_seconds: u32,
    },
    Error {
        message: String,
    },
}
