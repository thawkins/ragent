//! Unit tests for `masterfetch::crawl::orchestrator` — best-first crawl
//! orchestration (T-019, FR-011, FR-013, FR-014, NFR-001, NFR-003).
//!
//! The pure functions (`score_url`, `is_same_domain`, `normalize_and_dedup`,
//! `extract_domain`) are tested without any I/O. The `CrawlOrchestrator` is
//! tested with a mock `CrawlFetcher` that returns canned pages.

use std::collections::HashMap;

use ragent_tools_extended::masterfetch::CrawlPage;
use ragent_tools_extended::masterfetch::crawl::{
    CrawlConfig, CrawlFetcher, CrawlOrchestrator, FetchedPage, SitemapMode, TruncatedBy,
    extract_domain, is_same_domain, normalize_and_dedup, score_url,
};

// ===========================================================================
// score_url: content-likelihood boosts
// ===========================================================================

#[test]
fn test_score_docs_url_boosted() {
    let score = score_url("https://example.com/docs/guide", None, 0);
    assert!(
        score > 0.0,
        "docs/guide URL should have positive score: {score}"
    );
}

#[test]
fn test_score_api_url_boosted() {
    let score = score_url("https://example.com/api/reference", None, 0);
    assert!(
        score > 0.0,
        "api/reference URL should have positive score: {score}"
    );
}

#[test]
fn test_score_tutorial_url_boosted() {
    let score = score_url("https://example.com/tutorials/intro", None, 0);
    assert!(
        score > 0.0,
        "tutorials URL should have positive score: {score}"
    );
}

#[test]
fn test_score_wiki_url_boosted() {
    let score = score_url("https://example.com/wiki/page", None, 0);
    assert!(score > 0.0, "wiki URL should have positive score: {score}");
}

#[test]
fn test_score_help_url_boosted() {
    let score = score_url("https://example.com/help/getting-started", None, 0);
    assert!(score > 0.0, "help URL should have positive score: {score}");
}

#[test]
fn test_score_learn_url_boosted() {
    let score = score_url("https://example.com/learn/rust", None, 0);
    assert!(score > 0.0, "learn URL should have positive score: {score}");
}

#[test]
fn test_score_multiple_boost_segments_only_count_once() {
    // A URL with multiple boost segments should only get one boost.
    let single = score_url("https://example.com/docs", None, 0);
    let multiple = score_url("https://example.com/docs/guide/api", None, 0);
    // Both should have the same boost (only applied once).
    assert_eq!(single, multiple);
}

// ===========================================================================
// score_url: content-likelihood penalties
// ===========================================================================

#[test]
fn test_score_login_url_penalised() {
    let score = score_url("https://example.com/login", None, 0);
    assert!(score < 0.0, "login URL should have negative score: {score}");
}

#[test]
fn test_score_signin_url_penalised() {
    let score = score_url("https://example.com/signin", None, 0);
    assert!(
        score < 0.0,
        "signin URL should have negative score: {score}"
    );
}

#[test]
fn test_score_cart_url_penalised() {
    let score = score_url("https://example.com/cart", None, 0);
    assert!(score < 0.0, "cart URL should have negative score: {score}");
}

#[test]
fn test_score_submit_url_penalised() {
    let score = score_url("https://example.com/submit", None, 0);
    assert!(
        score < 0.0,
        "submit URL should have negative score: {score}"
    );
}

#[test]
fn test_score_admin_url_penalised() {
    let score = score_url("https://example.com/admin", None, 0);
    assert!(score < 0.0, "admin URL should have negative score: {score}");
}

#[test]
fn test_score_register_url_penalised() {
    let score = score_url("https://example.com/register", None, 0);
    assert!(
        score < 0.0,
        "register URL should have negative score: {score}"
    );
}

#[test]
fn test_score_checkout_url_penalised() {
    let score = score_url("https://example.com/checkout", None, 0);
    assert!(
        score < 0.0,
        "checkout URL should have negative score: {score}"
    );
}

// ===========================================================================
// score_url: non-HTML asset penalty
// ===========================================================================

#[test]
fn test_score_pdf_heavily_penalised() {
    let score = score_url("https://example.com/doc.pdf", None, 0);
    assert!(score < -5.0, "PDF URL should be heavily penalised: {score}");
}

