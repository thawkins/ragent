//! Unit tests for the Wikipedia search backend's pure functions.
//!
//! These tests exercise `build_search_request`, `parse_search_response`,
//! `parse_summary_response`, `build_summary_url`, and `truncate_query` — all
//! of which are pure functions that accept plain inputs and return plain
//! outputs, enabling tests without network I/O (NFR-002).

use ragent_tools_extended::masterfetch::search::engine::SearchOptions;
use ragent_tools_extended::masterfetch::search::wikipedia::{
    ENGINE_NAME, MAX_QUERY_CHARS, MAX_SEARCH_LIMIT, build_search_request, build_summary_url,
    parse_search_response, parse_summary_response, truncate_query,
};
use serde_json::json;

// ---------------------------------------------------------------------------
// build_search_request
// ---------------------------------------------------------------------------

#[test]
fn test_build_search_request_basic() {
    let opts = SearchOptions::new(10);
    let (url, params) = build_search_request("rust programming language", &opts);
    assert_eq!(url, "https://en.wikipedia.org/w/api.php");

    let map: std::collections::HashMap<&str, &str> = params
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    assert_eq!(map["action"], "query");
    assert_eq!(map["list"], "search");
    assert_eq!(map["srsearch"], "rust programming language");
    assert_eq!(map["srlimit"], "10");
    assert_eq!(map["srprop"], "snippet");
    assert_eq!(map["format"], "json");
    assert_eq!(map["origin"], "*");
}

#[test]
fn test_build_search_request_trims_query() {
    let opts = SearchOptions::new(5);
    let (_url, params) = build_search_request("  spaced query  ", &opts);
    let map: std::collections::HashMap<&str, &str> = params
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    assert_eq!(map["srsearch"], "spaced query");
}

#[test]
fn test_build_search_request_clamps_srlimit_to_max() {
    let opts = SearchOptions::new(10_000);
    let (_url, params) = build_search_request("test", &opts);
    let map: std::collections::HashMap<&str, &str> = params
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    assert_eq!(map["srlimit"], MAX_SEARCH_LIMIT.to_string());
}

#[test]
fn test_build_search_request_clamps_srlimit_to_min() {
    let opts = SearchOptions::new(0);
    let (_url, params) = build_search_request("test", &opts);
    let map: std::collections::HashMap<&str, &str> = params
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    // SearchOptions::new clamps to 1, so srlimit should be at least 1
    let srlimit: usize = map["srlimit"].parse().unwrap_or(0);
    assert!(srlimit >= 1);
}

#[test]
fn test_build_search_request_truncates_long_query() {
    let long_query = "a".repeat(MAX_QUERY_CHARS + 100);
    let opts = SearchOptions::new(5);
    let (_url, params) = build_search_request(&long_query, &opts);
    let map: std::collections::HashMap<&str, &str> = params
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let srsearch = map["srsearch"];
    assert!(srsearch.chars().count() <= MAX_QUERY_CHARS);
}

// ---------------------------------------------------------------------------
// parse_search_response
// ---------------------------------------------------------------------------

#[test]
fn test_parse_search_response_basic() {
    let value = json!({
        "query": {
            "search": [
                { "title": "Rust (programming language)" },
                { "title": "Rust" },
                { "title": "Ferrous" }
            ]
        }
    });
    let titles = parse_search_response(&value);
    assert_eq!(
        titles,
        vec![
            "Rust (programming language)".to_string(),
            "Rust".to_string(),
            "Ferrous".to_string()
        ]
    );
}

#[test]
fn test_parse_search_response_skips_entries_without_title() {
    let value = json!({
        "query": {
            "search": [
                { "title": "Photosynthesis" },
                { "ns": 0, "pageid": 123 },
                { "title": "" },
                { "title": "Mitosis" }
            ]
        }
    });
    let titles = parse_search_response(&value);
    assert_eq!(titles, vec!["Photosynthesis", "Mitosis"]);
}

#[test]
fn test_parse_search_response_empty_search_array() {
    let value = json!({ "query": { "search": [] } });
    let titles = parse_search_response(&value);
    assert!(titles.is_empty());
}

#[test]
fn test_parse_search_response_missing_query_field() {
    let value = json!({ "batchcomplete": "" });
    let titles = parse_search_response(&value);
    assert!(titles.is_empty());
}

#[test]
fn test_parse_search_response_missing_search_field() {
    let value = json!({ "query": { "pages": {} } });
    let titles = parse_search_response(&value);
    assert!(titles.is_empty());
}

#[test]
fn test_parse_search_response_not_an_object() {
    let value = json!([1, 2, 3]);
    let titles = parse_search_response(&value);
    assert!(titles.is_empty());
}

// ---------------------------------------------------------------------------
// build_summary_url
// ---------------------------------------------------------------------------

#[test]
fn test_build_summary_url_simple_title() {
    let url = build_summary_url("Photosynthesis");
    assert_eq!(
        url,
        "https://en.wikipedia.org/api/rest_v1/page/summary/Photosynthesis"
    );
}

#[test]
fn test_build_summary_url_title_with_spaces() {
    let url = build_summary_url("Rust (programming language)");
    assert_eq!(
        url,
        "https://en.wikipedia.org/api/rest_v1/page/summary/Rust%20%28programming%20language%29"
    );
}

#[test]
fn test_build_summary_url_title_with_special_chars() {
    let url = build_summary_url("Café & résumé");
    // Special characters should be percent-encoded
    assert!(url.starts_with("https://en.wikipedia.org/api/rest_v1/page/summary/"));
    assert!(url.contains("Caf"));
    assert!(url.contains("%20"));
}

