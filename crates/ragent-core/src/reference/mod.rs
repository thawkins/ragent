//! @ file reference parsing, resolution, and fuzzy matching.
//!
//! This module implements SPEC §3.34 — the `@` syntax for inline file,
//! directory, and URL references in prompts. When a user types `@filename`,
//! `@path/to/file`, `@path/to/dir/`, or `@https://example.com`, the reference
//! is detected, resolved, and its content appended to the prompt.
//!
//! # Modules
//!
//! - [`parse`] — Detect and classify `@` references in input text.
//! - [`resolve`] — Resolve references to actual file/directory/URL content.
//! - [`fuzzy`] — Fuzzy file matching for bare `@filename` references.

pub mod fuzzy;
pub mod parse;
pub mod resolve;

pub use fuzzy::{FuzzyMatch, collect_project_files, fuzzy_match};
pub use parse::{FileRef, ParsedRef, parse_refs};
pub use resolve::{ResolvedRef, resolve_all_refs, resolve_ref};
