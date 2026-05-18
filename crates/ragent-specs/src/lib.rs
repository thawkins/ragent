//! Spec management system for ragent
//!
//! This crate provides:
//! - Core data structures for specs (Spec, Plan, Requirement, Task)
//! - Spec directory creation and template generation
//! - Spec discovery (walk `specs/` and parse directories)
//! - Atomic file read/write for spec files

pub mod commands;
pub mod error;
pub mod io;
pub mod manager;
pub mod spec;
pub mod templates;
pub mod validate;

pub use commands::SpecCommand;
pub use error::SpecError;
pub use io::SpecIo;
pub use manager::{SpecManager, SpecFilter, SortBy, SpecSearchResult, is_valid_transition, next_statuses};
pub use spec::{Plan, Requirement, Spec, SpecId, SpecStatus, Task, TaskStatus};
pub use templates::{PlanTemplate, SpecTemplate};
pub use validate::{validate, Report, Issue, Severity, Category, detect_ears_template, parse_requirements};
