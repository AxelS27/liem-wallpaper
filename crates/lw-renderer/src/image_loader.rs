use lw_core::error::LwError;
use std::path::Path;
use windows::Win32::Foundation::GENERIC_ACCESS_RIGHTS;
use windows::Win32::Graphics::Direct3D11::{
    ID3D11Device, ID3D11ShaderResourceView, ID3D11Texture2D, D3D11_BIND_SHADER_RESOURCE,
    D3D11_SUBRESOURCE_DATA, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICImagingFactory,
    WICBitmapDitherTypeNone, WICBitmapPaletteTypeCustom, WICDecodeMetadataCacheOnDemand,
};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};

const GENERIC_READ: GENERIC_ACCESS_RIGHTS = GENERIC_ACCESS_RIGHTS(0x8000_0000);

/// Decodes an image file (PNG, JPG, BMP) using Windows Imaging Component (WIC)
/// and loads it as a Direct3D 11 2D texture and Shader Resource View.
pub fn load_texture_from_file(
    d3d_device: &ID3D11Device,
    path: &Path,
) -> Result<(ID3D11Texture2D, ID3D11ShaderResourceView, u32, u32), LwError> {
    let path_str = path.to_string_lossy();
    let path_w: Vec<u16> = path_str.encode_utf16().chain(std::iter::once(0)).collect();
    let path_pcwstr = windows::core::PCWSTR(path_w.as_ptr());

    unsafe {
        // Create WIC imaging factory
        let factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).map_err(
                |e| LwError::Renderer(format!("Failed to create WIC Imaging Factory: {e}")),
            )?;

        // Create decoder from filename
        let decoder = factory
            .CreateDecoderFromFilename(
                path_pcwstr,
                None,
                GENERIC_READ,
                WICDecodeMetadataCacheOnDemand,
            )
            .map_err(|e| LwError::Renderer(format!("Failed to load image file {path_str}: {e}")))?;

        // Get first frame
        let frame = decoder
            .GetFrame(0)
            .map_err(|e| LwError::Renderer(format!("Failed to get WIC frame: {e}")))?;

        // Convert format to 32bppPBGRA
        let converter = factory.CreateFormatConverter().map_err(|e| {
            LwError::Renderer(format!("Failed to create WIC format converter: {e}"))
        })?;

        converter
            .Initialize(
                &frame,
                &GUID_WICPixelFormat32bppPBGRA,
                WICBitmapDitherTypeNone,
                None,
                0.0,
                WICBitmapPaletteTypeCustom,
            )
            .map_err(|e| {
                LwError::Renderer(format!("Failed to initialize WIC format converter: {e}"))
            })?;

        let (mut width, mut height) = (0, 0);
        converter
            .GetSize(&mut width, &mut height)
            .map_err(|e| LwError::Renderer(format!("Failed to get image size: {e}")))?;

        let stride = width * 4;
        let mut buffer = vec![0u8; (stride * height) as usize];

        converter
            .CopyPixels(std::ptr::null(), stride, &mut buffer)
            .map_err(|e| LwError::Renderer(format!("Failed to copy WIC pixels: {e}")))?;

        // Create D3D11 Texture
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };

        let subresource = D3D11_SUBRESOURCE_DATA {
            pSysMem: buffer.as_ptr().cast(),
            SysMemPitch: stride,
            SysMemSlicePitch: 0,
        };

        let mut texture: Option<ID3D11Texture2D> = None;
        d3d_device.CreateTexture2D(&desc, Some(&subresource), Some(&mut texture)).map_err(|e| {
            LwError::Renderer(format!("Failed to create D3D11 texture from WIC pixels: {e}"))
        })?;

        let texture =
            texture.ok_or_else(|| LwError::Renderer("D3D11 texture is null".to_string()))?;

        // Create SRV
        let mut srv: Option<ID3D11ShaderResourceView> = None;
        d3d_device.CreateShaderResourceView(&texture, None, Some(&mut srv)).map_err(|e| {
            LwError::Renderer(format!("Failed to create SRV for D3D11 texture: {e}"))
        })?;

        let srv = srv.ok_or_else(|| LwError::Renderer("SRV is null".to_string()))?;

        Ok((texture, srv, width, height))
    }
}
