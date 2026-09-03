#![allow(clippy::assert_is_empty)]
//! Unit tests for the Tavily backend request builder and response parser.
//!
//! These tests exercise [`ragent_tools_extended::masterfetch::search::tavily`]
//! without making any network requests (FR-003, FR-004, NFR-003).

use ragent_tools_extended::masterfetch::search::engine::SearchOptions;
use ragent_tools_extended::masterfetch::search::tavily::{
    ENGINE_NAME, MAX_QUERY_CHARS, build_request_body, mask_key, parse_response_json, truncate_query,
};
use serde_json::json;

#[test]
fn test_truncate_query_short_unchanged() {
    assert_eq!(truncate_query("rust async"), "rust async");
}

#[test]
fn test_truncate_query_long() {
    let long = "a".repeat(500);
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

#[test]
fn test_build_request_body_clamps_max_results() {
    let opts = SearchOptions::new(25);
    let body = build_request_body("rust", &opts);
    assert_eq!(body["max_results"], 20);

    let opts2 = SearchOptions::new(0);
    let body2 = build_request_body("rust", &opts2);
    assert_eq!(body2["max_results"], 1);
}

#[test]
fn test_build_request_body_truncates_query() {
    let long = "a".repeat(500);
    let body = build_request_body(&long, &SearchOptions::new(5));
    assert_eq!(
        body["query"].as_str().unwrap().chars().count(),
        MAX_QUERY_CHARS
    );
}

#[test]
fn test_build_request_body_default_values() {
    let body = build_request_body("rust async", &SearchOptions::new(5));
    assert_eq!(body["query"], "rust async");
    assert_eq!(body["max_results"], 5);
    assert_eq!(body["include_answer"], false);
    assert_eq!(body["search_depth"], "basic");
}

#[test]
fn test_parse_response_json_extracts_results() {
    let value = json!({
        "query": "rust",
        "results": [
            {
                "title": "Rust Language",
                "url": "https://www.rust-lang.org",
                "content": "A systems programming language.",
                "score": 0.9
            },
            {
                "title": "Rust Docs",
                "url": "https://doc.rust-lang.org",
                "content": "The Rust standard library documentation."
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
}

#[test]
fn test_parse_response_json_missing_results() {
    let value = json!({"query": "rust"});
    assert!(parse_response_json(&value).is_empty());
}

#[test]
fn test_parse_response_json_invalid_items_ignored() {
    let value = json!({
        "results": [
            {"title": "Valid", "url": "https://example.com", "content": "ok"},
            {"title": "No URL"},
            {"url": "https://other.com"}
        ]
    });
    let results = parse_response_json(&value);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Valid");
}

#[test]
fn test_parse_response_json_snippet_truncation() {
    let content = "a".repeat(250);
    let value = json!({
        "results": [
            {"title": "Long", "url": "https://example.com", "content": content}
        ]
    });
    let results = parse_response_json(&value);
    assert_eq!(results.len(), 1);
    let snippet = &results[0].snippet;
    assert!(snippet.chars().count() <= 201);
    assert!(snippet.ends_with('…'));
}

#[test]
fn test_mask_key_short() {
    assert_eq!(mask_key("abc"), "***");
}

#[test]
fn test_mask_key_long() {
    let masked = mask_key("tvly-abcd1234efgh5678");
    assert_eq!(masked, "tv*****************78");
}
