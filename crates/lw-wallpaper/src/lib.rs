use lw_core::error::LwError;
use lw_core::traits::{MonitorRect, WallpaperManager};
use std::path::{Path, PathBuf};
use windows::core::PCWSTR;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED,
};
use windows::Win32::UI::Shell::{DesktopWallpaper, IDesktopWallpaper};

pub mod monitor;

pub use monitor::{get_monitors, MonitorInfo};

pub struct DesktopWallpaperManager;

unsafe impl Send for DesktopWallpaperManager {}
unsafe impl Sync for DesktopWallpaperManager {}

impl DesktopWallpaperManager {
    /// Creates a new instance of `DesktopWallpaperManager`.
    pub fn new() -> Result<Self, LwError> {
        Ok(Self)
    }

    fn get_wallpaper_interface(&self) -> Result<IDesktopWallpaper, LwError> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        let wallpaper: IDesktopWallpaper = unsafe {
            CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL).map_err(|e| {
                LwError::Wallpaper(format!("Failed to create IDesktopWallpaper instance: {e}"))
            })?
        };
        Ok(wallpaper)
    }

    /// Retrieve bounds of all monitors.
    pub fn get_monitors(&self) -> Result<Vec<MonitorInfo>, LwError> {
        let wallpaper = self.get_wallpaper_interface()?;
        get_monitors(&wallpaper)
    }

    fn get_wallpaper_from_registry(&self) -> Result<PathBuf, LwError> {
        use windows::Win32::System::Registry::{
            RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER, KEY_READ,
        };
        use windows::core::PCWSTR;

        unsafe {
            let key_w: Vec<u16> = "Control Panel\\Desktop".encode_utf16().chain(std::iter::once(0)).collect();
            let name_w: Vec<u16> = "Wallpaper".encode_utf16().chain(std::iter::once(0)).collect();

            let mut hkey = windows::Win32::System::Registry::HKEY::default();
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(key_w.as_ptr()),
                0,
                KEY_READ,
                &mut hkey,
            ).map_err(|e| LwError::Wallpaper(format!("Failed to open registry key: {e}")))?;

            let mut data_len = 0u32;
            let _ = RegQueryValueExW(
                hkey,
                PCWSTR(name_w.as_ptr()),
                None,
                None,
                None,
                Some(&mut data_len),
            );

            let mut path_buf = Vec::new();
            if data_len > 0 {
                let mut buf = vec![0u16; (data_len as usize / 2) + 1];
                if RegQueryValueExW(
                    hkey,
                    PCWSTR(name_w.as_ptr()),
                    None,
                    None,
                    Some(buf.as_mut_ptr() as *mut u8),
                    Some(&mut data_len),
                ).is_ok() {
                    while buf.last() == Some(&0) {
                        buf.pop();
                    }
                    path_buf = buf;
                }
            }

            let _ = RegCloseKey(hkey);

            if path_buf.is_empty() {
                return Err(LwError::Wallpaper("Registry wallpaper path is empty".to_string()));
            }

            let path_str = String::from_utf16(&path_buf)
                .map_err(|_| LwError::Wallpaper("Invalid UTF-16 in registry".to_string()))?;
            Ok(PathBuf::from(path_str))
        }
    }
}

impl WallpaperManager for DesktopWallpaperManager {
    fn get_current_wallpaper(&self) -> Result<PathBuf, LwError> {
        // Try reading from the registry first, as it is updated immediately by set_wallpaper_registry_only
        if let Ok(path) = self.get_wallpaper_from_registry() {
            if path.exists() {
                return Ok(path);
            }
        }

        let wallpaper = self.get_wallpaper_interface()?;
        unsafe {
            // Get the first monitor device path (monitor 0)
            let monitor_id = wallpaper
                .GetMonitorDevicePathAt(0)
                .map_err(|e| LwError::Wallpaper(format!("Failed to get monitor path: {e}")))?;

            let monitor_id_pcwstr = PCWSTR(monitor_id.0);

            // Get the wallpaper image path currently active on this monitor
            let path_pwstr = wallpaper
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
        // Use SystemParametersInfoW(SPI_SETDESKWALLPAPER) instead of IDesktopWallpaper::SetWallpaper
        // because IDesktopWallpaper always triggers a native fade animation that cannot be disabled.
        // SystemParametersInfoW applies the wallpaper instantly with zero animation.
        use windows::Win32::UI::WindowsAndMessaging::{
            SystemParametersInfoW, SPI_SETDESKWALLPAPER, SPIF_UPDATEINIFILE, SPIF_SENDCHANGE,
        };

        let path_str = path.to_string_lossy();
        let mut path_w: Vec<u16> = path_str.encode_utf16().chain(std::iter::once(0)).collect();

        let result = unsafe {
            SystemParametersInfoW(
                SPI_SETDESKWALLPAPER,
                0,
                Some(path_w.as_mut_ptr() as *mut _),
                SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
            )
        };

        result.map_err(|e| {
            LwError::Wallpaper(format!("Failed to set wallpaper via SystemParametersInfoW: {e}"))
        })
    }

    fn set_wallpaper_registry_only(&self, path: &Path) -> Result<(), LwError> {
        use windows::Win32::System::Registry::{
            RegCloseKey, RegCreateKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
            KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ,
        };
        use windows::core::PCWSTR;

        unsafe {
            let key_w: Vec<u16> = "Control Panel\\Desktop".encode_utf16().chain(std::iter::once(0)).collect();
            let name_w: Vec<u16> = "Wallpaper".encode_utf16().chain(std::iter::once(0)).collect();

            let path_str = path.to_string_lossy();
            let value_w: Vec<u16> = path_str.encode_utf16().chain(std::iter::once(0)).collect();

            let mut hkey = HKEY::default();
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(key_w.as_ptr()),
                0,
                None,
                REG_OPTION_NON_VOLATILE,
                KEY_WRITE,
                None,
                &mut hkey,
                None,
            ).map_err(|e| LwError::Wallpaper(format!("Failed to open/create registry key: {e}")))?;

            let res = RegSetValueExW(
                hkey,
                PCWSTR(name_w.as_ptr()),
                0,
                REG_SZ,
                Some(std::slice::from_raw_parts(value_w.as_ptr() as *const u8, value_w.len() * 2)),
            );

            let _ = RegCloseKey(hkey);

            res.map_err(|e| LwError::Wallpaper(format!("Failed to set registry value: {e}")))?;
            Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_read_registry() {
        let manager = DesktopWallpaperManager;
        let test_path = PathBuf::from(r"D:\Downloads\red.jpg");
        let write_res = manager.set_wallpaper_registry_only(&test_path);
        println!("WRITE RESULT: {:?}", write_res);

        let read_res = manager.get_wallpaper_from_registry();
        println!("READ RESULT: {:?}", read_res);
        assert_eq!(read_res.unwrap(), test_path);
    }
}
