//! LSP server descriptor types.
//!
//! [`LspServer`] holds the configuration and runtime state for a single Language
//! Server Protocol server. [`LspStatus`] captures the connection lifecycle.

use serde::{Deserialize, Serialize};

use crate::config::LspServerConfig;

/// Connection status of an LSP server.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LspStatus {
    /// Server is starting up (process spawned, awaiting initialize response).
    Starting,
    /// Server is connected and ready to answer queries.
    Connected,
    /// Server is configured but disabled — will not be started.
    Disabled,
    /// Server failed to start or the connection was lost.
    Failed {
        /// Human-readable error message.
        error: String,
    },
}

impl std::fmt::Display for LspStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Starting => write!(f, "starting"),
            Self::Connected => write!(f, "connected"),
            Self::Disabled => write!(f, "disabled"),
            Self::Failed { error } => write!(f, "failed: {error}"),
        }
    }
}

/// A registered LSP server with its configuration, connection status, and
/// reported capabilities summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspServer {
    /// Unique identifier for this server (the key from `ragent.json`).
    pub id: String,
    /// Language identifier(s) this server handles (e.g. `"rust"`, `"typescript"`).
    pub language: String,
    /// Configuration used to start the server.
    pub config: LspServerConfig,
    /// Current connection status.
    pub status: LspStatus,
    /// Human-readable summary of the server capabilities.
    /// Populated after a successful `initialize` handshake.
    pub capabilities_summary: Option<String>,
}

impl LspServer {
    /// Create a new descriptor for a server that has not yet been started.
    ///
    /// # Errors
    ///
    /// This function is infallible.
    #[must_use]
    pub const fn new(id: String, language: String, config: LspServerConfig) -> Self {
        let status = if config.disabled {
            LspStatus::Disabled
        } else {
            LspStatus::Starting
        };
        Self {
            id,
            language,
            config,
            status,
            capabilities_summary: None,
        }
    }

    /// Create a minimal placeholder descriptor for an unknown server id.
    ///
    /// Used when a [`crate::event::Event::LspStatusChanged`] arrives for a
    /// server that has not yet been registered (race at startup).
    #[must_use]
    pub fn unknown(id: String) -> Self {
        Self {
            language: id.clone(),
            id,
            config: LspServerConfig::default(),
            status: LspStatus::Starting,
            capabilities_summary: None,
        }
    }
}
