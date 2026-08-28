//! Envelope signal computation for the masterfetch toolset.
//!
//! Implements **FR-003**, **FR-029**, **FR-030**, and **NFR-003** (T-008).
//!
//! This module computes the Hound v10 "envelope" signals that make every fetch
//! and crawl response actionable:
//!
//! - **Page-type detection** ([`detect_page_type`]) — classifies the HTML
//!   structure into [`PageType`] variants (article, docs, list, forum, qa,
//!   `js_shell`, `auth_wall`, paywall, redirect, image, json, unknown). Drives the
//!   `next_action` suggestion (FR-029).
//! - **Source-authority classification** ([`classify_source_type`]) — maps the
//!   URL domain to [`SourceType`] (gov, edu, github, `vendor_docs`, `docs_site`,
//!   qa, forum, blog, news, ecommerce, unknown) and computes `is_official`
//!   (FR-030).
//! - **Freshness computation** ([`compute_freshness`]) — parses the page's
//!   published/modified date metadata, prefers modified over published,
//!   computes `content_age_days` and `is_stale` (stale = age > 365 days)
//!   (FR-030).
//! - **Envelope assembly** ([`build_envelope`]) — combines all three signals
//!   plus `content_ok` into an [`EnvelopeSignals`] struct ready for embedding
//!   in `ToolOutput.metadata` (FR-003).
//!
//! # Testability (NFR-003)
//!
//! All functions are pure — no network I/O. They take HTML strings, URLs, and
//! [`PageMetadata`] by reference and return plain structs. This enables unit
//! tests with fixture HTML without any live pages.
//!
//! # Examples
//!
//! ```
//! use ragent_tools_extended::masterfetch::envelope::{build_envelope, detect_page_type};
//! use ragent_tools_extended::masterfetch::PageMetadata;
//!
//! let html = r#"<html><body><article><p>Hello world this is a long article with enough text to pass thresholds.</p></article></body></html>"#;
//! let metadata = PageMetadata::default();
//! let envelope = build_envelope(html, "https://example.com/blog/post", &metadata, true, 200);
//! assert_eq!(envelope.content_ok, true);
//! ```

use chrono::{DateTime, Utc};

use super::{EnvelopeSignals, PageMetadata, PageType, SourceType};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Content age (in days) above which a page is considered stale (FR-030).
pub const STALE_THRESHOLD_DAYS: i64 = 365;

/// Minimum extracted text length for a page to be considered an article
/// (rather than a JS shell or list page).
const MIN_ARTICLE_TEXT_CHARS: usize = 200;

/// Minimum body bytes for a JS shell diagnosis (body is large but text is
/// tiny). Below this, a low-text page is just "unknown".
const JS_SHELL_MIN_BODY_BYTES: usize = 3000;

/// Maximum extracted text length for a JS shell diagnosis.
const JS_SHELL_MAX_TEXT_CHARS: usize = 200;

/// Signal phrases that indicate a JavaScript-rendered shell (FR-029).
const JS_SHELL_SIGNALS: &[&str] = &[
    "enable javascript",
    "you need to enable javascript",
    "javascript is required",
    "javascript is disabled",
    "javascript to run this app",
    "javascript must be enabled",
    "please enable javascript",
    "requires javascript",
    "we've detected that javascript is disabled",
    "javascript is disabled in this browser",
    "enable javascript to run this app",
    "please enable js",
    "enable js",
];

/// Signal phrases that indicate a login / authentication wall (FR-029).
const AUTH_WALL_SIGNALS: &[&str] = &[
    "sign in",
    "log in",
    "login required",
    "please log in",
    "please sign in",
    "you must be logged in",
    "authentication required",
    "sign in to continue",
    "log in to continue",
];

/// Signal phrases that indicate a paywall (FR-029).
const PAYWALL_SIGNALS: &[&str] = &[
    "subscribe to continue",
    "subscribe to read",
    "subscription required",
    "paywall",
    "premium content",
    "unlock full article",
    "subscribe now",
    "already a subscriber",
    "sign up to continue reading",
    "this content is for subscribers",
];

