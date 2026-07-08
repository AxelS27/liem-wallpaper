use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTOPRIMARY,
};
use windows::Win32::UI::WindowsAndMessaging::{GetClassNameW, GetForegroundWindow, GetWindowRect};

/// Detects if the currently active foreground application is running in fullscreen mode
/// (e.g. games, media players, presentations).
#[must_use]
pub fn is_fullscreen_app_running() -> bool {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0 == 0 {
            return false;
        }

        // 1. Skip Desktop and Taskbar windows by checking their window class names
        let mut class_name = [0u16; 256];
        let len = GetClassNameW(hwnd, &mut class_name);
        if len > 0 {
            let len_usize = usize::try_from(len).unwrap_or(0);
            let name = String::from_utf16_lossy(&class_name[..len_usize]);
            if name == "Progman" || name == "WorkerW" || name == "Shell_TrayWnd" {
                return false;
            }
        }

        // 2. Retrieve foreground window bounds
        let mut window_rect = RECT::default();
        if GetWindowRect(hwnd, &mut window_rect).is_err() {
            return false;
        }

        // 3. Retrieve monitor information for the monitor displaying the window
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTOPRIMARY);
        let mut monitor_info = MONITORINFO {
            cbSize: u32::try_from(std::mem::size_of::<MONITORINFO>()).unwrap_or(0),
            ..Default::default()
        };

        if GetMonitorInfoW(monitor, &mut monitor_info).as_bool() {
            let monitor_rect = monitor_info.rcMonitor;

            // 4. Compare window bounds with monitor bounds.
            // If the window fully covers or exceeds the monitor screen size, it is fullscreen.
            if window_rect.left <= monitor_rect.left
                && window_rect.top <= monitor_rect.top
                && window_rect.right >= monitor_rect.right
                && window_rect.bottom >= monitor_rect.bottom
            {
                return true;
            }
        }

        false
    }
}
