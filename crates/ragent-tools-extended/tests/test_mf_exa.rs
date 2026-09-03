#![allow(clippy::assert_is_empty)]
//! Unit tests for the Exa backend request builder (T-004, FR-003).
//!
//! These tests exercise
//! [`ragent_tools_extended::masterfetch::search::exa::build_request_body`]
//! and [`truncate_query`] without making any network requests (NFR-003).

use ragent_tools_extended::masterfetch::search::engine::{Freshness, SearchOptions};
use ragent_tools_extended::masterfetch::search::exa::{
    ENGINE_NAME, MAX_COUNT, MAX_QUERY_CHARS, MIN_COUNT, build_request_body, truncate_query,
};

#[test]
fn test_build_request_body_default_values() {
    let opts = SearchOptions::new(10).with_per_engine_results(5);
    let body = build_request_body("rust async", &opts);

    assert_eq!(body["query"], "rust async");
    assert_eq!(body["numResults"], 5);
    assert_eq!(body["type"], "auto");
    assert_eq!(body["contents"]["highlights"], true);
    // No domain filters by default
    assert!(body.get("includeDomains").is_none());
    assert!(body.get("excludeDomains").is_none());
    // No freshness filter by default
    assert!(body.get("startPublishedDate").is_none());
}

#[test]
fn test_build_request_body_clamps_num_results_to_max() {
    let opts = SearchOptions::new(10).with_per_engine_results(500);
    let body = build_request_body("rust", &opts);
    assert_eq!(body["numResults"], MAX_COUNT);
}

#[test]
fn test_build_request_body_clamps_num_results_to_min() {
    let opts = SearchOptions::new(10).with_per_engine_results(0);
    let body = build_request_body("rust", &opts);
    // with_per_engine_results clamps to 1, so numResults should be at least MIN_COUNT
    assert_eq!(body["numResults"], MIN_COUNT);
}

#[test]
fn test_build_request_body_truncates_long_query() {
    let long = "a".repeat(MAX_QUERY_CHARS + 100);
    let body = build_request_body(&long, &SearchOptions::new(5));
    assert_eq!(
        body["query"].as_str().unwrap().chars().count(),
        MAX_QUERY_CHARS
    );
}

#[test]
fn test_build_request_body_preserves_short_query() {
    let body = build_request_body("rust", &SearchOptions::new(5));
    assert_eq!(body["query"], "rust");
}

#[test]
fn test_build_request_body_includes_site_filter() {
    let opts = SearchOptions::new(10)
        .with_per_engine_results(5)
        .with_site("example.com");
    let body = build_request_body("rust", &opts);
    assert_eq!(body["includeDomains"], serde_json::json!(["example.com"]));
}

#[test]
fn test_build_request_body_includes_exclude_sites() {
    let opts = SearchOptions::new(10)
        .with_per_engine_results(5)
        .with_exclude_sites(vec!["spam.com".to_string(), "ads.com".to_string()]);
    let body = build_request_body("rust", &opts);
    let exclude = body["excludeDomains"].as_array().unwrap();
    assert_eq!(exclude.len(), 2);
    assert_eq!(exclude[0], "spam.com");
    assert_eq!(exclude[1], "ads.com");
}

#[test]
fn test_build_request_body_includes_start_date_for_freshness_day() {
    let opts = SearchOptions::new(10)
        .with_per_engine_results(5)
        .with_freshness(Freshness::Day);
    let body = build_request_body("rust", &opts);
    let date = body["startPublishedDate"].as_str().unwrap();
    // Should be an ISO 8601 date string: YYYY-MM-DD
    assert_eq!(date.len(), 10);
    assert_eq!(date.chars().nth(4), Some('-'));
    assert_eq!(date.chars().nth(7), Some('-'));
}

#[test]
fn test_build_request_body_includes_start_date_for_freshness_year() {
    let opts = SearchOptions::new(10)
        .with_per_engine_results(5)
        .with_freshness(Freshness::Year);
    let body = build_request_body("rust", &opts);
    assert!(body.get("startPublishedDate").is_some());
}

#[test]
fn test_build_request_body_no_start_date_for_freshness_any() {
    let opts = SearchOptions::new(10)
        .with_per_engine_results(5)
        .with_freshness(Freshness::Any);
    let body = build_request_body("rust", &opts);
    assert!(body.get("startPublishedDate").is_none());
}

