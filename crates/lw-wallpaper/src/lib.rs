use lw_core::error::LwError;
use lw_core::traits::{MonitorRect, WallpaperManager};
use std::path::{Path, PathBuf};
use windows::core::PCWSTR;
use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_ALL};
use windows::Win32::UI::Shell::{DesktopWallpaper, IDesktopWallpaper};

pub mod monitor;

pub use monitor::{get_monitors, MonitorInfo};

pub struct DesktopWallpaperManager {
    wallpaper: IDesktopWallpaper,
}

unsafe impl Send for DesktopWallpaperManager {}
unsafe impl Sync for DesktopWallpaperManager {}

impl DesktopWallpaperManager {
    /// Creates a new instance of `DesktopWallpaperManager` binding to Windows COM `IDesktopWallpaper`.
    pub fn new() -> Result<Self, LwError> {
        let wallpaper: IDesktopWallpaper = unsafe {
            CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL).map_err(|e| {
                LwError::Wallpaper(format!("Failed to create IDesktopWallpaper instance: {e}"))
            })?
        };
        Ok(Self { wallpaper })
    }

    /// Retrieve bounds of all monitors.
    pub fn get_monitors(&self) -> Result<Vec<MonitorInfo>, LwError> {
        get_monitors(&self.wallpaper)
    }
}

impl WallpaperManager for DesktopWallpaperManager {
    fn get_current_wallpaper(&self) -> Result<PathBuf, LwError> {
        unsafe {
            // Get the first monitor device path (monitor 0)
            let monitor_id = self
                .wallpaper
                .GetMonitorDevicePathAt(0)
                .map_err(|e| LwError::Wallpaper(format!("Failed to get monitor path: {e}")))?;

            let monitor_id_pcwstr = PCWSTR(monitor_id.0);

            // Get the wallpaper image path currently active on this monitor
            let path_pwstr = self
                .wallpaper
                .GetWallpaper(monitor_id_pcwstr)
                .map_err(|e| LwError::Wallpaper(format!("Failed to get wallpaper: {e}")))?;

            let path_str = path_pwstr
                .to_string()
                .map_err(|_| LwError::Wallpaper("Invalid UTF-16 wallpaper path".to_string()));

            // Free COM-allocated strings immediately to prevent leaks
            CoTaskMemFree(Some(monitor_id.0 as *const _));

            let res_path = path_str.map(PathBuf::from);
            CoTaskMemFree(Some(path_pwstr.0 as *const _));

            res_path
        }
    }

    fn set_wallpaper(&self, path: &Path) -> Result<(), LwError> {
        let path_str = path.to_string_lossy();
        let path_w: Vec<u16> = path_str.encode_utf16().chain(std::iter::once(0)).collect();
        let path_pcwstr = PCWSTR(path_w.as_ptr());

        unsafe {
            let monitor_id = self
                .wallpaper
                .GetMonitorDevicePathAt(0)
                .map_err(|e| LwError::Wallpaper(format!("Failed to get monitor path: {e}")))?;

            let monitor_id_pcwstr = PCWSTR(monitor_id.0);

            let res = self
                .wallpaper
                .SetWallpaper(monitor_id_pcwstr, path_pcwstr)
                .map_err(|e| LwError::Wallpaper(format!("Failed to set wallpaper: {e}")));

            CoTaskMemFree(Some(monitor_id.0 as *const _));
            res
        }
    }

    fn get_monitor_rects(&self) -> Result<Vec<MonitorRect>, LwError> {
        let monitors = self.get_monitors()?;
        Ok(monitors
            .into_iter()
            .map(|m| MonitorRect {
                left: m.bounds.left,
                top: m.bounds.top,
                right: m.bounds.right,
                bottom: m.bounds.bottom,
            })
            .collect())
    }
}
