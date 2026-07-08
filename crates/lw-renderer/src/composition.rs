use lw_core::error::LwError;
use windows::core::ComInterface;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D11::ID3D11Device;
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice, IDCompositionDevice, IDCompositionTarget, IDCompositionVisual,
};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;

pub struct CompositionContext {
    device: IDCompositionDevice,
    target: IDCompositionTarget,
    root_visual: IDCompositionVisual,
}

impl CompositionContext {
    pub fn new(d3d_device: &ID3D11Device, hwnd: HWND) -> Result<Self, LwError> {
        let dxgi_device: IDXGIDevice = d3d_device.cast().map_err(|e| {
            LwError::Renderer(format!("Failed to query IDXGIDevice from D3D11 device: {e}"))
        })?;

        let device: IDCompositionDevice = unsafe {
            DCompositionCreateDevice(&dxgi_device).map_err(|e| {
                LwError::Renderer(format!("Failed to create DComposition device: {e}"))
            })?
        };

        let target = unsafe {
            device.CreateTargetForHwnd(hwnd, true).map_err(|e| {
                LwError::Renderer(format!("Failed to create DComposition target: {e}"))
            })?
        };

        let root_visual = unsafe {
            device.CreateVisual().map_err(|e| {
                LwError::Renderer(format!("Failed to create DComposition root visual: {e}"))
            })?
        };

        unsafe {
            target.SetRoot(&root_visual).map_err(|e| {
                LwError::Renderer(format!("Failed to set root visual on target: {e}"))
            })?;
            device.Commit().map_err(|e| {
                LwError::Renderer(format!("Failed to commit DComposition device: {e}"))
            })?;
        }

        Ok(Self { device, target, root_visual })
    }

    #[must_use]
    pub fn device(&self) -> &IDCompositionDevice {
        &self.device
    }

    #[must_use]
    pub fn target(&self) -> &IDCompositionTarget {
        &self.target
    }

    #[must_use]
    pub fn root_visual(&self) -> &IDCompositionVisual {
        &self.root_visual
    }

    pub fn commit(&self) -> Result<(), LwError> {
        unsafe {
            self.device.Commit().map_err(|e| {
                LwError::Renderer(format!("Failed to commit DComposition device: {e}"))
            })?;
        }
        Ok(())
    }
}

unsafe impl Send for CompositionContext {}
unsafe impl Sync for CompositionContext {}
