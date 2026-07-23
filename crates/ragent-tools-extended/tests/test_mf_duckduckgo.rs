//! Unit tests for `masterfetch::search::duckduckgo` — DuckDuckGo keyless search
//! backend (T-013, FR-008, NFR-003).
//!
//! The HTML parsing and form-parameter construction are pure functions and are
//! tested thoroughly here. The full `search()` method requires network I/O and
//! is tested with `#[ignore]`-gated integration tests.

use ragent_tools_extended::masterfetch::search::duckduckgo::{
    build_form_params, parse_results_html,
};
use ragent_tools_extended::masterfetch::search::engine::{
    EngineReport, Freshness, SearchEngine, SearchOptions,
};

// ===========================================================================
// parse_results_html: basic parsing
// ===========================================================================

#[test]
fn test_parse_single_result() {
    let html = r#"<div class="result">
  <h2 class="result__title">
    <a class="result__a" href="https://example.com/page">Example Title</a>
  </h2>
  <a class="result__snippet" href="https://example.com/page">A snippet.</a>
</div>"#;
    let results = parse_results_html(html, "duckduckgo");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Example Title");
    assert_eq!(results[0].url, "https://example.com/page");
    assert_eq!(results[0].snippet, "A snippet.");
    assert_eq!(results[0].source, "duckduckgo");
}

#[test]
fn test_parse_multiple_results() {
    let html = r#"
<div class="result">
  <h2 class="result__title">
    <a class="result__a" href="https://a.com">First</a>
  </h2>
  <a class="result__snippet" href="https://a.com">First snippet.</a>
</div>
<div class="result">
  <h2 class="result__title">
    <a class="result__a" href="https://b.com">Second</a>
  </h2>
  <a class="result__snippet" href="https://b.com">Second snippet.</a>
</div>
<div class="result">
  <h2 class="result__title">
    <a class="result__a" href="https://c.com">Third</a>
  </h2>
  <a class="result__snippet" href="https://c.com">Third snippet.</a>
</div>
"#;
    let results = parse_results_html(html, "duckduckgo");
    assert_eq!(results.len(), 3);
    assert_eq!(results[0].title, "First");
    assert_eq!(results[1].title, "Second");
    assert_eq!(results[2].title, "Third");
}

#[test]
fn test_parse_empty_html() {
    let results = parse_results_html("", "duckduckgo");
    assert!(results.is_empty());
}

#[test]
fn test_parse_html_with_no_results() {
    let html = r#"<html><body><p>No results found.</p></body></html>"#;
    let results = parse_results_html(html, "duckduckgo");
    assert!(results.is_empty());
}

#[test]
fn test_parse_preserves_order() {
    let html = r#"
<div class="result">
  <h2 class="result__title"><a class="result__a" href="https://a.com">Alpha</a></h2>
  <a class="result__snippet" href="https://a.com">A snippet.</a>
</div>
<div class="result">
  <h2 class="result__title"><a class="result__a" href="https://b.com">Beta</a></h2>
  <a class="result__snippet" href="https://b.com">B snippet.</a>
</div>
<div class="result">
  <h2 class="result__title"><a class="result__a" href="https://c.com">Gamma</a></h2>
  <a class="result__snippet" href="https://c.com">C snippet.</a>
</div>
"#;
    let results = parse_results_html(html, "duckduckgo");
    assert_eq!(results.len(), 3);
    assert_eq!(results[0].title, "Alpha");
    assert_eq!(results[1].title, "Beta");
    assert_eq!(results[2].title, "Gamma");
}

// ===========================================================================
// parse_results_html: URL unwrapping
// ===========================================================================

#[test]
fn test_parse_unwraps_ddg_redirect_url() {
    let html = r#"<div class="result">
  <h2 class="result__title">
    <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fpage&rut=abc">Title</a>
  </h2>
  <a class="result__snippet" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fpage&rut=abc">Snippet</a>
</div>"#;
    let results = parse_results_html(html, "duckduckgo");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].url, "https://example.com/page");
}

#[test]
fn test_parse_unwraps_ddg_redirect_https() {
    let html = r#"<div class="result">
  <h2 class="result__title">
    <a class="result__a" href="https://duckduckgo.com/l/?uddg=https%3A%2F%2Fdocs.rs%2Fserde&rut=xyz">Title</a>
  </h2>
  <a class="result__snippet" href="https://duckduckgo.com/l/?uddg=https%3A%2F%2Fdocs.rs%2Fserde&rut=xyz">Snippet</a>
</div>"#;
    let results = parse_results_html(html, "duckduckgo");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].url, "https://docs.rs/serde");
}

