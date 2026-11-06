//! Serde-only data types.
//!
//! Nothing here performs I/O or validation beyond what serde enforces, so the
//! types can be reused unchanged by the CLI, the MCP server, and the HTTP
//! renderer.

pub mod claim;
pub mod note;
pub mod run;
pub mod scholarly;

pub use crate::models::claim::{Claim, VerificationState};
pub use crate::models::note::{Note, NoteMeta};
pub use crate::models::run::{RunRecord, RunStatus};
pub use crate::models::scholarly::{Resolution, ScholarlyRecord, StudyDesign};
