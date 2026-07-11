use std::path::PathBuf;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DispatchMessageW, GetCursorPos,
    GetMessageW, LoadImageW, RegisterClassW, SetForegroundWindow, TrackPopupMenu, TranslateMessage,
    HICON, IMAGE_ICON, LR_LOADFROMFILE, MF_STRING, MSG, TPM_RETURNCMD,
    WM_RBUTTONUP, WM_USER, WNDCLASSW,
};

const WM_TRAYICON: u32 = WM_USER + 100;
const TRAY_ICON_ID: u32 = 1;

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TRAYICON => {
            let event = lparam.0 as u32;
            if event == WM_RBUTTONUP {
                // Show Context Menu
                let mut pos = windows::Win32::Foundation::POINT::default();
                let _ = GetCursorPos(&mut pos);

                let hmenu = CreatePopupMenu().unwrap();
                let _ = AppendMenuW(
                    hmenu,
                    MF_STRING,
                    1,
                    PCWSTR(
                        "Skip Wallpaper"
                            .encode_utf16()
                            .chain(std::iter::once(0))
                            .collect::<Vec<u16>>()
                            .as_ptr(),
                    ),
                );
                let _ = AppendMenuW(
                    hmenu,
                    MF_STRING,
                    2,
                    PCWSTR(
                        "Exit"
                            .encode_utf16()
                            .chain(std::iter::once(0))
                            .collect::<Vec<u16>>()
                            .as_ptr(),
                    ),
                );

                let _ = SetForegroundWindow(hwnd);
                let command = TrackPopupMenu(hmenu, TPM_RETURNCMD, pos.x, pos.y, 0, hwnd, None);

                match command.0 {
                    1 => {
                        // Skip Wallpaper via Named Pipe
                        tokio::spawn(async {
                            let client = tokio::net::windows::named_pipe::ClientOptions::new()
                                .open(r"\\.\pipe\liem-wallpaper");
                            if let Ok(mut pipe) = client {
                                use tokio::io::AsyncWriteExt;
                                let req = lw_core::ipc::IpcRequest::NextWallpaper;
                                if let Ok(bytes) = serde_json::to_vec(&req) {
                                    let mut payload = bytes;
                                    payload.push(b'\n');
                                    let _ = pipe.write_all(&payload).await;
                                }
                            }
                        });
                    }
                    2 => {
                        // Exit daemon
                        let _ = Shell_NotifyIconW(
                            NIM_DELETE,
                            &get_tray_icon_data(hwnd, HICON::default()),
                        );
                        std::process::exit(0);
                    }
                    _ => {}
                }
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn get_tray_icon_data(hwnd: HWND, hicon: HICON) -> NOTIFYICONDATAW {
    let mut nid = NOTIFYICONDATAW::default();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_ICON_ID;
    nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    nid.uCallbackMessage = WM_TRAYICON;
    nid.hIcon = hicon;

    // Set tooltip text
    let tip = "Liem Wallpaper";
    let tip_w: Vec<u16> = tip.encode_utf16().chain(std::iter::once(0)).collect();
    let len = tip_w.len().min(nid.szTip.len());
    nid.szTip[..len].copy_from_slice(&tip_w[..len]);

    nid
}

pub fn start_tray_icon() {
    std::thread::spawn(|| unsafe {
        let class_name: Vec<u16> =
            "LiemWallpaperTrayClass".encode_utf16().chain(std::iter::once(0)).collect();
        let instance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap();

        let wnd_class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };

        RegisterClassW(&wnd_class);

        let hwnd = CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
            PCWSTR(class_name.as_ptr()),
            PCWSTR(std::ptr::null()),
            windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            HWND::default(),
            windows::Win32::UI::WindowsAndMessaging::HMENU::default(),
            instance,
            None,
        );

        // Load Icon
        let exe_path = std::env::current_exe().unwrap_or_default();
        let install_dir = exe_path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."));
        let icon_path = install_dir.join("icon.ico");
        let icon_path_w: Vec<u16> =
            icon_path.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();

        let hicon =
            LoadImageW(None, PCWSTR(icon_path_w.as_ptr()), IMAGE_ICON, 0, 0, LR_LOADFROMFILE);

        if let Ok(hicon) = hicon {
            let hicon = HICON(hicon.0);
            let nid = get_tray_icon_data(hwnd, hicon);
            let _ = Shell_NotifyIconW(NIM_ADD, &nid);
        }

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, HWND::default(), 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
}
