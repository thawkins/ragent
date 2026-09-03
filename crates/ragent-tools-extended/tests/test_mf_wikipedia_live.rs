#![allow(clippy::assert_is_empty)]
//! Integration test for the Wikipedia search backend hitting the live API.
//!
//! This test is `#[ignore]` by default to avoid network dependencies in CI.
//! Run with: `cargo test -p ragent-tools-extended --test test_mf_wikipedia_live -- --ignored --nocapture`

use ragent_tools_extended::masterfetch::search::engine::{SearchEngine, SearchOptions};
use ragent_tools_extended::masterfetch::search::wikipedia::WikipediaEngine;

#[tokio::test]
#[ignore = "requires network access to en.wikipedia.org"]
async fn test_wikipedia_engine_live_search() {
    let engine = WikipediaEngine::new();
    let opts = SearchOptions::new(5);

    let report = engine.search("rust programming language", &opts).await;

    // The engine should not be blocked or errored.
    assert!(
        !report.engine_blocked,
        "engine should not be blocked: {}",
        report.error
    );
    assert!(
        report.error.is_empty(),
        "engine should not have an error: {}",
        report.error
    );

    // Should return some results.
    assert!(
        !report.results.is_empty(),
        "expected at least one result from Wikipedia"
    );

    // Each result should have the correct source.
    for result in &report.results {
        assert_eq!(result.source, "wikipedia");
        assert!(!result.title.is_empty(), "result title should not be empty");
        assert!(!result.url.is_empty(), "result URL should not be empty");
        assert!(
            result.url.contains("wikipedia.org"),
            "result URL should point to wikipedia.org: {}",
            result.url
        );
    }

    println!(
        "wikipedia live search returned {} results",
        report.results.len()
    );
    for (i, result) in report.results.iter().enumerate().take(3) {
        println!(
            "  {}: {} — {}",
            i + 1,
            result.title,
            &result.snippet[..result.snippet.len().min(80)]
        );
    }
}

#[tokio::test]
#[ignore = "requires network access to en.wikipedia.org"]
async fn test_wikipedia_engine_live_empty_query() {
    let engine = WikipediaEngine::new();
    let opts = SearchOptions::new(5);

    let report = engine.search("", &opts).await;

    // Empty query should return an error report, not panic.
    assert!(
        !report.error.is_empty(),
        "empty query should produce an error"
    );
    assert!(report.results.is_empty());
}

#[tokio::test]
#[ignore = "requires network access to en.wikipedia.org"]
async fn test_wikipedia_engine_live_respects_max_results() {
    let engine = WikipediaEngine::new();
    let opts = SearchOptions::new(3);

    let report = engine.search("photosynthesis", &opts).await;

    if !report.engine_blocked {
        assert!(
            report.results.len() <= 3,
            "should respect max_results cap: got {}",
            report.results.len()
        );
    }
}
