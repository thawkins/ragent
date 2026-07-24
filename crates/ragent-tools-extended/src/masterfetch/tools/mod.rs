//! `MasterFetch` tool structs implementing the [`Tool`](crate::Tool) trait.
//!
//! This module aggregates the six `mf_*` tool structs:
//!
//! - [`fetch::MfFetchTool`] — `mf_fetch`
//! - [`crawl_tool::MfCrawlTool`] — `mf_crawl`
//! - [`search_tool::MfSearchTool`] — `mf_search`
//! - [`screenshot::MfScreenshotTool`] — `mf_screenshot`
//! - [`cache_clear::MfCacheClearTool`] — `mf_cache_clear`
//! - [`version::MfVersionTool`] — `mf_version`
//!
//! Each tool is registered in [`crate::create_extended_registry`] and surfaced
//! to agents via the existing `ExtractedExtendedToolAdapter` flow.
//!
//! # Requirement
//!
//! - **FR-020** — all six `mf_*` tools registered in `create_extended_registry()`.

pub mod cache_clear;
pub mod crawl_tool;
pub mod fetch;
pub mod screenshot;
pub mod search_tool;
pub mod version;
