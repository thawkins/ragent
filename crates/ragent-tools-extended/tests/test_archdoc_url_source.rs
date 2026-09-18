//! Unit tests for `archdoc::url_source` — bounded URL acquisition for
//! `/spec govcreate` (spec `govdoc` T-006, FR-004, FR-016, NFR-002, NFR-003).
//!
//! The budget and its exhaustion check are pure and tested without any I/O.
//! The acquisition path is driven through a mock [`CrawlFetcher`], so no test
//! touches the network.
//!
//! The robots-disallowed branch is not unit-tested here: it is reached through
//! the real `RobotsChecker`, which needs network I/O. `RobotsChecker` itself is
//! covered by `test_mf_robots.rs`; this file covers the SSRF refusal instead.

use std::collections::HashMap;

use ragent_tools_extended::archdoc::{
    AcquisitionBudget, AcquisitionStats, BudgetReason, ExclusionReason, acquire_url_with_fetcher,
    budget_exhausted,
};
use ragent_tools_extended::masterfetch::CrawlPage;
use ragent_tools_extended::masterfetch::crawl::orchestrator::{
    DEFAULT_DEADLINE_MS, DEFAULT_MAX_DEPTH, DEFAULT_MAX_PAGES, DEFAULT_MAX_TOTAL_CHARS,
};
use ragent_tools_extended::masterfetch::crawl::{CrawlFetcher, FetchedPage};
use url::Url;

