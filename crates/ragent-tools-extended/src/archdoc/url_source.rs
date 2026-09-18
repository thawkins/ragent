//! Bounded URL acquisition for `/spec govcreate` (spec `govdoc` T-006).
//!
//! Implements **FR-004** (URL content-reference acquisition), **FR-016**
//! (bounded acquisition), **NFR-002** (pure, network-free budget logic), and
//! **NFR-003** (SSRF validation and `robots.txt` compliance).
//!
//! A URL content reference is acquired by driving the shared masterfetch crawl
//! engine ([`CrawlOrchestrator`]) with a bounded [`AcquisitionBudget`] and
//! normalising the engine's [`CrawlResult`] into the source-agnostic
//! [`GatheredCorpus`] shape the architecture-extraction stage (T-008) consumes.
//! The local acquisition stage (T-007) produces the same shape, so extraction
//! never needs to know where the text came from.
//!
//! # Testability (NFR-002)
//!
//! The budget and its exhaustion check are pure functions over plain data
//! ([`budget_exhausted`]), so FR-016 is unit-tested without any network access.
//! The crawl itself is driven through a [`CrawlFetcher`], so the acquisition
//! path is exercised with a mock fetcher in the tests.
//!
//! # Safety (NFR-003)
//!
//! The start URL is validated against the masterfetch SSRF rules before any
//! fetch, and `robots.txt` is honoured by default. A blocked or disallowed URL
//! yields [`UrlAcquisitionError`] and no acquisition. The production fetcher
//! ([`HttpCrawlFetcher`]) revalidates every discovered link, so a same-domain
//! crawl cannot be redirected off-domain into a private address.
//!
//! # Depth cap
//!
//! The budget carries `max_depth` and the crawl engine enforces it structurally:
//! a discovered link is never enqueued beyond that depth, so the crawl simply
//! stops descending instead of truncating. It is therefore not reported as a
//! [`BudgetReason`], unlike the page, character, and deadline caps which the
//! engine reports through [`TruncatedBy`].
//!
//! # Scope
//!
//! An acquisition that yields no usable text returns an empty corpus rather
//! than an error: the FR-006 empty/unreadable handling belongs to the
//! orchestration stage (T-012), which sees the empty corpus before scaffolding.

use std::collections::HashSet;
use std::time::Instant;

use url::Url;

use crate::masterfetch::PageType;
use crate::masterfetch::crawl::orchestrator::{
    DEFAULT_DEADLINE_MS, DEFAULT_MAX_DEPTH, DEFAULT_MAX_PAGES, DEFAULT_MAX_TOTAL_CHARS,
};
use crate::masterfetch::crawl::{
    CrawlConfig, CrawlFetcher, CrawlOrchestrator, CrawlResult, TruncatedBy, extract_domain,
};
use crate::masterfetch::robots::{DEFAULT_USER_AGENT, RobotsChecker};
use crate::masterfetch::security::validate_url;
use crate::masterfetch::tools::crawl_tool::HttpCrawlFetcher;

// ---------------------------------------------------------------------------
// Budget (FR-016)
// ---------------------------------------------------------------------------

/// The bounded budget for one content acquisition (FR-016).
///
/// The page cap, depth cap, character budget, and wall-clock deadline are
/// enforced by the crawl engine; [`Default`] reuses the engine's own
/// `DEFAULT_*` constants rather than re-literalising them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcquisitionBudget {
    /// Maximum pages to fetch (crawl page cap).
    pub max_pages: usize,
    /// Maximum crawl depth from the start URL.
    pub max_depth: usize,
    /// Total character budget across all fetched pages.
    pub max_total_chars: usize,
    /// Wall-clock time budget in milliseconds.
    pub deadline_ms: u64,
}

