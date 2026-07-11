use lw_core::logging::init_logging;
use lw_service::ipc::run_ipc_server;
use lw_service::scheduler::{run_scheduler, SchedulerState};
use lw_wallpaper::DesktopWallpaperManager;
use std::sync::{Arc, Mutex};
use std::os::windows::process::CommandExt;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize tracing logging
    init_logging();
    tracing::info!("Starting Liem Wallpaper background daemon...");

    // 2. Initialize COM (Multi-threaded Apartment) for WIC and IDesktopWallpaper
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }

    // 3. Setup configuration path and load configuration
    let exe_path = std::env::current_exe()?;
    let config_dir = exe_path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| std::path::PathBuf::from("."));
    let config_path = config_dir.join("config.toml");

    // 3b. Setup default HLSL shaders inside local shaders/ directory next to executable
    let shader_dir = config_dir.join("shaders");
    let _ = std::fs::create_dir_all(&shader_dir);
    
    // Clean up old obsolete shaders
    let _ = std::fs::remove_file(shader_dir.join("wipe.hlsl"));
    let _ = std::fs::remove_file(shader_dir.join("slide.hlsl"));
    let _ = std::fs::remove_file(shader_dir.join("wipe-left.hlsl"));
    let _ = std::fs::remove_file(shader_dir.join("wipe-right.hlsl"));
    let _ = std::fs::remove_file(shader_dir.join("wipe-up.hlsl"));
    let _ = std::fs::remove_file(shader_dir.join("wipe-down.hlsl"));
    let _ = std::fs::remove_file(shader_dir.join("push-left.hlsl"));
    let _ = std::fs::remove_file(shader_dir.join("push-right.hlsl"));
    let _ = std::fs::remove_file(shader_dir.join("push-up.hlsl"));
    let _ = std::fs::remove_file(shader_dir.join("push-down.hlsl"));
    let _ = std::fs::remove_file(shader_dir.join("zoom.hlsl"));

    // Write all available shaders
    let _ = std::fs::write(shader_dir.join("fade.hlsl"), include_str!("../../../shaders/fade.hlsl"));
    let _ = std::fs::write(shader_dir.join("zoom-in.hlsl"), include_str!("../../../shaders/zoom-in.hlsl"));
    let _ = std::fs::write(shader_dir.join("zoom-out.hlsl"), include_str!("../../../shaders/zoom-out.hlsl"));
    let _ = std::fs::write(shader_dir.join("pixelate.hlsl"), include_str!("../../../shaders/pixelate.hlsl"));
    let _ = std::fs::write(shader_dir.join("glitch.hlsl"), include_str!("../../../shaders/glitch.hlsl"));
    
    let _ = std::fs::write(shader_dir.join("radial-in.hlsl"), include_str!("../../../shaders/radial-in.hlsl"));
    let _ = std::fs::write(shader_dir.join("radial-out.hlsl"), include_str!("../../../shaders/radial-out.hlsl"));

    let _ = std::fs::write(shader_dir.join("slide-left.hlsl"), include_str!("../../../shaders/slide-left.hlsl"));
    let _ = std::fs::write(shader_dir.join("slide-right.hlsl"), include_str!("../../../shaders/slide-right.hlsl"));
    let _ = std::fs::write(shader_dir.join("slide-up.hlsl"), include_str!("../../../shaders/slide-up.hlsl"));
    let _ = std::fs::write(shader_dir.join("slide-down.hlsl"), include_str!("../../../shaders/slide-down.hlsl"));

    // 3c. Setup default icon in AppData
    let _ = std::fs::write(config_dir.join("icon.ico"), include_bytes!("../../../assets/icon.ico"));

    let config_val = lw_core::Config::load_from_file(&config_path).unwrap_or_else(|_| {
        let default_cfg = lw_core::Config::default();
        // Try to save default config so user has a template
        let _ = default_cfg.save_to_file(&config_path);
        default_cfg
    });

    // Ensure the registry run-on-startup key matches the configuration
    let _ = lw_service::startup::set_startup_run(config_val.scheduler.run_on_startup);

    let config = Arc::new(Mutex::new(config_val));

    // 4. Create native COM desktop wallpaper manager
    let wallpaper_manager = Arc::new(DesktopWallpaperManager::new()?);

    // 5. Create shared scheduler state
    let scheduler_state = Arc::new(Mutex::new(SchedulerState::default()));

    // 6. Spawn the background rotation scheduler task
    let scheduler_config = Arc::clone(&config);
    let scheduler_wm = Arc::clone(&wallpaper_manager);
    let scheduler_st = Arc::clone(&scheduler_state);
    tokio::spawn(async move {
        run_scheduler(scheduler_config, scheduler_wm, scheduler_st).await;
    });

    // 6b. Spawn background silent check-update task
    tokio::spawn(async {
        check_and_perform_silent_update();
    });

    // 6c. Start the System Tray Icon (runs in background Win32 thread)
    lw_service::tray::start_tray_icon();

    // 7. Start Named Pipe IPC Server loop (runs on main thread)
    run_ipc_server(Arc::clone(&config), wallpaper_manager, scheduler_state).await?;

    Ok(())
}

fn check_and_perform_silent_update() {
    let current_version = env!("CARGO_PKG_VERSION");
    let repo = "AxelS27/liem-wallpaper";

    let ps_script = format!(
        "$version = '{}'; \
         $repo = '{}'; \
         try {{ \
             $r = Invoke-RestMethod -Uri \"https://api.github.com/repos/$repo/releases/latest\" -UserAgent \"LiemWallpaper\" -ErrorAction Stop; \
             $latest = $r.tag_name.TrimStart('v'); \
             if ($latest -ne $version) {{ \
                 $asset = $r.assets | Where-Object {{ $_.name -like '*Setup.exe' -or $_.name -like '*.exe' }} | Select-Object -First 1; \
                 if ($asset) {{ \
                     $tempPath = [System.IO.Path]::Combine([System.IO.Path]::GetTempPath(), $asset.name); \
                     Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $tempPath -UserAgent \"LiemWallpaper\" -ErrorAction Stop; \
                     Start-Process -FilePath $tempPath -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART'; \
                 }} \
             }} \
         }} catch {{ \
             # Silent ignore on background startup errors \
         }}",
        current_version, repo
    );

    let _ = std::process::Command::new("powershell")
        .args(&["-NoProfile", "-Command", &ps_script])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .spawn();
}
