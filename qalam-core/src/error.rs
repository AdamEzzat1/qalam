//! Error types for Qalam.

use thiserror::Error;

pub type Result<T> = std::result::Result<T, QalamError>;

#[derive(Debug, Error)]
pub enum QalamError {
    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("lexicon hash mismatch: expected {expected}, got {actual}")]
    LexiconHashMismatch { expected: String, actual: String },

    #[error("config hash mismatch: expected {expected}, got {actual}")]
    ConfigHashMismatch { expected: String, actual: String },

    #[error("malformed lexicon: {0}")]
    MalformedLexicon(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}