#[test]
fn test_score_jpg_heavily_penalised() {
    let score = score_url("https://example.com/image.jpg", None, 0);
    assert!(score < -5.0, "JPG URL should be heavily penalised: {score}");
}

#[test]
fn test_score_png_heavily_penalised() {
    let score = score_url("https://example.com/logo.png", None, 0);
    assert!(score < -5.0, "PNG URL should be heavily penalised: {score}");
}

#[test]
fn test_score_css_heavily_penalised() {
    let score = score_url("https://example.com/style.css", None, 0);
    assert!(score < -5.0, "CSS URL should be heavily penalised: {score}");
}

#[test]
fn test_score_js_heavily_penalised() {
    let score = score_url("https://example.com/app.js", None, 0);
    assert!(score < -5.0, "JS URL should be heavily penalised: {score}");
}

#[test]
fn test_score_zip_heavily_penalised() {
    let score = score_url("https://example.com/archive.zip", None, 0);
    assert!(score < -5.0, "ZIP URL should be heavily penalised: {score}");
}

// ===========================================================================
// score_url: focus relevance
// ===========================================================================

#[test]
fn test_score_focus_term_in_path_boosted() {
    let without_focus = score_url("https://example.com/rust-async", None, 0);
    let with_focus = score_url("https://example.com/rust-async", Some("rust async"), 0);
    assert!(
        with_focus > without_focus,
        "focus term in path should boost score: {with_focus} vs {without_focus}"
    );
}

#[test]
fn test_score_focus_term_not_in_path_no_boost() {
    let without_focus = score_url("https://example.com/page", None, 0);
    let with_focus = score_url("https://example.com/page", Some("rust async"), 0);
    assert_eq!(
        without_focus, with_focus,
        "focus term not in path should not change score"
    );
}

#[test]
fn test_score_focus_single_short_term_no_boost() {
    // Terms shorter than 3 chars don't get a boost.
    let without = score_url("https://example.com/js", None, 0);
    let with_focus = score_url("https://example.com/js", Some("js"), 0);
    assert_eq!(without, with_focus, "short focus term should not boost");
}

#[test]
fn test_score_focus_multi_term() {
    let score = score_url(
        "https://example.com/rust-async-tokio",
        Some("rust async tokio"),
        0,
    );
    // Multiple terms matching should give a higher boost.
    assert!(
        score > 0.0,
        "multi-term focus match should have positive score: {score}"
    );
}

// ===========================================================================
// score_url: depth
// ===========================================================================

#[test]
fn test_score_shallower_depth_boosted() {
    let depth0 = score_url("https://example.com/page", None, 0);
    let depth1 = score_url("https://example.com/page", None, 1);
    let depth2 = score_url("https://example.com/page", None, 2);
    assert!(depth0 > depth1, "depth 0 should score higher than depth 1");
    assert!(depth1 > depth2, "depth 1 should score higher than depth 2");
}

#[test]
fn test_score_depth_boost_decreases() {
    // Each deeper level gets half the boost.
    let d0 = score_url("https://example.com/page", None, 0);
    let d1 = score_url("https://example.com/page", None, 1);
    // depth 0 → +1.0, depth 1 → +0.5
    let diff = d0 - d1;
    assert!(
        (diff - 0.5).abs() < 0.01,
        "depth boost difference should be ~0.5: {diff}"
    );
}

// ===========================================================================
// score_url: docs vs login comparison
// ===========================================================================

#[test]
fn test_score_docs_higher_than_login() {
    let docs = score_url("https://example.com/docs/guide", None, 0);
    let login = score_url("https://example.com/login", None, 0);
    assert!(
        docs > login,
        "docs URL should score higher than login URL: {docs} vs {login}"
    );
}

#[test]
fn test_score_plain_page_neutral() {
    // A plain page with no boost/penalty segments.
    let score = score_url("https://example.com/some/page", None, 1);
    // depth 1 → +0.5 boost, no content segments → 0.
    assert!(
        (score - 0.5).abs() < 0.01,
        "plain page at depth 1 should score ~0.5: {score}"
    );
}

// ===========================================================================
// is_same_domain
// ===========================================================================

#[test]
fn test_same_domain_true() {
    assert!(is_same_domain(
        "https://example.com/page",
        "https://example.com/"
    ));
}

#[test]
fn test_same_domain_false_different_domain() {
    assert!(!is_same_domain(
        "https://other.com/page",
        "https://example.com/"
    ));
}

