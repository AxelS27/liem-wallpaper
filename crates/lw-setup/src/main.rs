#![windows_subsystem = "windows"]

use std::fs;
use std::path::{Path, PathBuf};
use windows::core::PCWSTR;
use windows::Win32::System::Registry::{
    RegCreateKeyExW, RegSetValueExW, RegDeleteKeyW, RegCloseKey, HKEY_CURRENT_USER, REG_SZ, HKEY,
    KEY_WRITE, REG_OPTION_NON_VOLATILE,
};

// Embed release binaries
const SERVICE_BIN: &[u8] = include_bytes!("../../../target/release/lw-service.exe");
const CLI_BIN: &[u8] = include_bytes!("../../../target/release/lw-cli.exe");
const GUI_BIN: &[u8] = include_bytes!("../../../target/release/lw-gui.exe");

// Embed shaders
const SHADER_FADE: &[u8] = include_bytes!("../../../shaders/fade.hlsl");
const SHADER_WIPE: &[u8] = include_bytes!("../../../shaders/wipe.hlsl");
const SHADER_SLIDE: &[u8] = include_bytes!("../../../shaders/slide.hlsl");

fn get_install_dir() -> PathBuf {
    let local_app_data = std::env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(local_app_data)
        .join("AppData")
        .join("Local")
        .join("Programs")
        .join("LiemWallpaper")
}

fn get_start_menu_shortcut_path() -> PathBuf {
    let app_data = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(app_data)
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs")
        .join("Liem Wallpaper.lnk")
}

fn get_desktop_shortcut_path() -> PathBuf {
    let user_profile = std::env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(user_profile)
        .join("Desktop")
        .join("Liem Wallpaper.lnk")
}

fn create_shortcut(target: &Path, shortcut_path: &Path) -> std::io::Result<()> {
    let ps_script = format!(
        "$s = (New-Object -ComObject WScript.Shell).CreateShortcut('{}'); $s.TargetPath = '{}'; $s.Save()",
        shortcut_path.to_string_lossy(),
        target.to_string_lossy()
    );
    let status = std::process::Command::new("powershell")
        .args(&["-NoProfile", "-Command", &ps_script])
        .status()?;
    if !status.success() {
        return Err(std::io::Error::new(std::io::ErrorKind::Other, "PowerShell shortcut creation failed"));
    }
    Ok(())
}

fn set_registry_value(subkey: &str, name: &str, value: &str) -> Result<(), String> {
    unsafe {
        let subkey_w: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
        let name_w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let value_w: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();

        let mut hkey = HKEY::default();
        if let Err(e) = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey_w.as_ptr()),
            0,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut hkey,
            None,
        ) {
            return Err(format!("Failed to create/open registry key: {e}"));
        }

        let res = RegSetValueExW(
            hkey,
            PCWSTR(name_w.as_ptr()),
            0,
            REG_SZ,
            Some(std::slice::from_raw_parts(
                value_w.as_ptr() as *const u8,
                value_w.len() * 2,
            )),
        );

        let _ = RegCloseKey(hkey);

        if let Err(e) = res {
            return Err(format!("Failed to set registry value: {e}"));
        }
        Ok(())
    }
}

fn delete_registry_key(subkey: &str) -> Result<(), String> {
    unsafe {
        let subkey_w: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
        if let Err(e) = RegDeleteKeyW(HKEY_CURRENT_USER, PCWSTR(subkey_w.as_ptr())) {
            if e.code().0 as u32 != 2 { // 2 = ERROR_FILE_NOT_FOUND is fine
                return Err(format!("Failed to delete registry key: {e}"));
            }
        }
        Ok(())
    }
}

