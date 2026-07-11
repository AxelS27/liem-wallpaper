use lw_core::{traits::WallpaperManager, IpcRequest, IpcResponse, Result};
use lw_service::ipc::{run_ipc_server, PIPE_NAME};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::ClientOptions;

struct MockWallpaperManager {
    current: Mutex<PathBuf>,
}

impl MockWallpaperManager {
    fn new() -> Self {
        Self { current: Mutex::new(PathBuf::new()) }
    }
}

impl WallpaperManager for MockWallpaperManager {
    fn get_current_wallpaper(&self) -> Result<PathBuf> {
        Ok(self.current.lock().unwrap().clone())
    }

    fn set_wallpaper(&self, path: &Path) -> Result<()> {
        *self.current.lock().unwrap() = path.to_path_buf();
        Ok(())
    }

    fn set_wallpaper_registry_only(&self, path: &Path) -> Result<()> {
        *self.current.lock().unwrap() = path.to_path_buf();
        Ok(())
    }

    fn get_monitor_rects(&self) -> Result<Vec<lw_core::traits::MonitorRect>> {
        Ok(vec![lw_core::traits::MonitorRect { left: 0, top: 0, right: 1920, bottom: 1080 }])
    }
}

#[tokio::test]
async fn test_ipc_commands() {
    let wm = Arc::new(MockWallpaperManager::new());
    let server_wm = Arc::clone(&wm);

    // Spawn server in background
    let server_handle = tokio::spawn(async move {
        let _ = run_ipc_server(
            Arc::new(Mutex::new(Default::default())),
            server_wm,
            Arc::new(Mutex::new(lw_service::scheduler::SchedulerState::default())),
        )
        .await;
    });

    // Wait a brief moment for the pipe server to start listening
    tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;

    // Connect client
    let mut client = ClientOptions::new().open(PIPE_NAME).expect("Failed to connect to named pipe");

    // 1. Send GetStatus request
    let request = IpcRequest::GetStatus;
    let mut request_bytes = serde_json::to_vec(&request).unwrap();
    request_bytes.push(b'\n');

    client.write_all(&request_bytes).await.unwrap();
    client.flush().await.unwrap();

    let mut reader = BufReader::new(client);
    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap();

    let response: IpcResponse = serde_json::from_str(&line).unwrap();
    if let IpcResponse::StatusResponse { current_wallpaper, .. } = response {
        assert_eq!(current_wallpaper, Some(PathBuf::new()));
    } else {
        panic!("Expected StatusResponse, got {response:?}");
    }

    // Get client stream back
    let mut client = reader.into_inner();

    // 2. Send SetWallpaper request
    let test_path = PathBuf::from("C:\\wallpaper.jpg");
    let request = IpcRequest::SetWallpaper { path: test_path.clone(), transition: None };
    let mut request_bytes = serde_json::to_vec(&request).unwrap();
    request_bytes.push(b'\n');

    client.write_all(&request_bytes).await.unwrap();
    client.flush().await.unwrap();

    let mut reader = BufReader::new(client);
    line.clear();
    reader.read_line(&mut line).await.unwrap();

    let response: IpcResponse = serde_json::from_str(&line).unwrap();
    assert_eq!(response, IpcResponse::Success);

    // Verify manager state changed
    assert_eq!(wm.get_current_wallpaper().unwrap(), test_path);

    // Cleanup server task
    server_handle.abort();
}
