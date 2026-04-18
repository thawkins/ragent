//! Error types for AIWiki.

use thiserror::Error;

/// Result type alias for AIWiki operations.
pub type Result<T> = std::result::Result<T, AiwikiError>;

/// Errors that can occur in AIWiki operations.
#[derive(Error, Debug)]
pub enum AiwikiError {
    /// The wiki has not been initialized.
    #[error("AIWiki not initialized. Run `/aiwiki init` first.")]
    NotInitialized,
    
    /// The wiki is already initialized.
    #[error("AIWiki already initialized at this location.")]
    AlreadyInitialized,
    
    /// Configuration file error.
    #[error("Configuration error: {0}")]
    Config(String),
    
    /// State file error.
    #[error("State error: {0}")]
    State(String),
    
    /// File system error.
    #[error("File system error: {0}")]
    Io(#[from] std::io::Error),
    
    /// Serialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    
    /// YAML serialization error.
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    
    /// Hashing error.
    #[error("Hash calculation error: {0}")]
    Hash(String),
}

impl From<anyhow::Error> for AiwikiError {
    fn from(err: anyhow::Error) -> Self {
        AiwikiError::Config(err.to_string())
    }
}
