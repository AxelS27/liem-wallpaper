#![windows_subsystem = "windows"]

slint::include_modules!();

pub mod updater;

use lw_core::{Config, EasingType, IpcRequest, IpcResponse};
use std::os::windows::process::CommandExt;
use slint::ComponentHandle;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, Duration};

async fn send_ipc_request(request: IpcRequest) -> Result<IpcResponse, String> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::windows::named_pipe::ClientOptions;

    let mut client = ClientOptions::new()
        .open(r"\\.\pipe\liem-wallpaper")
        .map_err(|e| format!("Failed to connect to named pipe: {e}"))?;

    let mut request_bytes =
        serde_json::to_vec(&request).map_err(|e| format!("Failed to serialize request: {e}"))?;
    request_bytes.push(b'\n');

    client.write_all(&request_bytes).await.map_err(|e| format!("Failed to write request: {e}"))?;
    client.flush().await.map_err(|e| format!("Failed to flush stream: {e}"))?;

    let mut reader = BufReader::new(client);
    let mut line = String::new();
    reader.read_line(&mut line).await.map_err(|e| format!("Failed to read response: {e}"))?;

    let response: IpcResponse =
        serde_json::from_str(&line).map_err(|e| format!("Failed to deserialize response: {e}"))?;

    Ok(response)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Setup configuration path and load configuration
    let app_data = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    let config_dir = PathBuf::from(app_data).join("LiemWallpaper");
    let _ = std::fs::create_dir_all(&config_dir);
    let config_path = config_dir.join("config.toml");

    let config = Arc::new(Mutex::new(Config::load_from_file(&config_path).unwrap_or_else(|_| {
        let default_cfg = Config::default();
        let _ = default_cfg.save_to_file(&config_path);
        default_cfg
    })));

    // 2. Instantiate Slint App Window
    let app = AppWindow::new()?;

    const CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

    // 3. Populate initial UI values from configuration
    {
        let cfg = config.lock().unwrap();
        app.set_active_effect(cfg.transition_default.effect_type.clone().into());
        app.set_duration_ms(cfg.transition_default.duration_ms as i32);
        app.set_scheduler_enabled(cfg.scheduler.enabled);
        app.set_scheduler_interval(cfg.scheduler.interval_mins as i32);
        app.set_run_on_startup(cfg.scheduler.run_on_startup);
        app.set_update_status(format!("v{}", CARGO_PKG_VERSION).into());

        let easing_str = match cfg.transition_default.easing {
            EasingType::Linear => "linear",
            EasingType::EaseIn => "ease-in",
            EasingType::EaseOut => "ease-out",
            EasingType::EaseInOut => "ease-in-out",
        };
        app.set_active_easing(easing_str.into());
    }

    // 4. Background daemon monitoring task
    let ui_weak = app.as_weak();
    tokio::spawn(async move {
        loop {
            match send_ipc_request(IpcRequest::GetStatus).await {
                Ok(IpcResponse::StatusResponse {
                    current_wallpaper,
                    scheduler_active: _,
                    next_change_in_seconds: _,
                }) => {
                    let wp_str = current_wallpaper
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "None".to_string());

                    let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        ui.set_is_connected(true);
                        ui.set_daemon_status("Connected".into());
                        ui.set_current_wallpaper(wp_str.into());
                    });
                }
                _ => {
                    let _ = ui_weak.upgrade_in_event_loop(|ui| {
                        ui.set_is_connected(false);
                        ui.set_daemon_status("Disconnected".into());
                        ui.set_current_wallpaper("None".into());
                    });

                    // Try to auto-start daemon in the background
                    if let Ok(exe_path) = std::env::current_exe() {
                        if let Some(exe_dir) = exe_path.parent() {
                            let daemon_path = exe_dir.join("lw-service.exe");
                            if daemon_path.exists() {
                                let _ = std::process::Command::new(daemon_path)
                                    .creation_flags(0x08000000) // CREATE_NO_WINDOW
                                    .spawn();
                            }
                        }
                    }
                }
            }
            sleep(Duration::from_secs(2)).await;
        }
    });

    // 5. Connect UI Callbacks
    let config_clone = Arc::clone(&config);
    let config_path_clone = config_path.clone();
    app.on_apply_settings(
        move |effect, duration, scheduler_enabled, interval, easing, run_on_startup| {
            let mut cfg = config_clone.lock().unwrap();
            cfg.transition_default.effect_type = effect.to_string();
            cfg.transition_default.duration_ms = duration as u32;
            cfg.scheduler.enabled = scheduler_enabled;
            cfg.scheduler.interval_mins = interval as u32;
            cfg.scheduler.run_on_startup = run_on_startup;

            cfg.transition_default.easing = match easing.as_str() {
                "linear" => EasingType::Linear,
                "ease-in" => EasingType::EaseIn,
                "ease-out" => EasingType::EaseOut,
                _ => EasingType::EaseInOut,
            };

            // Save locally
            let _ = cfg.save_to_file(&config_path_clone);

            // Notify daemon asynchronously
            let cfg_payload = cfg.clone();
            tokio::spawn(async move {
                let _ = send_ipc_request(IpcRequest::UpdateConfig { config: cfg_payload }).await;
            });
        },
    );

    app.on_trigger_next(move || {
        tokio::spawn(async move {
            let _ = send_ipc_request(IpcRequest::NextWallpaper).await;
        });
    });

    let ui_weak = app.as_weak();
    app.on_check_for_updates(move || {
        let ui_weak = ui_weak.clone();
        tokio::spawn(async move {
            let _ = ui_weak.upgrade_in_event_loop(|ui| {
                ui.set_update_status("Checking...".into());
            });

            let check_res =
                tokio::task::spawn_blocking(move || updater::check_for_updates(CARGO_PKG_VERSION))
                    .await;

            match check_res {
                Ok(Ok(Some(info))) => {
                    let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        ui.set_update_status(format!("New version: {}", info.version).into());
                        ui.set_update_available(true);
                        ui.set_latest_version_url(info.download_url.into());
                    });
                }
                Ok(Ok(None)) => {
                    let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        ui.set_update_status(format!("v{} (Up to date)", CARGO_PKG_VERSION).into());
                        ui.set_update_available(false);
                    });
                }
                Ok(Err(e)) => {
                    let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        ui.set_update_status(format!("Error: {e}").into());
                        ui.set_update_available(false);
                    });
                }
                Err(_) => {
                    let _ = ui_weak.upgrade_in_event_loop(|ui| {
                        ui.set_update_status("Error checking updates".into());
                        ui.set_update_available(false);
                    });
                }
            }
        });
    });

    let ui_weak = app.as_weak();
    app.on_apply_update(move || {
        let ui_weak = ui_weak.clone();
        tokio::spawn(async move {
            let url = ui_weak
                .upgrade()
                .map(|ui| ui.get_latest_version_url().to_string())
                .unwrap_or_default();
            if url.is_empty() {
                return;
            }

            let _ = ui_weak.upgrade_in_event_loop(|ui| {
                ui.set_update_status("Downloading...".into());
            });

            let download_res =
                tokio::task::spawn_blocking(move || updater::download_and_run_installer(&url))
                    .await;

            match download_res {
                Ok(Ok(())) => {
                    let _ = ui_weak.upgrade_in_event_loop(|ui| {
                        ui.set_update_status("Installing...".into());
                    });
                    let _ = std::process::Command::new("taskkill")
                        .args(&["/F", "/IM", "lw-service.exe"])
                        .status();
                    std::process::exit(0);
                }
                Ok(Err(e)) => {
                    let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        ui.set_update_status(format!("Download failed: {e}").into());
                    });
                }
                Err(_) => {
                    let _ = ui_weak.upgrade_in_event_loop(|ui| {
                        ui.set_update_status("Download failed".into());
                    });
                }
            }
        });
    });

    // 6. Run Slint application loop (blocks main thread)
    app.run()?;

    Ok(())
}
