#![allow(clippy::assert_is_empty)]
//! Unit tests for `masterfetch::search::brave` — Brave keyless search backend
//! (T-014, FR-008, NFR-003).
//!
//! The HTML parsing and URL-construction functions are pure and tested
//! thoroughly here. The full `search()` method requires network I/O and is
//! tested with `#[ignore]`-gated integration tests.

use ragent_tools_extended::masterfetch::search::brave::{
    build_search_params, build_search_url, parse_results_html,
};
use ragent_tools_extended::masterfetch::search::engine::{
    EngineReport, Freshness, SearchEngine, SearchOptions,
};

// ===========================================================================
// parse_results_html: basic parsing (Pattern 1 — result-header)
// ===========================================================================

#[test]
fn test_parse_single_result_pattern1() {
    let html = r#"<div class="snippet fdb">
  <a class="result-header" href="https://example.com/page">
    <span class="snippet-title">Example Title</span>
  </a>
  <div class="snippet-description"><p>A snippet.</p></div>
</div>"#;
    let results = parse_results_html(html, "brave");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Example Title");
    assert_eq!(results[0].url, "https://example.com/page");
    assert_eq!(results[0].snippet, "A snippet.");
    assert_eq!(results[0].source, "brave");
}

#[test]
fn test_parse_multiple_results_pattern1() {
    let html = r#"
<div class="snippet fdb">
  <a class="result-header" href="https://a.com"><span class="snippet-title">First</span></a>
  <div class="snippet-description"><p>First snippet.</p></div>
</div>
<div class="snippet fdb">
  <a class="result-header" href="https://b.com"><span class="snippet-title">Second</span></a>
  <div class="snippet-description"><p>Second snippet.</p></div>
</div>
<div class="snippet fdb">
  <a class="result-header" href="https://c.com"><span class="snippet-title">Third</span></a>
  <div class="snippet-description"><p>Third snippet.</p></div>
</div>
"#;
    let results = parse_results_html(html, "brave");
    assert_eq!(results.len(), 3);
    assert_eq!(results[0].title, "First");
    assert_eq!(results[1].title, "Second");
    assert_eq!(results[2].title, "Third");
}

// ===========================================================================
// parse_results_html: Pattern 2 — title-class link
// ===========================================================================

#[test]
fn test_parse_single_result_pattern2() {
    let html = r#"<div class="search-result" data-type="web">
  <a class="title" href="https://example.com/page">Example Title</a>
  <p class="snippet-description">A snippet.</p>
</div>"#;
    let results = parse_results_html(html, "brave");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Example Title");
    assert_eq!(results[0].url, "https://example.com/page");
}

#[test]
fn test_parse_multiple_results_pattern2() {
    let html = r#"
<div class="search-result">
  <a class="item-title" href="https://a.com">Alpha</a>
  <p class="snippet-description">A snippet.</p>
</div>
<div class="search-result">
  <a class="result-title" href="https://b.com">Beta</a>
  <p class="snippet-description">B snippet.</p>
</div>
"#;
    let results = parse_results_html(html, "brave");
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].title, "Alpha");
    assert_eq!(results[1].title, "Beta");
}

// ===========================================================================
// parse_results_html: edge cases
// ===========================================================================

#[test]
fn test_parse_empty_html() {
    let results = parse_results_html("", "brave");
    assert!(results.is_empty());
}

#[test]
fn test_parse_html_with_no_results() {
    let html = r"<html><body><p>No results found.</p></body></html>";
    let results = parse_results_html(html, "brave");
    assert!(results.is_empty());
}

#[test]
fn test_parse_preserves_order() {
    let html = r#"
<div class="snippet fdb">
  <a class="result-header" href="https://a.com"><span class="snippet-title">Alpha</span></a>
  <div class="snippet-description"><p>A snippet.</p></div>
</div>
<div class="snippet fdb">
  <a class="result-header" href="https://b.com"><span class="snippet-title">Beta</span></a>
  <div class="snippet-description"><p>B snippet.</p></div>
</div>
<div class="snippet fdb">
  <a class="result-header" href="https://c.com"><span class="snippet-title">Gamma</span></a>
  <div class="snippet-description"><p>C snippet.</p></div>
</div>
"#;
    let results = parse_results_html(html, "brave");
    assert_eq!(results.len(), 3);
    assert_eq!(results[0].title, "Alpha");
    assert_eq!(results[1].title, "Beta");
    assert_eq!(results[2].title, "Gamma");
}

#[test]
fn test_parse_result_without_snippet() {
    let html = r#"<div class="snippet fdb">
  <a class="result-header" href="https://example.com">
    <span class="snippet-title">Title Only</span>
  </a>
</div>"#;
    let results = parse_results_html(html, "brave");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Title Only");
    assert_eq!(results[0].snippet, "");
}

