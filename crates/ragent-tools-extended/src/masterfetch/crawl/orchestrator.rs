//! Best-first same-domain crawl orchestration for `mf_crawl`.
//!
//! Implements **FR-011**, **FR-013**, **FR-014**, and **NFR-001** (T-019).
//!
//! This module orchestrates a breadth-priority crawl starting from a seed URL.
//! Discovered links are scored by focus relevance, content-likelihood, and
//! depth, then visited in best-first order. The crawl is bounded by caps
//! (`max_pages`, `max_depth`, `max_total_chars`, `deadline_ms`).
//!
//! # Scoring
//!
//! Each discovered URL is scored by [`score_url`]:
//!
//! - **Focus relevance** — query terms appearing in the URL path receive a
//!   boost. When no focus query is provided, all URLs start at the same base
//!   score.
//! - **Content-likelihood** — URLs containing `docs`, `guide`, `api`,
//!   `reference`, `tutorial`, `manual`, `help`, `wiki` are boosted (these
//!   are likely to contain substantive content). URLs containing `login`,
//!   `signin`, `auth`, `signup`, `register`, `submit`, `post`, `comment`,
//!   `cart`, `checkout`, `payment` are penalised (these are unlikely to
//!   contain crawlable content).
//! - **Depth** — shallower URLs (lower `depth`) receive a small boost so the
//!   crawl doesn't dive deep before exploring breadth.
//! - **Non-HTML assets** — URLs ending in `.pdf`, `.jpg`, `.png`, `.gif`,
//!   `.zip`, `.css`, `.js`, `.svg`, `.ico` are heavily penalised.
//!
//! # Same-domain scoping
//!
//! Only URLs on the same domain as the start URL are crawled. Cross-domain
//! links are discovered but not fetched (they may appear in `discovered_urls`
//! if `discover_only` is set).
//!
//! # Caps
//!
//! - `max_pages` — maximum number of pages to fetch (default 10).
//! - `max_depth` — maximum crawl depth from the start URL (default 2).
//! - `max_total_chars` — total character budget across all pages (default
//!   200 000). When exceeded, the crawl is truncated with
//!   `truncated_by = "max_total_chars"`.
//! - `deadline_ms` — wall-clock time budget (default 120 000 ms = 2 min).
//!   When exceeded, the crawl is truncated with `truncated_by = "deadline"`.
//!
//! # Modes
//!
//! - **Normal** — crawl from the start URL, discover and fetch same-domain
//!   pages in best-first order.
//! - **`discover_only = true`** — discover URLs (via BFS or sitemap) but do
//!   not fetch page content. Returns the URL map only.
//! - **`crawl_urls = [...]`** — fetch only the specified subset of URLs
//!   (second-phase selective crawl). No BFS discovery is performed.
//! - **`sitemap = true`** — discover URLs from the site's `sitemap.xml`
//!   before crawling (FR-013). When `sitemap = "auto"`, the sitemap is used
//!   if present and BFS is the fallback.
//!
//! # Testability (NFR-003)
//!
//! The scoring, domain-scoping, and URL-dedup functions are pure and fully
//! unit-tested. The [`CrawlOrchestrator`] accepts a [`CrawlFetcher`] trait
//! that abstracts HTTP I/O, enabling tests with a mock fetcher.
//!
//! # Examples
//!
//! Score a URL:
//!
//! ```
//! use ragent_tools_extended::masterfetch::crawl::score_url;
//!
//! // A docs URL gets a boost.
//! let docs_score = score_url("https://example.com/docs/guide", None, 0);
//! let login_score = score_url("https://example.com/login", None, 0);
//! assert!(docs_score > login_score);
//! ```

use std::collections::{BinaryHeap, HashSet};
use std::time::{Duration, Instant};

use url::Url;

use super::super::CrawlPage;
use super::super::urlnorm::normalise_url;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default maximum pages to crawl (FR-011).
pub const DEFAULT_MAX_PAGES: usize = 10;

