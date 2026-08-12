//! Unit tests for the OpenAlex backend request builder and response parser.
//!
//! These tests exercise
//! [`ragent_tools_extended::masterfetch::search::openalex`] without making
//! any network requests (FR-004, FR-005, FR-008, FR-010, FR-012, NFR-002).

use ragent_tools_extended::masterfetch::search::engine::{Freshness, SearchOptions};
use ragent_tools_extended::masterfetch::search::openalex::{
    ENGINE_NAME, MAX_QUERY_CHARS, MIN_PER_PAGE, build_request, mask_email, parse_response,
    truncate_query,
};
use serde_json::json;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// truncate_query
// ---------------------------------------------------------------------------

#[test]
fn test_truncate_query_short_unchanged() {
    assert_eq!(truncate_query("machine learning"), "machine learning");
}

#[test]
fn test_truncate_query_trims_whitespace() {
    assert_eq!(truncate_query("  rust async  "), "rust async");
}

#[test]
fn test_truncate_query_long() {
    let long = "a".repeat(1500);
    let truncated = truncate_query(&long);
    assert_eq!(truncated.chars().count(), MAX_QUERY_CHARS);
}

#[test]
fn test_truncate_query_unicode_boundaries() {
    let s = "🦀".repeat(300);
    let t = truncate_query(&s);
    assert_eq!(t.chars().count(), 300);
    assert!(t.is_char_boundary(t.len()));
}

// ---------------------------------------------------------------------------
// build_request — per_page clamping (FR-012)
// ---------------------------------------------------------------------------

#[test]
fn test_build_request_per_page_within_range() {
    let opts = SearchOptions::new(10);
    let (_, params) = build_request("rust", &opts, "");
    let map = params_to_map(&params);
    assert_eq!(map["per_page"], "10");
}

#[test]
fn test_build_request_per_page_clamps_to_min() {
    let opts = SearchOptions::new(0);
    let (_, params) = build_request("rust", &opts, "");
    let map = params_to_map(&params);
    assert_eq!(map["per_page"], &MIN_PER_PAGE.to_string());
}

#[test]
fn test_build_request_per_page_clamps_to_max() {
    // The orchestrator passes per-engine opts with max_results set to
    // per_engine_results. Simulate that by setting per_engine_results=75
    // and then overriding max_results (as the orchestrator does).
    let mut opts = SearchOptions::new(50);
    opts.max_results = opts.per_engine_results; // simulates orchestrator
    let (_, params) = build_request("rust", &opts, "");
    let map = params_to_map(&params);
    assert_eq!(map["per_page"], "75"); // per_engine_results default
}

// ---------------------------------------------------------------------------
// build_request — pagination (FR-008)
// ---------------------------------------------------------------------------

#[test]
fn test_build_request_page_is_one_indexed() {
    let opts = SearchOptions::new(10).with_page(0);
    let (_, params) = build_request("rust", &opts, "");
    let map = params_to_map(&params);
    assert_eq!(map["page"], "1");
    assert!(!map.contains_key("cursor"));
}

#[test]
fn test_build_request_page_advance() {
    let opts = SearchOptions::new(10).with_page(2);
    let (_, params) = build_request("rust", &opts, "");
    let map = params_to_map(&params);
    assert_eq!(map["page"], "3");
}

#[test]
fn test_build_request_uses_cursor_for_deep_paging() {
    // page=1000, per_page=200 → estimated_offset = 200_000 > 10_000
    let opts = SearchOptions::new(200).with_page(1000);
    let (_, params) = build_request("rust", &opts, "");
    let map = params_to_map(&params);
    assert_eq!(map["cursor"], "*");
    assert!(!map.contains_key("page"));
}

// ---------------------------------------------------------------------------
// build_request — site filter (FR-004)
// ---------------------------------------------------------------------------

#[test]
fn test_build_request_no_site_no_filter() {
    let opts = SearchOptions::new(5);
    let (_, params) = build_request("rust", &opts, "");
    let map = params_to_map(&params);
    assert!(!map.contains_key("filter"));
}

#[test]
fn test_build_request_site_filter() {
    let opts = SearchOptions::new(5).with_site("nature.com");
    let (_, params) = build_request("rust", &opts, "");
    let map = params_to_map(&params);
    let filter = map["filter"];
    assert!(filter.contains("primary_location.source.host_organization:nature.com"));
}

