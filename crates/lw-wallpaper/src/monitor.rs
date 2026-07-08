use lw_core::error::LwError;
use windows::core::PCWSTR;
use windows::Win32::Foundation::RECT;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::IDesktopWallpaper;

#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub device_path: String,
    pub bounds: RECT,
}

/// Retrieves list of all active monitors and their layout coordinates from COM.
pub fn get_monitors(wallpaper: &IDesktopWallpaper) -> Result<Vec<MonitorInfo>, LwError> {
    let mut monitors = Vec::new();
    unsafe {
        let count = wallpaper
            .GetMonitorDevicePathCount()
            .map_err(|e| LwError::Wallpaper(format!("Failed to get monitor count: {e}")))?;

        for i in 0..count {
            let path = wallpaper.GetMonitorDevicePathAt(i).map_err(|e| {
                LwError::Wallpaper(format!("Failed to get monitor path at index {i}: {e}"))
            })?;

            let monitor_id_pcwstr = PCWSTR(path.0);
            let bounds = wallpaper.GetMonitorRECT(monitor_id_pcwstr).map_err(|e| {
                LwError::Wallpaper(format!("Failed to get monitor bounds for index {i}: {e}"))
            })?;

            let path_str = path
                .to_string()
                .map_err(|_| LwError::Wallpaper("Invalid UTF-16 monitor path".to_string()))?;

            CoTaskMemFree(Some(path.0 as *const _));

            monitors.push(MonitorInfo { device_path: path_str, bounds });
        }
    }
    Ok(monitors)
}
