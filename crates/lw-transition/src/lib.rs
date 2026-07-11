pub mod easing;
pub mod shader;

pub use easing::interpolate;

use lw_core::error::LwError;
use lw_core::traits::TransitionRenderer;
use lw_core::{EasingStyle, EasingDirection};
use lw_renderer::{create_overlay_window, load_texture_from_file, D3D11Context};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use windows::core::ComInterface;
use windows::Win32::Foundation::{FALSE, HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
};
use windows::Win32::Graphics::Direct3D11::{
    ID3D11RenderTargetView, ID3D11SamplerState, ID3D11Texture2D, ID3D11VertexShader, D3D11_BIND_CONSTANT_BUFFER,
    D3D11_BUFFER_DESC, D3D11_CPU_ACCESS_WRITE, D3D11_FILTER_MIN_MAG_MIP_LINEAR,
    D3D11_MAP_WRITE_DISCARD, D3D11_SAMPLER_DESC, D3D11_TEXTURE_ADDRESS_CLAMP, D3D11_USAGE_DYNAMIC,
    D3D11_VIEWPORT,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_IGNORE, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    IDXGIFactory2, IDXGISwapChain1, DXGI_SCALING_NONE, DXGI_SWAP_CHAIN_DESC1,
    DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL, DXGI_USAGE_RENDER_TARGET_OUTPUT,
};

pub struct MonitorRenderContext {
    pub hwnd: HWND,
    pub swapchain: IDXGISwapChain1,
    pub rtv: ID3D11RenderTargetView,
    pub bounds: RECT,
}

pub struct TransitionEngine {
    d3d_context: Arc<D3D11Context>,
    monitors: Vec<MonitorRenderContext>,
    sampler: ID3D11SamplerState,
    vertex_shader: ID3D11VertexShader,
    pub default_easing_style: EasingStyle,
    pub default_easing_direction: EasingDirection,
    pub target_fps: u32,
    shader_dir: PathBuf,
}

unsafe impl Send for TransitionEngine {}
unsafe impl Sync for TransitionEngine {}

const DEFAULT_VS_SRC: &str = r"
struct VS_OUTPUT {
    float4 pos : SV_POSITION;
    float2 tex : TEXCOORD0;
};
VS_OUTPUT main(uint val : SV_VertexID) {
    VS_OUTPUT output;
    output.tex = float2((val << 1) & 2, val & 2);
    output.pos = float4(output.tex * float2(2.0, -2.0) + float2(-1.0, 1.0), 0.0, 1.0);
    return output;
}
";

const DEFAULT_PS_SRC: &str = r"
Texture2D textureFrom : register(t0);
Texture2D textureTo : register(t1);
SamplerState samplerState : register(s0);

cbuffer TransitionParams : register(b0) {
    float progress;
    float3 padding;
};

struct PS_INPUT {
    float4 pos : SV_POSITION;
    float2 tex : TEXCOORD0;
};

float4 main(PS_INPUT input) : SV_Target {
    if (input.tex.x < progress) {
        return textureTo.Sample(samplerState, input.tex);
    } else {
        return textureFrom.Sample(samplerState, input.tex);
    }
}
";

