use thiserror::Error;

/// Errors that can occur in the spec management system.
#[derive(Debug, Error)]
pub enum SpecError {
    /// A spec directory or file already exists.
    #[error("spec already exists: {0}")]
    AlreadyExists(String),

    /// A required spec directory or file was not found.
    #[error("spec not found: {0}")]
    NotFound(String),

    /// An I/O operation failed.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// A spec identifier is invalid.
    #[error("invalid spec id: {0}")]
    InvalidSpecId(String),

    /// A status transition is not allowed.
    #[error("invalid status transition from {from} to {to}")]
    InvalidStatusTransition { from: String, to: String },

    /// Validation of a spec failed.
    #[error("validation failed: {0}")]
    Validation(String),

    /// A spec directory has an invalid structure.
    #[error("invalid spec structure: {0}")]
    InvalidStructure(String),

    /// A task ID or requirement ID is unknown.
    #[error("unknown id: {0}")]
    UnknownId(String),
}
