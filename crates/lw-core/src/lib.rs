pub mod config;
pub mod error;
pub mod logging;
pub mod traits;
pub mod ipc;

pub use config::{Config, TransitionConfig, SchedulerConfig, EasingType, WallpaperPosition};
pub use error::{LwError, Result};
pub use logging::init_logging;
pub use traits::{WallpaperManager, TransitionRenderer};
pub use ipc::{IpcRequest, IpcResponse, TransitionParams};

