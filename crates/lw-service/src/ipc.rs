use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::ServerOptions;
use tracing::{error, info};
use lw_core::{IpcRequest, IpcResponse, traits::{WallpaperManager, TransitionRenderer}};
use windows::Win32::Foundation::RECT;

pub const PIPE_NAME: &str = r"\\.\pipe\liem-wallpaper";

/// Runs the IPC Named Pipe server command processing loop.
/// Runs the IPC Named Pipe server command processing loop.
pub async fn run_ipc_server<W>(
    config: Arc<std::sync::Mutex<lw_core::Config>>,
    wallpaper_manager: Arc<W>,
    scheduler_state: Arc<std::sync::Mutex<crate::scheduler::SchedulerState>>,
) -> Result<(), lw_core::LwError>
where
    W: WallpaperManager + 'static,
{
    info!("Starting IPC server on {}", PIPE_NAME);

    let mut is_first = true;
    loop {
        let server = ServerOptions::new()
            .first_pipe_instance(is_first)
            .create(PIPE_NAME)
            .map_err(|e| lw_core::LwError::Ipc(format!("Failed to create named pipe server: {e}")))?;

        is_first = false;

        // Wait for a client to connect
        if server.connect().await.is_ok() {
            let cfg = Arc::clone(&config);
            let wm = Arc::clone(&wallpaper_manager);
            let state = Arc::clone(&scheduler_state);
            tokio::spawn(async move {
                if let Err(e) = handle_client(server, cfg, wm, state).await {
                    error!("Error handling IPC client: {:?}", e);
                }
            });
        }
    }
}

async fn handle_client<W>(
    stream: tokio::net::windows::named_pipe::NamedPipeServer,
    config: Arc<std::sync::Mutex<lw_core::Config>>,
    wallpaper_manager: Arc<W>,
    scheduler_state: Arc<std::sync::Mutex<crate::scheduler::SchedulerState>>,
) -> Result<(), lw_core::LwError>
where
    W: WallpaperManager + 'static,
{
    let (reader, mut writer) = tokio::io::split(stream);
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line).await?;

        if bytes_read == 0 {
            break; // Client disconnected
        }

        // Deserialize request
        let request: IpcRequest = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(e) => {
                let response = IpcResponse::Error {
                    message: format!("Invalid JSON request: {e}"),
                };
                let mut response_bytes = serde_json::to_vec(&response).unwrap_or_default();
                response_bytes.push(b'\n');
                let _ = writer.write_all(&response_bytes).await;
                let _ = writer.flush().await;
                continue;
            }
        };

        // Process request
        let response = match request {
            IpcRequest::GetStatus => {
                let current = wallpaper_manager.get_current_wallpaper().ok();
                let (active, next_change) = {
                    let st = scheduler_state.lock().unwrap();
                    let seconds = st.next_change_at.map_or(0, |t| {
                        let now = std::time::Instant::now();
                        if t > now {
                            u32::try_from((t - now).as_secs()).unwrap_or(0)
                        } else {
                            0
                        }
                    });
                    (st.active, seconds)
                };

                IpcResponse::StatusResponse {
                    current_wallpaper: current,
                    scheduler_active: active,
                    next_change_in_seconds: next_change,
                }
            }
            IpcRequest::SetWallpaper { path, transition } => {
                let res = if let Some(ref t_params) = transition {
                    match run_transition_and_set(&path, t_params, wallpaper_manager.as_ref()) {
                        Ok(()) => Ok(()),
                        Err(e) => {
                            error!("Transition failed: {e:?}. Falling back to native wallpaper set.");
                            wallpaper_manager.set_wallpaper(&path)
                        }
                    }
                } else {
                    wallpaper_manager.set_wallpaper(&path)
                };

                match res {
                    Ok(()) => IpcResponse::Success,
                    Err(e) => IpcResponse::Error {
                        message: format!("Failed to set wallpaper: {e}"),
                    },
                }
            }
            IpcRequest::NextWallpaper | IpcRequest::PrevWallpaper => {
                let (dir, shuffle) = {
                    let cfg = config.lock().unwrap();
                    (cfg.wallpaper_dir.clone(), cfg.shuffle)
                };
                let files = crate::scheduler::get_wallpaper_files(&dir);
                if files.is_empty() {
                    IpcResponse::Error {
                        message: "No wallpaper files found in directory".to_string(),
                    }
                } else {
                    let current = wallpaper_manager.get_current_wallpaper().unwrap_or_default();
                    if let Some(next_wp) = crate::scheduler::select_next_wallpaper(&files, &current, shuffle) {
                        let params = {
                            let cfg = config.lock().unwrap();
                            lw_core::ipc::TransitionParams {
                                effect_type: cfg.transition_default.effect_type.clone(),
                                duration_ms: cfg.transition_default.duration_ms,
                                easing: cfg.transition_default.easing,
                            }
                        };
                        match run_transition_and_set(&next_wp, &params, wallpaper_manager.as_ref()) {
                            Ok(()) => IpcResponse::Success,
                            Err(e) => IpcResponse::Error {
                                message: format!("Failed to set next wallpaper: {e}"),
                            },
                        }
                    } else {
                        IpcResponse::Error {
                            message: "Failed to select next wallpaper".to_string(),
                        }
                    }
                }
            }
            IpcRequest::UpdateConfig { config: new_config } => {
                let mut cfg = config.lock().unwrap();
                *cfg = new_config;
                let _ = crate::startup::set_startup_run(cfg.scheduler.run_on_startup);
                IpcResponse::Success
            }
        };

        // Serialize and send response
        let mut response_bytes = serde_json::to_vec(&response)
            .map_err(|e| lw_core::LwError::Serialization(format!("Failed to serialize response: {e}")))?;
        response_bytes.push(b'\n');

        writer.write_all(&response_bytes).await?;
        writer.flush().await?;
    }

    Ok(())
}

