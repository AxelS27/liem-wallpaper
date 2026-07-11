use lw_core::{
    traits::WallpaperManager,
    IpcRequest, IpcResponse,
};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::ServerOptions;
use tracing::{error, info};
use windows::Win32::Foundation::{HWND, RECT};

struct ActiveOverlayContext {
    hwnd: isize,
    swapchain: windows::Win32::Graphics::Dxgi::IDXGISwapChain1,
    bounds: RECT,
}

static ACTIVE_OVERLAYS: std::sync::Mutex<Vec<ActiveOverlayContext>> = std::sync::Mutex::new(Vec::new());
static ACTIVE_D3D_CONTEXT: std::sync::Mutex<Option<Arc<lw_renderer::D3D11Context>>> = std::sync::Mutex::new(None);

/// Destroys any previously active overlay windows.
/// MUST only be called after new overlay windows are already created and visible,
/// so the user never sees the bare desktop underneath.
fn destroy_active_overlays() {
    let mut overlays = ACTIVE_OVERLAYS.lock().unwrap_or_else(|e| e.into_inner());
    if !overlays.is_empty() {
        tracing::info!("Destroying {} previous overlay window(s)...", overlays.len());
        for ctx in overlays.drain(..) {
            unsafe {
                let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(HWND(ctx.hwnd));
            }
        }
    }
}

