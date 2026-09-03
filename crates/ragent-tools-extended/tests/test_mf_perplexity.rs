#![allow(clippy::assert_is_empty)]
//! Unit tests for the Perplexity backend request builder and response parser.
//!
//! These tests exercise [`ragent_tools_extended::masterfetch::search::perplexity`]
//! without making any network requests.

use ragent_tools_extended::masterfetch::search::engine::{Freshness, SearchOptions};
use ragent_tools_extended::masterfetch::search::perplexity::{
    ENGINE_NAME, MAX_QUERY_CHARS, build_request_body, mask_key, parse_response_json, truncate_query,
};
use serde_json::json;

// ===========================================================================
// Query truncation
// ===========================================================================

#[test]
fn test_truncate_query_short_unchanged() {
    assert_eq!(truncate_query("rust async"), "rust async");
}

#[test]
fn test_truncate_query_long() {
    let long = "a".repeat(5000);
    let truncated = truncate_query(&long);
    assert_eq!(truncated.chars().count(), MAX_QUERY_CHARS);
}

#[test]
fn test_truncate_query_unicode_boundaries() {
    let s = "🦀".repeat(3000);
    let t = truncate_query(&s);
    assert!(t.chars().count() <= MAX_QUERY_CHARS);
    assert!(t.is_char_boundary(t.len()));
}

// ===========================================================================
// Request body mapping
// ===========================================================================

#[test]
fn test_build_request_body_default_values() {
    let body = build_request_body("rust async", &SearchOptions::new(5), "sonar");
    assert_eq!(body["model"], "sonar");
    assert_eq!(body["messages"][0]["role"], "user");
    assert_eq!(body["messages"][0]["content"], "rust async");
    assert_eq!(body["max_tokens"], 1000); // 5 * 200
}

#[test]
fn test_build_request_body_clamps_max_tokens() {
    let body = build_request_body("rust", &SearchOptions::new(50), "sonar");
    assert_eq!(body["max_tokens"], 4000); // 50 * 200 = 10000, clamped to 4000
}

#[test]
fn test_build_request_body_custom_model() {
    let body = build_request_body("rust", &SearchOptions::new(5), "sonar-pro");
    assert_eq!(body["model"], "sonar-pro");
}

#[test]
fn test_build_request_body_truncates_query() {
    let long = "a".repeat(5000);
    let body = build_request_body(&long, &SearchOptions::new(5), "sonar");
    assert_eq!(
        body["messages"][0]["content"]
            .as_str()
            .unwrap()
            .chars()
            .count(),
        MAX_QUERY_CHARS
    );
}

#[test]
fn test_build_request_body_freshness_day() {
    let opts = SearchOptions::new(5).with_freshness(Freshness::Day);
    let body = build_request_body("query", &opts, "sonar");
    assert_eq!(body["search_recency_filter"], "day");
}

#[test]
fn test_build_request_body_freshness_week() {
    let opts = SearchOptions::new(5).with_freshness(Freshness::Week);
    let body = build_request_body("query", &opts, "sonar");
    assert_eq!(body["search_recency_filter"], "week");
}

#[test]
fn test_build_request_body_freshness_month() {
    let opts = SearchOptions::new(5).with_freshness(Freshness::Month);
    let body = build_request_body("query", &opts, "sonar");
    assert_eq!(body["search_recency_filter"], "month");
}

#[test]
fn test_build_request_body_freshness_year_maps_to_hour() {
    let opts = SearchOptions::new(5).with_freshness(Freshness::Year);
    let body = build_request_body("query", &opts, "sonar");
    assert_eq!(body["search_recency_filter"], "hour");
}

#[test]
fn test_build_request_body_freshness_any_omitted() {
    let opts = SearchOptions::new(5).with_freshness(Freshness::Any);
    let body = build_request_body("query", &opts, "sonar");
    assert!(
        body.get("search_recency_filter").is_none(),
        "search_recency_filter should be omitted for Any"
    );
}

// ===========================================================================
// Response parsing — search_results
// ===========================================================================

#[test]
fn test_parse_response_json_extracts_search_results() {
    let value = json!({
        "id": "resp-123",
        "model": "sonar",
        "choices": [
            {
                "message": {
                    "role": "assistant",
                    "content": "Some answer text."
                }
            }
        ],
        "search_results": [
            {
                "title": "Rust Language",
                "url": "https://www.rust-lang.org",
                "snippet": "A systems programming language."
            },
            {
                "title": "Rust Docs",
                "url": "https://doc.rust-lang.org",
                "text": "The Rust standard library documentation."
            }
        ]
    });
    let results = parse_response_json(&value);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].title, "Rust Language");
    assert_eq!(results[0].url, "https://www.rust-lang.org");
    assert_eq!(results[0].snippet, "A systems programming language.");
    assert_eq!(results[0].source, ENGINE_NAME);
    assert_eq!(results[1].title, "Rust Docs");
    // Second result uses `text` field as snippet fallback
    assert_eq!(
        results[1].snippet,
        "The Rust standard library documentation."
    );
}

#[test]
fn test_parse_response_json_missing_search_results() {
    let value = json!({"choices": [{"message": {"content": "answer"}}]});
    assert!(parse_response_json(&value).is_empty());
}

#[test]
fn test_parse_response_json_invalid_items_ignored() {
    let value = json!({
        "search_results": [
            {"title": "Valid", "url": "https://example.com", "snippet": "ok"},
            {"title": "No URL"},
            {"url": "https://no-title.com", "snippet": "no title ok"},
            "not-an-object"
        ]
    });
    let results = parse_response_json(&value);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].title, "Valid");
    assert_eq!(results[1].title, "");
    assert_eq!(results[1].url, "https://no-title.com");
}

#[test]
fn test_parse_response_json_snippet_truncation() {
    let content = "a".repeat(250);
    let value = json!({
        "search_results": [
            {"title": "Long", "url": "https://example.com", "snippet": content}
        ]
    });
    let results = parse_response_json(&value);
    assert_eq!(results.len(), 1);
    let snippet = &results[0].snippet;
    assert!(snippet.chars().count() <= 201);
    assert!(snippet.ends_with('…'));
}

// ===========================================================================
// Response parsing — citations fallback
// ===========================================================================

#[test]
fn test_parse_response_json_falls_back_to_citations() {
    let value = json!({
        "choices": [{"message": {"content": "answer"}}],
        "citations": [
            "https://example.com/page1",
            "https://example.com/page2"
        ]
    });
    let results = parse_response_json(&value);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].url, "https://example.com/page1");
    assert_eq!(results[0].title, "");
    assert_eq!(results[0].snippet, "");
    assert_eq!(results[1].url, "https://example.com/page2");
    assert_eq!(results[1].source, ENGINE_NAME);
}

#[test]
fn test_parse_response_json_empty_citations() {
    let value = json!({"citations": []});
    let results = parse_response_json(&value);
    assert!(results.is_empty());
}

#[test]
fn test_parse_response_json_citations_with_invalid_entries() {
    let value = json!({
        "citations": [
            "https://valid.com",
            12345,
            null,
            "https://also-valid.com"
        ]
    });
    let results = parse_response_json(&value);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].url, "https://valid.com");
    assert_eq!(results[1].url, "https://also-valid.com");
}

// ===========================================================================
// API key masking
// ===========================================================================

#[test]
fn test_mask_key_short() {
    assert_eq!(mask_key("abc"), "***");
}

#[test]
fn test_mask_key_long() {
    let masked = mask_key("pplx-abcd1234efgh5678");
    assert_eq!(masked, "pp*****************78");
}