/// HTML indicators of a redirect page (FR-029).
const REDIRECT_INDICATORS: &[&str] = &["http-equiv=\"refresh\"", "http-equiv='refresh'"];

/// Known Q&A site domains (FR-030).
const QA_DOMAINS: &[&str] = &[
    "stackoverflow.com",
    "stackexchange.com",
    "serverfault.com",
    "superuser.com",
    "askubuntu.com",
    "mathoverflow.net",
    "quora.com",
    "answers.com",
    "experts-exchange.com",
];

/// Known forum site domains (FR-030).
const FORUM_DOMAINS: &[&str] = &[
    "reddit.com",
    "old.reddit.com",
    "discourse.org",
    "discourse.com",
    "forum.nginx.org",
    "forums.python.org",
    "www.linuxquestions.org",
    "community.swift.org",
    "forums.swift.org",
    "community.rust-lang.org",
    "users.rust-lang.org",
    "elixirforum.com",
    "forum.djangoproject.com",
    "groups.google.com",
];

/// Known blog platform domains (FR-030).
const BLOG_DOMAINS: &[&str] = &[
    "medium.com",
    "substack.com",
    "blogspot.com",
    "wordpress.com",
    "tumblr.com",
    "dev.to",
    "hashnode.com",
    "hackernoon.com",
    "freecodecamp.org",
];

/// Known news organisation domains (FR-030).
const NEWS_DOMAINS: &[&str] = &[
    "cnn.com",
    "bbc.com",
    "bbc.co.uk",
    "nytimes.com",
    "reuters.com",
    "apnews.com",
    "theguardian.com",
    "washingtonpost.com",
    "bloomberg.com",
    "ft.com",
    "wsj.com",
    "forbes.com",
    "techcrunch.com",
    "arstechnica.com",
    "theverge.com",
    "wired.com",
    "aljazeera.com",
    "npr.org",
    "economist.com",
    "politico.com",
];

/// Known e-commerce domains (FR-030).
const ECOMMERCE_DOMAINS: &[&str] = &[
    "amazon.com",
    "ebay.com",
    "etsy.com",
    "shopify.com",
    "aliexpress.com",
    "walmart.com",
    "target.com",
    "bestbuy.com",
    "alibaba.com",
];

/// Known vendor documentation domains (FR-030).
const VENDOR_DOCS_DOMAINS: &[&str] = &[
    "docs.microsoft.com",
    "learn.microsoft.com",
    "developer.android.com",
    "developers.google.com",
    "cloud.google.com",
    "developer.mozilla.org",
    "developer.apple.com",
    "docs.aws.amazon.com",
    "docs.python.org",
    "docs.rs",
    "doc.rust-lang.org",
    "kubernetes.io",
    "nodejs.org",
];

/// Domain prefixes that indicate a documentation site (FR-030).
const DOCS_SITE_PREFIXES: &[&str] = &["docs.", "developer.", "developers.", "documentation."];

// ---------------------------------------------------------------------------
// Page-type detection (FR-029)
// ---------------------------------------------------------------------------

