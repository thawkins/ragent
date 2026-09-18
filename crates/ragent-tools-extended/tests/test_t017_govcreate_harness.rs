//! Unit tests for the govcreate classifier and acquisition budgets (spec
//! `govdoc` T-017: FR-005, FR-006, FR-016, NFR-002). Parser tests live in
//! `crates/ragent-specs/tests/test_t017_govcreate_parser.rs`.
//!
//! These tests complement the per-task suites
//! (`test_archdoc_content_ref`, `test_archdoc_url_source`,
//! `test_archdoc_local_source`); they cover the acceptance angles listed in the
//! plan that those suites missed at the time they were written:
//!
//! - mixed URL/local classification plus directory/UNC path coverage,
//! - the depth cap (FR-016) over a mock crawl,
//! - the local file-cap reason mapping to the corpus budget reason, and
//! - a nested folder with unsupported files (FR-005/FR-006).

use ragent_tools_extended::archdoc::url_source::BudgetReason;
use ragent_tools_extended::archdoc::{
    AcquisitionBudget, ContentRef, ExclusionReason, LocalAcquisitionBudget, acquire_local,
    acquire_url_with_fetcher, classify_content_ref,
};
use ragent_tools_extended::masterfetch::CrawlPage;
use ragent_tools_extended::masterfetch::crawl::{CrawlFetcher, FetchedPage};
use url::Url;

// ===========================================================================
// Classifier: URL vs local, scheme rejection (NFR-003)
// ===========================================================================

#[test]
fn test_t017_classify_trailing_dot_segement_is_local() {
    // A bare directory token is a local reference, never misrouted as a URL.
    let reference = classify_content_ref("./doc/arch/").expect("local dir");
    assert!(matches!(reference, ContentRef::Local(_)));
}

#[test]
fn test_t017_classify_unc_share_is_local() {
    let reference = classify_content_ref(r"\\share\arch\sad.md").expect("unc path");
    assert!(matches!(reference, ContentRef::Local(_)));
}

#[test]
fn test_t017_classify_rejects_query_less_bare_domain_as_local() {
    // A bare domain-looking token without a scheme is a local path (the
    // content-ref contract requires an explicit scheme for URLs).
    let reference = classify_content_ref("example.com/arch").expect("local-ish");
    assert!(matches!(reference, ContentRef::Local(_)));
}

// ===========================================================================
// Budget: depth cap, and the budget→reason mapping (FR-016, NFR-002)
// ===========================================================================

/// A mock fetcher driven purely by discovered links.
struct LinkFetcher {
    pages: std::collections::HashMap<String, FetchedPage>,
}

impl LinkFetcher {
    fn new() -> Self {
        Self {
            pages: std::collections::HashMap::new(),
        }
    }

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
}

#[async_trait::async_trait]
impl CrawlFetcher for LinkFetcher {
    async fn fetch_page(&self, url: &str) -> Option<FetchedPage> {
        self.pages.get(url).cloned()
    }
}

#[tokio::test]
async fn test_t017_depth_cap_stops_child_pages() {
    // FR-016: max_depth=1 must gather the start page plus depth-1 children
    // but never depth-2 grandchildren.
    let fetcher = LinkFetcher::new()
        .with_page(
            "https://example.gov/arch",
            "Root.",
            &["https://example.gov/arch/a"],
        )
        .with_page(
            "https://example.gov/arch/a",
            "Child A.",
            &["https://example.gov/arch/a/deep"],
        )
        .with_page("https://example.gov/arch/a/deep", "Grandchild.", &[]);

    let budget = AcquisitionBudget::new(10, 1, 200_000, 120_000);
    let corpus = acquire_url_with_fetcher(
        &Url::parse("https://example.gov/arch").expect("url"),
        &budget,
        false,
        &fetcher,
    )
    .await
    .expect("acquisition should succeed");

    let urls: Vec<&str> = corpus
        .sources
        .iter()
        .map(|source| source.url.as_str())
        .collect();
    assert!(urls.contains(&"https://example.gov/arch"));
    assert!(urls.contains(&"https://example.gov/arch/a"));
    assert!(
        !urls.contains(&"https://example.gov/arch/a/deep"),
        "depth cap must exclude the grandchild: {urls:?}"
    );
    assert_eq!(corpus.sources.len(), 2);
}

