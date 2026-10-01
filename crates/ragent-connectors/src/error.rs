//! Contained error types for the connector subsystem (spec `connectors`).
//!
//! Every connector-facing failure is converted into a report string or a Rust
//! `Result`, never a panic, matching the plugin-system error policy (spec
//! `plugins` FR-026). T-002 introduces the descriptor-validation variants, T-003
//! the store/manifest I/O variants, T-004 the manifest/staging I/O chain helper,
//! T-007 the catalogue and auth variants, and T-017 the endpoint-refusal variant;
//! later tasks extend this enum with the connect variants as those modules land.

use std::fmt::Write as _;
use std::path::PathBuf;

use crate::store_index::CatalogueEndpointError;

/// Errors reported by the connector subsystem.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConnectorError {
    /// A connector id is empty, contains a path separator, is longer than
    /// [`MAX_IDENTIFIER_LEN`](ragent_types::guard::MAX_IDENTIFIER_LEN), or is
    /// otherwise not usable as a single path component (FR-002). The id becomes
    /// the connector store directory name, so it must be one ordinary
    /// component.
    #[error("invalid connector id: {detail}")]
    InvalidId {
        /// Why the id was rejected, as reported by the shared identifier guard.
        detail: String,
    },

    /// A descriptor declares no MCP server at all (FR-002). A connector with
    /// nothing to connect can never expose a tool, so it is rejected.
    #[error("connector '{id}' declares no MCP server")]
    NoServers {
        /// The connector id the descriptor declared.
        id: String,
    },

    /// Every declared server uses a capability ragent cannot express as an MCP
    /// server (an unknown transport, or one with neither a command nor a URL),
    /// so there is nothing to connect (FR-025). `reason` names the recorded
    /// unsupported-capability labels; the descriptor still retains them so the
    /// caller can report each one in `/connectors list`.
    #[error("connector '{id}' declares no supported MCP server: {reason}")]
    NoSupportedServers {
        /// The connector id the descriptor declared.
        id: String,
        /// The recorded unsupported-capability labels, joined for display.
        reason: String,
    },

    /// An I/O failure while accessing the connector store or a connector
    /// directory (FR-001). Carries the full source-error chain so the underlying
    /// `io::Error` is reported rather than a bare "I/O error".
    #[error("connector I/O error: {0}")]
    Io(String),

    /// A `connector.json` manifest could not be parsed or its contents violated
    /// the descriptor schema (FR-002). Carries the manifest path and the failure
    /// detail (JSON error position on parse failures).
    #[error("connector manifest-parse failed: {detail}")]
    ManifestParse {
        /// Path of the manifest that failed to parse.
        manifest: PathBuf,
        /// Failure detail, including the JSON error position when known.
        detail: String,
    },

    /// A catalogue fetch failed before any document was parsed: a connection
    /// failure, a non-2xx status, a refused redirect, or a timeout (FR-031). The
    /// detail names the cause; any previously cached catalogue is left untouched.
    #[error("connector catalogue fetch failed: {detail}")]
    CatalogueFetch {
        /// The transport failure detail.
        detail: String,
    },

    /// The fetched catalogue document was not valid JSON (FR-031).
    #[error("connector catalogue is not valid JSON: {detail}")]
    CatalogueParse {
        /// The serde JSON failure detail, including the error position.
        detail: String,
    },

    /// The fetched catalogue document parsed as JSON but was not a recognised
    /// catalogue shape (FR-031).
    #[error("connector catalogue has an unexpected shape: {detail}")]
    CatalogueShape {
        /// What was expected instead.
        detail: String,
    },

    /// The fetched catalogue document exceeded `connectors.max_index_bytes`
    /// (FR-031).
    #[error("connector catalogue exceeds the {limit} byte limit")]
    CatalogueTooLarge {
        /// The configured ceiling, in bytes.
        limit: u64,
    },

    /// An authentication-shape operation failed: a shape that needs a secret
    /// named no credential, or the encrypted credential store could not be read
    /// or written (FR-005, FR-032). The detail never contains a secret value.
    #[error("connector auth failed: {detail}")]
    Auth {
        /// The contained auth failure detail (no secret material).
        detail: String,
    },

    /// A catalogue endpoint could not be resolved: the compiled default or a
    /// configured override is not an absolute `https` URL carrying a host
    /// (FR-037). Surfaced **before** any fetch is attempted, so a refused
    /// endpoint never falls back to an unvalidated value and never starts a
    /// network request.
    #[error("connector catalogue endpoint refused: {0}")]
    CatalogueEndpoint(#[from] CatalogueEndpointError),
}

impl ConnectorError {
    /// Construct an I/O error from any error implementing [`std::error::Error`],
    /// including the error's full source chain so the underlying `io::Error` (or
    /// lower-level OS error) is reported, per AGENTS.md "meaningful error
    /// messages and context".
    pub fn io(err: impl std::error::Error + 'static) -> Self {
        Self::Io(flatten_chain(&err))
    }
}

/// Flatten any error's full source chain into one `": "`-joined message.
pub(crate) fn flatten_chain(err: &(impl std::error::Error + 'static)) -> String {
    let mut detail = err.to_string();
    let mut source = err.source();
    while let Some(cause) = source {
        let _ = write!(detail, ": {cause}"); // INTENTIONAL: write to a String buffer is infallible
        source = cause.source();
    }
    detail
}
