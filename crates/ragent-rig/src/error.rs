//! Error types for the `ragent-rig` adapter.

use thiserror::Error;

/// The primary error type returned by `ragent-rig` operations.
#[derive(Error, Debug)]
pub enum RigError {
    /// The requested Rig provider is not enabled at compile time.
    #[error("Rig provider '{0}' is not enabled in this build")]
    ProviderNotEnabled(String),

    /// The requested Rig provider alias is not defined in configuration.
    #[error("Rig provider alias '{0}' is not configured")]
    ProviderAliasNotFound(String),

    /// A Rig backend returned an error.
    #[error("Rig backend error: {0}")]
    BackendError(String),

    /// Streaming was requested but is not supported for the selected backend.
    #[error("Streaming is not supported for provider '{0}'")]
    StreamingNotSupported(String),

    /// Embedding support is required but was not compiled in.
    #[error("Rig embedding support is not enabled in this build")]
    EmbeddingsNotEnabled,

    /// Memory support is required but was not compiled in.
    #[error("Rig memory support is not enabled in this build")]
    MemoryNotEnabled,

    /// Vector-store support is required but was not compiled in.
    #[error("Rig vector-store support is not enabled in this build")]
    VectorStoreNotEnabled,

    /// A configuration value is invalid.
    #[error("Invalid Rig configuration: {0}")]
    InvalidConfiguration(String),

    /// A wrapped external error.
    #[error(transparent)]
    External(#[from] anyhow::Error),
}

/// Convenience `Result` alias.
pub type Result<T> = std::result::Result<T, RigError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_includes_context() {
        let err = RigError::ProviderNotEnabled("openai".to_owned());
        assert!(err.to_string().contains("openai"));
    }
}