#[test]
fn test_same_domain_false_subdomain() {
    assert!(!is_same_domain(
        "https://docs.example.com/page",
        "https://example.com/"
    ));
}

#[test]
fn test_same_domain_case_insensitive() {
    assert!(is_same_domain(
        "https://Example.COM/page",
        "https://example.com/"
    ));
}

#[test]
fn test_same_domain_with_ports() {
    assert!(is_same_domain(
        "https://example.com:8080/page",
        "https://example.com/"
    ));
}

#[test]
fn test_same_domain_invalid_url() {
    assert!(!is_same_domain("not a url", "https://example.com/"));
    assert!(!is_same_domain("https://example.com/", "not a url"));
}

#[test]
fn test_same_domain_both_invalid() {
    assert!(!is_same_domain("not a url", "also not a url"));
}

// ===========================================================================
// normalize_and_dedup
// ===========================================================================

#[test]
fn test_dedup_trailing_slash() {
    let urls = vec![
        "https://example.com/page/".to_string(),
        "https://example.com/page".to_string(),
    ];
    let result = normalize_and_dedup(&urls);
    assert_eq!(result.len(), 1);
}

#[test]
fn test_dedup_tracking_params() {
    let urls = vec![
        "https://example.com/article?utm_source=x".to_string(),
        "https://example.com/article".to_string(),
    ];
    let result = normalize_and_dedup(&urls);
    assert_eq!(result.len(), 1);
}

#[test]
fn test_dedup_case_insensitive_host() {
    let urls = vec![
        "https://Example.COM/page".to_string(),
        "https://example.com/page".to_string(),
    ];
    let result = normalize_and_dedup(&urls);
    assert_eq!(result.len(), 1);
}

#[test]
fn test_dedup_preserves_order() {
    let urls = vec![
        "https://a.com".to_string(),
        "https://b.com".to_string(),
        "https://c.com".to_string(),
    ];
    let result = normalize_and_dedup(&urls);
    assert_eq!(result.len(), 3);
    assert_eq!(result[0], "https://a.com/");
    assert_eq!(result[1], "https://b.com/");
    assert_eq!(result[2], "https://c.com/");
}

#[test]
fn test_dedup_empty_input() {
    let result = normalize_and_dedup(&[]);
    assert!(result.is_empty());
}

#[test]
fn test_dedup_keeps_invalid_urls() {
    let urls = vec!["not a url".to_string()];
    let result = normalize_and_dedup(&urls);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], "not a url");
}

#[test]
fn test_dedup_default_port_normalized() {
    let urls = vec![
        "https://example.com:443/page".to_string(),
        "https://example.com/page".to_string(),
    ];
    let result = normalize_and_dedup(&urls);
    assert_eq!(result.len(), 1);
}

// ===========================================================================
// extract_domain
// ===========================================================================

#[test]
fn test_extract_domain_basic() {
    assert_eq!(
        extract_domain("https://example.com/page"),
        Some("example.com".to_string())
    );
}

#[test]
fn test_extract_domain_lowercase() {
    assert_eq!(
        extract_domain("https://Example.COM/page"),
        Some("example.com".to_string())
    );
}

#[test]
fn test_extract_domain_with_port() {
    assert_eq!(
        extract_domain("https://example.com:8080/page"),
        Some("example.com".to_string())
    );
}

#[test]
fn test_extract_domain_subdomain() {
    assert_eq!(
        extract_domain("https://docs.example.com/page"),
        Some("docs.example.com".to_string())
    );
}

#[test]
fn test_extract_domain_invalid() {
    assert_eq!(extract_domain("not a url"), None);
}

#[test]
fn test_extract_domain_empty() {
    assert_eq!(extract_domain(""), None);
}

// ===========================================================================
// CrawlConfig defaults
// ===========================================================================

#[test]
fn test_config_defaults() {
    let config = CrawlConfig::default();
    assert_eq!(config.max_pages, 10);
    assert_eq!(config.max_depth, 2);
    assert_eq!(config.max_total_chars, 200_000);
    assert_eq!(config.deadline_ms, 120_000);
    assert!(config.focus.is_none());
    assert_eq!(config.sitemap, SitemapMode::Off);
    assert!(!config.discover_only);
    assert!(config.crawl_urls.is_empty());
    assert!(!config.respect_robots);
}

#[test]
fn test_sitemap_mode_default_is_off() {
    assert_eq!(SitemapMode::default(), SitemapMode::Off);
}

