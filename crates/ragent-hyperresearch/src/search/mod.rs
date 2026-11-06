//! Full-text index and evidence filters (FR-017).
//!
//! The index covers note titles, bodies, and frontmatter tags; evidence filters
//! narrow a result set by minimum source quality, study design, and publication
//! year range.

pub mod evidence;
pub mod index;
