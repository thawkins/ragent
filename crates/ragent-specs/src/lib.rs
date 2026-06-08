//! Spec management system for ragent
//!
//! This crate provides:
//! - Core data structures for specs (Spec, Plan, Requirement, Task)
//! - Spec directory creation and template generation
//! - Spec discovery (walk `specs/` and parse directories)
//! - Atomic file read/write for spec files

pub mod commands;
pub mod error;
pub mod id_scanner;
pub mod impl_runner;
pub mod io;
pub mod manager;
pub mod plan_parser;
pub mod spec;
pub mod templates;
pub mod validate;

pub use commands::SpecCommand;
pub use error::SpecError;
pub use impl_runner::{ImplOptions, ImplResult, SpecImplRunner};
pub use io::SpecIo;
pub use manager::{
    SortBy, SpecFilter, SpecManager, SpecSearchResult, is_valid_transition, next_statuses,
};
pub use plan_parser::{Effort, PlanParser, PlanTask, Priority};
pub use spec::{Plan, Requirement, Spec, SpecId, SpecStatus, Task, TaskStatus};
pub use templates::{PlanTemplate, SpecTemplate};
pub use validate::{
    Category, Issue, Report, Severity, detect_ears_template, parse_requirements, validate,
};