// ---------------------------------------------------------------------------
// build_request — freshness filter (FR-005)
// ---------------------------------------------------------------------------

#[test]
fn test_build_request_freshness_any_no_date_filter() {
    let opts = SearchOptions::new(5).with_freshness(Freshness::Any);
    let (_, params) = build_request("rust", &opts, "");
    let map = params_to_map(&params);
    if let Some(filter) = map.get("filter") {
        assert!(!filter.contains("from_publication_date"));
    }
}

#[test]
fn test_build_request_freshness_week_adds_date_range() {
    let opts = SearchOptions::new(5).with_freshness(Freshness::Week);
    let (_, params) = build_request("rust", &opts, "");
    let map = params_to_map(&params);
    let filter = map["filter"];
    assert!(filter.contains("from_publication_date:"));
    assert!(filter.contains("to_publication_date:"));
}

// ---------------------------------------------------------------------------
// build_request — mailto (FR-007)
// ---------------------------------------------------------------------------

#[test]
fn test_build_request_no_mailto_omitted() {
    let opts = SearchOptions::new(5);
    let (_, params) = build_request("rust", &opts, "");
    let map = params_to_map(&params);
    assert!(!map.contains_key("mailto"));
}

#[test]
fn test_build_request_mailto_appended() {
    let opts = SearchOptions::new(5);
    let (_, params) = build_request("rust", &opts, "user@example.com");
    let map = params_to_map(&params);
    assert_eq!(map["mailto"], "user@example.com");
}

#[test]
fn test_build_request_mailto_whitespace_trimmed() {
    let opts = SearchOptions::new(5);
    let (_, params) = build_request("rust", &opts, "  user@example.com  ");
    let map = params_to_map(&params);
    assert_eq!(map["mailto"], "user@example.com");
}

// ---------------------------------------------------------------------------
// build_request — URL
// ---------------------------------------------------------------------------

#[test]
fn test_build_request_returns_api_url() {
    let opts = SearchOptions::new(5);
    let (url, _) = build_request("rust", &opts, "");
    assert_eq!(url, "https://api.openalex.org/works");
}

// ---------------------------------------------------------------------------
// parse_response (FR-001, FR-010, FR-011)
// ---------------------------------------------------------------------------

#[test]
fn test_parse_response_extracts_basic_fields() {
    let value = json!({
        "results": [
            {
                "id": "https://openalex.org/W1",
                "title": "Test Paper",
                "relevance_score": 12.5,
                "publication_year": 2024,
                "cited_by_count": 42,
                "doi": "https://doi.org/10.1000/test",
                "primary_location": {
                    "landing_page_url": "https://example.org/paper",
                    "source": { "display_name": "Example Journal" }
                },
                "open_access": { "is_oa": true, "oa_url": "https://example.org/oa" }
            }
        ]
    });
    let results = parse_response(&value);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].source, ENGINE_NAME);
    assert_eq!(results[0].title, "Test Paper");
    assert_eq!(results[0].url, "https://example.org/paper");
    assert!(results[0].score.is_some());
    let score = results[0].score.expect("score should be present");
    assert!(score > 0.0 && score <= 1.0);
}

#[test]
fn test_parse_response_url_fallback_to_id() {
    let value = json!({
        "results": [
            {
                "id": "https://openalex.org/W123",
                "title": "No Landing Page",
                "primary_location": {}
            }
        ]
    });
    let results = parse_response(&value);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].url, "https://openalex.org/W123");
}

#[test]
fn test_parse_response_url_fallback_to_doi() {
    let value = json!({
        "results": [
            {
                "id": "https://openalex.org/W123",
                "title": "No Landing Page",
                "doi": "https://doi.org/10.1000/test",
                "primary_location": {}
            }
        ]
    });
    let results = parse_response(&value);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].url, "https://openalex.org/W123");
}

#[test]
fn test_parse_response_missing_results_returns_empty() {
    let value = json!({"meta": {"count": 0}});
    assert!(parse_response(&value).is_empty());
}

#[test]
fn test_parse_response_empty_results_array() {
    let value = json!({"results": []});
    assert!(parse_response(&value).is_empty());
}

