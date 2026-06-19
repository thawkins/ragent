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
//!   letter, 3-64 chars) and FR-017 path-traversal rejection.
//! - [`status`] — the `ResearchStatus` enum (draft, in-progress, complete,
//!   archived) covering FR-013.
//! - [`source`] — the `Source` enum (Web/Local/Spec/Other) backing the
//!   References Index block in every `RESEARCH.md`.
//! - [`item`] — the `ResearchItem` struct that ties name, title, status,
//!   timestamps, and sources together for FR-005. Includes YAML frontmatter
//!   rendering and parsing.
//! - [`web_gatherer`] — the `WebGatherer` that orchestrates web discovery
//!   and capture for FR-006 and FR-007.
//! - [`local_gatherer`] — the `LocalGatherer` that orchestrates local
//!   cross-referencing and FR-019 `--sources-dir` scanning for FR-006,
//!   FR-008, and FR-009.
//! - [`plan_dep`] — the parser for `research: <name>` dependency lines in
//!   `specs/<id>/PLAN.md` for FR-015.
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

pub mod item;
pub mod local_gatherer;
pub mod plan_dep;
pub mod research_name;
pub mod source;
pub mod status;
pub mod web_gatherer;

pub use item::{ResearchItem, ResearchItemError};
pub use local_gatherer::{
    DEFAULT_GLOBS, DEFAULT_MAX_LOCAL_SOURCES, GrepMatch, LocalGatherConfig, LocalGatherError,
    LocalGatherer, LocalTool, derive_terms, local_body_path,
};
pub use plan_dep::{
    ResearchDependency, ResearchDependencyError, parse_research_dependencies,
    research_dependency_names,
};
pub use research_name::{ResearchName, ResearchNameError, MAX_LEN, MIN_LEN, is_path_traversal};
pub use source::{LocalSourceKind, Source};
pub use status::ResearchStatus;
pub use web_gatherer::{
    WebFetchTool, WebFetchedPage, WebGatherError, WebGatherer, WebSearchHit, WebSearchTool,
};