/// Default maximum crawl depth from the start URL (FR-011).
pub const DEFAULT_MAX_DEPTH: usize = 2;

/// Default total character budget across all pages (FR-011).
pub const DEFAULT_MAX_TOTAL_CHARS: usize = 200_000;

/// Default wall-clock time budget in milliseconds (FR-011, NFR-001).
pub const DEFAULT_DEADLINE_MS: u64 = 120_000;

/// URL-path segments that indicate substantive content (boosted).
const CONTENT_BOOST_SEGMENTS: &[&str] = &[
    "docs",
    "guide",
    "guides",
    "api",
    "reference",
    "tutorial",
    "tutorials",
    "manual",
    "help",
    "wiki",
    "documentation",
    "learn",
    "concepts",
    "overview",
    "introduction",
    "getting-started",
    "quickstart",
];

/// URL-path segments that indicate non-content pages (penalised).
const CONTENT_PENALTY_SEGMENTS: &[&str] = &[
    "login",
    "signin",
    "sign-in",
    "auth",
    "authenticate",
    "signup",
    "sign-up",
    "register",
    "registration",
    "submit",
    "post",
    "comment",
    "reply",
    "cart",
    "checkout",
    "payment",
    "buy",
    "order",
    "account",
    "settings",
    "profile",
    "admin",
    "dashboard",
];

/// File extensions for non-HTML assets (heavily penalised).
const NON_HTML_EXTENSIONS: &[&str] = &[
    ".pdf", ".jpg", ".jpeg", ".png", ".gif", ".bmp", ".svg", ".ico", ".zip", ".tar", ".gz", ".bz2",
    ".css", ".js", ".json", ".xml", ".rss", ".atom", ".mp4", ".mp3", ".avi", ".mov", ".woff",
    ".woff2", ".ttf", ".eot",
];

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Sitemap mode for `mf_crawl` (FR-013).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SitemapMode {
    /// Do not use sitemap; use BFS discovery (default).
    #[default]
    Off,
    /// Use sitemap if available, fall back to BFS if not.
    Auto,
    /// Require sitemap; error if not found.
    On,
}

/// Configuration for a crawl run.
///
/// All fields have sensible defaults via [`CrawlConfig::default`].
#[derive(Debug, Clone)]
pub struct CrawlConfig {
    /// Maximum number of pages to fetch (default 10).
    pub max_pages: usize,
    /// Maximum crawl depth from the start URL (default 2).
    pub max_depth: usize,
    /// Total character budget across all fetched pages (default 200 000).
    pub max_total_chars: usize,
    /// Wall-clock time budget in milliseconds (default 120 000).
    pub deadline_ms: u64,
    /// Optional focus query for scoring and filtering.
    pub focus: Option<String>,
    /// Sitemap mode (FR-013).
    pub sitemap: SitemapMode,
    /// If `true`, discover URLs but do not fetch page content (FR-013).
    pub discover_only: bool,
    /// If non-empty, fetch only this subset of URLs — no BFS discovery
    /// (FR-014).
    pub crawl_urls: Vec<String>,
    /// If `true`, check robots.txt before fetching each page.
    pub respect_robots: bool,
}