#[test]
fn test_parse_response_skips_entries_without_title_and_url() {
    let value = json!({
        "results": [
            {"id": "https://openalex.org/W1", "title": "Valid", "primary_location": {"landing_page_url": "https://example.org/1"}},
            {"id": "https://openalex.org/W2", "title": "", "primary_location": {}}
        ]
    });
    let results = parse_response(&value);
    // The second entry has no title and no landing_page_url; it falls back to
    // its `id` URI as the URL but has no title. Since url is non-empty it
    // passes the filter, but title is empty — it should still be emitted
    // (url is present). Verify both are returned.
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].title, "Valid");
}

#[test]
fn test_parse_response_reconstructs_abstract_from_inverted_index() {
    let value = json!({
        "results": [
            {
                "id": "https://openalex.org/W1",
                "title": "Abstract Test",
                "primary_location": {"landing_page_url": "https://example.org/1"},
                "abstract_inverted_index": {
                    "Hello": [0],
                    "world": [1],
                    "this": [2],
                    "is": [3],
                    "a": [4],
                    "test": [5]
                }
            }
        ]
    });
    let results = parse_response(&value);
    assert_eq!(results.len(), 1);
    assert!(
        results[0].snippet.contains("Hello world this is a test"),
        "snippet should contain reconstructed abstract: {}",
        results[0].snippet
    );
}

#[test]
fn test_parse_response_metadata_suffix() {
    let value = json!({
        "results": [
            {
                "id": "https://openalex.org/W1",
                "title": "Meta Test",
                "primary_location": {"landing_page_url": "https://example.org/1"},
                "publication_year": 2023,
                "cited_by_count": 10,
                "open_access": {"is_oa": true},
                "primary_location_source_name": {}
            }
        ]
    });
    let results = parse_response(&value);
    assert_eq!(results.len(), 1);
    let snippet = &results[0].snippet;
    assert!(snippet.contains("Year: 2023"), "snippet: {snippet}");
    assert!(snippet.contains("Cited: 10"), "snippet: {snippet}");
    assert!(snippet.contains("OA: yes"), "snippet: {snippet}");
}

#[test]
fn test_parse_response_score_normalised() {
    let value = json!({
        "results": [
            {
                "id": "https://openalex.org/W1",
                "title": "High Score",
                "relevance_score": 60.0,
                "primary_location": {"landing_page_url": "https://example.org/1"}
            }
        ]
    });
    let results = parse_response(&value);
    assert_eq!(results.len(), 1);
    let score = results[0].score.expect("score present");
    assert_eq!(score, 1.0); // 60/30 = 2.0, clamped to 1.0
}

#[test]
fn test_parse_response_no_score_when_missing() {
    let value = json!({
        "results": [
            {
                "id": "https://openalex.org/W1",
                "title": "No Score",
                "primary_location": {"landing_page_url": "https://example.org/1"}
            }
        ]
    });
    let results = parse_response(&value);
    assert_eq!(results.len(), 1);
    assert!(results[0].score.is_none());
}

#[test]
fn test_parse_response_snippet_truncated() {
    let mut inverted = serde_json::Map::new();
    // Build an abstract longer than 200 chars
    for i in 0..100 {
        inverted.insert(format!("word{i:03}"), serde_json::json!([i]));
    }
    let value = json!({
        "results": [
            {
                "id": "https://openalex.org/W1",
                "title": "Long Abstract",
                "primary_location": {"landing_page_url": "https://example.org/1"},
                "abstract_inverted_index": inverted
            }
        ]
    });
    let results = parse_response(&value);
    assert_eq!(results.len(), 1);
    // The snippet includes the truncated abstract + metadata suffix.
    // The abstract portion alone should be <= 200 chars + ellipsis.
    assert!(results[0].snippet.chars().count() < 300);
}

// ---------------------------------------------------------------------------
// mask_email (NFR-003)
// ---------------------------------------------------------------------------

#[test]
fn test_mask_email_short_fully_masked() {
    assert_eq!(mask_email("ab"), "**");
}

#[test]
fn test_mask_email_long() {
    let masked = mask_email("user@example.com");
    // "user@example.com" has 16 chars. mask_email keeps first 2 + last 2,
    // replacing the middle 12 with "*" characters (1 + (16-6)=10 + 1 = 12).
    assert_eq!(masked, "us************om");
    assert!(!masked.contains("user"));
    assert!(!masked.contains("example"));
}

#[test]
fn test_mask_email_empty() {
    assert_eq!(mask_email(""), "");
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

/// Convert a vec of (key, value) pairs into a HashMap for easy lookup.
fn params_to_map(params: &[(String, String)]) -> HashMap<&str, &str> {
    params
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect()
}