#[test]
fn test_parse_result_with_html_entities_in_title() {
    let html = r#"<div class="snippet fdb">
  <a class="result-header" href="https://example.com">
    <span class="snippet-title">Tom &amp; Jerry Show</span>
  </a>
  <div class="snippet-description"><p>A &quot;great&quot; show.</p></div>
</div>"#;
    let results = parse_results_html(html, "brave");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Tom & Jerry Show");
    assert_eq!(results[0].snippet, "A \"great\" show.");
}

#[test]
fn test_parse_result_with_nested_tags_in_title() {
    let html = r#"<div class="snippet fdb">
  <a class="result-header" href="https://example.com">
    <span class="snippet-title"><b>Bold</b> Title</span>
  </a>
  <div class="snippet-description"><p>Snippet.</p></div>
</div>"#;
    let results = parse_results_html(html, "brave");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Bold Title");
}

#[test]
fn test_parse_result_with_empty_title_skipped() {
    let html = r#"<div class="snippet fdb">
  <a class="result-header" href="https://example.com">
    <span class="snippet-title"></span>
  </a>
  <div class="snippet-description"><p>Snippet.</p></div>
</div>
<div class="snippet fdb">
  <a class="result-header" href="https://other.com">
    <span class="snippet-title">Real Title</span>
  </a>
  <div class="snippet-description"><p>Other snippet.</p></div>
</div>"#;
    let results = parse_results_html(html, "brave");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Real Title");
}

#[test]
fn test_parse_malformed_html_does_not_panic() {
    let html = r##"<div class="snippet fdb"><a class="result-header" href="#">Unclosed</a></div>"##;
    let results = parse_results_html(html, "brave");
    let _ = results.len();
}

#[test]
fn test_parse_source_field_set() {
    let html = r#"<div class="snippet fdb">
  <a class="result-header" href="https://example.com">
    <span class="snippet-title">Title</span>
  </a>
</div>"#;
    let results = parse_results_html(html, "brv");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].source, "brv");
}

#[test]
fn test_parse_snippet_with_html_tags() {
    let html = r#"<div class="snippet fdb">
  <a class="result-header" href="https://example.com">
    <span class="snippet-title">Title</span>
  </a>
  <div class="snippet-description"><p>This is a <b>bold</b> snippet.</p></div>
</div>"#;
    let results = parse_results_html(html, "brave");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].snippet, "This is a bold snippet.");
}

#[test]
fn test_parse_large_html_page() {
    let mut html = String::new();
    for i in 0..50 {
        html.push_str(&format!(
            r#"<div class="snippet fdb">
  <a class="result-header" href="https://example.com/page{i}"><span class="snippet-title">Result {i}</span></a>
  <div class="snippet-description"><p>Snippet {i}.</p></div>
</div>
"#,
        ));
    }
    let results = parse_results_html(&html, "brave");
    assert_eq!(results.len(), 50);
    assert_eq!(results[0].title, "Result 0");
    assert_eq!(results[49].title, "Result 49");
}

#[test]
fn test_parse_protocol_relative_url() {
    let html = r#"<div class="snippet fdb">
  <a class="result-header" href="//example.com/page">
    <span class="snippet-title">Protocol Relative</span>
  </a>
</div>"#;
    let results = parse_results_html(html, "brave");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].url, "https://example.com/page");
}

#[test]
fn test_parse_pattern1_preferred_over_pattern2() {
    // When both patterns are present, Pattern 1 (result-header) should be
    // used.
    let html = r#"
<div class="snippet fdb">
  <a class="result-header" href="https://preferred.com">
    <span class="snippet-title">Preferred</span>
  </a>
</div>
<div class="search-result">
  <a class="title" href="https://fallback.com">Fallback</a>
</div>
"#;
    let results = parse_results_html(html, "brave");
    // Pattern 1 matches should be used.
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Preferred");
    assert_eq!(results[0].url, "https://preferred.com");
}

// ===========================================================================
// build_search_params: query construction
// ===========================================================================

#[test]
fn test_build_search_params_basic_query() {
    let opts = SearchOptions::default();
    let params = build_search_params("rust programming", &opts);
    let q = params.iter().find(|(k, _)| k == "q").expect("q field");
    assert_eq!(q.1, "rust programming");
}

#[test]
fn test_build_search_params_trims_query() {
    let opts = SearchOptions::default();
    let params = build_search_params("  rust  ", &opts);
    let q = params.iter().find(|(k, _)| k == "q").unwrap();
    assert_eq!(q.1, "rust");
}

#[test]
fn test_build_search_params_with_site_filter() {
    let opts = SearchOptions::default().with_site("github.com");
    let params = build_search_params("rust async", &opts);
    let q = params.iter().find(|(k, _)| k == "q").unwrap();
    assert!(q.1.contains("site:github.com"));
    assert!(q.1.contains("rust async"));
}