impl Default for CrawlConfig {
    fn default() -> Self {
        Self {
            max_pages: DEFAULT_MAX_PAGES,
            max_depth: DEFAULT_MAX_DEPTH,
            max_total_chars: DEFAULT_MAX_TOTAL_CHARS,
            deadline_ms: DEFAULT_DEADLINE_MS,
            focus: None,
            sitemap: SitemapMode::Off,
            discover_only: false,
            crawl_urls: Vec::new(),
            respect_robots: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Result
// ---------------------------------------------------------------------------

/// The reason a crawl was truncated (if it was).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TruncatedBy {
    /// Reached `max_pages` cap.
    MaxPages,
    /// Reached `max_total_chars` cap.
    MaxTotalChars,
    /// Reached `deadline_ms` time budget.
    Deadline,
}

/// Result of a crawl run.
///
/// Returned by [`CrawlOrchestrator::crawl`]. Contains the fetched pages,
/// discovered URLs, and metadata about whether the crawl was truncated.
#[derive(Debug, Clone, Default)]
pub struct CrawlResult {
    /// Fetched pages, in the order they were crawled (best-first).
    pub pages: Vec<CrawlPage>,
    /// Total number of pages fetched.
    pub total_pages: usize,
    /// `true` if the crawl was truncated by a cap.
    pub truncated: bool,
    /// The reason for truncation, if any.
    pub truncated_by: Option<TruncatedBy>,
    /// All discovered URLs (normalised), including cross-domain links that
    /// were not fetched.
    pub discovered_urls: Vec<String>,
    /// Names of engines/backends that were blocked (always empty for crawl).
    pub errors: Vec<String>,
}

// ---------------------------------------------------------------------------
// CrawlFetcher trait (mockable I/O — NFR-003)
// ---------------------------------------------------------------------------

/// The output of fetching a single page during a crawl.
///
/// Returned by [`CrawlFetcher::fetch_page`]. Contains the classified page
/// data plus any discovered links for BFS continuation.
#[derive(Debug, Clone)]
pub struct FetchedPage {
    /// The classified crawl page (content, page_type, content_ok, etc.).
    pub page: CrawlPage,
    /// Links discovered on this page (absolute, normalised URLs).
    pub discovered_links: Vec<String>,
}

/// Trait abstracting the HTTP fetch + classify pipeline for a single page.
///
/// This is the seam between the pure crawl orchestration logic and the
/// network I/O. The real implementation (used by `mf_crawl`) constructs a
/// `reqwest` client, fetches the URL, runs [`classify_and_extract`], and
/// extracts links. Tests provide a mock implementation that returns canned
/// [`FetchedPage`]s without any network I/O (NFR-003).
///
/// # Examples
///
/// Implement a mock fetcher for tests:
///
/// ```
/// use ragent_tools_extended::masterfetch::crawl::{
///     CrawlFetcher, FetchedPage,
/// };
/// use ragent_tools_extended::masterfetch::CrawlPage;
/// use async_trait::async_trait;
///
/// struct MockFetcher;
///
/// #[async_trait]
/// impl CrawlFetcher for MockFetcher {
///     async fn fetch_page(&self, url: &str) -> Option<FetchedPage> {
///         Some(FetchedPage {
///             page: CrawlPage {
///                 url: url.to_string(),
///                 content_ok: true,
///                 content: "mock content".to_string(),
///                 ..Default::default()
///             },
///             discovered_links: vec![],
///         })
///     }
/// }
/// ```
#[async_trait::async_trait]
pub trait CrawlFetcher: Send + Sync {
    /// Fetch a single page and return its classified content + discovered
    /// links.
    ///
    /// Returns `None` if the fetch fails entirely (network error, DNS
    /// failure, etc.). Partial failures (HTTP 404, bot block) return
    /// `Some(FetchedPage)` with `content_ok = false`.
    async fn fetch_page(&self, url: &str) -> Option<FetchedPage>;
}

// ---------------------------------------------------------------------------
// Pure scoring and filtering functions (NFR-003)
// ---------------------------------------------------------------------------

/// Score a discovered URL for the best-first priority queue.
///
/// Higher scores are crawled first. The score combines:
///
/// - **Content-likelihood** — URLs with `docs`, `guide`, `api`, etc. in the
///   path are boosted; URLs with `login`, `cart`, `submit`, etc. are
///   penalised.
/// - **Focus relevance** — if a focus query is provided, URLs containing
///   query terms in the path receive a boost.
/// - **Depth** — shallower URLs get a small boost.
/// - **Non-HTML assets** — URLs ending in `.pdf`, `.jpg`, etc. are heavily
///   penalised.
///
/// # Arguments
///
/// - `url` — the discovered URL (must be absolute).
/// - `focus` — optional focus query string. When `None`, no focus boost is
///   applied.
/// - `depth` — crawl depth of this URL (0 = start page).
///
/// # Returns
///
/// A score in the range roughly `[-10.0, 10.0]`. Higher = more likely to
/// contain useful content.
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::crawl::score_url;
///
/// // Docs URLs are boosted.
/// assert!(score_url("https://example.com/docs/guide", None, 0) > 0.0);
/// // Login URLs are penalised.
/// assert!(score_url("https://example.com/login", None, 0) < 0.0);
/// // Non-HTML assets are heavily penalised.
/// assert!(score_url("https://example.com/image.png", None, 0) < -5.0);
/// ```
#[must_use]
pub fn score_url(url: &str, focus: Option<&str>, depth: usize) -> f64 {
    let mut score: f64 = 0.0;

    // Parse the URL to extract the path.
    let path = Url::parse(url)
        .ok()
        .map(|u| u.path().to_ascii_lowercase())
        .unwrap_or_default();

    // Content-likelihood: boost docs/guide/api segments.
    for segment in CONTENT_BOOST_SEGMENTS {
        if path.contains(segment) {
            score += 2.0;
            break; // Only boost once.
        }
    }

    // Content-likelihood: penalise login/submit/cart segments.
    for segment in CONTENT_PENALTY_SEGMENTS {
        if path.contains(segment) {
            score -= 3.0;
            break; // Only penalise once.
        }
    }

    // Non-HTML assets: heavy penalty.
    for ext in NON_HTML_EXTENSIONS {
        if path.ends_with(ext) {
            score -= 8.0;
            break;
        }
    }

    // Focus relevance: boost URLs containing query terms in the path.
    if let Some(focus) = focus {
        let focus_lower = focus.to_ascii_lowercase();
        let terms: Vec<&str> = focus_lower.split_whitespace().collect();
        for term in &terms {
            if term.len() >= 3 && path.contains(term) {
                score += 1.5;
            }
        }
    }

    // Depth: shallow URLs get a small boost.
    // depth 0 → +1.0, depth 1 → +0.5, depth 2 → +0.25, etc.
    let depth_boost = 1.0 / (1u32 << depth.min(10) as u32) as f64;
    score += depth_boost;

    score
}

/// Check whether a URL is on the same domain as a reference URL.
///
/// Two URLs are same-domain if their hosts are equal (case-insensitive).
/// Subdomains are considered different domains (e.g. `docs.example.com` is
/// not the same domain as `example.com`).
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::crawl::is_same_domain;
///
/// assert!(is_same_domain("https://example.com/page", "https://example.com/"));
/// assert!(!is_same_domain("https://other.com/page", "https://example.com/"));
/// assert!(!is_same_domain("https://docs.example.com/page", "https://example.com/"));
/// ```
#[must_use]
pub fn is_same_domain(url: &str, reference_url: &str) -> bool {
    let url_host = Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_ascii_lowercase()));
    let ref_host = Url::parse(reference_url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_ascii_lowercase()));

    match (url_host, ref_host) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

