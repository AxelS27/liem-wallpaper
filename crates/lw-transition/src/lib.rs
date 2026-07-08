pub mod easing;
pub mod shader;

pub use easing::interpolate;

use lw_core::error::LwError;
use lw_core::traits::TransitionRenderer;
use lw_core::EasingType;
use lw_renderer::{load_texture_from_file, CompositionContext, D3D11Context};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use windows::core::ComInterface;
use windows::Win32::Foundation::{FALSE, RECT};
use windows::Win32::Graphics::Direct3D11::{
    ID3D11RenderTargetView, ID3D11SamplerState, ID3D11VertexShader, D3D11_BIND_CONSTANT_BUFFER,
    D3D11_BUFFER_DESC, D3D11_CPU_ACCESS_WRITE, D3D11_FILTER_MIN_MAG_MIP_LINEAR,
    D3D11_MAP_WRITE_DISCARD, D3D11_SAMPLER_DESC, D3D11_TEXTURE_ADDRESS_CLAMP, D3D11_USAGE_DYNAMIC,
    D3D11_VIEWPORT,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    IDXGIFactory2, IDXGISwapChain1, DXGI_SCALING_STRETCH, DXGI_SWAP_CHAIN_DESC1,
    DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL, DXGI_USAGE_RENDER_TARGET_OUTPUT,
};

pub struct MonitorRenderContext {
    pub swapchain: IDXGISwapChain1,
    pub rtv: ID3D11RenderTargetView,
    pub visual: windows::Win32::Graphics::DirectComposition::IDCompositionVisual,
    pub bounds: RECT,
}

pub struct TransitionEngine {
    d3d_context: Arc<D3D11Context>,
    comp_context: Arc<CompositionContext>,
    monitors: Vec<MonitorRenderContext>,
    sampler: ID3D11SamplerState,
    vertex_shader: ID3D11VertexShader,
    default_easing: EasingType,
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
    float4 colorFrom = textureFrom.Sample(samplerState, input.tex);
    float4 colorTo = textureTo.Sample(samplerState, input.tex);
    return lerp(colorFrom, colorTo, progress);
}
";

impl TransitionEngine {
    pub fn new(
        d3d_context: Arc<D3D11Context>,
        comp_context: &Arc<CompositionContext>,
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

        // Allocate a separate swapchain and composition visual per monitor
        for bounds in monitors_bounds {
            let width = u32::try_from(bounds.right - bounds.left).unwrap_or(0);
            let height = u32::try_from(bounds.bottom - bounds.top).unwrap_or(0);

            // Create child visual in DComposition visual tree
            let visual = unsafe { comp_context.device().CreateVisual() }
                .map_err(|e| LwError::Renderer(format!("Failed to create child visual: {e}")))?;

            unsafe {
                #[allow(clippy::cast_precision_loss)]
                visual.SetOffsetX2(bounds.left as f32).map_err(|e| {
                    LwError::Renderer(format!("Failed to set visual offset X: {e}"))
                })?;
                #[allow(clippy::cast_precision_loss)]
                visual.SetOffsetY2(bounds.top as f32).map_err(|e| {
                    LwError::Renderer(format!("Failed to set visual offset Y: {e}"))
                })?;
            }

            // Create swapchain for this monitor's bounds
            let swapchain_desc = DXGI_SWAP_CHAIN_DESC1 {
                Width: width,
                Height: height,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                Stereo: FALSE,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: 2,
                Scaling: DXGI_SCALING_STRETCH,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
                Flags: 0,
            };

            let swapchain = unsafe {
                dxgi_factory
                    .CreateSwapChainForComposition(d3d_device, &swapchain_desc, None)
                    .map_err(|e| {
                        LwError::Renderer(format!(
                            "Failed to create swapchain for composition: {e}"
                        ))
                    })?
            };

            // Bind swapchain and attach to root visual tree
            unsafe {
                visual
                    .SetContent(&swapchain)
                    .map_err(|e| LwError::Renderer(format!("Failed to set visual content: {e}")))?;
                comp_context.root_visual().AddVisual(&visual, true, None).map_err(|e| {
                    LwError::Renderer(format!("Failed to add visual to root tree: {e}"))
                })?;
            }

            // Get Render Target View (RTV)
            let back_buffer: windows::Win32::Graphics::Direct3D11::ID3D11Texture2D = unsafe {
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

            monitors.push(MonitorRenderContext { swapchain, rtv, visual, bounds: *bounds });
        }

        comp_context.commit()?;

        Ok(Self {
            d3d_context,
            comp_context: Arc::clone(comp_context),
            monitors,
            sampler,
            vertex_shader,
            default_easing: EasingType::EaseInOut,
            shader_dir,
        })
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
        let d3d_device = self.d3d_context.device();
        let d3d_device_context = self.d3d_context.device_context();

        // 1. Load textures
        let (_tex_from, srv_from, _width_from, _height_from) =
            load_texture_from_file(d3d_device, from_image)?;
        let (_tex_to, srv_to, _width_to, _height_to) =
            load_texture_from_file(d3d_device, to_image)?;

        // 2. Load and Compile Pixel Shader
        let shader_path = self.shader_dir.join(format!("{effect_type}.hlsl"));
        let ps_src =
            std::fs::read_to_string(&shader_path).unwrap_or_else(|_| DEFAULT_PS_SRC.to_string());

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

        loop {
            let elapsed = start_time.elapsed();
            if elapsed >= duration {
                break;
            }

            let t = elapsed.as_secs_f32() / duration.as_secs_f32();
            let progress = easing::interpolate(t, self.default_easing);

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
            for monitor in &self.monitors {
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

                    // Present immediately
                    let _ = monitor.swapchain.Present(0, 0);
                }
            }

            // Commit atomic update for all monitors
            let _ = self.comp_context.commit();

            // V-sync on primary monitor
            if !self.monitors.is_empty() {
                unsafe {
                    let _ = self.monitors[0].swapchain.Present(1, 0);
                }
            }
        }

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

            for monitor in &self.monitors {
                d3d_device_context.ClearRenderTargetView(&monitor.rtv, &[0.0f32, 0.0, 0.0, 0.0]);
                d3d_device_context.Draw(3, 0);
                let _ = monitor.swapchain.Present(0, 0);
            }
            let _ = self.comp_context.commit();
            if !self.monitors.is_empty() {
                let _ = self.monitors[0].swapchain.Present(1, 0);
            }
        }

        // Unbind resources
        unsafe {
            d3d_device_context.ClearState();
        }

        Ok(())
    }
}

impl Drop for TransitionEngine {
    fn drop(&mut self) {
        unsafe {
            let _ = self.comp_context.root_visual().RemoveAllVisuals();
            let _ = self.comp_context.commit();
        }
    }
}
