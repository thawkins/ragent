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

pub mod parse;
pub mod resolve;
pub mod fuzzy;

pub use parse::{FileRef, ParsedRef, parse_refs};
pub use resolve::{ResolvedRef, resolve_ref, resolve_all_refs};
pub use fuzzy::{collect_project_files, fuzzy_match, FuzzyMatch};