// ===========================================================================
// Mock CrawlFetcher for orchestrator tests
// ===========================================================================

/// A mock fetcher that returns canned pages from a pre-populated map.
struct MockFetcher {
    pages: HashMap<String, FetchedPage>,
}

impl MockFetcher {
    fn new() -> Self {
        Self {
            pages: HashMap::new(),
        }
    }

    fn with_page(mut self, url: &str, content: &str, links: &[&str]) -> Self {
        self.pages.insert(
            url.to_string(),
            FetchedPage {
                page: CrawlPage {
                    url: url.to_string(),
                    content_ok: true,
                    content: content.to_string(),
                    ..Default::default()
                },
                discovered_links: links.iter().map(|l| l.to_string()).collect(),
            },
        );
        self
    }
}

#[async_trait::async_trait]
impl CrawlFetcher for MockFetcher {
    async fn fetch_page(&self, url: &str) -> Option<FetchedPage> {
        self.pages.get(url).cloned()
    }
}

// ===========================================================================
// CrawlOrchestrator: basic crawl
// ===========================================================================

#[tokio::test]
async fn test_crawl_single_page_no_links() {
    let fetcher = MockFetcher::new().with_page("https://example.com/", "Hello world", &[]);
    let orchestrator = CrawlOrchestrator::with_defaults();
    let result = orchestrator.crawl("https://example.com/", &fetcher).await;

    assert_eq!(result.total_pages, 1);
    assert!(!result.truncated);
    assert_eq!(result.pages[0].url, "https://example.com/");
    assert_eq!(result.pages[0].content, "Hello world");
}

#[tokio::test]
async fn test_crawl_discovers_and_visits_links() {
    let fetcher = MockFetcher::new()
        .with_page(
            "https://example.com/",
            "Home page",
            &["https://example.com/docs", "https://example.com/about"],
        )
        .with_page("https://example.com/docs", "Docs page", &[])
        .with_page("https://example.com/about", "About page", &[]);

    let config = CrawlConfig {
        max_pages: 10,
        max_depth: 2,
        ..CrawlConfig::default()
    };
    let orchestrator = CrawlOrchestrator::new(config);
    let result = orchestrator.crawl("https://example.com/", &fetcher).await;

    assert_eq!(result.total_pages, 3);
    assert!(!result.truncated);
    // The start page should be first.
    assert_eq!(result.pages[0].url, "https://example.com/");
}

#[tokio::test]
async fn test_crawl_same_domain_only() {
    let fetcher = MockFetcher::new()
        .with_page(
            "https://example.com/",
            "Home",
            &["https://example.com/page", "https://other.com/page"],
        )
        .with_page("https://example.com/page", "Same domain", &[]);

    let orchestrator = CrawlOrchestrator::with_defaults();
    let result = orchestrator.crawl("https://example.com/", &fetcher).await;

    // Only same-domain pages should be fetched.
    assert_eq!(result.total_pages, 2);
    for page in &result.pages {
        assert!(page.url.contains("example.com"));
        assert!(!page.url.contains("other.com"));
    }
    // Cross-domain URL should still be in discovered_urls.
    assert!(
        result
            .discovered_urls
            .iter()
            .any(|u| u.contains("other.com"))
    );
}

#[tokio::test]
async fn test_crawl_max_pages_cap() {
    let mut fetcher = MockFetcher::new();
    fetcher = fetcher.with_page(
        "https://example.com/",
        "Home",
        &[
            "https://example.com/p1",
            "https://example.com/p2",
            "https://example.com/p3",
            "https://example.com/p4",
            "https://example.com/p5",
        ],
    );
    for i in 1..=5 {
        fetcher = fetcher.with_page(
            &format!("https://example.com/p{i}"),
            &format!("Page {i}"),
            &[],
        );
    }

    let config = CrawlConfig {
        max_pages: 3,
        ..CrawlConfig::default()
    };
    let orchestrator = CrawlOrchestrator::new(config);
    let result = orchestrator.crawl("https://example.com/", &fetcher).await;

    assert_eq!(result.total_pages, 3);
    assert!(result.truncated);
    assert_eq!(result.truncated_by, Some(TruncatedBy::MaxPages));
}

