use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization/deserialization JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Framing codec error: {0}")]
    Frame(String),

    #[error("Connection closed unexpectedly")]
    ConnectionClosed,

    #[error("Unexpected message: expected {expected}, got {actual}")]
    UnexpectedMessage {
        expected: &'static str,
        actual: String,
    },
}
