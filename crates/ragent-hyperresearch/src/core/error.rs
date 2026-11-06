//! Crate error type and result alias.
//!
//! Domain errors use [`thiserror`]-derived types (NFR-006). `anyhow` is
//! confined to the binary edge (`src/main.rs`); library code returns
//! [`Error`] so callers can match on the failure kind.

use std::path::PathBuf;

/// Result alias used throughout the crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Every failure this crate can report.
///
/// Variants are grouped by the layer that raises them: vault and note
/// handling (FR-005..FR-007), fetch (FR-008..FR-011), scholarly access
/// (FR-012..FR-014), search and claims (FR-017..FR-019), and the CLI/transport
/// surfaces (FR-023..FR-026).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A note id or path outside the vault was requested.
    #[error("invalid note id {id:?}: {reason}")]
    InvalidNoteId {
        /// The rejected id.
        id: String,
        /// Why it was rejected.
        reason: String,
    },

    /// A create or rename would have collided with an existing note (FR-006).
    #[error("note id {id:?} already exists at {path}")]
    NoteIdConflict {
        /// The conflicting id.
        id: String,
        /// Path of the note that already holds it.
        path: PathBuf,
    },

    /// A note does not exist in the vault.
    #[error("note not found: {0}")]
    NoteNotFound(String),

    /// A note's YAML frontmatter could not be parsed.
    #[error("malformed frontmatter in {path}: {reason}")]
    Frontmatter {
        /// The offending file.
        path: PathBuf,
        /// Parser diagnostic.
        reason: String,
    },

    /// A URL failed validation or the fetch policy refused it (FR-009).
    #[error("fetch refused for {url}: {reason}")]
    FetchRefused {
        /// The rejected URL.
        url: String,
        /// Why the policy refused it.
        reason: String,
    },

    /// A provider failed, timed out, or returned unusable content (FR-011).
    #[error("provider {provider:?} failed for {url}: {reason}")]
    ProviderFailed {
        /// Provider name.
        provider: String,
        /// The URL being fetched.
        url: String,
        /// Failure detail.
        reason: String,
    },

    /// Configuration could not be resolved from its layers (FR-004).
    #[error("configuration error: {0}")]
    Config(String),

    /// A CLI argument or MCP tool argument was invalid (FR-023, FR-025).
    #[error("invalid argument {}: {reason}", .argument.as_deref().unwrap_or("(unnamed)"))]
    InvalidArgument {
        /// The offending argument, when named.
        argument: Option<String>,
        /// Why it was invalid.
        reason: String,
    },

    /// Underlying I/O failure.
    #[error("i/o error at {path}: {source}")]
    Io {
        /// Path involved, if known.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },

    /// Underlying network failure.
    #[error("network error for {url}: {reason}")]
    Network {
        /// The URL being contacted.
        url: String,
        /// Transport diagnostic.
        reason: String,
    },

    /// JSON (de)serialisation failure.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// YAML (de)serialisation failure.
    #[error("yaml error: {0}")]
    Yaml(#[from] serde_norway::Error),

    /// A feature this build or configuration does not provide was requested.
    #[error("unsupported: {0}")]
    Unsupported(String),
}

impl Error {
    /// Wrap an [`std::io::Error`] with the path that produced it.
    ///
    /// Taking the path at the call site is what lets the CLI print a usable
    /// message without re-discovering the file it was working on.
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