pub const PIPE_NAME: &str = r"\\.\pipe\liem-wallpaper";

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
        let server =
            ServerOptions::new().first_pipe_instance(is_first).create(PIPE_NAME).map_err(|e| {
                lw_core::LwError::Ipc(format!("Failed to create named pipe server: {e}"))
            })?;

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
                let response = IpcResponse::Error { message: format!("Invalid JSON request: {e}") };
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
                            error!(
                                "Transition failed: {e:?}. Falling back to native wallpaper set."
                            );
                            let r = wallpaper_manager.set_wallpaper(&path);
                            destroy_active_overlays();
                            r
                        }
                    }
                } else {
                    let r = wallpaper_manager.set_wallpaper(&path);
                    destroy_active_overlays();
                    r
                };

                match res {
                    Ok(()) => IpcResponse::Success,
                    Err(e) => {
                        IpcResponse::Error { message: format!("Failed to set wallpaper: {e}") }
                    }
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
                    if let Some(next_wp) =
                        crate::scheduler::select_next_wallpaper(&files, &current, shuffle)
                    {
                        let params = {
                            let cfg = config.lock().unwrap();
                            lw_core::ipc::TransitionParams {
                                effect_type: cfg.transition_default.effect_type.clone(),
                                duration_ms: cfg.transition_default.duration_ms,
                                easing_style: cfg.transition_default.easing_style,
                                easing_direction: cfg.transition_default.easing_direction,
                            }
                        };
                        match run_transition_and_set(&next_wp, &params, wallpaper_manager.as_ref())
                        {
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
                
                // Save config.toml next to the executable
                if let Ok(exe_path) = std::env::current_exe() {
                    if let Some(exe_dir) = exe_path.parent() {
                        let _ = cfg.save_to_file(&exe_dir.join("config.toml"));
                    }
                }
                IpcResponse::Success
            }
        };

        // Serialize and send response
        let mut response_bytes = serde_json::to_vec(&response).map_err(|e| {
            lw_core::LwError::Serialization(format!("Failed to serialize response: {e}"))
        })?;
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
    // 1. Get current wallpaper path and normalize/map transition name
    let from_path = wallpaper_manager.get_current_wallpaper()?;
    let mut effect_type = params.effect_type.clone();
    if effect_type == "zoom" {
        effect_type = "zoom-in".to_string();
    } else if effect_type == "slide" {
        effect_type = "slide-left".to_string();
    }

    // 2. Query active monitor bounds
    let monitors_rects = wallpaper_manager.get_monitor_rects()?;
    let monitors_bounds: Vec<RECT> = monitors_rects
        .iter()
        .map(|r| RECT { left: r.left, top: r.top, right: r.right, bottom: r.bottom })
        .collect();

    if monitors_bounds.is_empty() {
        return Err(lw_core::LwError::Renderer("No monitors detected".to_string()));
    }

    // 3. Initialize D3D11 and locate WorkerW (reuse the D3D context if available to prevent device mismatches)
    let d3d_context = {
        let mut d3d_ctx_store = ACTIVE_D3D_CONTEXT.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(ref ctx) = *d3d_ctx_store {
            Arc::clone(ctx)
        } else {
            let ctx = Arc::new(lw_renderer::D3D11Context::new()?);
            *d3d_ctx_store = Some(Arc::clone(&ctx));
            ctx
        }
    };
    let worker_w = lw_renderer::find_worker_w()?;

    let exe_path = std::env::current_exe().unwrap_or_default();
    let install_dir = exe_path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| std::path::PathBuf::from("."));
    let shader_dir = install_dir.join("shaders");

    // Check if we can reuse the existing overlay windows and swapchains.
    // If they match in count and dimensions, we reuse them to prevent taskbar Z-order/painting flickers.
    let mut existing_contexts = Vec::new();
    {
        let mut overlays = ACTIVE_OVERLAYS.lock().unwrap_or_else(|e| e.into_inner());
        if !overlays.is_empty() && overlays.len() == monitors_bounds.len() {
            let all_match = overlays.iter().zip(&monitors_bounds).all(|(ov, mb)| {
                ov.bounds.left == mb.left
                    && ov.bounds.top == mb.top
                    && ov.bounds.right == mb.right
                    && ov.bounds.bottom == mb.bottom
            });
            if all_match {
                existing_contexts = overlays
                    .drain(..)
                    .map(|ctx| (HWND(ctx.hwnd), ctx.swapchain, ctx.bounds))
                    .collect();
            }
        }
    }

    let was_reused = !existing_contexts.is_empty();

    let mut engine = if was_reused {
        tracing::info!("Reusing existing {} overlay window(s) and swapchain(s).", existing_contexts.len());
        lw_transition::TransitionEngine::new_from_existing(
            Arc::clone(&d3d_context),
            existing_contexts,
            shader_dir,
        )?
    } else {
        // If we cannot reuse them, we destroy any existing overlays first.
        destroy_active_overlays();
        lw_transition::TransitionEngine::new(
            Arc::clone(&d3d_context),
            worker_w,
            &monitors_bounds,
            shader_dir,
        )?
    };

    engine.default_easing_style = params.easing_style;
    engine.default_easing_direction = params.easing_direction;

    // 5. Render transition animation.
    //    If we are NOT reusing existing windows (meaning new ones were created), we trigger
    //    destroy_active_overlays immediately after the first frame is presented to prevent start-of-transition flicker.
    //    If we ARE reusing, there is nothing to destroy.
    engine.render_transition_with_callback(
        &from_path,
        target_path,
        params.duration_ms,
        &effect_type,
        || {
            if !was_reused {
                destroy_active_overlays();
            }
        },
    )?;

    // 6. Take overlay handles, swapchains, and bounds out of the engine so they persist.
    let contexts = engine.take_overlay_contexts_with_bounds();

    // 7. Store overlay contexts globally to keep swapchains and windows alive, avoiding end-of-transition flicker!
    {
        let mut overlays = ACTIVE_OVERLAYS.lock().unwrap_or_else(|e| e.into_inner());
        *overlays = contexts
            .into_iter()
            .map(|(hwnd, sc, bounds)| ActiveOverlayContext {
                hwnd: hwnd.0,
                swapchain: sc,
                bounds,
            })
            .collect();
    }

    // 8. Store D3D context globally to keep the D3D11 device and context alive
    {
        let mut d3d_ctx_store = ACTIVE_D3D_CONTEXT.lock().unwrap_or_else(|e| e.into_inner());
        *d3d_ctx_store = Some(d3d_context);
    }

    // 9. Update the actual Windows desktop wallpaper. Since our overlay window is active and fully opaque,
    //    the user will not see any native Windows slide/fade animation.
    wallpaper_manager.set_wallpaper(target_path)?;

    tracing::info!("Transition complete. Overlay and swapchains persist, Windows desktop wallpaper updated.");
    Ok(())
}

