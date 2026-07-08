use lw_core::error::LwError;
use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use windows::Win32::Graphics::Direct3D::ID3DBlob;
use windows::core::PCSTR;

/// Compiles an HLSL shader string at runtime.
/// Returns the compiled shader byte code as an `ID3DBlob` or an error containing HLSL compiler errors.
pub fn compile_shader(
    source_code: &str,
    entry_point: &str,
    target_profile: &str,
) -> Result<ID3DBlob, LwError> {
    let source_bytes = source_code.as_bytes();
    let entry_point_c = std::ffi::CString::new(entry_point)
        .map_err(|e| LwError::Other(format!("Invalid entry point: {e}")))?;
    let target_profile_c = std::ffi::CString::new(target_profile)
        .map_err(|e| LwError::Other(format!("Invalid target profile: {e}")))?;

    let mut shader_blob: Option<ID3DBlob> = None;
    let mut error_blob: Option<ID3DBlob> = None;

    let res = unsafe {
        D3DCompile(
            source_bytes.as_ptr().cast(),
            source_bytes.len(),
            None,
            None,
            None,
            PCSTR(entry_point_c.as_ptr().cast()),
            PCSTR(target_profile_c.as_ptr().cast()),
            0,
            0,
            std::ptr::addr_of_mut!(shader_blob),
            Some(std::ptr::addr_of_mut!(error_blob)),
        )
    };

    if res.is_err() {
        if let Some(err) = error_blob {
            let buffer_ptr = unsafe { err.GetBufferPointer() };
            let buffer_size = unsafe { err.GetBufferSize() };
            let error_slice = unsafe { std::slice::from_raw_parts(buffer_ptr as *const u8, buffer_size) };
            let error_str = String::from_utf8_lossy(error_slice).into_owned();
            return Err(LwError::Renderer(format!("HLSL compilation error: {error_str}")));
        }
        res.map_err(|e| LwError::Renderer(format!("HLSL compilation failed: {e}")))?;
    }

    shader_blob.ok_or_else(|| LwError::Renderer("Shader compilation returned null blob".to_string()))
}
