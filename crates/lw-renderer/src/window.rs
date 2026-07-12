use lw_core::error::LwError;
use std::sync::Once;
use windows::core::w;
use windows::Win32::Foundation::{BOOL, FALSE, HWND, LPARAM, LRESULT, RECT, TRUE, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, EnumWindows, FindWindowExW, FindWindowW,
    GetAncestor, GetWindowLongPtrW, GetWindowRect, IsWindowVisible, RegisterClassW,
    SendMessageTimeoutW, SetWindowLongPtrW, SetWindowPos, ShowWindow, GA_PARENT, GWL_EXSTYLE,
    GWL_STYLE, SMTO_NORMAL, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
    SWP_SHOWWINDOW, SW_SHOW, WNDCLASSW, WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TRANSPARENT, WS_VISIBLE,
};

struct SearchContext {
    worker_w: HWND,
}

// Standard EnumWindows callback to locate the active desktop WorkerW window containing SHELLDLL_DefView.
unsafe extern "system" fn enum_windows_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let context = &mut *(lparam.0 as *mut SearchContext);

    let shell_view = FindWindowExW(hwnd, None, w!("SHELLDLL_DefView"), None);
    if shell_view.0 != 0 {
        context.worker_w = hwnd;
        return FALSE; // Stop enumeration once the active WorkerW is found
    }
    TRUE
}

/// Locates the Win32 `WorkerW` window that serves as the active desktop view container.
/// If `WorkerW` cannot be found or created, it falls back to the top-level `Progman` window.
pub fn find_worker_w() -> Result<HWND, LwError> {
    let progman = unsafe { FindWindowW(w!("Progman"), None) };
    if progman.0 == 0 {
        return Err(LwError::Renderer("Progman window not found".to_string()));
    }
    tracing::info!("Progman window found at handle: {:?}", progman.0);

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
        let _ =
            EnumWindows(Some(enum_windows_callback), LPARAM(std::ptr::addr_of_mut!(ctx) as isize));
    }

    // If discovery fails, return Progman as our last resort.
    if ctx.worker_w.0 == 0 {
        tracing::warn!(
            "Failed to find WorkerW containing SHELLDLL_DefView. Falling back to Progman: {:?}",
            progman.0
        );
        Ok(progman)
    } else {
        tracing::info!("Found active desktop WorkerW window at handle: {:?}", ctx.worker_w.0);
        Ok(ctx.worker_w)
    }
}

/// Modifies the target window styles to make it completely click-through,
/// transparent, and prevents it from grabbing keyboard focus.
pub fn set_click_through(hwnd: HWND) -> Result<(), LwError> {
    unsafe {
        let current_exstyle = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        // Cast U32 flags to i32 first to prevent clippy warning on 32-bit targets
        let flags =
            i32::try_from(WS_EX_TRANSPARENT.0 | WS_EX_LAYERED.0 | WS_EX_NOACTIVATE.0).unwrap_or(0);
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

unsafe extern "system" fn overlay_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

static REGISTER_OVERLAY_CLASS: Once = Once::new();

/// Creates a temporary transparent overlay window parented to WorkerW for rendering transitions.
pub fn create_overlay_window(parent: HWND, rect: RECT) -> Result<HWND, LwError> {
    let instance = unsafe {
        windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
            .map_err(|e| LwError::Renderer(format!("Failed to get module handle: {e}")))?
    };

    let class_name = w!("LiemWallpaperOverlayClass");

    REGISTER_OVERLAY_CLASS.call_once(|| unsafe {
        let wnd_class = WNDCLASSW {
            lpfnWndProc: Some(overlay_window_proc),
            hInstance: instance.into(),
            lpszClassName: class_name,
            ..Default::default()
        };
        RegisterClassW(&wnd_class);
    });

    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;

    unsafe {
        // Query parent screen rect to calculate client coordinates relative to WorkerW
        let mut parent_rect = RECT::default();
        let _ = GetWindowRect(parent, &mut parent_rect);
        let x = rect.left - parent_rect.left;
        let y = rect.top - parent_rect.top;

        // Force parent WorkerW window to be visible and enable WS_CLIPCHILDREN
        // so it does not draw its background wallpaper over our child transition overlay window.
        let parent_visible_before = IsWindowVisible(parent).as_bool();
        let _ = ShowWindow(parent, SW_SHOW);
        let parent_visible_after = IsWindowVisible(parent).as_bool();
        tracing::info!(
            "Parent WorkerW visibility - Before ShowWindow: {}, After ShowWindow: {}",
            parent_visible_before,
            parent_visible_after
        );

        let parent_style = GetWindowLongPtrW(parent, GWL_STYLE);
        if (parent_style & (WS_CLIPCHILDREN.0 as isize)) == 0 {
            let new_parent_style = parent_style | (WS_CLIPCHILDREN.0 as isize);
            let _ = SetWindowLongPtrW(parent, GWL_STYLE, new_parent_style);
            let _ = SetWindowPos(
                parent,
                HWND(0),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOZORDER | SWP_FRAMECHANGED,
            );
            tracing::info!(
                "Applied WS_CLIPCHILDREN to parent WorkerW. Style before: 0x{:X}, after: 0x{:X}",
                parent_style,
                new_parent_style
            );
        }

        // Create child window directly parented to WorkerW
        let hwnd = CreateWindowExW(
            WS_EX_TRANSPARENT | WS_EX_NOACTIVATE,
            class_name,
            w!("Liem Wallpaper Transition Overlay"),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
            x,
            y,
            width,
            height,
            parent,
            None,
            instance,
            None,
        );

        if hwnd.0 == 0 {
            return Err(LwError::Renderer("Failed to create overlay window".to_string()));
        }
        tracing::info!(
            "Created child overlay HWND: {:?} parented to {:?} at relative position: ({}, {}) size: {}x{}",
            hwnd.0, parent.0, x, y, width, height
        );

        // Position our overlay window immediately after (behind) SHELLDLL_DefView sibling
        let shell_view = FindWindowExW(parent, None, w!("SHELLDLL_DefView"), None);
        if shell_view.0 != 0 {
            let _ = SetWindowPos(
                hwnd,
                shell_view, // Place behind SHELLDLL_DefView
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            tracing::info!(
                "Positioned child overlay HWND: {:?} behind SHELLDLL_DefView: {:?}",
                hwnd.0,
                shell_view.0
            );
        } else {
            // Sibling view not found, fallback to HWND_TOP
            let _ = SetWindowPos(
                hwnd,
                HWND(0), // HWND_TOP
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }

        // Verify parent was set correctly
        let current_parent = GetAncestor(hwnd, GA_PARENT);
        tracing::info!("Verified parent HWND via GetAncestor: {:?}", current_parent.0);

        // Window state diagnostics
        let is_visible = IsWindowVisible(hwnd).as_bool();
        let mut window_rect = RECT::default();
        let _ = GetWindowRect(hwnd, &mut window_rect);
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        let exstyle = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        tracing::info!(
            "Diagnostic Window Info - IsVisible: {}, Rect: [L:{}, T:{}, R:{}, B:{}], Style: 0x{:X}, ExStyle: 0x{:X}",
            is_visible, window_rect.left, window_rect.top, window_rect.right, window_rect.bottom, style, exstyle
        );

        if current_parent != parent {
            let _ = DestroyWindow(hwnd);
            return Err(LwError::Renderer("Failed to set overlay parent to WorkerW".to_string()));
        }

        Ok(hwnd)
    }
}