impl Default for AcquisitionBudget {
    fn default() -> Self {
        Self {
            max_pages: DEFAULT_MAX_PAGES,
            max_depth: DEFAULT_MAX_DEPTH,
            max_total_chars: DEFAULT_MAX_TOTAL_CHARS,
            deadline_ms: DEFAULT_DEADLINE_MS,
        }
    }
}

impl AcquisitionBudget {
    /// Build a budget from explicit caps.
    #[must_use]
    pub const fn new(
        max_pages: usize,
        max_depth: usize,
        max_total_chars: usize,
        deadline_ms: u64,
    ) -> Self {
        Self {
            max_pages,
            max_depth,
            max_total_chars,
            deadline_ms,
        }
    }

    /// Translate the budget into a crawl-engine configuration (FR-016).
    ///
    /// `respect_robots` is passed through to the engine configuration; every
    /// other crawl field keeps its engine default.
    #[must_use]
    pub fn crawl_config(&self, respect_robots: bool) -> CrawlConfig {
        CrawlConfig {
            max_pages: self.max_pages,
            max_depth: self.max_depth,
            max_total_chars: self.max_total_chars,
            deadline_ms: self.deadline_ms,
            respect_robots,
            ..CrawlConfig::default()
        }
    }
}

/// Which acquisition budget stopped a crawl (FR-016).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetReason {
    /// The page cap was reached.
    MaxPages,
    /// The total-character budget was reached.
    MaxTotalChars,
    /// The wall-clock deadline was reached.
    Deadline,
}

impl std::fmt::Display for BudgetReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::MaxPages => "page cap",
            Self::MaxTotalChars => "character budget",
            Self::Deadline => "deadline",
        };
        f.write_str(label)
    }
}

/// The measured outcome of an acquisition, as the pure budget check needs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AcquisitionStats {
    /// Number of pages the crawl fetched, including pages whose content was not
    /// usable.
    pub pages_fetched: usize,
    /// Total characters of gathered (usable) source text.
    pub total_chars: usize,
    /// Wall-clock duration of the acquisition in milliseconds.
    pub elapsed_ms: u64,
}

/// Report which budget stopped an acquisition (FR-016, NFR-002).
///
/// Pure: it compares measured [`AcquisitionStats`] against an
/// [`AcquisitionBudget`] with no I/O, checking the deadline first, then the page
/// cap, then the character budget - the same precedence the crawl engine
/// applies.
#[must_use]
pub fn budget_exhausted(
    stats: &AcquisitionStats,
    limits: &AcquisitionBudget,
) -> Option<BudgetReason> {
    if stats.elapsed_ms >= limits.deadline_ms {
        return Some(BudgetReason::Deadline);
    }
    if stats.pages_fetched >= limits.max_pages {
        return Some(BudgetReason::MaxPages);
    }
    if stats.total_chars >= limits.max_total_chars {
        return Some(BudgetReason::MaxTotalChars);
    }
    None
}

// ---------------------------------------------------------------------------
// Gathered corpus (FR-004)
// ---------------------------------------------------------------------------

/// One gathered source document/page in a [`GatheredCorpus`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatheredSource {
    /// The source URL (normalised, final after redirects).
    pub url: String,
    /// One-line summary supplied by the crawl engine, used as the source label.
    pub summary: String,
    /// The detected page type.
    pub page_type: PageType,
    /// The gathered plain text for this source.
    pub content: String,
}

/// Why a discovered source was not included in the gathered corpus (FR-004).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExclusionReason {
    /// The page was fetched but yielded no usable text.
    NoContent,
    /// The URL was discovered but the page could not be fetched.
    FetchFailed,
    /// The URL belongs to another domain and was not crawled.
    CrossDomain,
}

impl std::fmt::Display for ExclusionReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::NoContent => "no usable content",
            Self::FetchFailed => "fetch failed",
            Self::CrossDomain => "cross-domain (not crawled)",
        };
        f.write_str(label)
    }
}

