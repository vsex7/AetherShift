use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DaemonError {
    #[error("Another instance of AetherShift daemon is already running (active socket at {})", .0.display())]
    AlreadyRunning(PathBuf),

    #[error("Hyprland error: {0}")]
    Hyprland(#[from] aethershift_hyprland::HyprlandError),

    #[error("Core state error: {0}")]
    Core(#[from] aethershift_core::CoreError),

    #[error("Protocol error: {0}")]
    Protocol(#[from] aethershift_protocol::ProtocolError),

    #[error("I/O error at '{}': {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Serialization error: {0}")]
    Json(#[from] serde_json::Error),
}