/// Detect the page type from its HTML structure and content metrics.
///
/// This is a pure function that examines the HTML for structural signals and
/// uses the provided text/body metrics to distinguish between articles, lists,
/// JS shells, auth walls, paywalls, and redirects.
///
/// # Arguments
///
/// - `html` — the raw HTML response body.
/// - `url` — the final URL (after redirects), used for JSON content-type
///   inference and domain-based heuristics.
/// - `extracted_text_length` — the length of the extracted text content (after
///   readability/html2text processing). Used to distinguish JS shells (large
///   HTML, tiny text) from real articles.
///
/// # Returns
///
/// A [`PageType`] variant. Never panics — malformed HTML produces
/// [`PageType::Unknown`].
///
/// # Detection order
///
/// 1. **JSON** — body starts with `{` or `[` and looks like JSON.
/// 2. **Redirect** — `<meta http-equiv="refresh">` present.
/// 3. **Auth wall** — login form signals in the first 5000 chars.
/// 4. **Paywall** — subscription signals in the first 5000 chars.
/// 5. **JS shell** — large body + tiny text + JS-required signals.
/// 6. **Article** — `<article>` tag or substantial text with low link density.
/// 7. **Docs** — documentation signals (code blocks, `<nav>` with many links).
/// 8. **List** — high link density, many `<a>` tags relative to text.
/// 9. **Forum** — forum/thread signals or forum domain.
/// 10. **QA** — Q&A signals or Q&A domain.
/// 11. **Image** — page is mostly a single `<img>`.
/// 12. **Unknown** — fallback.
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::envelope::detect_page_type;
/// use ragent_tools_extended::masterfetch::PageType;
///
/// let html = r#"<html><body><article><p>Article text here that is long enough.</p></article></body></html>"#;
/// assert_eq!(detect_page_type(html, "https://example.com/post", 200), PageType::Article);
///
/// // JS shell: large body (>3KB) with tiny text and a JS-required signal.
/// let js_shell = format!("<html><body>Please enable JavaScript to run this app.{}</body></html>", "x".repeat(4000));
/// assert_eq!(detect_page_type(&js_shell, "https://app.example.com", 20), PageType::JsShell);
/// ```
#[must_use]
pub fn detect_page_type(html: &str, url: &str, extracted_text_length: usize) -> PageType {
    let html_lower = html.to_ascii_lowercase();
    let body_bytes = html.len();

    // 1. JSON — body starts with { or [ (after optional whitespace).
    let trimmed = html.trim_start();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        // Quick sanity: try to verify it looks like JSON (not just HTML starting
        // with those chars, which is rare but possible).
        if !html_lower.contains("</html>") {
            return PageType::Json;
        }
    }

    // 2. Redirect — meta refresh.
    for indicator in REDIRECT_INDICATORS {
        if html_lower.contains(indicator) {
            // Meta refresh with a very short delay (0-3 seconds) is a redirect.
            // Longer delays are just auto-refresh.
            if let Some(content_attr) = find_meta_refresh_content(&html_lower) {
                if parse_meta_refresh_seconds(&content_attr) <= 3 {
                    return PageType::Redirect;
                }
            } else {
                return PageType::Redirect;
            }
        }
    }

    // 3. Auth wall — login signals in the first 5000 chars (where the
    //    above-the-fold content would be).
    let head = &html_lower[..head_len(html_lower.as_str(), 5000)];
    for signal in AUTH_WALL_SIGNALS {
        if head.contains(signal) {
            // Only classify as auth wall if the text content is short (a login
            // page doesn't have article-length content).
            if extracted_text_length < MIN_ARTICLE_TEXT_CHARS {
                return PageType::AuthWall;
            }
        }
    }

    // 4. Paywall — subscription signals.
    for signal in PAYWALL_SIGNALS {
        if head.contains(signal) {
            // Paywall pages may have article preview text, but the signals are
            // strong enough to classify even with some content.
            return PageType::Paywall;
        }
    }

    // 5. JS shell — large body + tiny text + JS-required signals.
    if body_bytes >= JS_SHELL_MIN_BODY_BYTES && extracted_text_length < JS_SHELL_MAX_TEXT_CHARS {
        for signal in JS_SHELL_SIGNALS {
            if html_lower.contains(signal) {
                return PageType::JsShell;
            }
        }
        // Also detect JS shell without explicit signals: very large body but
        // almost no extracted text.
        if body_bytes >= JS_SHELL_MIN_BODY_BYTES * 3 && extracted_text_length < 50 {
            return PageType::JsShell;
        }
    }

    // 6. Article — <article> tag with substantial text.
    if html_lower.contains("<article") && extracted_text_length >= MIN_ARTICLE_TEXT_CHARS {
        return PageType::Article;
    }

    // 7. Docs — documentation signals.
    if is_docs_page(&html_lower, url) {
        return PageType::Docs;
    }

    // 8. List — high link density.
    if is_list_page(&html_lower, extracted_text_length) {
        return PageType::List;
    }

    // 9. Forum — forum signals or forum domain.
    if is_forum_page(&html_lower, url) {
        return PageType::Forum;
    }

    // 10. QA — Q&A signals or Q&A domain.
    if is_qa_page(&html_lower, url) {
        return PageType::Qa;
    }

    // 11. Article fallback — substantial text without other signals.
    if extracted_text_length >= MIN_ARTICLE_TEXT_CHARS {
        // Check for article-like structure: paragraphs.
        let p_count = html_lower.matches("<p").count();
        if p_count >= 3 {
            return PageType::Article;
        }
    }

    // 12. Image — page is mostly a single <img>.
    let img_count = html_lower.matches("<img").count();
    if img_count == 1 && extracted_text_length < 100 {
        return PageType::Image;
    }

    PageType::Unknown
}