/// A discovered source that was excluded or failed, with the reason (FR-004).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExcludedSource {
    /// The URL that was not gathered.
    pub url: String,
    /// Why the URL was not gathered.
    pub reason: ExclusionReason,
}

/// The normalised, bounded corpus produced from a content reference (FR-004).
///
/// One shape serves both acquisition paths (URL here, local in T-007) so the
/// extraction stage is source-agnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatheredCorpus {
    /// Human-readable label for the content reference (the URL, or later a
    /// local path).
    pub reference: String,
    /// The gathered sources, in crawl order.
    pub sources: Vec<GatheredSource>,
    /// Discovered sources that were excluded or failed, with the reason.
    pub excluded: Vec<ExcludedSource>,
    /// The flattened, bounded plain text: every source's content under a
    /// `## <summary> (<url>)` header, in crawl order.
    pub text: String,
    /// Acquisition statistics.
    pub stats: AcquisitionStats,
    /// Which budget stopped the crawl, if any (FR-016).
    pub budget_reached: Option<BudgetReason>,
}

impl GatheredCorpus {
    /// `true` when no usable text was gathered (the FR-006 empty case).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Why a URL content reference could not be acquired.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UrlAcquisitionError {
    /// The URL failed SSRF / security validation (NFR-003).
    Blocked {
        /// The rejected URL.
        url: String,
        /// The underlying security reason.
        detail: String,
    },
    /// `robots.txt` disallows crawling the URL (NFR-003).
    RobotsDisallowed {
        /// The disallowed URL.
        url: String,
    },
}

impl std::fmt::Display for UrlAcquisitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Blocked { url, detail } => {
                write!(f, "URL rejected by SSRF security check: {url} ({detail})")
            }
            Self::RobotsDisallowed { url } => {
                write!(f, "robots.txt disallows crawling {url}")
            }
        }
    }
}

impl std::error::Error for UrlAcquisitionError {}

// ---------------------------------------------------------------------------
// Acquisition (FR-004)
// ---------------------------------------------------------------------------

/// Acquire a URL content reference with the production HTTP crawl fetcher.
///
/// `robots.txt` is honoured (NFR-003 default).
pub async fn acquire_url(
    url: &Url,
    budget: &AcquisitionBudget,
) -> Result<GatheredCorpus, UrlAcquisitionError> {
    let fetcher = HttpCrawlFetcher::new(true);
    acquire_url_with_fetcher(url, budget, true, &fetcher).await
}

/// Acquire a URL content reference through a caller-supplied fetcher (FR-004).
///
/// Validates the start URL against the SSRF rules, checks `robots.txt` when
/// `respect_robots` is set, runs the bounded crawl, and normalises the result
/// into a [`GatheredCorpus`]. Passing a mock [`CrawlFetcher`] exercises the
/// whole path without network access.
pub async fn acquire_url_with_fetcher(
    url: &Url,
    budget: &AcquisitionBudget,
    respect_robots: bool,
    fetcher: &dyn CrawlFetcher,
) -> Result<GatheredCorpus, UrlAcquisitionError> {
    let raw = url.as_str();

    validate_url(raw).map_err(|err| UrlAcquisitionError::Blocked {
        url: raw.to_owned(),
        detail: err.to_string(),
    })?;

    if respect_robots && !robots_allows(raw).await {
        return Err(UrlAcquisitionError::RobotsDisallowed {
            url: raw.to_owned(),
        });
    }

    let started = Instant::now();
    let orchestrator = CrawlOrchestrator::new(budget.crawl_config(respect_robots));
    let result = orchestrator.crawl(raw, fetcher).await;
    let elapsed_ms = started.elapsed().as_millis() as u64;

    Ok(normalise(raw, &result, budget, elapsed_ms))
}