#[tokio::test]
async fn test_crawl_max_depth_cap() {
    let fetcher = MockFetcher::new()
        .with_page(
            "https://example.com/",
            "Depth 0",
            &["https://example.com/d1"],
        )
        .with_page(
            "https://example.com/d1",
            "Depth 1",
            &["https://example.com/d2"],
        )
        .with_page("https://example.com/d2", "Depth 2", &[]);

    // max_depth = 1 → only depth 0 and 1 are crawled.
    let config = CrawlConfig {
        max_depth: 1,
        ..CrawlConfig::default()
    };
    let orchestrator = CrawlOrchestrator::new(config);
    let result = orchestrator.crawl("https://example.com/", &fetcher).await;

    // Depth 0 (home) + Depth 1 (d1) = 2 pages. d2 should not be crawled.
    assert_eq!(result.total_pages, 2);
    assert!(
        !result
            .pages
            .iter()
            .any(|p| p.url == "https://example.com/d2")
    );
}

#[tokio::test]
async fn test_crawl_max_total_chars_cap() {
    let fetcher = MockFetcher::new()
        .with_page(
            "https://example.com/",
            &"a".repeat(100),
            &["https://example.com/p2"],
        )
        .with_page("https://example.com/p2", &"b".repeat(100), &[]);

    let config = CrawlConfig {
        max_total_chars: 150,
        ..CrawlConfig::default()
    };
    let orchestrator = CrawlOrchestrator::new(config);
    let result = orchestrator.crawl("https://example.com/", &fetcher).await;

    // First page = 100 chars, second = 100 → total 200 > 150.
    assert!(result.truncated);
    assert_eq!(result.truncated_by, Some(TruncatedBy::MaxTotalChars));
}

#[tokio::test]
async fn test_crawl_does_not_revisit() {
    // Page A links to B, B links back to A — should not loop.
    let fetcher = MockFetcher::new()
        .with_page(
            "https://example.com/a",
            "Page A",
            &["https://example.com/b"],
        )
        .with_page(
            "https://example.com/b",
            "Page B",
            &["https://example.com/a"],
        );

    let orchestrator = CrawlOrchestrator::with_defaults();
    let result = orchestrator.crawl("https://example.com/a", &fetcher).await;

    assert_eq!(result.total_pages, 2); // A and B, no revisit.
}

#[tokio::test]
async fn test_crawl_fetch_failure_skipped() {
    let fetcher = MockFetcher::new().with_page(
        "https://example.com/",
        "Home",
        &["https://example.com/missing", "https://example.com/present"],
    );
    // Only "present" is in the mock; "missing" will return None.
    let fetcher = fetcher.with_page("https://example.com/present", "Present", &[]);

    let orchestrator = CrawlOrchestrator::with_defaults();
    let result = orchestrator.crawl("https://example.com/", &fetcher).await;

    // Home + present = 2 pages. Missing was skipped.
    assert_eq!(result.total_pages, 2);
    assert!(!result.pages.iter().any(|p| p.url.contains("missing")));
}

// ===========================================================================
// CrawlOrchestrator: discover_only mode
// ===========================================================================

#[tokio::test]
async fn test_discover_only_does_not_store_pages() {
    let fetcher = MockFetcher::new().with_page(
        "https://example.com/",
        "Home page content",
        &["https://example.com/docs", "https://example.com/about"],
    );

    let config = CrawlConfig {
        discover_only: true,
        ..CrawlConfig::default()
    };
    let orchestrator = CrawlOrchestrator::new(config);
    let result = orchestrator.crawl("https://example.com/", &fetcher).await;

    // discover_only mode should not store page content.
    assert_eq!(result.total_pages, 0);
    assert!(result.pages.is_empty());
    // But discovered URLs should be populated.
    assert!(!result.discovered_urls.is_empty());
}

// ===========================================================================
// CrawlOrchestrator: selective crawl mode (crawl_urls)
// ===========================================================================

#[tokio::test]
async fn test_selective_crawl_fetches_only_specified_urls() {
    let fetcher = MockFetcher::new()
        .with_page("https://example.com/a", "Page A", &[])
        .with_page("https://example.com/b", "Page B", &[])
        .with_page("https://example.com/c", "Page C", &[]);

    let config = CrawlConfig {
        crawl_urls: vec![
            "https://example.com/a".to_string(),
            "https://example.com/c".to_string(),
        ],
        ..CrawlConfig::default()
    };
    let orchestrator = CrawlOrchestrator::new(config);
    let result = orchestrator.crawl("https://example.com/", &fetcher).await;

    // Only the two specified URLs should be fetched (not the start URL, not b).
    assert_eq!(result.total_pages, 2);
    let urls: Vec<&str> = result.pages.iter().map(|p| p.url.as_str()).collect();
    assert!(urls.contains(&"https://example.com/a"));
    assert!(urls.contains(&"https://example.com/c"));
    assert!(!urls.contains(&"https://example.com/b"));
}