impl TransitionEngine {
    pub fn new(
        d3d_context: Arc<D3D11Context>,
        worker_w: HWND,
        monitors_bounds: &[RECT],
        shader_dir: PathBuf,
    ) -> Result<Self, LwError> {
        let d3d_device = d3d_context.device();

        // Query DXGI Factory to create swapchains
        let dxgi_device: windows::Win32::Graphics::Dxgi::IDXGIDevice =
            d3d_device.cast().map_err(|e| {
                LwError::Renderer(format!("Failed to cast D3D11 device to DXGI device: {e}"))
            })?;
        let dxgi_adapter = unsafe { dxgi_device.GetAdapter() }
            .map_err(|e| LwError::Renderer(format!("Failed to get DXGI adapter: {e}")))?;
        let dxgi_factory: IDXGIFactory2 = unsafe { dxgi_adapter.GetParent() }
            .map_err(|e| LwError::Renderer(format!("Failed to get DXGI factory: {e}")))?;

        // Create Sampler State
        let sampler_desc = D3D11_SAMPLER_DESC {
            Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
            AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
            ComparisonFunc: windows::Win32::Graphics::Direct3D11::D3D11_COMPARISON_NEVER,
            MinLOD: 0.0,
            MaxLOD: f32::MAX,
            ..Default::default()
        };

        let mut sampler = None;
        unsafe {
            d3d_device
                .CreateSamplerState(&sampler_desc, Some(&mut sampler))
                .map_err(|e| LwError::Renderer(format!("Failed to create SamplerState: {e}")))?;
        }
        let sampler =
            sampler.ok_or_else(|| LwError::Renderer("SamplerState is null".to_string()))?;

        // Compile Vertex Shader
        let vs_blob = shader::compile_shader(DEFAULT_VS_SRC, "main", "vs_5_0")?;
        let mut vertex_shader = None;
        unsafe {
            d3d_device
                .CreateVertexShader(
                    std::slice::from_raw_parts(
                        vs_blob.GetBufferPointer() as *const u8,
                        vs_blob.GetBufferSize(),
                    ),
                    None,
                    Some(&mut vertex_shader),
                )
                .map_err(|e| LwError::Renderer(format!("Failed to create VertexShader: {e}")))?;
        }
        let vertex_shader =
            vertex_shader.ok_or_else(|| LwError::Renderer("VertexShader is null".to_string()))?;

        let mut monitors = Vec::new();

        // Allocate a separate window and swapchain per monitor
        for bounds in monitors_bounds {
            let width = u32::try_from(bounds.right - bounds.left).unwrap_or(0);
            let height = u32::try_from(bounds.bottom - bounds.top).unwrap_or(0);

            // Create a child overlay window for this monitor parented to worker_w
            let hwnd = create_overlay_window(worker_w, *bounds)?;

            // Create swapchain for this monitor's HWND
            let swapchain_desc = DXGI_SWAP_CHAIN_DESC1 {
                Width: width,
                Height: height,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                Stereo: FALSE,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: 2,
                Scaling: DXGI_SCALING_NONE,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                AlphaMode: DXGI_ALPHA_MODE_IGNORE,
                Flags: 0,
            };

            let swapchain = unsafe {
                dxgi_factory
                    .CreateSwapChainForHwnd(
                        d3d_device,
                        hwnd,
                        &swapchain_desc,
                        None,
                        None,
                    )
                    .map_err(|e| {
                        LwError::Renderer(format!(
                            "Failed to create swapchain for HWND: {e}"
                        ))
                    })?
            };

            // Get Render Target View (RTV)
            let back_buffer: ID3D11Texture2D = unsafe {
                swapchain.GetBuffer(0)
            }
            .map_err(|e| LwError::Renderer(format!("Failed to get swapchain back buffer: {e}")))?;

            let mut rtv = None;
            unsafe {
                d3d_device.CreateRenderTargetView(&back_buffer, None, Some(&mut rtv)).map_err(
                    |e| LwError::Renderer(format!("Failed to create RenderTargetView: {e}")),
                )?;
            }
            let rtv =
                rtv.ok_or_else(|| LwError::Renderer("RenderTargetView is null".to_string()))?;

            monitors.push(MonitorRenderContext { hwnd, swapchain, rtv, bounds: *bounds });
        }

        Ok(Self {
            d3d_context,
            monitors,
            sampler,
            vertex_shader,
            default_easing_style: EasingStyle::Quad,
            default_easing_direction: EasingDirection::InOut,
            target_fps: 60,
            shader_dir,
        })
    }

