use lw_wallpaper::DesktopWallpaperManager;

#[test]
fn test_monitor_bounds_discovery() {
    // Initialize COM for the test thread
    unsafe {
        let _ = windows::Win32::System::Com::CoInitializeEx(
            None,
            windows::Win32::System::Com::COINIT_MULTITHREADED,
        );
    }

    let manager = DesktopWallpaperManager::new();
    if let Ok(mgr) = manager {
        let monitors = mgr.get_monitors().expect("Failed to get monitors");
        assert!(!monitors.is_empty(), "Expected at least one monitor");
        for monitor in monitors {
            assert!(!monitor.device_path.is_empty(), "Monitor path cannot be empty");
            let bounds = monitor.bounds;
            let width = bounds.right - bounds.left;
            let height = bounds.bottom - bounds.top;
            assert!(width > 0, "Monitor width should be positive");
            assert!(height > 0, "Monitor height should be positive");
        }
    }
}