#[test]
fn test_parse_keeps_direct_url() {
    let html = r#"<div class="result">
  <h2 class="result__title">
    <a class="result__a" href="https://example.com/direct">Title</a>
  </h2>
  <a class="result__snippet" href="https://example.com/direct">Snippet</a>
</div>"#;
    let results = parse_results_html(html, "duckduckgo");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].url, "https://example.com/direct");
}

#[test]
fn test_parse_unwraps_url_with_query_params() {
    let html = r#"<div class="result">
  <h2 class="result__title">
    <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fgithub.com%2Fuser%2Frepo%3Ftab%3Dreadme&rut=abc">Title</a>
  </h2>
  <a class="result__snippet" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fgithub.com%2Fuser%2Frepo%3Ftab%3Dreadme&rut=abc">Snippet</a>
</div>"#;
    let results = parse_results_html(html, "duckduckgo");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].url, "https://github.com/user/repo?tab=readme");
}

// ===========================================================================
// parse_results_html: edge cases
// ===========================================================================

#[test]
fn test_parse_result_without_snippet() {
    let html = r#"<div class="result">
  <h2 class="result__title">
    <a class="result__a" href="https://example.com">Title Only</a>
  </h2>
</div>"#;
    let results = parse_results_html(html, "duckduckgo");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Title Only");
    assert_eq!(results[0].snippet, "");
}

#[test]
fn test_parse_result_with_html_entities_in_title() {
    let html = r#"<div class="result">
  <h2 class="result__title">
    <a class="result__a" href="https://example.com">Tom &amp; Jerry Show</a>
  </h2>
  <a class="result__snippet" href="https://example.com">A &quot;great&quot; show.</a>
</div>"#;
    let results = parse_results_html(html, "duckduckgo");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Tom & Jerry Show");
    assert_eq!(results[0].snippet, "A \"great\" show.");
}

#[test]
fn test_parse_result_with_nested_tags_in_title() {
    let html = r#"<div class="result">
  <h2 class="result__title">
    <a class="result__a" href="https://example.com"><b>Bold</b> Title</a>
  </h2>
  <a class="result__snippet" href="https://example.com">Snippet</a>
</div>"#;
    let results = parse_results_html(html, "duckduckgo");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Bold Title");
}

#[test]
fn test_parse_result_with_extra_class_attributes() {
    // DDG sometimes adds extra classes to result divs.
    let html = r#"<div class="result results_links results_links_deep web-result">
  <h2 class="result__title">
    <a class="result__a" href="https://example.com">Title</a>
  </h2>
  <a class="result__snippet" href="https://example.com">Snippet</a>
</div>"#;
    let results = parse_results_html(html, "duckduckgo");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Title");
}

#[test]
fn test_parse_result_with_empty_title_skipped() {
    let html = r#"<div class="result">
  <h2 class="result__title">
    <a class="result__a" href="https://example.com"></a>
  </h2>
  <a class="result__snippet" href="https://example.com">Snippet</a>
</div>
<div class="result">
  <h2 class="result__title">
    <a class="result__a" href="https://other.com">Real Title</a>
  </h2>
  <a class="result__snippet" href="https://other.com">Other snippet.</a>
</div>"#;
    let results = parse_results_html(html, "duckduckgo");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Real Title");
}

#[test]
fn test_parse_result_without_title_link_skipped() {
    let html = r#"<div class="result">
  <h2 class="result__title">No link here</h2>
</div>
<div class="result">
  <h2 class="result__title">
    <a class="result__a" href="https://example.com">Real Title</a>
  </h2>
  <a class="result__snippet" href="https://example.com">Snippet</a>
</div>"#;
    let results = parse_results_html(html, "duckduckgo");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Real Title");
}

#[test]
fn test_parse_malformed_html_does_not_panic() {
    // Malformed HTML — must not panic.
    let html = r##"<div class="result"><h2><a class="result__a" href="#">Unclosed</a></h2></div>"##;
    let results = parse_results_html(html, "duckduckgo");
    // Should not panic; may return 0 or partial results.
    let _ = results.len();
}

#[test]
fn test_parse_source_field_set() {
    let html = r#"<div class="result">
  <h2 class="result__title">
    <a class="result__a" href="https://example.com">Title</a>
  </h2>
</div>"#;
    let results = parse_results_html(html, "ddg");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].source, "ddg");
}

#[test]
fn test_parse_snippet_with_html_tags() {
    let html = r#"<div class="result">
  <h2 class="result__title">
    <a class="result__a" href="https://example.com">Title</a>
  </h2>
  <a class="result__snippet" href="https://example.com">This is a <b>bold</b> snippet.</a>
</div>"#;
    let results = parse_results_html(html, "duckduckgo");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].snippet, "This is a bold snippet.");
}

