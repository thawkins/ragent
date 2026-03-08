//! Error types for the ragent-core crate.
//!
//! Provides [`RagentError`], a structured error enum for all core operations.
//! Use `anyhow::Result` for internal convenience and `RagentError` at module boundaries.

use thiserror::Error;

/// Structured error type for ragent-core operations.
#[derive(Debug, Error)]
pub enum RagentError {
    /// Database or storage-layer error.
    #[error("storage error: {0}")]
    Storage(#[from] rusqlite::Error),

    /// LLM provider communication error.
    #[error("provider error ({provider}): {message}")]
    Provider {
        /// The provider that encountered the error.
        provider: String,
        /// A human-readable description of the error.
        message: String,
    },

    /// Tool execution error.
    #[error("tool error ({tool}): {message}")]
    Tool {
        /// The tool that failed.
        tool: String,
        /// A human-readable description of the error.
        message: String,
    },

    /// Configuration loading or validation error.
    #[error("config error: {0}")]
    Config(String),

    /// Permission denied for a requested operation.
    #[error("permission denied: {permission} on {pattern}")]
    PermissionDenied {
        /// The permission that was denied.
        permission: String,
        /// The resource pattern that was denied.
        pattern: String,
    },

    /// Requested session was not found.
    #[error("session not found: {0}")]
    SessionNotFound(String),

    /// JSON serialization or deserialization error.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Mutex lock was poisoned.
    #[error("lock poisoned: {0}")]
    LockPoisoned(String),

    /// Generic I/O error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