#[test]
fn test_build_search_params_with_exclude_sites() {
    let opts = SearchOptions::default()
        .with_exclude_sites(vec!["pinterest.com".to_string(), "quora.com".to_string()]);
    let params = build_search_params("rust", &opts);
    let q = params.iter().find(|(k, _)| k == "q").unwrap();
    assert!(q.1.contains("-site:pinterest.com"));
    assert!(q.1.contains("-site:quora.com"));
}

#[test]
fn test_build_search_params_freshness_day() {
    let opts = SearchOptions::default().with_freshness(Freshness::Day);
    let params = build_search_params("rust", &opts);
    let tf = params.iter().find(|(k, _)| k == "tf");
    assert_eq!(tf.unwrap().1, "pd");
}

#[test]
fn test_build_search_params_freshness_week() {
    let opts = SearchOptions::default().with_freshness(Freshness::Week);
    let params = build_search_params("rust", &opts);
    let tf = params.iter().find(|(k, _)| k == "tf");
    assert_eq!(tf.unwrap().1, "pw");
}

#[test]
fn test_build_search_params_freshness_month() {
    let opts = SearchOptions::default().with_freshness(Freshness::Month);
    let params = build_search_params("rust", &opts);
    let tf = params.iter().find(|(k, _)| k == "tf");
    assert_eq!(tf.unwrap().1, "pm");
}

#[test]
fn test_build_search_params_freshness_year() {
    let opts = SearchOptions::default().with_freshness(Freshness::Year);
    let params = build_search_params("rust", &opts);
    let tf = params.iter().find(|(k, _)| k == "tf");
    assert_eq!(tf.unwrap().1, "py");
}

#[test]
fn test_build_search_params_freshness_any_omits_tf() {
    let opts = SearchOptions::default();
    let params = build_search_params("rust", &opts);
    let tf = params.iter().find(|(k, _)| k == "tf");
    assert!(tf.is_none(), "tf should be omitted for Freshness::Any");
}

#[test]
fn test_build_search_params_page_zero_omits_offset() {
    let opts = SearchOptions::default().with_page(0);
    let params = build_search_params("rust", &opts);
    let offset = params.iter().find(|(k, _)| k == "offset");
    assert!(offset.is_none(), "offset should be omitted for page 0");
}

#[test]
fn test_build_search_params_page_one_has_offset_10() {
    let opts = SearchOptions::default().with_page(1);
    let params = build_search_params("rust", &opts);
    let offset = params.iter().find(|(k, _)| k == "offset").unwrap();
    assert_eq!(offset.1, "10");
}

#[test]
fn test_build_search_params_page_two_has_offset_20() {
    let opts = SearchOptions::default().with_page(2);
    let params = build_search_params("rust", &opts);
    let offset = params.iter().find(|(k, _)| k == "offset").unwrap();
    assert_eq!(offset.1, "20");
}

#[test]
fn test_build_search_params_combined_filters() {
    let opts = SearchOptions::new(15)
        .with_site("stackoverflow.com")
        .with_freshness(Freshness::Month)
        .with_page(1);
    let params = build_search_params("rust async tokio", &opts);

    let q = params.iter().find(|(k, _)| k == "q").unwrap();
    assert!(q.1.contains("rust async tokio"));
    assert!(q.1.contains("site:stackoverflow.com"));

    let tf = params.iter().find(|(k, _)| k == "tf").unwrap();
    assert_eq!(tf.1, "pm");

    let offset = params.iter().find(|(k, _)| k == "offset").unwrap();
    assert_eq!(offset.1, "10");
}

// ===========================================================================
// build_search_url: full URL construction
// ===========================================================================

#[test]
fn test_build_search_url_basic() {
    let opts = SearchOptions::default();
    let url = build_search_url("rust programming", &opts);
    assert!(url.starts_with("https://search.brave.com/search?"));
    assert!(url.contains("q=rust+programming"));
}

#[test]
fn test_build_search_url_with_freshness() {
    let opts = SearchOptions::default().with_freshness(Freshness::Week);
    let url = build_search_url("rust", &opts);
    assert!(url.contains("tf=pw"));
}

#[test]
fn test_build_search_url_with_offset() {
    let opts = SearchOptions::default().with_page(2);
    let url = build_search_url("rust", &opts);
    assert!(url.contains("offset=20"));
}

#[test]
fn test_build_search_url_no_tf_for_any() {
    let opts = SearchOptions::default();
    let url = build_search_url("rust", &opts);
    assert!(!url.contains("tf="));
}

