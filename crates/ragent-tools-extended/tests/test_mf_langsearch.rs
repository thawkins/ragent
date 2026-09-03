#![allow(clippy::assert_is_empty)]
//! Unit tests for `masterfetch::search::langsearch` — LangSearch API backend
//! (T-003, FR-001, FR-003, FR-005).
//!
//! The request-body builder and response parser are pure functions and are
//! tested here without any network I/O. The full `search()` method requires a
//! real API key and is tested with `#[ignore]`-gated integration tests.

use ragent_tools_extended::masterfetch::search::engine::{Freshness, RawResult, SearchOptions};
use ragent_tools_extended::masterfetch::search::langsearch::{
    ENGINE_NAME, build_request_body, mask_key, parse_response_json,
};

// ===========================================================================
// Request body mapping
// ===========================================================================

#[test]
fn test_build_request_body_maps_query() {
    let opts = SearchOptions::new(5);
    let body = build_request_body("rust async", &opts);
    assert_eq!(body["query"], "rust async");
}

#[test]
fn test_build_request_body_trims_query() {
    let opts = SearchOptions::new(5);
    let body = build_request_body("  rust async  ", &opts);
    assert_eq!(body["query"], "rust async");
}

#[test]
fn test_build_request_body_clamps_high_count() {
    let opts = SearchOptions::new(50);
    let body = build_request_body("rust async", &opts);
    assert_eq!(body["count"], 10);
}

#[test]
fn test_build_request_body_clamps_low_count() {
    let opts = SearchOptions::new(0);
    let body = build_request_body("rust async", &opts);
    assert_eq!(body["count"], 1);
}

#[test]
fn test_build_request_body_count_boundary_ten() {
    let opts = SearchOptions::new(10);
    let body = build_request_body("rust async", &opts);
    assert_eq!(body["count"], 10);
}

#[test]
fn test_build_request_body_count_boundary_one() {
    let opts = SearchOptions::new(1);
    let body = build_request_body("rust async", &opts);
    assert_eq!(body["count"], 1);
}

#[test]
fn test_build_request_body_freshness_mapping() {
    let cases = [
        (Freshness::Day, "oneDay"),
        (Freshness::Week, "oneWeek"),
        (Freshness::Month, "oneMonth"),
        (Freshness::Year, "oneYear"),
        (Freshness::Any, "noLimit"),
    ];
    for (freshness, expected) in cases {
        let opts = SearchOptions::new(5).with_freshness(freshness);
        let body = build_request_body("query", &opts);
        assert_eq!(body["freshness"], expected, "freshness {freshness:?}");
    }
}

#[test]
fn test_build_request_body_summary_true() {
    let opts = SearchOptions::new(5);
    let body = build_request_body("query", &opts);
    assert_eq!(body["summary"], true);
}

// ===========================================================================
// Response parsing
// ===========================================================================

#[test]
fn test_parse_response_json_extracts_web_pages() {
    let response = serde_json::json!({
        "data": {
            "webPages": {
                "value": [
                    {
                        "name": "Example",
                        "url": "https://example.com",
                        "summary": "A generated summary."
                    }
                ]
            }
        }
    });

    let results = parse_response_json(&response);
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0],
        RawResult::new(
            "Example",
            "https://example.com",
            "A generated summary.",
            ENGINE_NAME
        )
    );
}

#[test]
fn test_parse_response_json_falls_back_to_snippet() {
    let response = serde_json::json!({
        "data": {
            "webPages": {
                "value": [
                    {
                        "name": "Fallback",
                        "url": "https://fallback.com",
                        "snippet": "A plain snippet."
                    }
                ]
            }
        }
    });

    let results = parse_response_json(&response);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].snippet, "A plain snippet.");
}

#[test]
fn test_parse_response_json_prefers_summary_over_snippet() {
    let response = serde_json::json!({
        "data": {
            "webPages": {
                "value": [
                    {
                        "name": "Both",
                        "url": "https://both.com",
                        "summary": "Summary wins.",
                        "snippet": "Snippet loses."
                    }
                ]
            }
        }
    });

    let results = parse_response_json(&response);
    assert_eq!(results[0].snippet, "Summary wins.");
}

#[test]
fn test_parse_response_json_returns_empty_on_missing_data() {
    let response = serde_json::json!({ "data": {} });
    let results = parse_response_json(&response);
    assert!(results.is_empty());
}

// ===========================================================================
// API key masking
// ===========================================================================

#[test]
fn test_mask_key_short() {
    assert_eq!(mask_key("abc"), "***");
}

#[test]
fn test_mask_key_normal() {
    assert_eq!(mask_key("ls-abcdefghijk"), "ls**********jk");
}

// ===========================================================================
// Live API gate (T-011)
// ===========================================================================

/// Live LangSearch API test.
///
/// Ignored by default because it requires a valid `LANGSEARCH_API_KEY`
/// environment variable. Set the variable and run with `--ignored` to execute.
#[test]
#[ignore = "requires a live LangSearch API key"]
fn test_live_langsearch_api_returns_results() {
    use ragent_tools_extended::masterfetch::search::langsearch::LangSearchEngine;
    use ragent_tools_extended::masterfetch::search::{SearchEngine, SearchOptions};

    let api_key = std::env::var("LANGSEARCH_API_KEY").expect("LANGSEARCH_API_KEY not set");
    let engine = LangSearchEngine::new(api_key);

    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let report =
        runtime.block_on(engine.search("rust programming language", &SearchOptions::new(3)));

    assert!(
        !report.results.is_empty(),
        "expected at least one result from live LangSearch API, got error: {:?}",
        report.error
    );
    assert_eq!(report.engine, "langsearch");
}