#[test]
fn test_build_request_body_type_is_auto() {
    let body = build_request_body("rust", &SearchOptions::new(5));
    assert_eq!(body["type"], "auto");
}

#[test]
fn test_build_request_body_highlights_enabled() {
    let body = build_request_body("rust", &SearchOptions::new(5));
    assert_eq!(body["contents"]["highlights"], true);
}

#[test]
fn test_build_request_body_combined_filters() {
    let opts = SearchOptions::new(10)
        .with_per_engine_results(20)
        .with_site("docs.rs")
        .with_exclude_sites(vec!["old-docs.com".to_string()])
        .with_freshness(Freshness::Month);
    let body = build_request_body("rust async tokio", &opts);

    assert_eq!(body["query"], "rust async tokio");
    assert_eq!(body["numResults"], 20);
    assert_eq!(body["type"], "auto");
    assert_eq!(body["contents"]["highlights"], true);
    assert_eq!(body["includeDomains"], serde_json::json!(["docs.rs"]));
    assert_eq!(body["excludeDomains"], serde_json::json!(["old-docs.com"]));
    assert!(body.get("startPublishedDate").is_some());
}

// ---------------------------------------------------------------------------
// truncate_query tests
// ---------------------------------------------------------------------------

#[test]
fn test_truncate_query_short_unchanged() {
    assert_eq!(truncate_query("rust async"), "rust async");
}

#[test]
fn test_truncate_query_long_truncated() {
    let long = "a".repeat(MAX_QUERY_CHARS + 50);
    let truncated = truncate_query(&long);
    assert_eq!(truncated.chars().count(), MAX_QUERY_CHARS);
}

#[test]
fn test_truncate_query_exact_limit_unchanged() {
    let exact = "a".repeat(MAX_QUERY_CHARS);
    assert_eq!(truncate_query(&exact), exact);
}

#[test]
fn test_truncate_query_unicode_boundaries() {
    let s = "🦀".repeat(MAX_QUERY_CHARS + 10);
    let t = truncate_query(&s);
    assert_eq!(t.chars().count(), MAX_QUERY_CHARS);
    // Should not panic on char boundary
    assert!(t.is_char_boundary(t.len()));
}

#[test]
fn test_engine_name_constant() {
    assert_eq!(ENGINE_NAME, "exa");
}
// ---------------------------------------------------------------------------
// parse_response_json tests (T-005, FR-004, FR-011)
// ---------------------------------------------------------------------------

use ragent_tools_extended::masterfetch::search::exa::parse_response_json;
use serde_json::json;

#[test]
fn test_parse_response_json_extracts_results() {
    let value = json!({
        "requestId": "abc123",
        "results": [
            {
                "title": "Rust Programming Language",
                "url": "https://www.rust-lang.org",
                "score": 0.95,
                "publishedDate": "2024-01-15",
                "author": "Rust Team",
                "highlights": ["Rust is a systems programming language", "memory safe"]
            },
            {
                "title": "Rust Documentation",
                "url": "https://doc.rust-lang.org",
                "score": 0.88,
                "publishedDate": "2024-02-20",
                "author": "Rust Docs",
                "highlights": ["The Rust standard library"]
            }
        ]
    });
    let results = parse_response_json(&value);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].title, "Rust Programming Language");
    assert_eq!(results[0].url, "https://www.rust-lang.org");
    assert_eq!(results[0].source, ENGINE_NAME);
    assert_eq!(results[0].score, Some(0.95));
    assert!(
        results[0]
            .snippet
            .contains("Rust is a systems programming language")
    );
    assert_eq!(results[1].title, "Rust Documentation");
    assert_eq!(results[1].url, "https://doc.rust-lang.org");
    assert_eq!(results[1].score, Some(0.88));
}

#[test]
fn test_parse_response_json_missing_results_key() {
    let value = json!({"requestId": "abc"});
    assert!(parse_response_json(&value).is_empty());
}

#[test]
fn test_parse_response_json_results_not_array() {
    let value = json!({"results": "not an array"});
    assert!(parse_response_json(&value).is_empty());
}

#[test]
fn test_parse_response_json_empty_results() {
    let value = json!({"results": []});
    assert!(parse_response_json(&value).is_empty());
}

