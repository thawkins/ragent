//! Error types for the plugin subsystem (spec `plugins`).
//!
//! Every plugin failure is contained and reported through these variants;
//! plugin code must never terminate the ragent process (FR-026).
//!
//! T-002 introduces the variants needed by descriptor recognition
//! ([`PluginError::AmbiguousManifest`] and I/O); later tasks extend the enum
//! with manifest-parse, runtime, store, and lifecycle variants as those
//! modules land.

use std::fmt::Write as _;
use std::path::PathBuf;

/// Errors reported by the plugin subsystem.
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    /// A directory matches both the Codex and the Claude recognition rules
    /// (FR-002). The plugin is `errored` with cause `ambiguous-manifest`.
    #[error("ambiguous plugin manifest: directory matches both Codex and Claude dialects")]
    AmbiguousManifest,

    /// A manifest file could not be parsed or its contents violated the
    /// dialect schema. Carries the manifest path and the failure detail
    /// (JSON error position on parse failures, FR-002 error handling).
    #[error("manifest-parse failed: {detail}")]
    ManifestParse {
        /// Path of the manifest that failed to parse.
        manifest: PathBuf,
        /// Failure detail, including the JSON error position when known.
        detail: String,
    },

    /// An I/O failure while accessing the plugin store or a plugin directory.
    #[error("plugin I/O error: {0}")]
    Io(String),

    /// The JavaScript engine itself failed to allocate or configure a
    /// runtime/context (FR-026).
    #[error("plugin engine error: {0}")]
    Engine(String),

    /// The sandbox deadline fired (FR-017): plugin execution was aborted by
    /// the interrupt handler.
    #[error("plugin execution timed out")]
    Timeout,

    /// The sandbox memory ceiling was reached (FR-017).
    #[error("plugin memory limit exceeded")]
    MemoryLimit,

    /// Plugin JavaScript threw an uncaught exception; `plugin` is the plugin
    /// id when known (empty during harness-only phases).
    #[error("plugin script error: {detail}")]
    Script {
        /// Plugin id responsible, when known.
        plugin: String,
        /// JavaScript exception message.
        detail: String,
    },

    /// A lifecycle operation referenced a plugin id not present in any store
    /// (T-009; reported as an `[err]` row by `/plugins` subcommands).
    #[error("unknown plugin: {0}")]
    UnknownPlugin(String),

    /// A plugin tried to contribute a tool (or command) whose registered name
    /// already exists in the session registry - built-in or another plugin's
    /// (T-010; FR-024). The full colliding name is carried; the session
    /// records the plugin `errored` with cause `name-collision`.
    #[error("name-collision: tool name '{0}' is already registered")]
    NameCollision(String),
}

impl PluginError {
    /// Construct an I/O error from any error implementing [`std::error::Error`],
    /// including the error's full source chain so the underlying `io::Error`
    /// (or lower-level OS error) is reported, per AGENTS.md "meaningful error
    /// messages and context".
    pub fn io(err: impl std::error::Error + 'static) -> Self {
        Self::Io(flatten_chain(&err))
    }
}

/// Flatten any error's full source chain into one `": "`-joined message.
///
/// Shared by [`PluginError::io`] and [`IoError::new`] so both preserve the
/// same structured chain (ANTIPAT M15).
fn flatten_chain(err: &(impl std::error::Error + 'static)) -> String {
    let mut detail = err.to_string();
    let mut source = err.source();
    while let Some(cause) = source {
        let _ = write!(detail, ": {cause}"); // INTENTIONAL: write to a String buffer is infallible
        source = cause.source();
    }
    detail
}

/// A stringified I/O failure that preserves its source-error chain.
///
/// The `add`/`remove` subsystem errors used to hold a bare `String`, which
/// discarded the chain [`PluginError::io`] works hard to build (ANTIPAT M15).
/// Holding this wrapper keeps the chain structured while its `Display` prints
/// exactly the same text the former `String` payload did, so every rendered
/// report and `to_string()` call is unchanged.
///
/// `IoError` also means a caller can no longer construct an `AddError::Io` /
/// `RemoveError::Io` by typing a bare `String`; the `message` constructor is
/// the explicit way to carry a source-less detail.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct IoError(String);

impl IoError {
    /// Build from any error, capturing its full source chain
    /// (see [`PluginError::io`]).
    #[must_use]
    pub fn new(err: impl std::error::Error + 'static) -> Self {
        Self(flatten_chain(&err))
    }

    /// Build from an already-formatted message with no source chain.
    #[must_use]
    pub fn message(detail: impl Into<String>) -> Self {
        Self(detail.into())
    }

    /// The flattened message.
    #[must_use]
    pub fn message_str(&self) -> &str {
        &self.0
    }
}

/// Attach the plugin id to a sandbox error.
///
/// The runtime classifies errors without knowing which plugin it served, so
/// each adapter re-tags a [`PluginError::Script`] with the owning plugin id.
/// Defined once here to keep the two JS adapters from drifting
/// (see `ANTIPAT.md` M3.11).
#[must_use]
pub(crate) fn with_plugin(error: PluginError, plugin_id: &str) -> PluginError {
    match error {
        PluginError::Script { detail, .. } => PluginError::Script {
            plugin: plugin_id.to_string(),
            detail,
        },
        other => other,
    }
}