#[test]
fn test_parse_large_html_page() {
    let mut html = String::new();
    for i in 0..50 {
        html.push_str(&format!(
            r#"<div class="result">
  <h2 class="result__title"><a class="result__a" href="https://example.com/page{i}">Result {i}</a></h2>
  <a class="result__snippet" href="https://example.com/page{i}">Snippet {i}.</a>
</div>
"#,
        ));
    }
    let results = parse_results_html(&html, "duckduckgo");
    assert_eq!(results.len(), 50);
    assert_eq!(results[0].title, "Result 0");
    assert_eq!(results[49].title, "Result 49");
}

// ===========================================================================
// build_form_params: query construction
// ===========================================================================

#[test]
fn test_build_form_params_basic_query() {
    let opts = SearchOptions::default();
    let form = build_form_params("rust programming", &opts);
    let q = form.iter().find(|(k, _)| k == "q").expect("q field");
    assert_eq!(q.1, "rust programming");
}

#[test]
fn test_build_form_params_trims_query() {
    let opts = SearchOptions::default();
    let form = build_form_params("  rust  ", &opts);
    let q = form.iter().find(|(k, _)| k == "q").unwrap();
    assert_eq!(q.1, "rust");
}

#[test]
fn test_build_form_params_with_site_filter() {
    let opts = SearchOptions::default().with_site("github.com");
    let form = build_form_params("rust async", &opts);
    let q = form.iter().find(|(k, _)| k == "q").unwrap();
    assert!(q.1.contains("site:github.com"));
    assert!(q.1.contains("rust async"));
}

#[test]
fn test_build_form_params_with_exclude_sites() {
    let opts = SearchOptions::default()
        .with_exclude_sites(vec!["pinterest.com".to_string(), "quora.com".to_string()]);
    let form = build_form_params("rust", &opts);
    let q = form.iter().find(|(k, _)| k == "q").unwrap();
    assert!(q.1.contains("-site:pinterest.com"));
    assert!(q.1.contains("-site:quora.com"));
}

#[test]
fn test_build_form_params_with_freshness_day() {
    let opts = SearchOptions::default().with_freshness(Freshness::Day);
    let form = build_form_params("rust", &opts);
    let df = form.iter().find(|(k, _)| k == "df");
    assert_eq!(df.unwrap().1, "d");
}

#[test]
fn test_build_form_params_with_freshness_week() {
    let opts = SearchOptions::default().with_freshness(Freshness::Week);
    let form = build_form_params("rust", &opts);
    let df = form.iter().find(|(k, _)| k == "df");
    assert_eq!(df.unwrap().1, "w");
}

#[test]
fn test_build_form_params_with_freshness_month() {
    let opts = SearchOptions::default().with_freshness(Freshness::Month);
    let form = build_form_params("rust", &opts);
    let df = form.iter().find(|(k, _)| k == "df");
    assert_eq!(df.unwrap().1, "m");
}

#[test]
fn test_build_form_params_with_freshness_year() {
    let opts = SearchOptions::default().with_freshness(Freshness::Year);
    let form = build_form_params("rust", &opts);
    let df = form.iter().find(|(k, _)| k == "df");
    assert_eq!(df.unwrap().1, "y");
}

#[test]
fn test_build_form_params_freshness_any_omits_df() {
    let opts = SearchOptions::default();
    let form = build_form_params("rust", &opts);
    let df = form.iter().find(|(k, _)| k == "df");
    assert!(df.is_none(), "df should be omitted for Freshness::Any");
}

#[test]
fn test_build_form_params_page_zero_omits_s() {
    let opts = SearchOptions::default().with_page(0);
    let form = build_form_params("rust", &opts);
    let s = form.iter().find(|(k, _)| k == "s");
    assert!(s.is_none(), "s should be omitted for page 0");
}

#[test]
fn test_build_form_params_page_one_has_offset_20() {
    let opts = SearchOptions::default().with_page(1);
    let form = build_form_params("rust", &opts);
    let s = form.iter().find(|(k, _)| k == "s").unwrap();
    assert_eq!(s.1, "20");
}

#[test]
fn test_build_form_params_page_two_has_offset_40() {
    let opts = SearchOptions::default().with_page(2);
    let form = build_form_params("rust", &opts);
    let s = form.iter().find(|(k, _)| k == "s").unwrap();
    assert_eq!(s.1, "40");
}