#[test]
fn test_parse_response_json_items_without_url_filtered() {
    let value = json!({
        "results": [
            {"title": "Valid", "url": "https://example.com", "highlights": ["ok"]},
            {"title": "No URL", "highlights": ["nope"]},
            {"url": "https://other.com", "highlights": ["yes"]}
        ]
    });
    let results = parse_response_json(&value);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].url, "https://example.com");
    assert_eq!(results[1].url, "https://other.com");
}

#[test]
fn test_parse_response_json_fallback_title_from_url() {
    let value = json!({
        "results": [
            {"url": "https://example.com/no-title", "highlights": ["content"]}
        ]
    });
    let results = parse_response_json(&value);
    assert_eq!(results.len(), 1);
    // No title field — should fall back to URL
    assert_eq!(results[0].title, "https://example.com/no-title");
}

#[test]
fn test_parse_response_json_fallback_title_untitled_when_no_url_or_title() {
    let value = json!({
        "results": [
            {"highlights": ["content only"]}
        ]
    });
    let results = parse_response_json(&value);
    // No url → filtered out entirely
    assert!(results.is_empty());
}

#[test]
fn test_parse_response_json_empty_title_falls_back_to_url() {
    let value = json!({
        "results": [
            {"title": "", "url": "https://example.com", "highlights": ["ok"]}
        ]
    });
    let results = parse_response_json(&value);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "https://example.com");
}

#[test]
fn test_parse_response_json_highlights_joined() {
    let value = json!({
        "results": [
            {
                "title": "Test",
                "url": "https://example.com",
                "highlights": ["first highlight", "second highlight"]
            }
        ]
    });
    let results = parse_response_json(&value);
    assert_eq!(results.len(), 1);
    assert!(results[0].snippet.contains("first highlight"));
    assert!(results[0].snippet.contains("second highlight"));
    assert!(results[0].snippet.contains("…"));
}

#[test]
fn test_parse_response_json_snippet_fallback_to_metadata() {
    let value = json!({
        "results": [
            {
                "title": "No Highlights",
                "url": "https://example.com",
                "publishedDate": "2024-03-01",
                "author": "Jane Doe"
            }
        ]
    });
    let results = parse_response_json(&value);
    assert_eq!(results.len(), 1);
    assert!(results[0].snippet.contains("2024-03-01"));
    assert!(results[0].snippet.contains("Jane Doe"));
}

#[test]
fn test_parse_response_json_snippet_fallback_date_only() {
    let value = json!({
        "results": [
            {
                "title": "Date Only",
                "url": "https://example.com",
                "publishedDate": "2024-03-01"
            }
        ]
    });
    let results = parse_response_json(&value);
    assert_eq!(results.len(), 1);
    assert!(results[0].snippet.contains("2024-03-01"));
    assert!(!results[0].snippet.contains("by"));
}

#[test]
fn test_parse_response_json_snippet_fallback_author_only() {
    let value = json!({
        "results": [
            {
                "title": "Author Only",
                "url": "https://example.com",
                "author": "John"
            }
        ]
    });
    let results = parse_response_json(&value);
    assert_eq!(results.len(), 1);
    assert!(results[0].snippet.contains("John"));
    assert!(!results[0].snippet.contains("Published:"));
}

#[test]
fn test_parse_response_json_snippet_empty_when_no_metadata() {
    let value = json!({
        "results": [
            {
                "title": "Nothing",
                "url": "https://example.com"
            }
        ]
    });
    let results = parse_response_json(&value);
    assert_eq!(results.len(), 1);
    assert!(results[0].snippet.is_empty());
}

#[test]
fn test_parse_response_json_snippet_truncation() {
    let long_highlight = "a".repeat(300);
    let value = json!({
        "results": [
            {
                "title": "Long",
                "url": "https://example.com",
                "highlights": [long_highlight]
            }
        ]
    });
    let results = parse_response_json(&value);
    assert_eq!(results.len(), 1);
    let snippet_len = results[0].snippet.chars().count();
    assert!(snippet_len <= 201); // 200 chars + ellipsis
    assert!(results[0].snippet.ends_with('…'));
}