pub fn run_transition_and_set<W>(
    target_path: &std::path::Path,
    params: &lw_core::ipc::TransitionParams,
    wallpaper_manager: &W,
) -> Result<(), lw_core::LwError>
where
    W: WallpaperManager + 'static,
{
    // 1. Get current wallpaper
    let from_path = wallpaper_manager.get_current_wallpaper()?;

    // 2. Query active monitor bounds from wallpaper manager
    let monitors_rects = wallpaper_manager.get_monitor_rects()?;
    let monitors_bounds: Vec<RECT> = monitors_rects
        .iter()
        .map(|r| RECT {
            left: r.left,
            top: r.top,
            right: r.right,
            bottom: r.bottom,
        })
        .collect();

    // 3. Initialize D3D11 and DirectComposition
    let d3d_context = Arc::new(lw_renderer::D3D11Context::new()?);
    let worker_w = lw_renderer::find_worker_w()?;

    let comp_context = Arc::new(lw_renderer::CompositionContext::new(d3d_context.device(), worker_w)?);
    let app_data = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    let shader_dir = std::path::PathBuf::from(app_data).join("LiemWallpaper").join("shaders");

    // 4. Setup transition engine
    let engine = lw_transition::TransitionEngine::new(
        d3d_context,
        &comp_context,
        &monitors_bounds,
        shader_dir,
    )?;

    // 5. Render transition
    engine.render_transition(&from_path, target_path, params.duration_ms, &params.effect_type)?;

    // 6. Update native wallpaper
    wallpaper_manager.set_wallpaper(target_path)?;

    // 7. Clear visual content to hide transition overlay
    unsafe {
        comp_context.root_visual().SetContent(None)
            .map_err(|e| lw_core::LwError::Renderer(format!("Failed to clear visual content: {e}")))?;
        comp_context.commit()?;
    }

    Ok(())
}
