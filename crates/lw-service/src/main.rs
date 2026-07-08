use lw_core::logging::init_logging;
use lw_wallpaper::DesktopWallpaperManager;
use lw_service::ipc::run_ipc_server;
use lw_service::scheduler::{run_scheduler, SchedulerState};
use std::sync::{Arc, Mutex};
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
    let app_data = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    let config_dir = std::path::PathBuf::from(app_data).join("LiemWallpaper");
    let _ = std::fs::create_dir_all(&config_dir);
    let config_path = config_dir.join("config.toml");

    // 3b. Setup default HLSL shaders inside AppData/LiemWallpaper/shaders
    let shader_dir = config_dir.join("shaders");
    let _ = std::fs::create_dir_all(&shader_dir);
    let _ = std::fs::write(shader_dir.join("fade.hlsl"), include_str!("../../../shaders/fade.hlsl"));
    let _ = std::fs::write(shader_dir.join("wipe.hlsl"), include_str!("../../../shaders/wipe.hlsl"));
    let _ = std::fs::write(shader_dir.join("slide.hlsl"), include_str!("../../../shaders/slide.hlsl"));

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

    // 6b. Start the System Tray Icon (runs in background Win32 thread)
    lw_service::tray::start_tray_icon();

    // 7. Start Named Pipe IPC Server loop (runs on main thread)
    run_ipc_server(Arc::clone(&config), wallpaper_manager, scheduler_state).await?;

    Ok(())
}