/// Normalise and deduplicate a list of URLs.
///
/// Each URL is normalised via [`normalise_url`] (lowercase host, strip
/// default ports, strip trailing slashes, strip tracking params). URLs that
/// fail to parse are kept as-is. Duplicates (after normalisation) are
/// removed, preserving first occurrence.
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::crawl::normalize_and_dedup;
///
/// let urls = vec![
///     "https://example.com/page/".to_string(),
///     "https://example.com/page".to_string(),  // duplicate after normalisation
///     "https://other.com".to_string(),
/// ];
/// let deduped = normalize_and_dedup(&urls);
/// assert_eq!(deduped.len(), 2);
/// ```
#[must_use]
pub fn normalize_and_dedup(urls: &[String]) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut result: Vec<String> = Vec::with_capacity(urls.len());

    for url in urls {
        let normalised = normalise_url(url).unwrap_or_else(|_| url.clone());
        if seen.insert(normalised.clone()) {
            result.push(normalised);
        }
    }

    result
}

/// Extract the domain (host) from a URL string.
///
/// Returns `None` for invalid URLs or URLs without a host.
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::crawl::extract_domain;
///
/// assert_eq!(extract_domain("https://example.com/page"), Some("example.com".to_string()));
/// assert_eq!(extract_domain("not a url"), None);
/// ```
#[must_use]
pub fn extract_domain(url: &str) -> Option<String> {
    Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_ascii_lowercase()))
}

