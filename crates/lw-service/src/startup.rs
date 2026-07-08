use lw_core::error::LwError;
use windows::core::PCWSTR;
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
    KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ,
};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APP_NAME: &str = "LiemWallpaper";

/// Registers or deregisters the background service in Windows HKCU Run registry key.
pub fn set_startup_run(enable: bool) -> Result<(), LwError> {
    unsafe {
        let key_w: Vec<u16> = RUN_KEY.encode_utf16().chain(std::iter::once(0)).collect();
        let name_w: Vec<u16> = APP_NAME.encode_utf16().chain(std::iter::once(0)).collect();

        let mut hkey = HKEY::default();
        if let Err(e) = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key_w.as_ptr()),
            0,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut hkey,
            None,
        ) {
            return Err(LwError::Ipc(format!("Failed to open Run registry key: {e}")));
        }

        let res = if enable {
            let exe_path = std::env::current_exe()
                .map_err(|e| LwError::Ipc(format!("Failed to get current executable path: {e}")))?;
            let exe_str = exe_path.to_string_lossy().into_owned();
            let value_w: Vec<u16> = exe_str.encode_utf16().chain(std::iter::once(0)).collect();

            if let Err(e) = RegSetValueExW(
                hkey,
                PCWSTR(name_w.as_ptr()),
                0,
                REG_SZ,
                Some(std::slice::from_raw_parts(value_w.as_ptr() as *const u8, value_w.len() * 2)),
            ) {
                Err(LwError::Ipc(format!("Failed to write Registry key: {e}")))
            } else {
                Ok(())
            }
        } else {
            match RegDeleteValueW(hkey, PCWSTR(name_w.as_ptr())) {
                Err(e) if e.code().0 as u32 != 2 => {
                    // 2 = ERROR_FILE_NOT_FOUND
                    Err(LwError::Ipc(format!("Failed to delete Registry key: {e}")))
                }
                _ => Ok(()),
            }
        };

        let _ = RegCloseKey(hkey);
        res
    }
}
