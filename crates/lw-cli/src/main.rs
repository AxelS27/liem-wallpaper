use clap::{Parser, Subcommand};
use lw_core::ipc::{IpcRequest, IpcResponse};
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::ClientOptions;

const PIPE_NAME: &str = r"\\.\pipe\liem-wallpaper";

#[derive(Parser)]
#[command(name = "lw-cli", author, version, about = "Liem Wallpaper CLI tool", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Get current wallpaper daemon status
    Status,

    /// Set a new wallpaper path
    Set {
        /// Absolute path to the wallpaper image
        path: PathBuf,

        /// Transition effect (e.g. fade)
        #[arg(long, default_value = "fade")]
        transition: String,

        /// Duration of transition in milliseconds
        #[arg(long, default_value_t = 1000)]
        duration: u32,

        /// Easing function type (linear, ease-in, ease-out, ease-in-out)
        #[arg(long, default_value = "ease-in-out")]
        easing: String,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let request = match cli.command {
        Commands::Status => IpcRequest::GetStatus,
        Commands::Set { path, transition, duration, easing } => {
            // Ensure absolute path
            let abs_path = if path.is_absolute() {
                path
            } else {
                std::env::current_dir().map(|d| d.join(&path)).unwrap_or(path)
            };

            let parsed_easing = match easing.to_lowercase().as_str() {
                "linear" => lw_core::config::EasingType::Linear,
                "ease-in" | "easein" => lw_core::config::EasingType::EaseIn,
                "ease-out" | "easeout" => lw_core::config::EasingType::EaseOut,
                _ => lw_core::config::EasingType::EaseInOut,
            };

            IpcRequest::SetWallpaper {
                path: abs_path,
                transition: Some(lw_core::ipc::TransitionParams {
                    effect_type: transition,
                    duration_ms: duration,
                    easing: parsed_easing,
                }),
            }
        }
    };

    if let Err(e) = send_request_and_print(request).await {
        eprintln!("CLI Error: {e}");
        std::process::exit(1);
    }
}

async fn send_request_and_print(request: IpcRequest) -> Result<(), String> {
    // 1. Connect to named pipe
    let mut client = ClientOptions::new()
        .open(PIPE_NAME)
        .map_err(|e| format!("Failed to connect to wallpaper daemon: {e}"))?;

    // 2. Serialize and write request
    let mut request_bytes =
        serde_json::to_vec(&request).map_err(|e| format!("Failed to serialize request: {e}"))?;
    request_bytes.push(b'\n');

    client
        .write_all(&request_bytes)
        .await
        .map_err(|e| format!("Failed to write request to pipe: {e}"))?;
    client.flush().await.map_err(|e| format!("Failed to flush pipe: {e}"))?;

    // 3. Read response
    let mut reader = BufReader::new(client);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .await
        .map_err(|e| format!("Failed to read response from pipe: {e}"))?;

    let response: IpcResponse = serde_json::from_str(&line)
        .map_err(|e| format!("Failed to parse daemon response: {e} (raw: {line})"))?;

    // 4. Output results based on response type
    match response {
        IpcResponse::Success => {
            println!("Success: Command executed successfully.");
        }
        IpcResponse::StatusResponse {
            current_wallpaper,
            scheduler_active,
            next_change_in_seconds,
        } => {
            println!("Daemon Status:");
            let current_str = current_wallpaper
                .map_or_else(|| "None".to_string(), |p| p.to_string_lossy().into_owned());
            println!("  Current Wallpaper: {current_str}");
            println!("  Scheduler Active:  {scheduler_active}");
            println!("  Next Change In:    {next_change_in_seconds} seconds");
        }
        IpcResponse::Error { message } => {
            return Err(format!("Daemon returned error: {message}"));
        }
    }

    Ok(())
}
