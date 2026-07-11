use clap::{Parser, Subcommand};
use lw_core::ipc::{IpcRequest, IpcResponse};
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::ClientOptions;

const PIPE_NAME: &str = r"\\.\pipe\liem-wallpaper";

#[derive(Parser)]
#[command(
    name = "lw",
    author,
    version,
    about = "Liem Wallpaper - A lightweight GPU-accelerated wallpaper manager",
    long_about = "Liem Wallpaper CLI\n\n\
                  COMMON USAGE:\n  \
                  lw set <path> [-t <transition>] [-d <duration>] [-s <style>] [-g <dir>]\n  \
                  lw status\n  \
                  lw next\n  \
                  lw prev\n  \
                  lw shaders\n  \
                  lw update\n\n\
                  TRANSITION FLAGS (for 'set' command):\n  \
                  -t, --transition <type>  Transition effect name (e.g. fade, pixelate, glitch, radial-in, slide-left, zoom-in) [default: fade]\n  \
                  -d, --duration <ms>      Duration of transition in milliseconds [default: 1000]\n  \
                  -s, --style <curve>      Easing style (linear, sine, quad, cubic, quart, quint, expo, circ, back, bounce, elastic) [default: quad]\n  \
                  -g, --dir <direction>    Easing direction (in, out, inout) [default: inout]"
)]
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

    /// Trigger the next wallpaper in rotation
    Next,

    /// Trigger the previous wallpaper in rotation
    Prev,

    /// List all available transition shaders
    Shaders,

    /// Check and perform application updates
    Update,
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
        Commands::Next => {
            if let Err(e) = send_request_and_print(IpcRequest::NextWallpaper).await {
                eprintln!("CLI Error: {e}");
                std::process::exit(1);
            }
        }
        Commands::Prev => {
            if let Err(e) = send_request_and_print(IpcRequest::PrevWallpaper).await {
                eprintln!("CLI Error: {e}");
                std::process::exit(1);
            }
        }
        Commands::Shaders => {
            let mut shaders = vec![
                "fade".to_string(), "zoom-in".to_string(), "zoom-out".to_string(),
                "pixelate".to_string(), "glitch".to_string(), "radial-in".to_string(),
                "radial-out".to_string(), "slide-left".to_string(), "slide-right".to_string(),
                "slide-up".to_string(), "slide-down".to_string()
            ];

            if let Ok(exe_path) = std::env::current_exe() {
                if let Some(exe_dir) = exe_path.parent() {
                    let local_shaders = exe_dir.join("shaders");
                    if let Ok(entries) = std::fs::read_dir(local_shaders) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("hlsl") {
                                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                                    let stem_str = stem.to_string();
                                    if !shaders.contains(&stem_str) {
                                        shaders.push(stem_str);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            shaders.sort();
            println!("Available Transition Shaders:");
            for shader in shaders {
                println!("  - {shader}");
            }
        }
        Commands::Update => {
            println!("Checking for updates from GitHub (AxelS27/liem-wallpaper)...");
            match run_interactive_update() {
                Ok(status) => {
                    println!("{status}");
                }
                Err(e) => {
                    eprintln!("Update failed: {e}");
                    std::process::exit(1);
                }
            }
        }
    }
}

fn run_interactive_update() -> Result<String, String> {
    let current_version = env!("CARGO_PKG_VERSION");
    let repo = "AxelS27/liem-wallpaper";

    let ps_script = format!(
        "$version = '{}'; \
         $repo = '{}'; \
         try {{ \
             $r = Invoke-RestMethod -Uri \"https://api.github.com/repos/$repo/releases/latest\" -UserAgent \"LiemWallpaper\" -ErrorAction Stop; \
             $latest = $r.tag_name.TrimStart('v'); \
             if ($latest -ne $version) {{ \
                 Write-Output \"NEW_VERSION:$latest\"; \
                 $asset = $r.assets | Where-Object {{ $_.name -like '*Setup.exe' -or $_.name -like '*.exe' }} | Select-Object -First 1; \
                 if ($asset) {{ \
                     Write-Output \"DOWNLOADING:$($asset.name)\"; \
                     $tempPath = [System.IO.Path]::Combine([System.IO.Path]::GetTempPath(), $asset.name); \
                     Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $tempPath -UserAgent \"LiemWallpaper\" -ErrorAction Stop; \
                     Write-Output \"INSTALLING\"; \
                     Start-Process -FilePath $tempPath -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART'; \
                     Write-Output \"SUCCESS\"; \
                 }} else {{ \
                     Write-Output \"ERROR:No installer asset found in the latest release\"; \
                 }} \
             }} else {{ \
                 Write-Output \"UPTODATE\"; \
             }} \
         }} catch {{ \
             Write-Output \"ERROR:$($_.Exception.Message)\"; \
         }}",
        current_version, repo
    );

    let output = std::process::Command::new("powershell")
        .args(&["-NoProfile", "-Command", &ps_script])
        .output()
        .map_err(|e| format!("Failed to run PowerShell: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut is_downloading = false;

    for line in stdout.lines() {
        let line = line.trim();
        if line.starts_with("NEW_VERSION:") {
            let latest_version = line.trim_start_matches("NEW_VERSION:");
            println!("New version v{} is available!", latest_version);
        } else if line.starts_with("DOWNLOADING:") {
            let asset_name = line.trim_start_matches("DOWNLOADING:");
            println!("Downloading latest installer ({}) to Temp folder...", asset_name);
            is_downloading = true;
        } else if line == "INSTALLING" {
            println!("Launching silent installer in background...");
        } else if line == "SUCCESS" {
            return Ok("Update launched successfully! Liem Wallpaper will restart shortly.".to_string());
        } else if line == "UPTODATE" {
            return Ok(format!("Liem Wallpaper is already up-to-date (v{current_version})."));
        } else if line.starts_with("ERROR:") {
            return Err(line.trim_start_matches("ERROR:").to_string());
        }
    }

    if is_downloading {
        Err("Update process terminated unexpectedly during download.".to_string())
    } else {
        Err(format!("Failed to retrieve update status. Raw output: {stdout}"))
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