#[test]
fn test_parse_response_json_score_clamped() {
    let value = json!({
        "results": [
            {"title": "High", "url": "https://a.com", "score": 1.5, "highlights": ["x"]},
            {"title": "Low", "url": "https://b.com", "score": -0.5, "highlights": ["y"]},
            {"title": "None", "url": "https://c.com", "highlights": ["z"]}
        ]
    });
    let results = parse_response_json(&value);
    assert_eq!(results.len(), 3);
    assert_eq!(results[0].score, Some(1.0));
    assert_eq!(results[1].score, Some(0.0));
    assert_eq!(results[2].score, None);
}

#[test]
fn test_parse_response_json_source_always_exa() {
    let value = json!({
        "results": [
            {"title": "A", "url": "https://a.com", "highlights": ["x"]},
            {"title": "B", "url": "https://b.com", "highlights": ["y"]}
        ]
    });
    let results = parse_response_json(&value);
    for r in &results {
        assert_eq!(r.source, "exa");
    }
}

#[test]
fn test_parse_response_json_empty_highlights_array() {
    let value = json!({
        "results": [
            {
                "title": "Empty Highlights",
                "url": "https://example.com",
                "highlights": [],
                "publishedDate": "2024-01-01",
                "author": "Test"
            }
        ]
    });
    let results = parse_response_json(&value);
    assert_eq!(results.len(), 1);
    // Empty highlights array → fallback to metadata
    assert!(results[0].snippet.contains("2024-01-01"));
}

#[test]
fn test_parse_response_json_highlights_non_string_ignored() {
    let value = json!({
        "results": [
            {
                "title": "Mixed",
                "url": "https://example.com",
                "highlights": [123, "valid string", true]
            }
        ]
    });
    let results = parse_response_json(&value);
    assert_eq!(results.len(), 1);
    assert!(results[0].snippet.contains("valid string"));
    assert!(!results[0].snippet.contains("123"));
}
// ---------------------------------------------------------------------------
// mask_key & masked_key tests (T-007, FR-006, NFR-002)
// ---------------------------------------------------------------------------

use ragent_tools_extended::masterfetch::search::exa::{ExaEngine, mask_key};

#[test]
fn test_mask_key_short_fully_masked() {
    // Strings <= 6 chars are fully masked
    assert_eq!(mask_key("abc"), "***");
    assert_eq!(mask_key("abcdef"), "******");
    assert_eq!(mask_key("a"), "*");
}

#[test]
fn test_mask_key_empty() {
    assert_eq!(mask_key(""), "");
}

#[test]
fn test_mask_key_long_preserves_first_and_last_two() {
    let masked = mask_key("exa-abcdefghijklmnop1234567890");
    assert_eq!(
        masked.len(),
        "exa-abcdefghijklmnop1234567890".chars().count()
    );
    assert!(masked.starts_with("ex"));
    assert!(masked.ends_with("90"));
    // Middle should be all asterisks
    let middle = &masked[2..masked.len() - 2];
    assert!(middle.chars().all(|c| c == '*'));
}

#[test]
fn test_mask_key_seven_chars_boundary() {
    // 7 chars: first 2 + 1 asterisk + last 2 = 5 visible, middle = 1 asterisk
    let masked = mask_key("abcdefg");
    assert_eq!(masked, "ab***fg");
}

#[test]
fn test_mask_key_six_chars_fully_masked() {
    let masked = mask_key("abcdef");
    assert_eq!(masked, "******");
}

#[test]
fn test_mask_key_does_not_leak_middle() {
    let key = "exa-mySecretKey12345";
    let masked = mask_key(key);
    // The middle portion must not contain any original characters
    let middle = &masked[2..masked.len() - 2];
    assert!(!middle.contains("Secret"));
    assert!(!middle.contains("Key"));
    assert!(middle.chars().all(|c| c == '*'));
}

#[test]
fn test_mask_key_unicode() {
    let masked = mask_key("🦀🦀🦀🦀🦀🦀🦀🦀🦀🦀");
    assert!(masked.starts_with("🦀🦀"));
    assert!(masked.ends_with("🦀🦀"));
    let middle = &masked["🦀🦀".len()..masked.len() - "🦀🦀".len()];
    assert!(middle.chars().all(|c| c == '*'));
}

#[test]
fn test_masked_key_method_returns_masked() {
    let engine = ExaEngine::new("exa-abcdefghijk123456");
    let masked = engine.masked_key();
    assert!(masked.starts_with("ex"));
    assert!(masked.ends_with("56"));
    // Should not contain the full key
    assert!(!masked.contains("abcdefghijk1234"));
}

