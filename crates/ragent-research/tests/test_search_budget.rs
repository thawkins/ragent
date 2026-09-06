//! Tests for the run-scoped search budget and shared query cache
//! (`crates/ragent-research/src/search_budget.rs`).

use std::sync::Arc;

use ragent_research::search_budget::{SearchBudget, SharedQueryCache};
use ragent_research::web_gatherer::WebSearchHit;

#[test]
fn test_search_budget_unlimited_always_acquires() {
    let b = SearchBudget::new(None);
    for _ in 0..100 {
        assert!(b.try_acquire());
    }
    assert!(!b.exhausted());
}

#[test]
fn test_search_budget_exhausts_at_limit() {
    let b = SearchBudget::new(Some(3));
    assert!(b.try_acquire());
    assert!(b.try_acquire());
    assert!(b.try_acquire());
    assert!(!b.try_acquire());
    assert_eq!(b.used(), 3);
    assert!(b.exhausted());
}

#[test]
fn test_search_budget_shared_counter_across_arc() {
    let b = Arc::new(SearchBudget::new(Some(2)));
    let b2 = b.clone();
    assert!(b.try_acquire());
    assert!(b2.try_acquire());
    assert!(!b.try_acquire());
    assert_eq!(b2.used(), 2);
}

fn sample_hit(url: &str, matched_query: &str) -> WebSearchHit {
    WebSearchHit {
        url: url.to_string(),
        title: "Example".to_string(),
        snippet: String::new(),
        matched_query: matched_query.to_string(),
        search_tool: "mf_search".to_string(),
        search_engine: "wikipedia".to_string(),
        author: None,
    }
}

#[test]
fn test_shared_query_cache_roundtrip_and_normalization() {
    let c = SharedQueryCache::new();
    assert!(c.get("rust lifetimes").is_none());
    c.insert(
        "Rust   LIFETIMES",
        vec![sample_hit("https://example.com", "rust")],
    );
    // Different casing/whitespace normalizes to the same key.
    let cached = c.get("rust lifetimes").expect("cache hit expected");
    assert_eq!(cached.len(), 1);
    assert_eq!(cached[0].url, "https://example.com");
}

#[test]
fn test_shared_query_cache_skips_empty_results() {
    let c = SharedQueryCache::new();
    c.insert("empty query", Vec::new());
    assert!(c.get("empty query").is_none());
}