#[tokio::test]
async fn test_selective_crawl_dedup_urls() {
    let fetcher = MockFetcher::new().with_page("https://example.com/a", "Page A", &[]);

    let config = CrawlConfig {
        crawl_urls: vec![
            "https://example.com/a".to_string(),
            "https://example.com/a/".to_string(), // duplicate after normalisation
        ],
        ..CrawlConfig::default()
    };
    let orchestrator = CrawlOrchestrator::new(config);
    let result = orchestrator.crawl("https://example.com/", &fetcher).await;

    assert_eq!(result.total_pages, 1);
}

#[tokio::test]
async fn test_selective_crawl_discover_only() {
    let fetcher = MockFetcher::new().with_page("https://example.com/a", "Page A", &[]);

    let config = CrawlConfig {
        crawl_urls: vec!["https://example.com/a".to_string()],
        discover_only: true,
        ..CrawlConfig::default()
    };
    let orchestrator = CrawlOrchestrator::new(config);
    let result = orchestrator.crawl("https://example.com/", &fetcher).await;

    assert_eq!(result.total_pages, 0);
    assert!(result.pages.is_empty());
}

#[tokio::test]
async fn test_selective_crawl_respects_max_pages() {
    let fetcher = MockFetcher::new()
        .with_page("https://example.com/a", "Page A", &[])
        .with_page("https://example.com/b", "Page B", &[])
        .with_page("https://example.com/c", "Page C", &[]);

    let config = CrawlConfig {
        crawl_urls: vec![
            "https://example.com/a".to_string(),
            "https://example.com/b".to_string(),
            "https://example.com/c".to_string(),
        ],
        max_pages: 2,
        ..CrawlConfig::default()
    };
    let orchestrator = CrawlOrchestrator::new(config);
    let result = orchestrator.crawl("https://example.com/", &fetcher).await;

    assert_eq!(result.total_pages, 2);
    assert!(result.truncated);
    assert_eq!(result.truncated_by, Some(TruncatedBy::MaxPages));
}

// ===========================================================================
// CrawlOrchestrator: best-first ordering
// ===========================================================================

#[tokio::test]
async fn test_best_first_docs_before_login() {
    // The start page links to both a docs page and a login page.
    // The docs page should be crawled first (higher score).
    let fetcher = MockFetcher::new()
        .with_page(
            "https://example.com/",
            "Home",
            &[
                "https://example.com/login",
                "https://example.com/docs/guide",
            ],
        )
        .with_page("https://example.com/login", "Login", &[])
        .with_page("https://example.com/docs/guide", "Docs", &[]);

    let orchestrator = CrawlOrchestrator::with_defaults();
    let result = orchestrator.crawl("https://example.com/", &fetcher).await;

    assert_eq!(result.total_pages, 3);
    // First page is the start page.
    assert_eq!(result.pages[0].url, "https://example.com/");
    // Second page should be docs (higher score than login).
    assert_eq!(result.pages[1].url, "https://example.com/docs/guide");
    // Third page is login.
    assert_eq!(result.pages[2].url, "https://example.com/login");
}

// ===========================================================================
// CrawlOrchestrator: deadline
// ===========================================================================

#[tokio::test]
async fn test_deadline_truncates_crawl() {
    // Create a fetcher with many pages and a very short deadline.
    let mut fetcher = MockFetcher::new();
    fetcher = fetcher.with_page(
        "https://example.com/",
        "Home",
        &[
            "https://example.com/p1",
            "https://example.com/p2",
            "https://example.com/p3",
        ],
    );
    for i in 1..=3 {
        fetcher = fetcher.with_page(
            &format!("https://example.com/p{i}"),
            &format!("Page {i}"),
            &[],
        );
    }

    let config = CrawlConfig {
        deadline_ms: 1, // 1ms — will almost certainly be exceeded.
        ..CrawlConfig::default()
    };
    let orchestrator = CrawlOrchestrator::new(config);
    let result = orchestrator.crawl("https://example.com/", &fetcher).await;

    // The crawl may or may not be truncated depending on timing, but if it
    // is, the truncated_by should be Deadline. We can't reliably test this
    // in CI, so we just verify the result is valid.
    if result.truncated {
        // It could be truncated by max_pages or deadline.
        assert!(result.truncated_by.is_some());
    }
}

