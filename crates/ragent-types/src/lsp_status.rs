//! LSP server status type.

use serde::{Deserialize, Serialize};

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
