#![allow(clippy::assert_is_empty)]
//! Integration test for the OpenAlex backend hitting the live OpenAlex API.
//!
//! This test is `#[ignore]` by default because it requires network access.
//! Run it explicitly with:
//!
//! ```sh
//! cargo test -p ragent-tools-extended --test test_mf_openalex_live -- --ignored --nocapture
//! ```

use ragent_tools_extended::masterfetch::search::SearchOptions;
use ragent_tools_extended::masterfetch::search::engine::SearchEngine;
use ragent_tools_extended::masterfetch::search::openalex::OpenAlexEngine;

/// Verify that a live OpenAlex search returns results for a simple query.
///
/// This test exercises the full HTTP path: request building, network call,
/// JSON parsing, abstract reconstruction, and metadata extraction. It is
/// `#[ignore]` to avoid network dependency in CI.
#[tokio::test]
#[ignore = "requires network access to https://api.openalex.org"]
async fn test_openalex_live_search_returns_results() {
    let engine = OpenAlexEngine::new();
    let opts = SearchOptions::new(5);
    let report = engine.search("machine learning", &opts).await;

    assert_eq!(report.engine, "openalex");
    assert!(
        report.is_success(),
        "engine should succeed, but got: {} (blocked={})",
        report.error,
        report.engine_blocked
    );
    assert!(
        !report.results.is_empty(),
        "live OpenAlex search should return at least one result"
    );

    let first = &report.results[0];
    assert_eq!(first.source, "openalex");
    assert!(!first.title.is_empty(), "result should have a title");
    assert!(!first.url.is_empty(), "result should have a url");
}

/// Verify that a live OpenAlex search with a `mailto` polite-pool email works.
#[tokio::test]
#[ignore = "requires network access to https://api.openalex.org"]
async fn test_openalex_live_search_with_mailto() {
    let engine = OpenAlexEngine::with_mailto("test@ragent.dev");
    let opts = SearchOptions::new(3);
    let report = engine.search("rust programming language", &opts).await;

    assert_eq!(report.engine, "openalex");
    assert!(report.is_success(), "engine should succeed with mailto");
    assert!(!report.results.is_empty());
}

/// Verify that an empty query returns an error report (not a panic).
#[tokio::test]
async fn test_openalex_empty_query_returns_error_report() {
    let engine = OpenAlexEngine::new();
    let opts = SearchOptions::new(5);
    let report = engine.search("   ", &opts).await;

    assert_eq!(report.engine, "openalex");
    assert!(!report.is_success());
    assert!(!report.error.is_empty());
    assert!(report.results.is_empty());
}