// ===========================================================================
// Mock CrawlFetcher
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

    /// Add a page with usable content and a list of discovered links.
    fn with_page(mut self, url: &str, content: &str, links: &[&str]) -> Self {
        self.pages.insert(
            url.to_string(),
            FetchedPage {
                page: CrawlPage {
                    url: url.to_string(),
                    content_ok: true,
                    summary: format!("summary for {url}"),
                    content: content.to_string(),
                    ..Default::default()
                },
                discovered_links: links.iter().map(std::string::ToString::to_string).collect(),
            },
        );
        self
    }

    /// Add a fetched page that yielded no usable content.
    fn with_empty_page(mut self, url: &str) -> Self {
        self.pages.insert(
            url.to_string(),
            FetchedPage {
                page: CrawlPage {
                    url: url.to_string(),
                    content_ok: false,
                    content: String::new(),
                    ..Default::default()
                },
                discovered_links: Vec::new(),
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

fn url(raw: &str) -> Url {
    Url::parse(raw).expect("test URL should parse")
}

// ===========================================================================
// Budget (FR-016, NFR-002)
// ===========================================================================

#[test]
fn test_budget_defaults_reuse_engine_constants() {
    let budget = AcquisitionBudget::default();
    assert_eq!(budget.max_pages, DEFAULT_MAX_PAGES);
    assert_eq!(budget.max_depth, DEFAULT_MAX_DEPTH);
    assert_eq!(budget.max_total_chars, DEFAULT_MAX_TOTAL_CHARS);
    assert_eq!(budget.deadline_ms, DEFAULT_DEADLINE_MS);
}

#[test]
fn test_budget_exhausted_within_limits_is_none() {
    let budget = AcquisitionBudget::new(10, 2, 200_000, 120_000);
    let stats = AcquisitionStats {
        pages_fetched: 3,
        total_chars: 1_000,
        elapsed_ms: 500,
    };
    assert_eq!(budget_exhausted(&stats, &budget), None);
}

#[test]
fn test_budget_exhausted_page_cap() {
    let budget = AcquisitionBudget::new(5, 2, 200_000, 120_000);
    let stats = AcquisitionStats {
        pages_fetched: 5,
        total_chars: 10,
        elapsed_ms: 10,
    };
    assert_eq!(
        budget_exhausted(&stats, &budget),
        Some(BudgetReason::MaxPages)
    );
}

#[test]
fn test_budget_exhausted_char_budget() {
    let budget = AcquisitionBudget::new(50, 2, 100, 120_000);
    let stats = AcquisitionStats {
        pages_fetched: 1,
        total_chars: 100,
        elapsed_ms: 10,
    };
    assert_eq!(
        budget_exhausted(&stats, &budget),
        Some(BudgetReason::MaxTotalChars)
    );
}

#[test]
fn test_budget_exhausted_deadline() {
    let budget = AcquisitionBudget::new(50, 2, 200_000, 500);
    let stats = AcquisitionStats {
        pages_fetched: 1,
        total_chars: 10,
        elapsed_ms: 500,
    };
    assert_eq!(
        budget_exhausted(&stats, &budget),
        Some(BudgetReason::Deadline)
    );
}

#[test]
fn test_budget_exhausted_deadline_takes_precedence() {
    // All three limits are exceeded; the deadline is reported first, matching
    // the crawl engine's check order.
    let budget = AcquisitionBudget::new(1, 2, 1, 1);
    let stats = AcquisitionStats {
        pages_fetched: 9,
        total_chars: 9,
        elapsed_ms: 9,
    };
    assert_eq!(
        budget_exhausted(&stats, &budget),
        Some(BudgetReason::Deadline)
    );
}

#[test]
fn test_budget_crawl_config_translates_values_and_robots() {
    let budget = AcquisitionBudget::new(7, 3, 3_000, 9_000);
    let config = budget.crawl_config(true);
    assert_eq!(config.max_pages, 7);
    assert_eq!(config.max_depth, 3);
    assert_eq!(config.max_total_chars, 3_000);
    assert_eq!(config.deadline_ms, 9_000);
    assert!(config.respect_robots);
    // Unset crawl fields keep their engine defaults.
    assert!(config.focus.is_none());
    assert!(config.crawl_urls.is_empty());
}

// ===========================================================================
// Safety refusal (NFR-003)
// ===========================================================================

#[tokio::test]
async fn test_acquire_rejects_private_host() {
    let fetcher = MockFetcher::new();
    let error = acquire_url_with_fetcher(
        &url("http://127.0.0.1/architecture"),
        &AcquisitionBudget::default(),
        false,
        &fetcher,
    )
    .await
    .expect_err("private host should be refused");
    assert!(
        format!("{error}").contains("SSRF"),
        "error should name the SSRF refusal: {error}"
    );
}

#[tokio::test]
async fn test_acquire_rejects_non_http_scheme() {
    let fetcher = MockFetcher::new();
    let error = acquire_url_with_fetcher(
        &url("ftp://example.com/architecture"),
        &AcquisitionBudget::default(),
        false,
        &fetcher,
    )
    .await
    .expect_err("non-HTTP scheme should be refused");
    assert!(
        format!("{error}").contains("security"),
        "error should be the security refusal: {error}"
    );
}

// ===========================================================================
// Acquisition and normalisation (FR-004)
// ===========================================================================

#[tokio::test]
async fn test_acquire_multi_page_corpus_and_stats() {
    let fetcher = MockFetcher::new()
        .with_page(
            "https://example.com/arch",
            "Root architecture overview.",
            &["https://example.com/arch/components"],
        )
        .with_page(
            "https://example.com/arch/components",
            "Component catalogue.",
            &[],
        );

    let corpus = acquire_url_with_fetcher(
        &url("https://example.com/arch"),
        &AcquisitionBudget::default(),
        false,
        &fetcher,
    )
    .await
    .expect("acquisition should succeed");

    assert_eq!(corpus.reference, "https://example.com/arch");
    assert_eq!(corpus.sources.len(), 2, "both pages should be gathered");
    assert_eq!(corpus.stats.pages_fetched, 2);
    assert!(corpus.stats.total_chars > 0);
    assert_eq!(corpus.budget_reached, None);
    assert!(!corpus.is_empty());
    // The flattened text carries a header per source.
    assert!(corpus.text.contains("Root architecture overview."));
    assert!(
        corpus
            .text
            .contains("## summary for https://example.com/arch")
    );
}

#[tokio::test]
async fn test_acquire_reports_page_budget_reached() {
    let fetcher = MockFetcher::new()
        .with_page(
            "https://example.com/arch",
            "Root architecture overview.",
            &["https://example.com/arch/a", "https://example.com/arch/b"],
        )
        .with_page("https://example.com/arch/a", "Page A.", &[])
        .with_page("https://example.com/arch/b", "Page B.", &[]);

    let budget = AcquisitionBudget::new(1, 2, 200_000, 120_000);
    let corpus =
        acquire_url_with_fetcher(&url("https://example.com/arch"), &budget, false, &fetcher)
            .await
            .expect("acquisition should succeed");

    assert_eq!(
        corpus.sources.len(),
        1,
        "the page cap should stop the crawl"
    );
    assert_eq!(corpus.budget_reached, Some(BudgetReason::MaxPages));
}

#[tokio::test]
async fn test_acquire_zero_deadline_reports_deadline() {
    let fetcher = MockFetcher::new().with_page("https://example.com/arch", "Root.", &[]);
    let budget = AcquisitionBudget::new(10, 2, 200_000, 0);
    let corpus =
        acquire_url_with_fetcher(&url("https://example.com/arch"), &budget, false, &fetcher)
            .await
            .expect("acquisition should succeed");

    assert_eq!(corpus.sources.len(), 0, "a zero deadline fetches nothing");
    assert_eq!(corpus.budget_reached, Some(BudgetReason::Deadline));
    assert!(corpus.is_empty());
}

#[tokio::test]
async fn test_acquire_skips_no_content_and_records_exclusion() {
    let fetcher = MockFetcher::new()
        .with_page(
            "https://example.com/arch",
            "Root architecture overview.",
            &["https://example.com/arch/empty"],
        )
        .with_empty_page("https://example.com/arch/empty");

    let corpus = acquire_url_with_fetcher(
        &url("https://example.com/arch"),
        &AcquisitionBudget::default(),
        false,
        &fetcher,
    )
    .await
    .expect("acquisition should succeed");

    assert_eq!(corpus.sources.len(), 1, "only usable content is gathered");
    assert_eq!(corpus.stats.pages_fetched, 2);
    assert_eq!(corpus.excluded.len(), 1);
    assert_eq!(corpus.excluded[0].reason, ExclusionReason::NoContent);
}

#[tokio::test]
async fn test_acquire_records_cross_domain_exclusion() {
    let fetcher = MockFetcher::new().with_page(
        "https://example.com/arch",
        "Root architecture overview.",
        &["https://other.example.org/elsewhere"],
    );

    let corpus = acquire_url_with_fetcher(
        &url("https://example.com/arch"),
        &AcquisitionBudget::default(),
        false,
        &fetcher,
    )
    .await
    .expect("acquisition should succeed");

    assert_eq!(corpus.sources.len(), 1);
    let cross_domain = corpus
        .excluded
        .iter()
        .find(|entry| entry.reason == ExclusionReason::CrossDomain)
        .expect("the cross-domain link should be recorded");
    assert!(cross_domain.url.contains("other.example.org"));
}

#[tokio::test]
async fn test_acquire_empty_page_yields_empty_corpus() {
    let fetcher = MockFetcher::new().with_empty_page("https://example.com/arch");
    let corpus = acquire_url_with_fetcher(
        &url("https://example.com/arch"),
        &AcquisitionBudget::default(),
        false,
        &fetcher,
    )
    .await
    .expect("acquisition should succeed even with no usable text");

    assert!(corpus.is_empty());
    assert!(corpus.text.is_empty());
    assert_eq!(corpus.excluded.len(), 1);
    assert_eq!(corpus.excluded[0].reason, ExclusionReason::NoContent);
}