    pub fn new_from_existing(
        d3d_context: Arc<D3D11Context>,
        existing: Vec<(HWND, IDXGISwapChain1, RECT)>,
        shader_dir: PathBuf,
    ) -> Result<Self, LwError> {
        let d3d_device = d3d_context.device();

        // Create Sampler State
        let sampler_desc = D3D11_SAMPLER_DESC {
            Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
            AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
            ComparisonFunc: windows::Win32::Graphics::Direct3D11::D3D11_COMPARISON_NEVER,
            MinLOD: 0.0,
            MaxLOD: f32::MAX,
            ..Default::default()
        };

        let mut sampler = None;
        unsafe {
            d3d_device
                .CreateSamplerState(&sampler_desc, Some(&mut sampler))
                .map_err(|e| LwError::Renderer(format!("Failed to create SamplerState: {e}")))?;
        }
        let sampler =
            sampler.ok_or_else(|| LwError::Renderer("SamplerState is null".to_string()))?;

        // Compile Vertex Shader
        let vs_blob = shader::compile_shader(DEFAULT_VS_SRC, "main", "vs_5_0")?;
        let mut vertex_shader = None;
        unsafe {
            d3d_device
                .CreateVertexShader(
                    std::slice::from_raw_parts(
                        vs_blob.GetBufferPointer() as *const u8,
                        vs_blob.GetBufferSize(),
                    ),
                    None,
                    Some(&mut vertex_shader),
                )
                .map_err(|e| LwError::Renderer(format!("Failed to create VertexShader: {e}")))?;
        }
        let vertex_shader =
            vertex_shader.ok_or_else(|| LwError::Renderer("VertexShader is null".to_string()))?;

        let mut monitors = Vec::new();

        for (hwnd, swapchain, bounds) in existing {
            // Get Render Target View (RTV)
            let back_buffer: ID3D11Texture2D = unsafe {
                swapchain.GetBuffer(0)
            }
            .map_err(|e| LwError::Renderer(format!("Failed to get swapchain back buffer: {e}")))?;

            let mut rtv = None;
            unsafe {
                d3d_device.CreateRenderTargetView(&back_buffer, None, Some(&mut rtv)).map_err(
                    |e| LwError::Renderer(format!("Failed to create RenderTargetView: {e}")),
                )?;
            }
            let rtv =
                rtv.ok_or_else(|| LwError::Renderer("RenderTargetView is null".to_string()))?;

            monitors.push(MonitorRenderContext { hwnd, swapchain, rtv, bounds });
        }

        Ok(Self {
            d3d_context,
            monitors,
            sampler,
            vertex_shader,
            default_easing_style: EasingStyle::Quad,
            default_easing_direction: EasingDirection::InOut,
            target_fps: 60,
            shader_dir,
        })
    }

    /// Takes ownership of the overlay window handles, swapchains, and bounds, preventing Drop from destroying them.
    /// The caller is responsible for destroying these windows later (e.g., at the start of the next transition).
    pub fn take_overlay_contexts_with_bounds(&mut self) -> Vec<(HWND, IDXGISwapChain1, RECT)> {
        let contexts: Vec<(HWND, IDXGISwapChain1, RECT)> = self.monitors
            .iter()
            .map(|m| (m.hwnd, m.swapchain.clone(), m.bounds))
            .collect();
        // Clear the monitors so Drop doesn't destroy them
        self.monitors.clear();
        tracing::info!("Took {} overlay contexts with bounds for persistent display.", contexts.len());
        contexts
    }