/// Extract the `content` attribute value from a `<meta http-equiv="refresh"
/// content="...">` tag.
fn find_meta_refresh_content(html_lower: &str) -> Option<String> {
    let meta_start = html_lower.find("<meta")?;
    let meta_end = html_lower[meta_start..].find('>')? + meta_start;
    let meta_tag = &html_lower[meta_start..meta_end];
    let content_pos = meta_tag.find("content=")?;
    let after_content = &meta_tag[content_pos + "content=".len()..];
    let quote = after_content.chars().next()?;
    if quote != '"' && quote != '\'' {
        // Unquoted value — read until whitespace.
        let end = after_content
            .find(|c: char| c.is_whitespace())
            .unwrap_or(after_content.len());
        return Some(after_content[..end].to_string());
    }
    let rest = &after_content[1..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

/// Parse the seconds value from a meta refresh `content` attribute.
///
/// Format: `N;url=...` or just `N`. Returns 999 if parsing fails (treat as
/// non-redirect).
#[must_use]
pub fn parse_meta_refresh_seconds(content: &str) -> u64 {
    let semicolon_pos = content.find(';').unwrap_or(content.len());
    let num_str = &content[..semicolon_pos];
    num_str.trim().parse::<u64>().unwrap_or(999)
}

/// Check if the page looks like a documentation page.
fn is_docs_page(html_lower: &str, url: &str) -> bool {
    // Code blocks are a strong docs signal.
    let code_blocks = html_lower.matches("<pre").count() + html_lower.matches("<code").count();
    if code_blocks >= 3 {
        return true;
    }

    // Documentation domains.
    let host = extract_host(url);
    if let Some(h) = host {
        for domain in VENDOR_DOCS_DOMAINS {
            if h == *domain || h.ends_with(&format!(".{domain}")) {
                return true;
            }
        }
        for prefix in DOCS_SITE_PREFIXES {
            if h.starts_with(prefix) {
                return true;
            }
        }
    }

    false
}

/// Check if the page looks like a list / index page (high link density).
fn is_list_page(html_lower: &str, extracted_text_length: usize) -> bool {
    let link_count = html_lower.matches("<a ").count() + html_lower.matches("<a>").count();
    if link_count < 10 {
        return false;
    }

    // High link density: many links relative to text content.
    if extracted_text_length < 500 && link_count >= 20 {
        return true;
    }

    // List/table structure.
    let li_count = html_lower.matches("<li").count();
    if li_count >= 15 && link_count >= 10 {
        return true;
    }

    // Table with many rows.
    let tr_count = html_lower.matches("<tr").count();
    tr_count >= 10
}

/// Check if the page looks like a forum thread.
fn is_forum_page(html_lower: &str, url: &str) -> bool {
    // Forum structural signals.
    let forum_signals = [
        "<div class=\"post\"",
        "class=\"thread",
        "class=\"forum-post",
        "class=\"reply",
        "class=\"comment",
        "data-post-id",
    ];
    for signal in forum_signals {
        if html_lower.contains(signal) {
            return true;
        }
    }

    // Forum domains.
    let host = extract_host(url);
    if let Some(h) = host {
        for domain in FORUM_DOMAINS {
            if h == *domain || h.ends_with(&format!(".{domain}")) {
                return true;
            }
        }
    }

    false
}

/// Check if the page looks like a Q&A page.
fn is_qa_page(html_lower: &str, url: &str) -> bool {
    // Q&A structural signals.
    let qa_signals = [
        "class=\"question",
        "class=\"answer",
        "class=\"qa",
        "class=\"accepted-answer",
        "id=\"question",
        "itemtype=\"https://schema.org/Question",
    ];
    for signal in qa_signals {
        if html_lower.contains(signal) {
            return true;
        }
    }

    // Q&A domains.
    let host = extract_host(url);
    if let Some(h) = host {
        for domain in QA_DOMAINS {
            if h == *domain || h.ends_with(&format!(".{domain}")) {
                return true;
            }
        }
    }

    false
}

/// Safely get the first `n` bytes of a string (or the whole string if shorter).
/// Return a byte index ≤ `n` that is also a valid UTF-8 char boundary.
///
/// `str::len()` counts **bytes**, so `&s[..min(s.len(), n)]` can panic when
/// byte index `n` lands inside a multi-byte UTF-8 sequence. This helper walks
/// backwards from `n` until it finds a char boundary, guaranteeing that
/// `&s[..head_len(s, n)]` never panics.
fn head_len(s: &str, n: usize) -> usize {
    let target = s.len().min(n);
    if target >= s.len() {
        return s.len();
    }
    let mut idx = target;
    while !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

// ---------------------------------------------------------------------------
// Source-authority classification (FR-030)
// ---------------------------------------------------------------------------

/// Classify the source authority of a URL's domain.
///
/// Returns the [`SourceType`] and `is_official` flag. `is_official` is `true`
/// only on strong signals: government, education, GitHub, or vendor
/// documentation sites (FR-030).
///
/// # Arguments
///
/// - `url` — the page URL (final URL after redirects).
///
/// # Returns
///
/// A tuple of (`SourceType`, `is_official`).
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::envelope::classify_source_type;
/// use ragent_tools_extended::masterfetch::SourceType;
///
/// let (st, official) = classify_source_type("https://www.gov.uk/policy");
/// assert_eq!(st, SourceType::Gov);
/// assert!(official);
///
/// let (st, official) = classify_source_type("https://medium.com/@user/post");
/// assert_eq!(st, SourceType::Blog);
/// assert!(!official);
/// ```
#[must_use]
pub fn classify_source_type(url: &str) -> (SourceType, bool) {
    let Some(host) = extract_host(url) else {
        return (SourceType::Unknown, false);
    };

    // Government (.gov).
    #[allow(clippy::case_sensitive_file_extension_comparisons)]
    if host.eq_ignore_ascii_case("gov") || host.ends_with(".gov") {
        return (SourceType::Gov, true);
    }
    // UK government.
    if host == "www.gov.uk" || host.ends_with(".gov.uk") {
        return (SourceType::Gov, true);
    }
    // US government.
    if host.ends_with(".gov.us") {
        return (SourceType::Gov, true);
    }

    // Education (.edu).
    #[allow(clippy::case_sensitive_file_extension_comparisons)]
    if host.eq_ignore_ascii_case("edu") || host.ends_with(".edu") {
        return (SourceType::Edu, true);
    }
    if host.ends_with(".edu.cn") || host.ends_with(".edu.au") || host.ends_with(".ac.uk") {
        return (SourceType::Edu, true);
    }

    // GitHub.
    if host == "github.com" || host == "gist.github.com" || host.ends_with(".github.io") {
        return (SourceType::Github, true);
    }

    // Vendor documentation sites.
    for domain in VENDOR_DOCS_DOMAINS {
        if host == *domain || host.ends_with(&format!(".{domain}")) {
            return (SourceType::VendorDocs, true);
        }
    }

    // Generic documentation site (docs.*, developer.*).
    for prefix in DOCS_SITE_PREFIXES {
        if host.starts_with(prefix) {
            return (SourceType::DocsSite, false);
        }
    }

    // Q&A sites.
    for domain in QA_DOMAINS {
        if host == *domain || host.ends_with(&format!(".{domain}")) {
            return (SourceType::Qa, false);
        }
    }

    // Forum sites.
    for domain in FORUM_DOMAINS {
        if host == *domain || host.ends_with(&format!(".{domain}")) {
            return (SourceType::Forum, false);
        }
    }

    // Blog platforms.
    for domain in BLOG_DOMAINS {
        if host == *domain || host.ends_with(&format!(".{domain}")) {
            return (SourceType::Blog, false);
        }
    }

    // News organisations.
    for domain in NEWS_DOMAINS {
        if host == *domain || host.ends_with(&format!(".{domain}")) {
            return (SourceType::News, false);
        }
    }

    // E-commerce.
    for domain in ECOMMERCE_DOMAINS {
        if host == *domain || host.ends_with(&format!(".{domain}")) {
            return (SourceType::Ecommerce, false);
        }
    }
    // Generic shop.* / store.* domains.
    if host.starts_with("shop.") || host.starts_with("store.") {
        return (SourceType::Ecommerce, false);
    }

    (SourceType::Unknown, false)
}

// ---------------------------------------------------------------------------
// Freshness computation (FR-030)
// ---------------------------------------------------------------------------

/// Compute freshness signals from page metadata.
///
/// Parses the `published_time` and `modified_time` fields from
/// [`PageMetadata`], preferring the modified date over the published date.
/// Computes `content_age_days` (age in days from now) and `is_stale` (true
/// when age > [`STALE_THRESHOLD_DAYS`]).
///
/// # Arguments
///
/// - `metadata` — the page's structured metadata (may have `published_time`
///   and/or `modified_time` as ISO 8601 strings).
///
/// # Returns
///
/// A tuple of (`content_age_days`, `is_stale`):
///
/// - `content_age_days` — number of days since the content date. `-1` when no
///   date is recoverable or the date is in the future.
/// - `is_stale` — `true` when `content_age_days > 365`.
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::envelope::compute_freshness;
/// use ragent_tools_extended::masterfetch::PageMetadata;
///
/// // No dates → content_age_days = -1.
/// let metadata = PageMetadata::default();
/// let (age, stale) = compute_freshness(&metadata);
/// assert_eq!(age, -1);
/// assert!(!stale);
/// ```
#[must_use]
pub fn compute_freshness(metadata: &PageMetadata) -> (i64, bool) {
    // Prefer modified_time over published_time.
    let date_str = metadata
        .modified_time
        .as_deref()
        .filter(|s| !s.is_empty())
        .or_else(|| metadata.published_time.as_deref().filter(|s| !s.is_empty()));

    let Some(date_str) = date_str else {
        return (-1, false);
    };

    let Some(page_date) = parse_iso_date(date_str) else {
        return (-1, false);
    };

    let now = Utc::now();

    // Future date → -1 (FR-030).
    if page_date > now {
        return (-1, false);
    }

    let duration = now - page_date;
    let age_days = duration.num_days();

    // Guard against negative age (shouldn't happen after the future check, but
    // timezone edge cases could produce it).
    if age_days < 0 {
        return (-1, false);
    }

    let is_stale = age_days > STALE_THRESHOLD_DAYS;
    (age_days, is_stale)
}

/// Parse an ISO 8601 date string into a `DateTime<Utc>`.
///
/// Handles common formats produced by `OpenGraph`, JSON-LD, and HTML meta tags:
///
/// - `2024-01-15T10:30:00Z` (RFC 3339 / ISO 8601 with Z)
/// - `2024-01-15T10:30:00+00:00` (with explicit offset)
/// - `2024-01-15T10:30:00` (no timezone → assumed UTC)
/// - `2024-01-15 10:30:00` (space separator)
/// - `2024-01-15` (date only)
#[must_use]
pub fn parse_iso_date(s: &str) -> Option<DateTime<Utc>> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // Try full RFC 3339 first (most common for og:article:published_time).
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }

    // Try date-time without timezone (assume UTC).
    // Replace space separator with T if needed.
    let normalized = s.replace(' ', "T");

    // Try parsing as naive date-time, then assume UTC.
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(&normalized, "%Y-%m-%dT%H:%M:%S") {
        return Some(naive.and_utc());
    }

    // Try with fractional seconds.
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(&normalized, "%Y-%m-%dT%H:%M:%S%.f") {
        return Some(naive.and_utc());
    }

    // Try date only.
    if let Ok(naive) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Some(naive.and_hms_opt(0, 0, 0)?.and_utc());
    }

    None
}

