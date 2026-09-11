use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("I/O error while accessing '{path}': {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to parse TOML profile at '{path}': {source}")]
    TomlParse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("Validation error in profile '{profile}': {message}")]
    Validation { profile: String, message: String },

    #[error("Profile '{0}' not found")]
    ProfileNotFound(String),

    #[error("Cannot delete protected profile '{0}'")]
    ProtectedProfile(String),

    #[error("Invalid key combo '{raw}': {reason}")]
    InvalidKeyCombo { raw: String, reason: String },

    #[error("Invalid action '{raw}': {reason}")]
    InvalidAction { raw: String, reason: String },
}