    /// Renders a transition from an old wallpaper image to a new wallpaper image, executing a callback
    /// immediately after the very first frame is presented.
    pub fn render_transition_with_callback<F>(
        &self,
        from_image: &Path,
        to_image: &Path,
        duration_ms: u32,
        effect_type: &str,
        on_first_frame: F,
    ) -> Result<(), LwError>
    where
        F: FnOnce(),
    {
        let d3d_device = self.d3d_context.device();
        let d3d_device_context = self.d3d_context.device_context();

        tracing::info!(
            "Starting GPU transition. Effect: {}, Duration: {}ms, From: {:?}, To: {:?}",
            effect_type, duration_ms, from_image, to_image
        );

        // 1. Load textures
        let (_tex_from, srv_from, _width_from, _height_from) =
            load_texture_from_file(d3d_device, from_image)?;
        let (_tex_to, srv_to, _width_to, _height_to) =
            load_texture_from_file(d3d_device, to_image)?;

        tracing::info!(
            "Transition textures loaded. From size: {}x{}, To size: {}x{}",
            _width_from, _height_from, _width_to, _height_to
        );

        // 2. Load and Compile Pixel Shader
        let mut shader_path = self.shader_dir.join(format!("{effect_type}.hlsl"));
        if !shader_path.exists() {
            if let Ok(exe_path) = std::env::current_exe() {
                if let Some(exe_dir) = exe_path.parent() {
                    let local_shader_path = exe_dir.join("shaders").join(format!("{effect_type}.hlsl"));
                    if local_shader_path.exists() {
                        shader_path = local_shader_path;
                    }
                }
            }
        }
        let ps_src = match std::fs::read_to_string(&shader_path) {
            Ok(src) => src,
            Err(e) => {
                tracing::warn!(
                    "Failed to read shader file at {}: {}. Falling back to default fade shader.",
                    shader_path.display(),
                    e
                );
                DEFAULT_PS_SRC.to_string()
            }
        };

        let ps_blob = shader::compile_shader(&ps_src, "main", "ps_5_0")?;
        let mut pixel_shader = None;
        unsafe {
            d3d_device
                .CreatePixelShader(
                    std::slice::from_raw_parts(
                        ps_blob.GetBufferPointer() as *const u8,
                        ps_blob.GetBufferSize(),
                    ),
                    None,
                    Some(&mut pixel_shader),
                )
                .map_err(|e| LwError::Renderer(format!("Failed to create PixelShader: {e}")))?;
        }
        let pixel_shader =
            pixel_shader.ok_or_else(|| LwError::Renderer("PixelShader is null".to_string()))?;

        // 3. Create Constant Buffer for transition progress
        let cb_desc = D3D11_BUFFER_DESC {
            ByteWidth: 16,
            Usage: D3D11_USAGE_DYNAMIC,
            BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
            CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
            MiscFlags: 0,
            StructureByteStride: 0,
        };

        let mut constant_buffer = None;
        unsafe {
            d3d_device
                .CreateBuffer(&cb_desc, None, Some(&mut constant_buffer))
                .map_err(|e| LwError::Renderer(format!("Failed to create constant buffer: {e}")))?;
        }
        let constant_buffer = constant_buffer
            .ok_or_else(|| LwError::Renderer("Constant buffer is null".to_string()))?;

        // 4. Render Loop
        let start_time = std::time::Instant::now();
        let duration = std::time::Duration::from_millis(u64::from(duration_ms));
        let mut frame_count = 0;
        let fps = if self.target_fps == 0 { 60 } else { self.target_fps };
        let target_frame_time = std::time::Duration::from_nanos(1_000_000_000 / u64::from(fps));
        let mut on_first_frame = Some(on_first_frame);

        tracing::info!("Entering GPU transition loop. Duration: {:?}.", duration);

        loop {
            let frame_start = std::time::Instant::now();
            let elapsed = start_time.elapsed();
            if elapsed >= duration {
                break;
            }

            let t = elapsed.as_secs_f32() / duration.as_secs_f32();
            let progress = easing::interpolate(t, self.default_easing_style, self.default_easing_direction);

            if frame_count % 30 == 0 {
                tracing::info!(
                    "GPU render loop - Frame: {}, Progress: {:.4}, Elapsed: {:?}",
                    frame_count, progress, elapsed
                );
            }
            frame_count += 1;

            // Update constant buffer
            unsafe {
                let mut mapped_resource =
                    windows::Win32::Graphics::Direct3D11::D3D11_MAPPED_SUBRESOURCE::default();
                d3d_device_context
                    .Map(
                        &constant_buffer,
                        0,
                        D3D11_MAP_WRITE_DISCARD,
                        0,
                        Some(&mut mapped_resource),
                    )
                    .map_err(|e| {
                        LwError::Renderer(format!("Failed to map constant buffer: {e}"))
                    })?;

                let data_ptr = mapped_resource.pData.cast::<f32>();
                *data_ptr = progress;

                d3d_device_context.Unmap(&constant_buffer, 0);
            }

            // Draw frame for each monitor
            for (i, monitor) in self.monitors.iter().enumerate() {
                let width = u32::try_from(monitor.bounds.right - monitor.bounds.left).unwrap_or(0);
                let height = u32::try_from(monitor.bounds.bottom - monitor.bounds.top).unwrap_or(0);

                let viewport = D3D11_VIEWPORT {
                    TopLeftX: 0.0,
                    TopLeftY: 0.0,
                    Width: f32::from(u16::try_from(width).unwrap_or(0)),
                    Height: f32::from(u16::try_from(height).unwrap_or(0)),
                    MinDepth: 0.0,
                    MaxDepth: 1.0,
                };

                unsafe {
                    d3d_device_context.RSSetViewports(Some(&[viewport]));
                    d3d_device_context.OMSetRenderTargets(Some(&[Some(monitor.rtv.clone())]), None);

                    let clear_color = [0.0f32, 0.0, 0.0, 0.0];
                    d3d_device_context.ClearRenderTargetView(&monitor.rtv, &clear_color);

                    d3d_device_context.IASetPrimitiveTopology(
                        windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
                    );
                    d3d_device_context.VSSetShader(&self.vertex_shader, None);
                    d3d_device_context.PSSetShader(&pixel_shader, None);

                    d3d_device_context.PSSetShaderResources(
                        0,
                        Some(&[Some(srv_from.clone()), Some(srv_to.clone())]),
                    );
                    d3d_device_context.PSSetSamplers(0, Some(&[Some(self.sampler.clone())]));
                    d3d_device_context
                        .PSSetConstantBuffers(0, Some(&[Some(constant_buffer.clone())]));

                    d3d_device_context.Draw(3, 0);

                    // Present: V-sync (1) on primary, immediate (0) on others
                    let sync_interval = if i == 0 { 1 } else { 0 };
                    let present_res = monitor.swapchain.Present(sync_interval, 0);
                    if let Err(e) = present_res.ok() {
                        tracing::error!("DXGI swapchain Present failed on monitor {}: {:?}", i, e);
                    }
                }
            }

            // Execute callback immediately after the first frame is presented
            if let Some(cb) = on_first_frame.take() {
                cb();
            }

            // Pump Win32 messages for the overlay window to keep it responsive and allow painting
            unsafe {
                let mut msg = MSG::default();
                while PeekMessageW(&mut msg, HWND(0), 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }

            // Cap framerate to 60 FPS to save CPU/GPU cycles
            let elapsed_frame = frame_start.elapsed();
            if elapsed_frame < target_frame_time {
                std::thread::sleep(target_frame_time - elapsed_frame);
            }
        }

        tracing::info!("GPU transition loop finished after {} frames. Rendering final frame...", frame_count);

        // Final Frame: present target image fully
        unsafe {
            let mut mapped_resource =
                windows::Win32::Graphics::Direct3D11::D3D11_MAPPED_SUBRESOURCE::default();
            let _ = d3d_device_context.Map(
                &constant_buffer,
                0,
                D3D11_MAP_WRITE_DISCARD,
                0,
                Some(&mut mapped_resource),
            );
            *(mapped_resource.pData.cast::<f32>()) = 1.0f32;
            d3d_device_context.Unmap(&constant_buffer, 0);

            for (i, monitor) in self.monitors.iter().enumerate() {
                let width = u32::try_from(monitor.bounds.right - monitor.bounds.left).unwrap_or(0);
                let height = u32::try_from(monitor.bounds.bottom - monitor.bounds.top).unwrap_or(0);

                let viewport = D3D11_VIEWPORT {
                    TopLeftX: 0.0,
                    TopLeftY: 0.0,
                    Width: f32::from(u16::try_from(width).unwrap_or(0)),
                    Height: f32::from(u16::try_from(height).unwrap_or(0)),
                    MinDepth: 0.0,
                    MaxDepth: 1.0,
                };

                d3d_device_context.RSSetViewports(Some(&[viewport]));
                d3d_device_context.OMSetRenderTargets(Some(&[Some(monitor.rtv.clone())]), None);

                let clear_color = [0.0f32, 0.0, 0.0, 0.0];
                d3d_device_context.ClearRenderTargetView(&monitor.rtv, &clear_color);

                d3d_device_context.IASetPrimitiveTopology(
                    windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
                );
                d3d_device_context.VSSetShader(&self.vertex_shader, None);
                d3d_device_context.PSSetShader(&pixel_shader, None);

                d3d_device_context.PSSetShaderResources(
                    0,
                    Some(&[Some(srv_from.clone()), Some(srv_to.clone())]),
                );
                d3d_device_context.PSSetSamplers(0, Some(&[Some(self.sampler.clone())]));
                d3d_device_context
                    .PSSetConstantBuffers(0, Some(&[Some(constant_buffer.clone())]));

                d3d_device_context.Draw(3, 0);

                let sync_interval = if i == 0 { 1 } else { 0 };
                let _ = monitor.swapchain.Present(sync_interval, 0);
            }
        }

        // Unbind resources
        unsafe {
            d3d_device_context.ClearState();
        }

        tracing::info!("GPU transition completed.");
        Ok(())
    }
}

impl TransitionRenderer for TransitionEngine {
    fn render_transition(
        &self,
        from_image: &Path,
        to_image: &Path,
        duration_ms: u32,
        effect_type: &str,
    ) -> Result<(), LwError> {
        self.render_transition_with_callback(from_image, to_image, duration_ms, effect_type, || {})
    }
}

impl Drop for TransitionEngine {
    fn drop(&mut self) {
        unsafe {
            for monitor in &self.monitors {
                let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(monitor.hwnd);
            }
        }
    }
}
