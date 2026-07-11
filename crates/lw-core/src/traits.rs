use crate::error::Result;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonitorRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

pub trait WallpaperManager: Send + Sync {
    /// Gets the current wallpaper path.
    fn get_current_wallpaper(&self) -> Result<PathBuf>;

    /// Sets the wallpaper path natively via Win32.
    fn set_wallpaper(&self, path: &Path) -> Result<()>;

    /// Updates the wallpaper path in the registry without notifying Explorer visually.
    fn set_wallpaper_registry_only(&self, path: &Path) -> Result<()>;

    /// Gets the bounds of all active monitors.
    fn get_monitor_rects(&self) -> Result<Vec<MonitorRect>>;
}

pub trait TransitionRenderer: Send + Sync {
    /// Renders a transition from an old wallpaper image to a new wallpaper image.
    fn render_transition(
        &self,
        from_image: &Path,
        to_image: &Path,
        duration_ms: u32,
        effect_type: &str,
    ) -> Result<()>;
}