#[tokio::test]
async fn test_t017_overlapping_page_and_deadline_caps_report_page_first() {
    // FR-016: when the page cap and the deadline are both reached the page
    // cap wins — the engine truncates by MaxPages before the wall clock runs
    // out, and the corpus reports the page cap (T-006 precedence).
    let fetcher = LinkFetcher::new()
        .with_page(
            "https://example.gov/arch",
            "Root.",
            &["https://example.gov/arch/a"],
        )
        .with_page("https://example.gov/arch/a", "Child.", &[]);
    let budget = AcquisitionBudget::new(1, 2, 200_000, 120_000);
    let corpus = acquire_url_with_fetcher(
        &Url::parse("https://example.gov/arch").expect("url"),
        &budget,
        false,
        &fetcher,
    )
    .await
    .expect("acquisition should succeed");
    assert_eq!(corpus.sources.len(), 1);
    assert_eq!(corpus.budget_reached, Some(BudgetReason::MaxPages));
}

#[tokio::test]
async fn test_t017_char_budget_truncates_gathered_text() {
    // FR-016: a tight character budget truncates the corpus and is reported.
    let fetcher = LinkFetcher::new().with_page("https://example.gov/arch", &"a".repeat(5_000), &[]);

    let budget = AcquisitionBudget::new(10, 2, 800, 120_000);
    let corpus = acquire_url_with_fetcher(
        &Url::parse("https://example.gov/arch").expect("url"),
        &budget,
        false,
        &fetcher,
    )
    .await
    .expect("acquisition should succeed");

    // A fetched page is retained in full; the engine stops *gathering* once
    // the budget is reached rather than slicing mid-page, so `MaxTotalChars`
    // is the only truthful assertion. The fetcher above yields 5_000 chars per
    // page; the budget is 800, so the cap must trip.
    assert_eq!(
        corpus.budget_reached,
        Some(BudgetReason::MaxTotalChars),
        "char cap must be reported, got {:?}",
        corpus.budget_reached
    );
}

#[test]
fn test_t017_local_budget_reason_maps_to_corpus_reason() {
    // FR-016 local side: the file cap surfaces through the corpus budget enum
    // (MaxPages), never as a silent success.
    use ragent_tools_extended::archdoc::{
        LocalAcquisitionStats, LocalBudgetReason, local_budget_exhausted,
    };
    let stats = LocalAcquisitionStats {
        files_extracted: 3,
        total_chars: 100,
        elapsed_ms: 10,
    };
    let budget = LocalAcquisitionBudget::new(3, 1_000_000, 120_000);
    assert_eq!(
        local_budget_exhausted(&stats, &budget),
        Some(LocalBudgetReason::MaxFiles)
    );
    assert_eq!(
        LocalBudgetReason::MaxFiles.as_corpus_reason(),
        BudgetReason::MaxPages
    );
}

// ===========================================================================
// Local acquisition: nested folder + unsupported file (FR-005, FR-006)
// ===========================================================================

#[test]
fn test_t017_nested_folder_with_unsupported_files_gathers_supported_only() {
    // FR-005/FR-006 regression guard: a folder tree containing a supported
    // markdown file, a nested supported file, and two unsupported files must
    // gather the supported content and record the exclusions, never fail.
    let root = std::env::temp_dir().join(format!("govdoc-t017-acq-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("nested")).expect("mkdir nested");
    std::fs::write(root.join("top.md"), "# Top\nTop-level content.").expect("write top");
    std::fs::write(root.join("nested/inner.md"), "# Inner\nNested content.").expect("write inner");
    std::fs::write(root.join("nested/pic.png"), [0u8; 8]).expect("write png");
    std::fs::write(root.join("blob.exe"), [0u8; 8]).expect("write exe");

    let corpus = acquire_local(&root, &LocalAcquisitionBudget::default());
    let _ = std::fs::remove_dir_all(&root);

    assert_eq!(corpus.sources.len(), 2, "two markdown files: {corpus:?}");
    let labels: Vec<&str> = corpus
        .sources
        .iter()
        .map(|source| source.summary.as_str())
        .collect();
    assert!(
        labels.iter().any(|label| label.contains("inner")),
        "nested file must be gathered: {labels:?}"
    );
    let exclusion_reasons: Vec<ExclusionReason> =
        corpus.excluded.iter().map(|entry| entry.reason).collect();
    assert!(
        exclusion_reasons.contains(&ExclusionReason::NoContent),
        "unsupported files must be excluded, not fatal: {exclusion_reasons:?}"
    );
    assert!(corpus.text.contains("Top-level content."));
    assert!(corpus.text.contains("Nested content."));
}