// ---------------------------------------------------------------------------
// parse_summary_response
// ---------------------------------------------------------------------------

#[test]
fn test_parse_summary_response_full() {
    let value = json!({
        "title": "Rust (programming language)",
        "extract": "Rust is a general-purpose programming language designed for performance and safety.",
        "description": "General-purpose programming language",
        "content_urls": {
            "desktop": {
                "page": "https://en.wikipedia.org/wiki/Rust_(programming_language)"
            }
        },
        "thumbnail": {
            "source": "https://upload.wikimedia.org/wikipedia/commons/thumb/d/d5/Rust.png"
        }
    });
    let result = parse_summary_response(&value).expect("should parse");
    assert_eq!(result.source, ENGINE_NAME);
    assert_eq!(result.title, "Rust (programming language)");
    assert_eq!(
        result.url,
        "https://en.wikipedia.org/wiki/Rust_(programming_language)"
    );
    // Snippet should contain description and extract
    assert!(
        result
            .snippet
            .contains("General-purpose programming language")
    );
    assert!(result.snippet.contains("Rust is a general-purpose"));
    // Snippet should contain thumbnail URL
    assert!(result.snippet.contains("thumbnail"));
    assert!(result.snippet.contains("Rust.png"));
}

#[test]
fn test_parse_summary_response_minimal() {
    let value = json!({
        "title": "Test Page",
        "extract": "A short extract.",
        "content_urls": {
            "desktop": {
                "page": "https://en.wikipedia.org/wiki/Test_Page"
            }
        }
    });
    let result = parse_summary_response(&value).expect("should parse");
    assert_eq!(result.source, ENGINE_NAME);
    assert_eq!(result.title, "Test Page");
    assert_eq!(result.url, "https://en.wikipedia.org/wiki/Test_Page");
    // No description, so snippet is just the extract
    assert_eq!(result.snippet, "A short extract.");
}

#[test]
fn test_parse_summary_response_fallback_url_from_title() {
    let value = json!({
        "title": "Some Title",
        "extract": "Some extract text."
    });
    let result = parse_summary_response(&value).expect("should parse");
    assert_eq!(result.title, "Some Title");
    // URL should fall back to constructed article URL
    assert!(result.url.contains("en.wikipedia.org/wiki/"));
    assert!(result.url.contains("Some%20Title"));
}

#[test]
fn test_parse_summary_response_returns_none_when_empty() {
    let value = json!({});
    let result = parse_summary_response(&value);
    assert!(result.is_none());
}

#[test]
fn test_parse_summary_response_returns_none_when_title_and_extract_empty() {
    let value = json!({
        "title": "",
        "extract": ""
    });
    let result = parse_summary_response(&value);
    assert!(result.is_none());
}

#[test]
fn test_parse_summary_response_with_title_but_no_extract() {
    let value = json!({
        "title": "Only Title",
        "content_urls": {
            "desktop": { "page": "https://en.wikipedia.org/wiki/Only_Title" }
        }
    });
    let result = parse_summary_response(&value).expect("should parse");
    assert_eq!(result.title, "Only Title");
    assert_eq!(result.url, "https://en.wikipedia.org/wiki/Only_Title");
    // Snippet is empty (no description, no extract)
    assert!(result.snippet.is_empty());
}

#[test]
fn test_parse_summary_response_extract_only_no_title() {
    let value = json!({
        "extract": "An extract without a title.",
        "content_urls": {
            "desktop": { "page": "https://en.wikipedia.org/wiki/Some_Page" }
        }
    });
    let result = parse_summary_response(&value).expect("should parse");
    assert_eq!(result.title, "");
    assert_eq!(result.snippet, "An extract without a title.");
}

#[test]
fn test_parse_summary_response_truncates_long_extract() {
    let long_extract = "A".repeat(500);
    let value = json!({
        "title": "Long Page",
        "extract": long_extract,
        "content_urls": {
            "desktop": { "page": "https://en.wikipedia.org/wiki/Long_Page" }
        }
    });
    let result = parse_summary_response(&value).expect("should parse");
    // Snippet should be truncated to ~300 chars + ellipsis
    assert!(result.snippet.chars().count() <= 310);
    assert!(result.snippet.ends_with('…'));
}

// ---------------------------------------------------------------------------
// truncate_query
// ---------------------------------------------------------------------------

#[test]
fn test_truncate_query_short_unchanged() {
    assert_eq!(truncate_query("hello"), "hello");
}

#[test]
fn test_truncate_query_trims_whitespace() {
    assert_eq!(truncate_query("  hello world  "), "hello world");
}

#[test]
fn test_truncate_query_long_truncated() {
    let long = "a".repeat(MAX_QUERY_CHARS + 50);
    let result = truncate_query(&long);
    assert!(result.chars().count() <= MAX_QUERY_CHARS);
}

#[test]
fn test_truncate_query_exactly_at_limit() {
    let exact = "b".repeat(MAX_QUERY_CHARS);
    let result = truncate_query(&exact);
    assert_eq!(result.chars().count(), MAX_QUERY_CHARS);
}

#[test]
fn test_truncate_query_unicode_boundaries() {
    // Unicode characters should not be split mid-codepoint
    let query = "é".repeat(MAX_QUERY_CHARS);
    let result = truncate_query(&query);
    assert!(result.chars().count() <= MAX_QUERY_CHARS);
    // Each char should be a valid é
    for c in result.chars() {
        assert_eq!(c, 'é');
    }
}
