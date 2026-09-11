use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HyprlandError {
    #[error("HYPRLAND_INSTANCE_SIGNATURE environment variable not found")]
    MissingInstanceSignature,

    #[error("XDG_RUNTIME_DIR environment variable not found")]
    MissingRuntimeDir,

    #[error("Hyprland socket not found at path: {path}")]
    SocketNotFound { path: PathBuf },

    #[error("I/O error communicating with Hyprland socket at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to parse Hyprland response as JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Hyprland eval error: {0}")]
    EvalError(String),

    #[error("Batch execution encountered errors: {0}")]
    BatchError(String),
}