#[test]
fn test_build_form_params_combined_filters() {
    let opts = SearchOptions::new(15)
        .with_site("stackoverflow.com")
        .with_freshness(Freshness::Month)
        .with_page(1);
    let form = build_form_params("rust async tokio", &opts);

    let q = form.iter().find(|(k, _)| k == "q").unwrap();
    assert!(q.1.contains("rust async tokio"));
    assert!(q.1.contains("site:stackoverflow.com"));

    let df = form.iter().find(|(k, _)| k == "df").unwrap();
    assert_eq!(df.1, "m");

    let s = form.iter().find(|(k, _)| k == "s").unwrap();
    assert_eq!(s.1, "20");
}

// ===========================================================================
// DuckDuckGoEngine: struct + trait
// ===========================================================================

#[test]
fn test_engine_name() {
    let engine = ragent_tools_extended::masterfetch::search::duckduckgo::DuckDuckGoEngine::new();
    assert_eq!(engine.name(), "duckduckgo");
}

#[test]
fn test_engine_default() {
    let engine =
        ragent_tools_extended::masterfetch::search::duckduckgo::DuckDuckGoEngine::default();
    assert_eq!(engine.name(), "duckduckgo");
}

#[tokio::test]
async fn test_engine_search_empty_query_returns_error_report() {
    let engine = ragent_tools_extended::masterfetch::search::duckduckgo::DuckDuckGoEngine::new();
    let report = engine.search("", &SearchOptions::default()).await;
    assert!(!report.is_success());
    assert!(!report.engine_blocked);
    assert!(report.error.contains("empty"));
    assert_eq!(report.engine, "duckduckgo");
}

#[tokio::test]
async fn test_engine_search_whitespace_query_returns_error_report() {
    let engine = ragent_tools_extended::masterfetch::search::duckduckgo::DuckDuckGoEngine::new();
    let report = engine.search("   ", &SearchOptions::default()).await;
    assert!(!report.is_success());
    assert!(report.error.contains("empty"));
}

// ===========================================================================
// SearchEngine trait object compatibility
// ===========================================================================

#[tokio::test]
async fn test_engine_as_trait_object() {
    use ragent_tools_extended::masterfetch::search::duckduckgo::DuckDuckGoEngine;

    let engine: Box<dyn SearchEngine> = Box::new(DuckDuckGoEngine::new());
    assert_eq!(engine.name(), "duckduckgo");

    let report = engine.search("", &SearchOptions::default()).await;
    assert_eq!(report.engine, "duckduckgo");
}

// ===========================================================================
// Integration tests (network — #[ignore])
// ===========================================================================

#[tokio::test]
#[ignore = "requires network access to duckduckgo.com"]
async fn test_live_search_returns_results() {
    use ragent_tools_extended::masterfetch::search::duckduckgo::DuckDuckGoEngine;

    let engine = DuckDuckGoEngine::new();
    let opts = SearchOptions::new(5);
    let report = engine.search("rust programming language", &opts).await;

    // The engine name must be set.
    assert_eq!(report.engine, "duckduckgo");

    // If not blocked, should have results.
    if !report.engine_blocked {
        assert!(
            report.has_results(),
            "expected results for 'rust programming language', got error: {}",
            report.error
        );
        // Each result should have a non-empty title and URL.
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
#[ignore = "requires network access to duckduckgo.com"]
async fn test_live_search_with_site_filter() {
    use ragent_tools_extended::masterfetch::search::duckduckgo::DuckDuckGoEngine;

    let engine = DuckDuckGoEngine::new();
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
#[ignore = "requires network access to duckduckgo.com"]
async fn test_live_search_respects_max_results() {
    use ragent_tools_extended::masterfetch::search::duckduckgo::DuckDuckGoEngine;

    let engine = DuckDuckGoEngine::new();
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
#[ignore = "requires network access to duckduckgo.com"]
async fn test_live_search_dedup_by_url() {
    use ragent_tools_extended::masterfetch::search::duckduckgo::DuckDuckGoEngine;

    let engine = DuckDuckGoEngine::new();
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
#[ignore = "requires network access to duckduckgo.com"]
async fn test_live_search_reports_blocked_on_rate_limit() {
    // This test would need to trigger rate-limiting, which is unreliable.
    // Instead, we verify the engine handles blocked responses correctly by
    // checking that a blocked report has the right structure.
    let report = EngineReport::blocked("duckduckgo", "rate-limited (HTTP 202)");
    assert_eq!(report.engine, "duckduckgo");
    assert!(report.engine_blocked);
    assert!(!report.has_results());
    assert!(report.error.contains("rate-limited"));
}