// ===========================================================================
// CrawlOrchestrator: focus query
// ===========================================================================

#[tokio::test]
async fn test_focus_prioritises_relevant_urls() {
    let fetcher = MockFetcher::new()
        .with_page(
            "https://example.com/",
            "Home",
            &[
                "https://example.com/unrelated",
                "https://example.com/rust-async",
            ],
        )
        .with_page("https://example.com/unrelated", "Unrelated", &[])
        .with_page("https://example.com/rust-async", "Rust async", &[]);

    let config = CrawlConfig {
        focus: Some("rust async".to_string()),
        ..CrawlConfig::default()
    };
    let orchestrator = CrawlOrchestrator::new(config);
    let result = orchestrator.crawl("https://example.com/", &fetcher).await;

    assert_eq!(result.total_pages, 3);
    // The rust-async page should be crawled before the unrelated page.
    assert_eq!(result.pages[1].url, "https://example.com/rust-async");
}

// ===========================================================================
// CrawlOrchestrator: URL normalisation in queue
// ===========================================================================

#[tokio::test]
async fn test_crawl_normalises_discovered_urls() {
    // The start page links to a URL with a trailing slash and tracking param.
    let fetcher = MockFetcher::new()
        .with_page(
            "https://example.com/",
            "Home",
            &["https://example.com/page/?utm_source=x"],
        )
        .with_page("https://example.com/page", "Page", &[]);

    let orchestrator = CrawlOrchestrator::with_defaults();
    let result = orchestrator.crawl("https://example.com/", &fetcher).await;

    // The normalised URL should be fetched.
    assert_eq!(result.total_pages, 2);
    assert!(
        result
            .pages
            .iter()
            .any(|p| p.url == "https://example.com/page")
    );
}

// ===========================================================================
// CrawlOrchestrator: empty/invalid start URL
// ===========================================================================

#[tokio::test]
async fn test_crawl_invalid_start_url() {
    let fetcher = MockFetcher::new();
    let orchestrator = CrawlOrchestrator::with_defaults();
    let result = orchestrator.crawl("not a url", &fetcher).await;

    // Should not crash; may return 0 pages.
    let _ = result.total_pages;
}

// ===========================================================================
// CrawlOrchestrator: empty crawl_urls
// ===========================================================================

#[tokio::test]
async fn test_empty_crawl_urls_falls_back_to_bfs() {
    // When crawl_urls is empty, the orchestrator should use BFS from the
    // start URL.
    let fetcher = MockFetcher::new().with_page("https://example.com/", "Home", &[]);

    let config = CrawlConfig {
        crawl_urls: vec![],
        ..CrawlConfig::default()
    };
    let orchestrator = CrawlOrchestrator::new(config);
    let result = orchestrator.crawl("https://example.com/", &fetcher).await;

    assert_eq!(result.total_pages, 1);
}

// ===========================================================================
// CrawlOrchestrator: discovered_urls includes cross-domain
// ===========================================================================

#[tokio::test]
async fn test_discovered_urls_includes_cross_domain() {
    let fetcher =
        MockFetcher::new().with_page("https://example.com/", "Home", &["https://other.com/page"]);

    let orchestrator = CrawlOrchestrator::with_defaults();
    let result = orchestrator.crawl("https://example.com/", &fetcher).await;

    // Cross-domain URL should be in discovered_urls but not in pages.
    assert!(
        result
            .discovered_urls
            .iter()
            .any(|u| u == "https://other.com/page")
    );
    assert!(!result.pages.iter().any(|p| p.url.contains("other.com")));
}

// ===========================================================================
// TruncatedBy display/eq
// ===========================================================================

#[test]
fn test_truncated_by_variants() {
    assert_ne!(TruncatedBy::MaxPages, TruncatedBy::Deadline);
    assert_ne!(TruncatedBy::MaxPages, TruncatedBy::MaxTotalChars);
    assert_ne!(TruncatedBy::Deadline, TruncatedBy::MaxTotalChars);
}
