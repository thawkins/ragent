//! Fetch layer: the builtin safe-HTTP lane and third-party providers.
//!
//! Requirements served here: FR-008 (fetch orchestration), FR-009 (safe HTTP
//! contract), FR-010 (provider abstraction), FR-011 (fallback in batch fetch).

pub mod pipeline;
pub mod provider;
pub mod safe_http;