#[test]
fn test_build_search_url_no_offset_for_page_zero() {
    let opts = SearchOptions::default().with_page(0);
    let url = build_search_url("rust", &opts);
    assert!(!url.contains("offset="));
}

// ===========================================================================
// BraveEngine: struct + trait
// ===========================================================================

#[test]
fn test_engine_name() {
    let engine = ragent_tools_extended::masterfetch::search::brave::BraveEngine::new();
    assert_eq!(engine.name(), "brave");
}

#[test]
fn test_engine_default() {
    let engine = ragent_tools_extended::masterfetch::search::brave::BraveEngine::default();
    assert_eq!(engine.name(), "brave");
}

#[tokio::test]
async fn test_engine_search_empty_query_returns_error_report() {
    let engine = ragent_tools_extended::masterfetch::search::brave::BraveEngine::new();
    let report = engine.search("", &SearchOptions::default()).await;
    assert!(!report.is_success());
    assert!(!report.engine_blocked);
    assert!(report.error.contains("empty"));
    assert_eq!(report.engine, "brave");
}

#[tokio::test]
async fn test_engine_search_whitespace_query_returns_error_report() {
    let engine = ragent_tools_extended::masterfetch::search::brave::BraveEngine::new();
    let report = engine.search("   ", &SearchOptions::default()).await;
    assert!(!report.is_success());
    assert!(report.error.contains("empty"));
}

// ===========================================================================
// SearchEngine trait object compatibility
// ===========================================================================

#[tokio::test]
async fn test_engine_as_trait_object() {
    use ragent_tools_extended::masterfetch::search::brave::BraveEngine;

    let engine: Box<dyn SearchEngine> = Box::new(BraveEngine::new());
    assert_eq!(engine.name(), "brave");

    let report = engine.search("", &SearchOptions::default()).await;
    assert_eq!(report.engine, "brave");
}

// ===========================================================================
// Integration tests (network — #[ignore])
// ===========================================================================

#[tokio::test]
#[ignore = "requires network access to search.brave.com"]
async fn test_live_search_returns_results() {
    use ragent_tools_extended::masterfetch::search::brave::BraveEngine;

    let engine = BraveEngine::new();
    let opts = SearchOptions::new(5);
    let report = engine.search("rust programming language", &opts).await;

    assert_eq!(report.engine, "brave");

    if !report.engine_blocked {
        assert!(
            report.has_results(),
            "expected results for 'rust programming language', got error: {}",
            report.error
        );
        for r in &report.results {
            assert!(!r.title.is_empty(), "result title should not be empty");
            assert!(!r.url.is_empty(), "result URL should not be empty");
            assert!(
                r.url.starts_with("http"),
                "URL should be absolute: {}",
                r.url
            );
        }
    }
}

#[tokio::test]
#[ignore = "requires network access to search.brave.com"]
async fn test_live_search_with_site_filter() {
    use ragent_tools_extended::masterfetch::search::brave::BraveEngine;

    let engine = BraveEngine::new();
    let opts = SearchOptions::new(5).with_site("github.com");
    let report = engine.search("rust", &opts).await;

    if !report.engine_blocked && report.has_results() {
        for r in &report.results {
            assert!(
                r.url.contains("github.com"),
                "site-filtered result should be from github.com: {}",
                r.url
            );
        }
    }
}

#[tokio::test]
#[ignore = "requires network access to search.brave.com"]
async fn test_live_search_respects_max_results() {
    use ragent_tools_extended::masterfetch::search::brave::BraveEngine;

    let engine = BraveEngine::new();
    let opts = SearchOptions::new(3);
    let report = engine.search("rust", &opts).await;

    if !report.engine_blocked {
        assert!(
            report.results.len() <= 3,
            "results should be capped at max_results=3, got {}",
            report.results.len()
        );
    }
}

#[tokio::test]
#[ignore = "requires network access to search.brave.com"]
async fn test_live_search_dedup_by_url() {
    use ragent_tools_extended::masterfetch::search::brave::BraveEngine;

    let engine = BraveEngine::new();
    let opts = SearchOptions::new(10);
    let report = engine.search("rust", &opts).await;

    if !report.engine_blocked && report.has_results() {
        let mut seen = std::collections::HashSet::new();
        for r in &report.results {
            let norm = r.normalised_url();
            assert!(
                seen.insert(norm),
                "duplicate URL after normalisation in results"
            );
        }
    }
}

#[tokio::test]
#[ignore = "requires network access to search.brave.com"]
async fn test_live_search_reports_blocked_on_rate_limit() {
    // Verify the blocked report structure.
    let report = EngineReport::blocked("brave", "rate-limited (HTTP 429)");
    assert_eq!(report.engine, "brave");
    assert!(report.engine_blocked);
    assert!(!report.has_results());
    assert!(report.error.contains("rate-limited"));
}
