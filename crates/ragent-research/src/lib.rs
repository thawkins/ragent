//! Research system for ragent.
//!
//! This crate provides the data structures and lifecycle helpers for the
//! `/research` slash command and the `ragent research` CLI. It enforces the
//! requirements defined in `specs/researchsystem/SPEC.md`.
//!
//! ## Modules
//!
//! - [`research_name`] — the URL-safe `ResearchName` newtype with full FR-002
//!   validation (lowercase ASCII letters/digits/hyphens, starting with a
//!   letter, 3-64 chars).
//! - [`status`] — the `ResearchStatus` enum (draft, in-progress, complete,
//!   archived) covering FR-013.
//! - [`source`] — the `Source` enum (Web/Local/Spec/Other) backing the
//!   References Index block in every `RESEARCH.md`.
//!
//! ## Future modules
//!
//! Additional modules will be added as later milestones land:
//!
//! - `manager` — `ResearchManager` with create/list/show/delete/archive
//! - `session` — gathering orchestration engine
//! - `io` — atomic file read/write for `RESEARCH.md`
//! - `index` — `research/INDEX.md` derived cache

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod research_name;
pub mod source;
pub mod status;

pub use research_name::{ResearchName, ResearchNameError, MAX_LEN, MIN_LEN};
pub use source::{LocalSourceKind, Source};
pub use status::ResearchStatus;