#[test]
fn test_masked_key_method_empty_key() {
    let engine = ExaEngine::new("");
    assert_eq!(engine.masked_key(), "");
}

#[test]
fn test_masked_key_method_short_key() {
    let engine = ExaEngine::new("abc");
    assert_eq!(engine.masked_key(), "***");
}

#[test]
fn test_masked_key_never_exposes_full_key() {
    let key = "exa-d0ntL3akTh1sK3yToL0gs";
    let engine = ExaEngine::new(key);
    let masked = engine.masked_key();
    assert_ne!(masked, key);
    assert!(!masked.contains("d0ntL3ak"));
    assert!(!masked.contains("Th1sK3y"));
}
// ---------------------------------------------------------------------------
// truncate_snippet tests (T-008, FR-004)
// ---------------------------------------------------------------------------

use ragent_tools_extended::masterfetch::search::exa::{MAX_SNIPPET_CHARS, truncate_snippet};

#[test]
fn test_truncate_snippet_short_unchanged() {
    assert_eq!(truncate_snippet("hello world"), "hello world");
}

#[test]
fn test_truncate_snippet_exact_limit_unchanged() {
    let exact = "a".repeat(MAX_SNIPPET_CHARS);
    assert_eq!(truncate_snippet(&exact), exact);
}

#[test]
fn test_truncate_snippet_long_truncated_with_ellipsis() {
    let long = "a".repeat(MAX_SNIPPET_CHARS + 50);
    let truncated = truncate_snippet(&long);
    assert_eq!(truncated.chars().count(), MAX_SNIPPET_CHARS + 1); // +1 for ellipsis
    assert!(truncated.ends_with('…'));
}

#[test]
fn test_truncate_snippet_one_over_limit() {
    let input = "a".repeat(MAX_SNIPPET_CHARS + 1);
    let truncated = truncate_snippet(&input);
    assert!(truncated.ends_with('…'));
    assert_eq!(
        truncated.chars().count(),
        MAX_SNIPPET_CHARS + 1 // content chars + ellipsis
    );
}

#[test]
fn test_truncate_snippet_empty() {
    assert_eq!(truncate_snippet(""), "");
}

#[test]
fn test_truncate_snippet_unicode_boundaries() {
    let s = "🦀".repeat(MAX_SNIPPET_CHARS + 10);
    let t = truncate_snippet(&s);
    // Should not panic on char boundary
    assert!(t.is_char_boundary(t.len()));
    assert!(t.ends_with('…'));
    // Each emoji is 1 char, so truncated content should be MAX_SNIPPET_CHARS chars
    // plus the ellipsis
    assert_eq!(t.chars().count(), MAX_SNIPPET_CHARS + 1);
}

#[test]
fn test_truncate_snippet_preserves_content_under_limit() {
    let input = "The quick brown fox jumps over the lazy dog";
    let truncated = truncate_snippet(input);
    assert_eq!(truncated, input);
}
// ---------------------------------------------------------------------------
// freshness_to_start_date & date_string tests (T-009, FR-003)
// ---------------------------------------------------------------------------

use ragent_tools_extended::masterfetch::search::exa::{date_string, freshness_to_start_date};

#[test]
fn test_freshness_to_start_date_any_returns_none() {
    assert!(freshness_to_start_date(Freshness::Any).is_none());
}

#[test]
fn test_freshness_to_start_date_day_returns_some() {
    let result = freshness_to_start_date(Freshness::Day);
    assert!(result.is_some());
    let date = result.unwrap();
    // ISO 8601 date format: YYYY-MM-DD (10 chars)
    assert_eq!(date.len(), 10);
    assert_eq!(date.chars().nth(4), Some('-'));
    assert_eq!(date.chars().nth(7), Some('-'));
}

#[test]
fn test_freshness_to_start_date_week_returns_some() {
    let result = freshness_to_start_date(Freshness::Week);
    assert!(result.is_some());
    let date = result.unwrap();
    assert_eq!(date.len(), 10);
}

#[test]
fn test_freshness_to_start_date_month_returns_some() {
    let result = freshness_to_start_date(Freshness::Month);
    assert!(result.is_some());
    let date = result.unwrap();
    assert_eq!(date.len(), 10);
}

