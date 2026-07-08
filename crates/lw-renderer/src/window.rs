use lw_core::error::LwError;
use windows::Win32::Foundation::{HWND, RECT, LPARAM, BOOL, TRUE, FALSE};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, FindWindowExW, SendMessageTimeoutW, EnumWindows, SMTO_NORMAL,
    GetClassNameW, SetWindowPos, GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE,
    WS_EX_TRANSPARENT, WS_EX_LAYERED, WS_EX_NOACTIVATE, SWP_NOZORDER,
    SWP_SHOWWINDOW, SWP_NOACTIVATE,
};
use windows::core::w;

struct SearchContext {
    worker_w: HWND,
}

// Standard EnumWindows callback to locate the spawned WorkerW sibling window.
unsafe extern "system" fn enum_windows_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let context = &mut *(lparam.0 as *mut SearchContext);

    let shell_view = FindWindowExW(hwnd, None, w!("SHELLDLL_DefView"), None);
    if shell_view.0 != 0 {
        // FindWindowExW with parent = None starts the search from the specified child sibling window.
        let sibling = FindWindowExW(None, hwnd, w!("WorkerW"), None);
        if sibling.0 != 0 {
            context.worker_w = sibling;
        }
    }
    TRUE
}

// Fallback search to scan all top-level WorkerW windows and return the empty one.
unsafe extern "system" fn fallback_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let context = &mut *(lparam.0 as *mut SearchContext);

    let mut class_name = [0u16; 256];
    let len = GetClassNameW(hwnd, &mut class_name);
    if len > 0 {
        let class_str = String::from_utf16_lossy(&class_name[..usize::try_from(len).unwrap_or(0)]);
        if class_str == "WorkerW" {
            let shell_view = FindWindowExW(hwnd, None, w!("SHELLDLL_DefView"), None);
            if shell_view.0 == 0 {
                context.worker_w = hwnd;
                return FALSE; // Stop enumeration once empty WorkerW is found
            }
        }
    }
    TRUE
}

/// Locates the Win32 `WorkerW` window that serves as the desktop background wallpaper container.
/// If `WorkerW` cannot be found or created, it falls back to the top-level `Progman` window.
pub fn find_worker_w() -> Result<HWND, LwError> {
    let progman = unsafe { FindWindowW(w!("Progman"), None) };
    if progman.0 == 0 {
        return Err(LwError::Renderer("Progman window not found".to_string()));
    }

    let mut result = 0;
    unsafe {
        // Send 0x052C to Progman. This triggers the creation of a WorkerW window behind icons.
        let _ = SendMessageTimeoutW(
            progman,
            0x052C,
            None,
            None,
            SMTO_NORMAL,
            1000,
            Some(std::ptr::addr_of_mut!(result)),
        );
    }

    let mut ctx = SearchContext { worker_w: HWND(0) };

    unsafe {
        let _ = EnumWindows(Some(enum_windows_callback), LPARAM(std::ptr::addr_of_mut!(ctx) as isize));
    }

    if ctx.worker_w.0 == 0 {
        unsafe {
            let _ = EnumWindows(Some(fallback_callback), LPARAM(std::ptr::addr_of_mut!(ctx) as isize));
        }
    }

    // If both direct discovery and fallback fail, return Progman as our last resort.
    if ctx.worker_w.0 == 0 {
        Ok(progman)
    } else {
        Ok(ctx.worker_w)
    }
}

/// Modifies the target window styles to make it completely click-through,
/// transparent, and prevents it from grabbing keyboard focus.
pub fn set_click_through(hwnd: HWND) -> Result<(), LwError> {
    unsafe {
        let current_exstyle = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        // Cast U32 flags to i32 first to prevent clippy warning on 32-bit targets
        let flags = i32::try_from(WS_EX_TRANSPARENT.0 | WS_EX_LAYERED.0 | WS_EX_NOACTIVATE.0).unwrap_or(0);
        let new_exstyle = current_exstyle | flags as isize;
        
        // Set the modified styles back
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_exstyle);
    }
    Ok(())
}

/// Moves and resizes the target window to cover the specified monitor bounding box bounds.
pub fn position_window(hwnd: HWND, rect: RECT) -> Result<(), LwError> {
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;

    unsafe {
        SetWindowPos(
            hwnd,
            HWND(0),
            rect.left,
            rect.top,
            width,
            height,
            SWP_NOZORDER | SWP_SHOWWINDOW | SWP_NOACTIVATE,
        )
        .map_err(|e| LwError::Renderer(format!("Failed to position window: {e}")))?;
    }
    Ok(())
}
