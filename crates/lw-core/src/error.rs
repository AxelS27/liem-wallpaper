use thiserror::Error;

#[derive(Error, Debug)]
pub enum LwError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Serialization/Deserialization error: {0}")]
    Serialization(String),

    #[error("Renderer error: {0}")]
    Renderer(String),

    #[error("Wallpaper error: {0}")]
    Wallpaper(String),

    #[error("IPC error: {0}")]
    Ipc(String),

    #[error("Win32 API error: {0}")]
    Win32(String),

    #[error("Other error: {0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, LwError>;
