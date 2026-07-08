use lw_service::startup::set_startup_run;
use windows::core::PCWSTR;
use windows::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
};

#[test]
fn test_registry_startup_toggle() {
    // 1. Enable startup run
    set_startup_run(true).expect("Failed to enable startup run");

    // 2. Query registry value to assert it is written
    unsafe {
        let key_w: Vec<u16> = r"Software\Microsoft\Windows\CurrentVersion\Run"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let name_w: Vec<u16> = "LiemWallpaper".encode_utf16().chain(std::iter::once(0)).collect();

        let mut hkey = HKEY::default();
        let status =
            RegOpenKeyExW(HKEY_CURRENT_USER, PCWSTR(key_w.as_ptr()), 0, KEY_READ, &mut hkey);

        assert!(status.is_ok(), "Failed to open registry key for verification");

        let mut data_len = 0u32;
        let status =
            RegQueryValueExW(hkey, PCWSTR(name_w.as_ptr()), None, None, None, Some(&mut data_len));

        let _ = RegCloseKey(hkey);

        assert!(status.is_ok(), "Registry value LiemWallpaper not found");
        assert!(data_len > 0, "Registry value data length should be non-zero");
    }

    // 3. Disable startup run
    set_startup_run(false).expect("Failed to disable startup run");

    // 4. Query again to assert it is deleted
    unsafe {
        let key_w: Vec<u16> = r"Software\Microsoft\Windows\CurrentVersion\Run"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let name_w: Vec<u16> = "LiemWallpaper".encode_utf16().chain(std::iter::once(0)).collect();

        let mut hkey = HKEY::default();
        let status =
            RegOpenKeyExW(HKEY_CURRENT_USER, PCWSTR(key_w.as_ptr()), 0, KEY_READ, &mut hkey);

        if status.is_ok() {
            let status = RegQueryValueExW(hkey, PCWSTR(name_w.as_ptr()), None, None, None, None);
            let _ = RegCloseKey(hkey);
            assert!(status.is_err(), "Registry value should have been deleted");
        }
    }
}