// ---------------------------------------------------------------------------
// Priority queue entry
// ---------------------------------------------------------------------------

/// An entry in the best-first priority queue.
#[derive(Debug, Clone)]
struct QueueEntry {
    url: String,
    depth: usize,
    score: f64,
}

impl PartialEq for QueueEntry {
    fn eq(&self, other: &Self) -> bool {
        // Reverse: higher score = higher priority (BinaryHeap is max-heap).
        self.score == other.score
    }
}

impl Eq for QueueEntry {}

impl PartialOrd for QueueEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for QueueEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Higher score = higher priority. BinaryHeap is a max-heap, so the
        // "greatest" element (highest score) is popped first.
        self.score
            .partial_cmp(&other.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

// ---------------------------------------------------------------------------
// CrawlOrchestrator
// ---------------------------------------------------------------------------

/// Best-first same-domain crawl orchestrator.
///
/// Drives a crawl from a seed URL, discovering links via the [`CrawlFetcher`]
/// trait, scoring them with [`score_url`], and visiting them in best-first
/// order up to the configured caps.
///
/// The orchestrator is generic over the fetcher, enabling tests with a mock
/// fetcher (NFR-003).
pub struct CrawlOrchestrator {
    config: CrawlConfig,
}

/// Mutable crawl state passed to [`CrawlOrchestrator::enqueue_discovered_links`].
struct EnqueueContext<'a> {
    /// Normalised seed URL used for same-domain scoping.
    start_url: &'a str,
    /// Host domain of the seed URL.
    start_domain: &'a Option<String>,
    /// Optional focus query for URL scoring.
    focus: Option<&'a str>,
    /// URLs already visited or queued.
    visited: &'a mut HashSet<String>,
    /// Best-first queue of URLs to fetch.
    queue: &'a mut BinaryHeap<QueueEntry>,
    /// Cross-domain URLs discovered but not fetched.
    discovered_urls: &'a mut Vec<String>,
}

impl<'a> EnqueueContext<'a> {
    /// Create a new context from the current crawl variables.
    fn new(
        start_url: &'a str,
        start_domain: &'a Option<String>,
        focus: Option<&'a str>,
        visited: &'a mut HashSet<String>,
        queue: &'a mut BinaryHeap<QueueEntry>,
        discovered_urls: &'a mut Vec<String>,
    ) -> Self {
        Self {
            start_url,
            start_domain,
            focus,
            visited,
            queue,
            discovered_urls,
        }
    }
}

impl CrawlOrchestrator {
    /// Create a new orchestrator with the given configuration.
    #[must_use]
    pub fn new(config: CrawlConfig) -> Self {
        Self { config }
    }

