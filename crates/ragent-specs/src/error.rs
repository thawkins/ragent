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
    InvalidStatusTransition {
        /// The source status.
        from: String,
        /// The destination status.
        to: String,
    },

    /// Validation of a spec failed.
    #[error("validation failed: {0}")]
    Validation(String),

    /// A spec directory has an invalid structure.
    #[error("invalid spec structure: {0}")]
    InvalidStructure(String),

    /// A task ID or requirement ID is unknown.
    #[error("unknown id: {0}")]
    UnknownId(String),

    /// A dependency cycle was detected in the task graph.
    #[error("dependency cycle detected involving tasks: {}", task_ids.join(", "))]
    DependencyCycle {
        /// Task IDs involved in the cycle.
        task_ids: Vec<String>,
    },

    /// A plan file could not be parsed.
    #[error("plan parse error: {0}")]
    PlanParse(String),

    /// A spec is already in a terminal status and re-implementation requires
    /// confirmation.
    #[error("spec {spec_id} is already {status}; re-implementation requires confirmation")]
    AlreadyImplemented {
        /// The spec identifier.
        spec_id: String,
        /// The current status string.
        status: String,
    },

    /// A spec cannot transition to `approved` while unresolved
    /// `[NEEDS CLARIFICATION]` markers remain in SPEC.md (FR-003).
    #[error("cannot approve spec: {count} unresolved [NEEDS CLARIFICATION] marker(s) remain")]
    UnresolvedClarifications {
        /// Number of unresolved clarification markers.
        count: usize,
    },

    /// A spec cannot transition to `in_progress` when Phase -1 gates
    /// (Simplicity, Anti-Abstraction, Integration-First) are unchecked or
    /// missing in PLAN.md (FR-008).
    #[error("cannot start implementation: {} Phase -1 gate(s) unchecked or missing: {}", gates.len(), gates.join(", "))]
    UncheckedPhaseGates {
        /// Names of the unchecked or missing required gates.
        gates: Vec<String>,
    },

    /// A constitutional amendment is invalid (FR-016).
    #[error("amendment error: {0}")]
    AmendmentError(String),
}
