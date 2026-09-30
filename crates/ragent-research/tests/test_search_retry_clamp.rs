//! ANTIPAT M0.2 regression tests: the search-retry budget is clamped.
//!
//! `--search-max-retries` is a CLI-supplied `u32` forwarded into
//! [`WebGatherer::with_search_max_retries`]. Before the fix the value was
//! stored unclamped and the retry backoff computed `1u64 << (attempt - 1)`,
//! which panicked for a value of 64 or more and slept for days for values in
//! the 40-63 range (ANTIPAT F-02).

use std::sync::Arc;

use ragent_research::web_gatherer::{
    MAX_SEARCH_RETRIES, WebFetchTool, WebFetchedPage, WebGatherer, WebSearchHit, WebSearchTool,
};

/// Minimal search tool: the clamp is a builder concern and no search runs here.
#[derive(Debug)]
struct NoopSearch;

#[async_trait::async_trait]
impl WebSearchTool for NoopSearch {
    async fn search(&self, _query: &str, _max_results: usize) -> anyhow::Result<Vec<WebSearchHit>> {
        Ok(Vec::new())
    }
}

/// Minimal fetch tool: never called by these tests.
#[derive(Debug)]
struct NoopFetch;

#[async_trait::async_trait]
impl WebFetchTool for NoopFetch {
    async fn fetch(&self, _url: &str) -> anyhow::Result<WebFetchedPage> {
        anyhow::bail!("noop fetch must not be called")
    }
}

fn gatherer_with_retries(n: u32) -> WebGatherer {
    WebGatherer::new(Arc::new(NoopSearch), Arc::new(NoopFetch)).with_search_max_retries(n)
}

/// A retry count far beyond the safe ceiling must not reach the gatherer.
///
/// The shift `1u64 << 64` is the exact panic ANTIPAT F-02 reported.
#[test]
fn test_search_max_retries_is_clamped_to_the_ceiling() {
    assert_eq!(
        gatherer_with_retries(64).search_max_retries(),
        MAX_SEARCH_RETRIES
    );
}

/// A value just below the old panic threshold is still clamped.
#[test]
fn test_search_max_retries_clamps_forty() {
    assert_eq!(
        gatherer_with_retries(40).search_max_retries(),
        MAX_SEARCH_RETRIES
    );
}

/// Values inside the accepted range and the meaningful lower bounds are kept.
#[test]
fn test_search_max_retries_keeps_small_values() {
    for n in [0, 1, 2, MAX_SEARCH_RETRIES] {
        assert_eq!(
            gatherer_with_retries(n).search_max_retries(),
            n,
            "value {n} must be kept"
        );
    }
}

/// `u32::MAX` (the worst CLI-supplied value) is clamped, not rejected.
#[test]
fn test_search_max_retries_clamps_u32_max() {
    assert_eq!(
        gatherer_with_retries(u32::MAX).search_max_retries(),
        MAX_SEARCH_RETRIES
    );
}