// ---------------------------------------------------------------------------
// Envelope assembly (FR-003)
// ---------------------------------------------------------------------------

/// Build a complete [`EnvelopeSignals`] struct from the page's HTML, URL,
/// metadata, and content status.
///
/// This is the primary entry point for envelope computation. It combines:
///
/// - [`detect_page_type`] → `page_type`
/// - [`classify_source_type`] → `source_type`, `is_official`
/// - [`compute_freshness`] → `content_age_days`, `is_stale`
/// - The caller-supplied `content_ok` flag
/// - A computed `next_action` suggestion based on the page type
/// - A computed `summary` (empty by default; the caller can populate it)
///
/// # Arguments
///
/// - `html` — the raw HTML response body (used for page-type detection).
/// - `url` ��� the final URL after redirects (used for source classification).
/// - `metadata` — the page's structured metadata (used for freshness).
/// - `content_ok` — `true` if usable content was extracted.
/// - `extracted_text_length` — the length of the extracted text (used for
///   page-type detection).
///
/// # Returns
///
/// An [`EnvelopeSignals`] struct with all fields populated.
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::envelope::build_envelope;
/// use ragent_tools_extended::masterfetch::PageMetadata;
///
/// let html = r#"<html><body><article><p>Article text here.</p></article></body></html>"#;
/// let metadata = PageMetadata::default();
/// let envelope = build_envelope(html, "https://example.com/post", &metadata, true, 200);
/// assert!(envelope.content_ok);
/// ```
#[must_use]
pub fn build_envelope(
    html: &str,
    url: &str,
    metadata: &PageMetadata,
    content_ok: bool,
    extracted_text_length: usize,
) -> EnvelopeSignals {
    let page_type = detect_page_type(html, url, extracted_text_length);
    let (source_type, is_official) = classify_source_type(url);
    let (content_age_days, is_stale) = compute_freshness(metadata);
    let next_action = compute_next_action(page_type, content_ok);

    EnvelopeSignals {
        page_type,
        source_type,
        is_official,
        content_age_days,
        is_stale,
        content_ok,
        next_action,
        summary: String::new(),
    }
}

