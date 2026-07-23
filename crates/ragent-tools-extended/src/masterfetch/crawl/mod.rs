//! Crawl orchestration modules for masterfetch.
//!
//! This module groups the content-adaptive crawl sub-modules:
//!
//! - [`classify`] — page-type classification + extraction dispatch (T-018)
//! - [`orchestrator`] — best-first same-domain crawl orchestration (T-019)
//!
//! # Requirements
//!
//! - **FR-011** — best-first same-domain crawl with priority queue.
//! - **FR-012** — content-adaptive extraction per page type.
//! - **FR-013** — sitemap mode and discover-only mode.
//! - **FR-014** — selective crawl (`crawl_urls` parameter).
//! - **FR-029** — page-type detection driving extraction strategy.
//! - **NFR-001** — performance: 10-page crawl within 60 seconds.
//! - **NFR-003** — pure scoring/filtering functions are testable without
//!   network I/O.

pub mod classify;
pub mod orchestrator;

// Re-export the most commonly used types at the module level for convenience.
pub use orchestrator::{
    CrawlConfig, CrawlFetcher, CrawlOrchestrator, CrawlResult, FetchedPage, SitemapMode,
    TruncatedBy, extract_domain, is_same_domain, normalize_and_dedup, score_url,
};
