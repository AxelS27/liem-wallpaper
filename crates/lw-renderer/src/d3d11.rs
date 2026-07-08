use lw_core::error::LwError;
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL, D3D_FEATURE_LEVEL_11_0,
    D3D_FEATURE_LEVEL_11_1,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
    D3D11_SDK_VERSION, D3D11_CREATE_DEVICE_FLAG,
};

pub struct D3D11Context {
    device: ID3D11Device,
    device_context: ID3D11DeviceContext,
    feature_level: D3D_FEATURE_LEVEL,
}

impl D3D11Context {
    pub fn new() -> Result<Self, LwError> {
        let mut device = None;
        let mut device_context = None;
        let mut feature_level = D3D_FEATURE_LEVEL::default();

        let feature_levels = [
            D3D_FEATURE_LEVEL_11_1,
            D3D_FEATURE_LEVEL_11_0,
        ];

        // Enable debug layer in debug builds (if SDK is available)
        let mut flags = D3D11_CREATE_DEVICE_BGRA_SUPPORT;
        #[cfg(debug_assertions)]
        {
            // Note: In some systems, enabling the debug flag might fail if the Graphics Tools SDK is not installed.
            // So we'll try to initialize with debug first, and if it fails, fallback without debug.
            flags |= D3D11_CREATE_DEVICE_FLAG(2); // D3D11_CREATE_DEVICE_DEBUG
        }

        let res = unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                None,
                flags,
                Some(&feature_levels),
                D3D11_SDK_VERSION,
                Some(std::ptr::addr_of_mut!(device)),
                Some(std::ptr::addr_of_mut!(feature_level)),
                Some(std::ptr::addr_of_mut!(device_context)),
            )
        };

        // Fallback if D3D11 Create Device fails (e.g. debug layer not available)
        let create_res = if res.is_err() && flags != D3D11_CREATE_DEVICE_BGRA_SUPPORT {
            unsafe {
                D3D11CreateDevice(
                    None,
                    D3D_DRIVER_TYPE_HARDWARE,
                    None,
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                    Some(&feature_levels),
                    D3D11_SDK_VERSION,
                    Some(std::ptr::addr_of_mut!(device)),
                    Some(std::ptr::addr_of_mut!(feature_level)),
                    Some(std::ptr::addr_of_mut!(device_context)),
                )
            }
        } else {
            res
        };

        create_res.map_err(|e| LwError::Renderer(format!("Failed to create D3D11 device: {e}")))?;

        let device = device.ok_or_else(|| LwError::Renderer("D3D11 device is null".to_string()))?;
        let device_context = device_context.ok_or_else(|| LwError::Renderer("D3D11 context is null".to_string()))?;

        Ok(Self {
            device,
            device_context,
            feature_level,
        })
    }

    #[must_use]
    pub fn device(&self) -> &ID3D11Device {
        &self.device
    }

    #[must_use]
    pub fn device_context(&self) -> &ID3D11DeviceContext {
        &self.device_context
    }

    #[must_use]
    pub fn feature_level(&self) -> D3D_FEATURE_LEVEL {
        self.feature_level
    }
}