fn install() -> std::io::Result<()> {
    let install_dir = get_install_dir();
    fs::create_dir_all(&install_dir)?;

    let shader_dir = install_dir.join("shaders");
    fs::create_dir_all(&shader_dir)?;

    // Write binaries
    fs::write(install_dir.join("lw-service.exe"), SERVICE_BIN)?;
    fs::write(install_dir.join("lw-cli.exe"), CLI_BIN)?;
    fs::write(install_dir.join("lw-gui.exe"), GUI_BIN)?;

    // Copy this installer itself as uninstall.exe
    let current_exe = std::env::current_exe()?;
    fs::copy(&current_exe, install_dir.join("uninstall.exe"))?;

    // Write shaders
    fs::write(shader_dir.join("fade.hlsl"), SHADER_FADE)?;
    fs::write(shader_dir.join("wipe.hlsl"), SHADER_WIPE)?;
    fs::write(shader_dir.join("slide.hlsl"), SHADER_SLIDE)?;

    // Create shortcuts
    let gui_path = install_dir.join("lw-gui.exe");
    let _ = create_shortcut(&gui_path, &get_start_menu_shortcut_path());
    let _ = create_shortcut(&gui_path, &get_desktop_shortcut_path());

    // Register Uninstaller in Windows Registry (Add/Remove Programs)
    let uninstall_key = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\LiemWallpaper";
    let uninstall_str = install_dir.join("uninstall.exe").to_string_lossy().into_owned() + " --uninstall";
    
    let _ = set_registry_value(uninstall_key, "DisplayName", "Liem Wallpaper");
    let _ = set_registry_value(uninstall_key, "DisplayVersion", "0.1.0");
    let _ = set_registry_value(uninstall_key, "Publisher", "Liem Wallpaper Contributors");
    let _ = set_registry_value(uninstall_key, "UninstallString", &uninstall_str);
    let _ = set_registry_value(uninstall_key, "InstallLocation", &install_dir.to_string_lossy());
    let _ = set_registry_value(uninstall_key, "DisplayIcon", &gui_path.to_string_lossy());

    // Show completion box
    let ps_msg = "Add-Type -AssemblyName PresentationFramework; [System.Windows.MessageBox]::Show('Liem Wallpaper has been installed successfully!', 'Installation Complete')";
    let _ = std::process::Command::new("powershell")
        .args(&["-NoProfile", "-Command", ps_msg])
        .status();

    // Start GUI after installation finishes
    let _ = std::process::Command::new(gui_path).spawn();

    Ok(())
}

fn uninstall() -> std::io::Result<()> {
    // 1. Kill running instances
    let _ = std::process::Command::new("taskkill")
        .args(&["/F", "/IM", "lw-service.exe"])
        .status();
    let _ = std::process::Command::new("taskkill")
        .args(&["/F", "/IM", "lw-gui.exe"])
        .status();

    // 2. Remove shortcuts
    let _ = fs::remove_file(get_start_menu_shortcut_path());
    let _ = fs::remove_file(get_desktop_shortcut_path());

    // 3. Remove Registry Uninstaller Entries
    let uninstall_key = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\LiemWallpaper";
    let _ = delete_registry_key(uninstall_key);

    // Remove startup run key if exists
    let run_key = r"Software\Microsoft\Windows\CurrentVersion\Run";
    unsafe {
        let run_key_w: Vec<u16> = run_key.encode_utf16().chain(std::iter::once(0)).collect();
        let mut hkey = HKEY::default();
        if RegCreateKeyExW(HKEY_CURRENT_USER, PCWSTR(run_key_w.as_ptr()), 0, None, REG_OPTION_NON_VOLATILE, KEY_WRITE, None, &mut hkey, None).is_ok() {
            let name_w: Vec<u16> = "LiemWallpaper".encode_utf16().chain(std::iter::once(0)).collect();
            let _ = windows::Win32::System::Registry::RegDeleteValueW(hkey, PCWSTR(name_w.as_ptr()));
            let _ = RegCloseKey(hkey);
        }
    }

    // 4. Trigger self-deletion of directory after exit
    let install_dir = get_install_dir();
    let cmd_script = format!(
        "timeout /T 1 & rmdir /S /Q \"{}\"",
        install_dir.to_string_lossy()
    );

    std::process::Command::new("cmd")
        .args(&["/C", "start", "/B", "", "cmd", "/C", &cmd_script])
        .spawn()?;

    // Show completion box
    let ps_msg = "Add-Type -AssemblyName PresentationFramework; [System.Windows.MessageBox]::Show('Liem Wallpaper has been uninstalled successfully!', 'Uninstallation Complete')";
    let _ = std::process::Command::new("powershell")
        .args(&["-NoProfile", "-Command", ps_msg])
        .status();

    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "--uninstall" {
        if let Err(e) = uninstall() {
            eprintln!("Uninstallation failed: {e}");
        }
    } else {
        if let Err(e) = install() {
            eprintln!("Installation failed: {e}");
        }
    }
}
