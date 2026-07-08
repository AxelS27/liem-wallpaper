use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Initializes the tracing instrumentation with env filter and formatting layers.
/// This will enable structured logging throughout the application.
/// It uses `try_init` so that multiple invocations (e.g. during tests) do not cause panics.
pub fn init_logging() {
    let fmt_layer = fmt::layer().with_thread_ids(true).with_target(true);

    let filter_layer = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let _ = tracing_subscriber::registry().with(filter_layer).with(fmt_layer).try_init();
}