/// Answer whether `robots.txt` allows crawling `url`, defaulting to allowed
/// when the check cannot be completed (NFR-003).
async fn robots_allows(url: &str) -> bool {
    match RobotsChecker::new()
        .is_allowed(url, DEFAULT_USER_AGENT)
        .await
    {
        Ok(allowed) => allowed,
        Err(err) => {
            tracing::warn!(url = url, error = %err, "govcreate: robots.txt check failed; allowing by default");
            true
        }
    }
}

/// Normalise a crawl result into a [`GatheredCorpus`] (FR-004, FR-016).
fn normalise(
    reference: &str,
    result: &CrawlResult,
    budget: &AcquisitionBudget,
    elapsed_ms: u64,
) -> GatheredCorpus {
    let start_domain = extract_domain(reference);
    let mut sources: Vec<GatheredSource> = Vec::new();
    let mut excluded: Vec<ExcludedSource> = Vec::new();
    let mut seen_excluded: HashSet<String> = HashSet::new();
    let mut fetched: HashSet<&str> = HashSet::new();
    let mut total_chars = 0usize;

    for page in &result.pages {
        fetched.insert(page.url.as_str());
        let content = page.content.trim();
        if !page.content_ok || content.is_empty() {
            push_excluded(
                &mut excluded,
                &mut seen_excluded,
                &page.url,
                ExclusionReason::NoContent,
            );
            continue;
        }
        total_chars += content.chars().count();
        sources.push(GatheredSource {
            url: page.url.clone(),
            summary: if page.summary.is_empty() {
                page.url.clone()
            } else {
                page.summary.clone()
            },
            page_type: page.page_type,
            content: content.to_owned(),
        });
    }

    for discovered in &result.discovered_urls {
        if fetched.contains(discovered.as_str()) {
            continue;
        }
        let reason = classify_exclusion(start_domain.as_deref(), discovered);
        push_excluded(&mut excluded, &mut seen_excluded, discovered, reason);
    }

    let text = join_text(&sources);
    let stats = AcquisitionStats {
        pages_fetched: result.total_pages,
        total_chars,
        elapsed_ms,
    };
    let budget_reached = result
        .truncated_by
        .map(reason_from_truncation)
        .or_else(|| budget_exhausted(&stats, budget));

    GatheredCorpus {
        reference: reference.to_owned(),
        sources,
        excluded,
        text,
        stats,
        budget_reached,
    }
}

/// Map the engine's truncation reason onto a [`BudgetReason`] (FR-016).
const fn reason_from_truncation(truncated_by: TruncatedBy) -> BudgetReason {
    match truncated_by {
        TruncatedBy::MaxPages => BudgetReason::MaxPages,
        TruncatedBy::MaxTotalChars => BudgetReason::MaxTotalChars,
        TruncatedBy::Deadline => BudgetReason::Deadline,
    }
}

/// Classify a discovered-but-ungathered URL as failed or cross-domain.
fn classify_exclusion(start_domain: Option<&str>, url: &str) -> ExclusionReason {
    match (start_domain, extract_domain(url)) {
        (Some(start), Some(domain)) if start != domain => ExclusionReason::CrossDomain,
        _ => ExclusionReason::FetchFailed,
    }
}

/// Record an excluded URL once, preserving first-occurrence order.
fn push_excluded(
    excluded: &mut Vec<ExcludedSource>,
    seen: &mut HashSet<String>,
    url: &str,
    reason: ExclusionReason,
) {
    if seen.insert(url.to_owned()) {
        excluded.push(ExcludedSource {
            url: url.to_owned(),
            reason,
        });
    }
}

/// Flatten the gathered sources into one bounded text block (FR-004).
///
/// This format is the shared corpus contract for both the URL and local
/// paths (FR-005 calls it via [`crate::archdoc::local_source`]).
pub(crate) fn join_text(sources: &[GatheredSource]) -> String {
    let mut out = String::new();
    for source in sources {
        out.push_str(&format!(
            "## {}\n{}\n\n{}\n\n",
            source.summary, source.url, source.content
        ));
    }
    out
}
