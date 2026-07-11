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
        #[arg(short, long, default_value = "fade")]
        transition: String,

        /// Duration of transition in milliseconds
        #[arg(short, long, default_value_t = 1000)]
        duration: u32,

        /// Easing style (linear, sine, quad, cubic, quart, quint, expo, circ, back, bounce, elastic)
        #[arg(short, long)]
        style: Option<String>,

        /// Easing direction (in, out, inout)
        #[arg(short = 'g', long = "dir")]
        direction: Option<String>,
    },

    /// Test transitions between blue.jpg and red.jpg
    Test {
        /// Transition effect (e.g. fade, slide-left)
        #[arg(short, long, default_value = "fade")]
        transition: String,

        /// Duration of transition in milliseconds
        #[arg(short, long, default_value_t = 1000)]
        duration: u32,

        /// Easing style (linear, sine, quad, cubic, quart, quint, expo, circ, back, bounce, elastic)
        #[arg(short, long)]
        style: Option<String>,

        /// Easing direction (in, out, inout)
        #[arg(short = 'g', long = "dir")]
        direction: Option<String>,
    },
}

fn parse_style_and_dir(
    style_opt: Option<&str>,
    dir_opt: Option<&str>,
) -> (lw_core::config::EasingStyle, lw_core::config::EasingDirection) {
    let mut style = lw_core::config::EasingStyle::Quad;
    let mut dir = lw_core::config::EasingDirection::InOut;

    if let Some(s) = style_opt {
        style = match s.to_lowercase().as_str() {
            "linear" | "lin" | "l" => lw_core::config::EasingStyle::Linear,
            "sine" | "sin" | "s" => lw_core::config::EasingStyle::Sine,
            "quad" | "q" => lw_core::config::EasingStyle::Quad,
            "cubic" | "c" => lw_core::config::EasingStyle::Cubic,
            "quart" | "qu" => lw_core::config::EasingStyle::Quart,
            "quint" | "qn" => lw_core::config::EasingStyle::Quint,
            "exponential" | "expo" | "e" => lw_core::config::EasingStyle::Exponential,
            "circular" | "circ" | "cr" => lw_core::config::EasingStyle::Circular,
            "back" | "bk" => lw_core::config::EasingStyle::Back,
            "bounce" | "bn" => lw_core::config::EasingStyle::Bounce,
            "elastic" | "el" => lw_core::config::EasingStyle::Elastic,
            _ => lw_core::config::EasingStyle::Quad,
        };
    }

    if let Some(d) = dir_opt {
        dir = match d.to_lowercase().as_str() {
            "in" | "i" => lw_core::config::EasingDirection::In,
            "out" | "o" => lw_core::config::EasingDirection::Out,
            "inout" | "in-out" | "io" => lw_core::config::EasingDirection::InOut,
            _ => lw_core::config::EasingDirection::InOut,
        };
    }

    (style, dir)
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Status => {
            if let Err(e) = send_request_and_print(IpcRequest::GetStatus).await {
                eprintln!("CLI Error: {e}");
                std::process::exit(1);
            }
        }
        Commands::Set { path, transition, duration, style, direction } => {
            let abs_path = if path.is_absolute() {
                path
            } else {
                std::env::current_dir().map(|d| d.join(&path)).unwrap_or(path)
            };

            let (parsed_style, parsed_dir) = parse_style_and_dir(style.as_deref(), direction.as_deref());
            let request = IpcRequest::SetWallpaper {
                path: abs_path,
                transition: Some(lw_core::ipc::TransitionParams {
                    effect_type: transition,
                    duration_ms: duration,
                    easing_style: parsed_style,
                    easing_direction: parsed_dir,
                }),
            };

            if let Err(e) = send_request_and_print(request).await {
                eprintln!("CLI Error: {e}");
                std::process::exit(1);
            }
        }
        Commands::Test { transition, duration, style, direction } => {
            let status_res = send_request(IpcRequest::GetStatus).await;
            let current_wp = match status_res {
                Ok(IpcResponse::StatusResponse { current_wallpaper, .. }) => current_wallpaper,
                _ => None,
            };

            let target_path = if let Some(ref path) = current_wp {
                let path_str = path.to_string_lossy().to_lowercase();
                if path_str.contains("blue.jpg") {
                    PathBuf::from(r"D:\Downloads\red.jpg")
                } else {
                    PathBuf::from(r"D:\Downloads\blue.jpg")
                }
            } else {
                PathBuf::from(r"D:\Downloads\blue.jpg")
            };

            println!("Current wallpaper: {:?}", current_wp);
            println!("Toggling test wallpaper to: {:?}", target_path);

            let (parsed_style, parsed_dir) = parse_style_and_dir(style.as_deref(), direction.as_deref());
            let request = IpcRequest::SetWallpaper {
                path: target_path,
                transition: Some(lw_core::ipc::TransitionParams {
                    effect_type: transition,
                    duration_ms: duration,
                    easing_style: parsed_style,
                    easing_direction: parsed_dir,
                }),
            };

            if let Err(e) = send_request_and_print(request).await {
                eprintln!("CLI Error: {e}");
                std::process::exit(1);
            }
        }
    }
}

async fn send_request(request: IpcRequest) -> Result<IpcResponse, String> {
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

    Ok(response)
}

async fn send_request_and_print(request: IpcRequest) -> Result<(), String> {
    let response = send_request(request).await?;

    // Output results based on response type
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
