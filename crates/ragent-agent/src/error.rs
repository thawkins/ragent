//! Error types for the ragent-agent crate.
//!
//! Provides [`RagentError`], a structured error enum for all core operations.
//! Use `anyhow::Result` for internal convenience and `RagentError` at module boundaries.

use crate::session::history::is_token_overflow_error_message;

use thiserror::Error;

/// Structured error type for ragent-agent operations.
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

/// Which loop stage produced a failure (spec `agentloop`, FR-011/FR-012).
///
/// Each stage maps to a fixed [`ErrorKind`] via [`classify_stage`]:
/// provider transport failures, permission hard-denies, tool panics, and
/// context overflow are unrecoverable (FR-011); tool-level failures are
/// recoverable by default (FR-012) so the error text can be appended as an
/// observation and the loop retried until the retry allowance is exceeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LoopStage {
    /// The LLM request or its stream failed (provider transport).
    LlmTransport,
    /// The permission layer hard-denied the action (user or policy deny).
    PermissionHardDeny,
    /// A tool task panicked or was aborted by the watchdog.
    ToolPanic,
    /// The request exceeded the provider's context window and no compaction
    /// path can recover it.
    ContextOverflow,
    /// A tool invocation returned an error result (observation-worthy).
    ToolFailure,
}

/// Classification of a loop-stage failure (spec `agentloop`, FR-011/FR-012).
///
/// `Unrecoverable` failures terminate the loop immediately with termination
/// status `error` and no retry of the failed stage; `Recoverable` failures
/// are appended as error observations and the loop continues until the
/// consecutive-failure retry allowance is exceeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// The failure is worth reporting to the model as an observation and
    /// retrying (bounded by the loop retry allowance, FR-012).
    Recoverable,
    /// The failure must terminate the loop with status `error`; the stage is
    /// not retried (FR-011).
    Unrecoverable,
}

impl ErrorKind {
    /// Whether this failure can be retried as an observation.
    pub const fn is_recoverable(self) -> bool {
        matches!(self, Self::Recoverable)
    }

    /// Whether this failure terminates the loop immediately (FR-011).
    pub const fn is_unrecoverable(self) -> bool {
        !self.is_recoverable()
    }
}

impl std::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Recoverable => f.write_str("recoverable"),
            Self::Unrecoverable => f.write_str("unrecoverable"),
        }
    }
}

/// Map a loop stage to its default error classification (FR-011, FR-012):
/// transport failure, permission hard-deny, panic, and context overflow are
/// unrecoverable; tool-level failures are recoverable by default.
pub const fn classify_stage(stage: LoopStage) -> ErrorKind {
    match stage {
        LoopStage::LlmTransport
        | LoopStage::PermissionHardDeny
        | LoopStage::ToolPanic
        | LoopStage::ContextOverflow => ErrorKind::Unrecoverable,
        LoopStage::ToolFailure => ErrorKind::Recoverable,
    }
}

/// Classify an arbitrary loop-stage error by its message content.
///
/// The loop's error paths travel as stringified `anyhow` errors, so this
/// sniffer bridges messages to [`ErrorKind`]. Markers:
///
/// - context-overflow diagnostics (`is_token_overflow_error_message`) -
///   unrecoverable (FR-011);
/// - permission hard-denies (`permission denied`, `blocked by hook`) -
///   unrecoverable (FR-011);
/// - tool panics and watchdog aborts (`panicked`, `watchdog`) -
///   unrecoverable (FR-011);
/// - anything else - recoverable, the tool-level default (FR-012).
pub fn classify_message(message: &str) -> ErrorKind {
    if is_token_overflow_error_message(message) {
        return ErrorKind::Unrecoverable;
    }
    let lower = message.to_lowercase();
    if lower.contains("permission denied")
        || lower.contains("blocked by hook")
        || lower.contains("panicked")
        || lower.contains("watchdog")
    {
        return ErrorKind::Unrecoverable;
    }
    ErrorKind::Recoverable
}