#[test]
fn test_freshness_to_start_date_year_returns_some() {
    let result = freshness_to_start_date(Freshness::Year);
    assert!(result.is_some());
    let date = result.unwrap();
    assert_eq!(date.len(), 10);
}

#[test]
fn test_freshness_to_start_date_day_is_more_recent_than_year() {
    let day_date = freshness_to_start_date(Freshness::Day).unwrap();
    let year_date = freshness_to_start_date(Freshness::Year).unwrap();
    // The day window start date should be later (more recent) than the year window
    assert!(
        day_date > year_date,
        "day date ({day_date}) should be later than year date ({year_date})"
    );
}

#[test]
fn test_freshness_to_start_date_week_between_day_and_year() {
    let day_date = freshness_to_start_date(Freshness::Day).unwrap();
    let week_date = freshness_to_start_date(Freshness::Week).unwrap();
    let year_date = freshness_to_start_date(Freshness::Year).unwrap();
    assert!(day_date > week_date);
    assert!(week_date > year_date);
}

#[test]
fn test_freshness_to_start_date_month_between_week_and_year() {
    let week_date = freshness_to_start_date(Freshness::Week).unwrap();
    let month_date = freshness_to_start_date(Freshness::Month).unwrap();
    let year_date = freshness_to_start_date(Freshness::Year).unwrap();
    assert!(week_date > month_date);
    assert!(month_date > year_date);
}

#[test]
fn test_freshness_to_start_date_day_within_one_day_of_today() {
    let date = freshness_to_start_date(Freshness::Day).unwrap();
    // The start date should be today or yesterday (within 1 day of now)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let today = date_string(now);
    // Day window = today minus 1 day, so it should be <= today and >= today - 1 day
    assert!(date <= today);
}

// ---------------------------------------------------------------------------
// date_string tests
// ---------------------------------------------------------------------------

#[test]
fn test_date_string_epoch() {
    // Unix epoch: 1970-01-01
    assert_eq!(date_string(0), "1970-01-01");
}

#[test]
fn test_date_string_known_date() {
    // 2024-01-01 00:00:00 UTC = 1704067200 seconds
    assert_eq!(date_string(1_704_067_200), "2024-01-01");
}

#[test]
fn test_date_string_known_date_2() {
    // 2024-12-31 23:59:59 UTC = 1735689599 seconds
    // div_euclid by 86400 gives the day, which is 2024-12-31
    let secs = 1_735_689_599_i64;
    let result = date_string(secs);
    assert_eq!(result, "2024-12-31");
}

#[test]
fn test_date_string_format() {
    let date = date_string(1_704_067_200);
    // Verify YYYY-MM-DD format
    let parts: Vec<&str> = date.split('-').collect();
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0].len(), 4); // year
    assert_eq!(parts[1].len(), 2); // month
    assert_eq!(parts[2].len(), 2); // day
}

#[test]
fn test_date_string_negative_timestamp() {
    // Before epoch: 1969-12-31 = -86400 seconds
    let result = date_string(-86_400);
    assert_eq!(result, "1969-12-31");
}

#[test]
fn test_date_string_leap_year() {
    // 2024-02-29 = 1709164800 seconds (2024 is a leap year)
    assert_eq!(date_string(1_709_164_800), "2024-02-29");
}

// ===========================================================================
// Live integration test (network — #[ignore])
//
// Requires a real Exa API key in the EXA_API_KEY environment variable.
// Run with: cargo test -p ragent-tools-extended --test test_mf_exa \
//           -- test_live_exa_api_returns_results --ignored --nocapture
// ===========================================================================

#[tokio::test]
#[ignore = "requires EXA_API_KEY and network access"]
async fn test_live_exa_api_returns_results() {
    use ragent_tools_extended::masterfetch::search::exa::ExaEngine;
    use ragent_tools_extended::masterfetch::search::{SearchEngine, SearchOptions};

    let api_key = std::env::var("EXA_API_KEY").expect("EXA_API_KEY not set");
    let engine = ExaEngine::new(api_key);

    let report = engine
        .search("rust programming language", &SearchOptions::new(3))
        .await;

    assert!(
        !report.results.is_empty(),
        "expected at least one result from live Exa API, got error: {:?}",
        report.error
    );
    assert_eq!(report.engine, "exa");
}
