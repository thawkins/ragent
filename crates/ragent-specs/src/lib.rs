//! Spec management system for ragent
//!
//! This crate provides:
//! - Core data structures for specs (Spec, Plan, Requirement, Task)
//! - Spec directory creation and template generation
//! - Spec discovery (walk `specs/` and parse directories)
//! - Atomic file read/write for spec files

pub mod commands;
pub mod constitution;
/// Typed errors for the spec management system.
pub mod error;
pub mod git;
pub mod id_scanner;
pub mod impl_runner;
pub mod io;
/// Spec lifecycle management: discovery, persistence, and state transitions.
pub mod manager;
pub mod plan_parser;
/// Spec data structures, templates, and validation.
pub mod spec;
pub mod templates;
pub mod validate;

pub use commands::SpecCommand;
pub use constitution::{
    Amendment, AmendmentIssue, AmendmentRequest, Article, Constitution, parse_constitution,
};
pub use error::SpecError;
pub use git::{BranchResult, create_spec_branch, spec_branch_name};
pub use impl_runner::{ImplOptions, ImplResult, MilestoneGroup, SpecImplRunner};
pub use io::SpecIo;
pub use manager::{
    SortBy, SpecFilter, SpecManager, SpecSearchResult, is_valid_transition, next_statuses,
};
pub use plan_parser::{
    Effort, Milestone, PhaseMinusOneGate, PhaseMinusOneGates, PlanParser, PlanTask, Priority,
    REQUIRED_GATE_NAMES,
};
pub use spec::{Plan, Requirement, Spec, SpecId, SpecStatus, Task, TaskStatus};
pub use templates::{ConstitutionTemplate, FeedbackTemplate, PlanTemplate, SpecTemplate};
pub use validate::{
    AmbiguityIssue, AmbiguityKind, Category, ClarificationMarker, ContradictionIssue,
    ContradictionKind, GapIssue, GapKind, Issue, Report, SddFlags, Severity, detect_ambiguity,
    detect_clarification_markers, detect_contradictions, detect_ears_template, detect_gaps,
    parse_requirements, validate, validate_clarifications, validate_consistency,
    validate_with_flags,
};
