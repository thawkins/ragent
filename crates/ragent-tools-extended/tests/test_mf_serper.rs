//! Unit tests for the Serper backend request builder and response parser.
//!
//! These tests exercise [`ragent_tools_extended::masterfetch::search::serper`]
//! without making any network requests.

use ragent_tools_extended::masterfetch::search::engine::{Freshness, SearchOptions};
use ragent_tools_extended::masterfetch::search::serper::{
    ENGINE_NAME, MAX_COUNT, MIN_COUNT, build_request_body, freshness_to_tbs, mask_key,
    parse_response_json,
};
use serde_json::json;

#[test]
fn test_build_request_body_default_values() {
    let body = build_request_body("rust async", &SearchOptions::new(5));
    assert_eq!(body["q"], "rust async");
    assert_eq!(body["num"], 75);
    assert_eq!(body["page"], 1);
    assert!(body.get("tbs").is_none());
}

#[test]
fn test_build_request_body_clamps_num() {
    let body = build_request_body("rust", &SearchOptions::new(5).with_per_engine_results(500));
    assert_eq!(body["num"], MAX_COUNT);

    let body2 = build_request_body("rust", &SearchOptions::new(5).with_per_engine_results(0));
    assert_eq!(body2["num"], MIN_COUNT);
}

#[test]
fn test_build_request_body_site_filters_appended_to_query() {
    let opts = SearchOptions::new(5)
        .with_site("example.com")
        .with_exclude_sites(vec!["spam.example".to_string()]);
    let body = build_request_body("rust", &opts);
    assert_eq!(body["q"], "rust site:example.com -site:spam.example");
}

#[test]
fn test_build_request_body_page_is_one_indexed() {
    let body = build_request_body("rust", &SearchOptions::new(5).with_page(2));
    assert_eq!(body["page"], 3);
}

#[test]
fn test_freshness_to_tbs_mapping() {
    assert_eq!(freshness_to_tbs(Freshness::Day), Some("qdr:d"));
    assert_eq!(freshness_to_tbs(Freshness::Week), Some("qdr:w"));
    assert_eq!(freshness_to_tbs(Freshness::Month), Some("qdr:m"));
    assert_eq!(freshness_to_tbs(Freshness::Year), Some("qdr:y"));
    assert_eq!(freshness_to_tbs(Freshness::Any), None);
}

#[test]
fn test_build_request_body_freshness_sets_tbs() {
    let opts = SearchOptions::new(5).with_freshness(Freshness::Week);
    let body = build_request_body("rust", &opts);
    assert_eq!(body["tbs"], "qdr:w");
}

#[test]
fn test_parse_response_json_extracts_results() {
    let value = json!({
        "searchParameters": {"q": "rust"},
        "organic": [
            {
                "title": "Rust Language",
                "link": "https://www.rust-lang.org",
                "snippet": "A systems programming language.",
                "position": 1
            },
            {
                "title": "Rust Docs",
                "link": "https://doc.rust-lang.org",
                "snippet": "The Rust standard library documentation.",
                "position": 2
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
fn test_parse_response_json_missing_organic() {
    let value = json!({"searchParameters": {"q": "rust"}});
    assert!(parse_response_json(&value).is_empty());
}

#[test]
fn test_parse_response_json_invalid_items_ignored() {
    let value = json!({
        "organic": [
            {"title": "Valid", "link": "https://example.com", "snippet": "ok"},
            {"title": "No link", "snippet": "missing url"},
            {"link": "https://example.com/x", "snippet": "missing title"}
        ]
    });
    let results = parse_response_json(&value);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Valid");
}

#[test]
fn test_parse_response_json_snippet_truncation() {
    let content = "x".repeat(500);
    let value = json!({
        "organic": [
            {"title": "Long", "link": "https://example.com", "snippet": content}
        ]
    });
    let results = parse_response_json(&value);
    assert_eq!(results[0].snippet.chars().count(), 201);
    assert!(results[0].snippet.ends_with('…'));
}

#[test]
fn test_mask_key_short() {
    assert_eq!(mask_key("abc"), "***");
}

#[test]
fn test_mask_key_long() {
    let masked = mask_key("abcdefghij");
    assert!(masked.starts_with("ab"));
    assert!(masked.ends_with("ij"));
    assert!(masked.contains('*'));
    assert!(!masked.contains("cdefgh"));
}

#[test]
fn test_build_request_body_strips_quotes_from_query() {
    // Free Serper accounts reject quoted queries with HTTP 400 ("Query
    // pattern not allowed for free accounts"); quotes must be stripped
    // before the body is sent.
    let opts = SearchOptions::new(10);
    let body = build_request_body("\"agentic loop\" 'architecture'", &opts);
    let q = body["q"].as_str().unwrap_or("");
    assert!(
        !q.contains('"') && !q.contains('\''),
        "quotes should be stripped, got: {q}"
    );
    assert_eq!(q, "agentic loop architecture");
}