/// Compute a `next_action` suggestion based on the page type and content status.
///
/// The suggestion tells the agent what to do next (e.g. fetch linked URLs for
/// list pages, switch sources for auth walls, try another URL for JS shells).
fn compute_next_action(page_type: PageType, content_ok: bool) -> String {
    if !content_ok {
        return match page_type {
            PageType::JsShell => {
                "page is JavaScript-rendered; try a different source or use a browser".to_string()
            }
            PageType::AuthWall => "page requires login; switch to a different source".to_string(),
            PageType::Paywall => "page is paywalled; switch to a free source".to_string(),
            PageType::Redirect => {
                "page redirected; follow the redirect or fetch the final URL".to_string()
            }
            _ => "content extraction failed; try a different URL or source".to_string(),
        };
    }

    match page_type {
        PageType::List => "fetch the linked URLs that match your goal".to_string(),
        PageType::AuthWall => "page may require login; content may be incomplete".to_string(),
        PageType::Paywall => "page may be paywalled; content may be truncated".to_string(),
        PageType::JsShell => "page content may be incomplete (JS-rendered)".to_string(),
        PageType::Redirect => "content was fetched from the redirect target".to_string(),
        _ => String::new(),
    }
}

// ---------------------------------------------------------------------------
// URL helpers
// ---------------------------------------------------------------------------

/// Extract the lowercased host from a URL string.
///
/// Returns `None` for invalid URLs or URLs without a host.
fn extract_host(url: &str) -> Option<String> {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_ascii_lowercase))
}

// ---------------------------------------------------------------------------
// Tests — see tests/test_mf_envelope.rs (T-033, NFR-003)
// ---------------------------------------------------------------------------