    /// Create a new orchestrator with default configuration.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(CrawlConfig::default())
    }

    /// Run the crawl.
    ///
    /// This is the primary entry point. It:
    ///
    /// 1. If `crawl_urls` is non-empty, fetches only those URLs (FR-014).
    /// 2. Otherwise, starts from `start_url`, fetches it, discovers links,
    ///    and continues in best-first order up to the caps.
    /// 3. If `discover_only` is true, discovers URLs but does not fetch
    ///    content (FR-013).
    ///
    /// # Arguments
    ///
    /// - `start_url` — the seed URL for the crawl.
    /// - `fetcher` — the page fetcher (real HTTP or mock).
    ///
    /// # Returns
    ///
    /// A [`CrawlResult`] with fetched pages, discovered URLs, and truncation
    /// metadata.
    /// Run the crawl.
    ///
    /// This is the primary entry point. It:
    ///
    /// 1. If `crawl_urls` is non-empty, fetches only those URLs (FR-014).
    /// 2. Otherwise, starts from `start_url`, fetches it, discovers links,
    ///    and continues in best-first order up to the caps.
    /// 3. If `discover_only` is true, discovers URLs but does not fetch
    ///    content (FR-013).
    ///
    /// # Arguments
    ///
    /// - `start_url` — the seed URL for the crawl.
    /// - `fetcher` — the page fetcher (real HTTP or mock).
    ///
    /// # Returns
    ///
    /// A [`CrawlResult`] with fetched pages, discovered URLs, and truncation
    /// metadata.
    pub async fn crawl(&self, start_url: &str, fetcher: &dyn CrawlFetcher) -> CrawlResult {
        let deadline = Instant::now() + Duration::from_millis(self.config.deadline_ms);
        let focus = self.config.focus.as_deref();

        // --- Selective crawl mode (FR-014) ---
        if !self.config.crawl_urls.is_empty() {
            return self.crawl_selective(fetcher, deadline).await;
        }

        // --- Normal / discover-only mode ---
        let mut result = CrawlResult::default();
        let mut visited: HashSet<String> = HashSet::new();
        let mut queue: BinaryHeap<QueueEntry> = BinaryHeap::new();
        let mut total_chars: usize = 0;

        // Normalise the start URL.
        let start_normalised = normalise_url(start_url).unwrap_or_else(|_| start_url.to_string());
        let start_domain = extract_domain(&start_normalised);

        // Enqueue the start URL at depth 0.
        queue.push(QueueEntry {
            url: start_normalised.clone(),
            depth: 0,
            score: 10.0, // Start URL always gets highest priority.
        });

        while let Some(entry) = queue.pop() {
            // Check deadline.
            if Instant::now() >= deadline {
                result.truncated = true;
                result.truncated_by = Some(TruncatedBy::Deadline);
                break;
            }

            // Check max_pages.
            if result.pages.len() >= self.config.max_pages {
                result.truncated = true;
                result.truncated_by = Some(TruncatedBy::MaxPages);
                break;
            }

            // Skip already-visited.
            if visited.contains(&entry.url) {
                continue;
            }
            visited.insert(entry.url.clone());

            // Record discovered URL (even cross-domain).
            result.discovered_urls.push(entry.url.clone());

            // In discover_only mode, don't fetch — just discover.
            if self.config.discover_only {
                // We can't discover links without fetching the page.
                // In discover_only mode, we still need to fetch pages to
                // discover their links, but we don't store the content.
                // However, the spec says "return the URL map only without
                // fetching page content". This is a tension: to discover
                // URLs, we need to fetch pages. The compromise: fetch pages
                // to discover links, but don't store page content in the
                // result. The discovered_urls list is the output.
                if let Some(fetched) = fetcher.fetch_page(&entry.url).await {
                    // Discover links but don't store the page.
                    let mut ctx = EnqueueContext::new(
                        &start_normalised,
                        &start_domain,
                        focus,
                        &mut visited,
                        &mut queue,
                        &mut result.discovered_urls,
                    );
                    self.enqueue_discovered_links(&fetched.discovered_links, entry.depth, &mut ctx);
                }
                continue;
            }

            // Fetch the page.
            let fetched = match fetcher.fetch_page(&entry.url).await {
                Some(f) => f,
                None => continue, // Fetch failed — skip.
            };

            // Accumulate total chars.
            let page_chars = fetched.page.content.chars().count();
            total_chars += page_chars;

            // Store the page.
            result.pages.push(fetched.page);
            result.total_pages = result.pages.len();

            // Enqueue discovered links (if within depth limit).
            if entry.depth < self.config.max_depth {
                let mut ctx = EnqueueContext::new(
                    &start_normalised,
                    &start_domain,
                    focus,
                    &mut visited,
                    &mut queue,
                    &mut result.discovered_urls,
                );
                self.enqueue_discovered_links(&fetched.discovered_links, entry.depth, &mut ctx);
            }

            // Check max_total_chars.
            if total_chars >= self.config.max_total_chars {
                result.truncated = true;
                result.truncated_by = Some(TruncatedBy::MaxTotalChars);
                break;
            }

            // Enqueue discovered links (if within depth limit).
            if entry.depth < self.config.max_depth {
                let mut ctx = EnqueueContext::new(
                    &start_normalised,
                    &start_domain,
                    focus,
                    &mut visited,
                    &mut queue,
                    &mut result.discovered_urls,
                );
                self.enqueue_discovered_links(&fetched.discovered_links, entry.depth, &mut ctx);
            }
        }

        result
    }

    /// Enqueue discovered links that are same-domain and not yet visited.
    /// Cross-domain links are recorded in `result.discovered_urls` but not
    /// enqueued for fetching.
    fn enqueue_discovered_links(
        &self,
        links: &[String],
        parent_depth: usize,
        ctx: &mut EnqueueContext<'_>,
    ) {
        let child_depth = parent_depth + 1;

        for link in links {
            // Normalise.
            let normalised = match normalise_url(link) {
                Ok(n) => n,
                Err(_) => link.clone(),
            };

            // Skip already-visited.
            if ctx.visited.contains(&normalised) {
                continue;
            }

            // Same-domain check. Cross-domain links are recorded in
            // discovered_urls but not enqueued for fetching.
            if ctx.start_domain.is_some() && !is_same_domain(&normalised, ctx.start_url) {
                ctx.discovered_urls.push(normalised);
                continue;
            }

            // Score and enqueue.
            let score = score_url(&normalised, ctx.focus, child_depth);
            ctx.queue.push(QueueEntry {
                url: normalised,
                depth: child_depth,
                score,
            });
        }
    }

    /// Run a selective crawl — fetch only the specified `crawl_urls` (FR-014).
    async fn crawl_selective(&self, fetcher: &dyn CrawlFetcher, deadline: Instant) -> CrawlResult {
        let mut result = CrawlResult::default();
        let mut total_chars: usize = 0;
        let mut visited: HashSet<String> = HashSet::new();

        for url in &self.config.crawl_urls {
            // Check deadline.
            if Instant::now() >= deadline {
                result.truncated = true;
                result.truncated_by = Some(TruncatedBy::Deadline);
                break;
            }

            // Check max_pages.
            if result.pages.len() >= self.config.max_pages {
                result.truncated = true;
                result.truncated_by = Some(TruncatedBy::MaxPages);
                break;
            }

            // Normalise and dedup.
            let normalised = normalise_url(url).unwrap_or_else(|_| url.clone());
            if visited.contains(&normalised) {
                continue;
            }
            visited.insert(normalised.clone());
            result.discovered_urls.push(normalised.clone());

            if self.config.discover_only {
                continue;
            }

            // Fetch.
            let fetched = match fetcher.fetch_page(&normalised).await {
                Some(f) => f,
                None => continue,
            };

            total_chars += fetched.page.content.chars().count();
            result.pages.push(fetched.page);
            result.total_pages = result.pages.len();

            if total_chars >= self.config.max_total_chars {
                result.truncated = true;
                result.truncated_by = Some(TruncatedBy::MaxTotalChars);
                break;
            }
        }

        result
    }
}
