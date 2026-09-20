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
    /// already exists in the session registry — built-in or another plugin's
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
        let mut detail = err.to_string();
        let mut source = err.source();
        while let Some(cause) = source {
            let _ = write!(detail, ": {cause}");
            source = cause.source();
        }
        Self::Io(detail)
    }
}
