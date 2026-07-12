use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Initializes the tracing instrumentation with env filter and formatting layers.
/// This will enable structured logging throughout the application.
/// It uses `try_init` so that multiple invocations (e.g. during tests) do not cause panics.
pub fn init_logging() {
    let filter_layer = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    // Try to log to a file next to the executable if we can get its path
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let log_path = exe_dir.join("lw-service.log");
            if let Ok(file) = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&log_path)
            {
                let fmt_layer = fmt::layer()
                    .with_thread_ids(true)
                    .with_target(true)
                    .with_writer(std::sync::Mutex::new(file));
                let _ = tracing_subscriber::registry().with(filter_layer).with(fmt_layer).try_init();
                return;
            }
        }
    }

    let fmt_layer = fmt::layer().with_thread_ids(true).with_target(true);
    let _ = tracing_subscriber::registry().with(filter_layer).with(fmt_layer).try_init();
}
