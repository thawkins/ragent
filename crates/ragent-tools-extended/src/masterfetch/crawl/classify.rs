//! Content-adaptive page classification and extraction for `mf_crawl`.
//!
//! Implements FR-012, FR-029, and NFR-003.
//!
//! This module is the heart of the crawl pipeline's content-adaptive
//! extraction. When `mf_crawl` fetches a page, it calls
//! [`classify_and_extract`] to determine the page type and extract content
//! accordingly:
//!
//! - **Article / Docs** → main content via the readability → html2text → raw
//!   text extraction chain ([`crate::masterfetch::extractor`]).
//! - **List / Index** → a structured `* [title](url)` link list built from
//!   classified outgoing links ([`crate::masterfetch::links`]).
//! - **JS shell** → `content_ok = false` with an honest report; the raw
//!   extraction output is included for diagnostics but the agent is told the
//!   page is JS-rendered.
//! - **Auth wall / Paywall** → `content_ok = false` with an honest report and a
//!   `next_action` suggestion to switch sources.
//! - **Redirect** → note the redirect; the extraction output from the redirect
//!   target is included if available.
//! - **Other / Unknown** → fall back to the extractor chain output;
//!   `content_ok` is `true` only when the extracted text is non-trivial.
//!
//! All functions are pure — no network I/O — enabling unit tests without live
//! pages (NFR-003).
//!
//! # Design
//!
//! The classification flow is:
//!
//! 1. Run the [`extractor`] chain to obtain text content and its length.
//! 2. Call [`envelope::detect_page_type`] with the HTML, URL, and extracted
//!    text length to classify the page.
//! 3. Based on the detected [`PageType`], potentially reformat the output:
//!    - **List** → discard the extracted text and build a link list instead.
//!    - **JS shell** → wrap the output in an honest "JS-rendered" report.
//!    - **Auth wall / Paywall** → wrap in an honest access-restricted report.
//! 4. Compute a one-line summary appropriate to the page type.
//! 5. Return a [`ClassifyResult`] carrying all signals.

use url::Url;

use super::super::PageMetadata;
use super::super::PageType;
use super::super::envelope::detect_page_type;
use super::super::extractor::{ExtractMethod, ExtractOptions, ExtractResult, extract};
use super::super::links::{ClassifiedLinks, classify_links};
use super::super::metadata::extract_metadata;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Minimum extracted text length for `content_ok` to be `true` on
/// unclassified (fallback) pages.
const MIN_CONTENT_OK_CHARS: usize = 100;

/// Maximum number of links to include in a list-page link list output.
/// Beyond this, links are counted but not individually listed.
const MAX_LIST_LINKS: usize = 200;

/// Maximum summary length in characters.
const MAX_SUMMARY_LEN: usize = 120;

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Options controlling the classify-and-extract pipeline.
///
/// Wraps [`ExtractOptions`] (for the underlying extraction chain) and adds
/// crawl-specific parameters like `max_links` for list pages.
#[derive(Debug, Clone)]
pub struct ClassifyOptions {
    /// Extraction options passed through to [`extractor::extract`].
    pub extract: ExtractOptions,
    /// Maximum number of links to render in a list-page link list.
    /// Defaults to [`MAX_LIST_LINKS`]. Set to `0` to suppress link listing
    /// (only a count is reported).
    pub max_links: usize,
}

impl Default for ClassifyOptions {
    fn default() -> Self {
        Self {
            extract: ExtractOptions::default(),
            max_links: MAX_LIST_LINKS,
        }
    }
}

// ---------------------------------------------------------------------------
// Result
// ---------------------------------------------------------------------------

/// Result of content-adaptive classification and extraction.
///
/// Returned by [`classify_and_extract`]. Carries the detected page type, the
/// extracted content (adapted to the page type), and diagnostic signals that
/// feed directly into a [`crate::masterfetch::CrawlPage`] or
/// [`crate::masterfetch::FetchResult`].
#[derive(Debug, Clone)]
pub struct ClassifyResult {
    /// Detected page type (FR-029).
    pub page_type: PageType,
    /// Extracted content. For articles/docs this is the readability output;
    /// for list pages this is a structured `* [title](url)` link list; for
    /// JS shells and auth walls this is an honest report.
    pub content: String,
    /// `true` when usable content was successfully extracted.
    pub content_ok: bool,
    /// One-line summary of the page content.
    pub summary: String,
    /// Which extraction method produced the content.
    pub method: ExtractMethod,
    /// Page title (from readability or metadata), if available.
    pub title: Option<String>,
    /// Number of outgoing links found (for list pages and diagnostics).
    pub link_count: usize,
    /// Number of characters in the extracted content (before type-specific
    /// reformatting).
    pub total_chars: usize,
    /// `true` when the content was truncated to fit `max_content_chars`.
    pub is_truncated: bool,
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Classify a page and extract content adaptively based on the detected page
/// type.
///
/// This is the primary entry point for `mf_crawl`'s per-page extraction
/// (FR-012). It runs the extraction chain, classifies the page type from the
/// HTML structure (FR-029), and reformats the output for list pages and JS
/// shells.
///
/// # Arguments
///
/// * `html` — the raw HTML response body.
/// * `url` — the final URL (after redirects), used for page-type detection
///   and relative-link resolution.
/// * `content_type` — the HTTP `Content-Type` header value. If it does not
///   contain `text/html` or `application/xhtml`, the body is treated as raw
///   content (no classification).
/// * `opts` — classify-and-extract options.
///
/// # Returns
///
/// A [`ClassifyResult`] with the page type, adapted content, and signals.
/// This function never returns `Err` — it degrades gracefully, matching the
/// extractor chain's behaviour.
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::crawl::classify::{
///     classify_and_extract, ClassifyOptions,
/// };
/// use ragent_tools_extended::masterfetch::PageType;
///
/// let html = r#"<html><head><title>My Article</title></head>
/// <body><article><p>This is a substantial article with enough text to pass
/// the readability threshold for article classification. It needs to be
/// at least two hundred characters long to ensure the extractor produces
/// meaningful output and the page type detector classifies it correctly
/// as an article rather than a list or a JavaScript shell.</p></article>
/// </body></html>"#;
/// let result = classify_and_extract(html, "https://example.com/post", "text/html", &ClassifyOptions::default());
/// assert_eq!(result.page_type, PageType::Article);
/// assert!(result.content_ok);
/// ```
#[must_use]
pub fn classify_and_extract(
    html: &str,
    url: &str,
    content_type: &str,
    opts: &ClassifyOptions,
) -> ClassifyResult {
    // Step 1 — run the extraction chain.
    let extract_result = match extract(html, url, content_type, &opts.extract) {
        Ok(r) => r,
        Err(_) => ExtractResult {
            content: String::new(),
            title: None,
            method: ExtractMethod::RawText,
            is_truncated: false,
            total_chars: 0,
        },
    };

    let text_len = extract_result.content.chars().count();

    // Step 2 — classify the page type.
    let page_type = detect_page_type(html, url, text_len);

    // Step 3 — extract metadata for summaries and link resolution.
    let metadata = extract_metadata(html);

    // Step 4 — adapt output based on page type.
    let (content, content_ok, summary, link_count) = adapt_output(
        page_type,
        &extract_result.content,
        html,
        url,
        &metadata,
        opts,
    );

    ClassifyResult {
        page_type,
        content,
        content_ok,
        summary,
        method: extract_result.method,
        title: extract_result.title.or(metadata.title),
        link_count,
        total_chars: text_len,
        is_truncated: extract_result.is_truncated,
    }
}

// ---------------------------------------------------------------------------
// Output adaptation
// ---------------------------------------------------------------------------

/// Adapt the extracted content based on the detected page type.
///
/// Returns `(content, content_ok, summary, link_count)`.
fn adapt_output(
    page_type: PageType,
    extracted_text: &str,
    html: &str,
    url: &str,
    metadata: &PageMetadata,
    opts: &ClassifyOptions,
) -> (String, bool, String, usize) {
    match page_type {
        PageType::List => {
            let links = classify_links(html, url, metadata);
            let link_count = links.citations.len() + links.navigation.len();
            let content = format_link_list(&links, url, opts.max_links, metadata);
            let summary = format!("List/index page — {link_count} links found");
            (content, true, summary, link_count)
        }
        PageType::JsShell => {
            let content = format_js_shell_report(extracted_text);
            let summary = "JavaScript-rendered page — no static content".to_string();
            (content, false, summary, 0)
        }
        PageType::AuthWall => {
            let content = format_auth_wall_report(extracted_text);
            let summary = "Login required — content behind authentication wall".to_string();
            (content, false, summary, 0)
        }
        PageType::Paywall => {
            let content = format_paywall_report(extracted_text);
            let summary = "Paywall restricted content — subscription required".to_string();
            (content, false, summary, 0)
        }
        PageType::Redirect => {
            // Keep the extracted content (from the redirect target) but note
            // the redirect in the summary.
            let content = extracted_text.to_string();
            let summary = "Redirect page — content from redirect target".to_string();
            let content_ok = extracted_text.chars().count() >= MIN_CONTENT_OK_CHARS;
            (content, content_ok, summary, 0)
        }
        PageType::Article | PageType::Docs => {
            let content = extracted_text.to_string();
            let content_ok = extracted_text.chars().count() >= MIN_CONTENT_OK_CHARS;
            let summary = build_summary(extracted_text, metadata);
            (content, content_ok, summary, 0)
        }
        PageType::Forum | PageType::Qa => {
            let content = extracted_text.to_string();
            let content_ok = extracted_text.chars().count() >= MIN_CONTENT_OK_CHARS;
            let summary = build_summary(extracted_text, metadata);
            (content, content_ok, summary, 0)
        }
        PageType::Image => {
            let content = extracted_text.to_string();
            let summary = "Image page".to_string();
            (content, false, summary, 0)
        }
        PageType::Json => {
            let content = extracted_text.to_string();
            let content_ok = !extracted_text.is_empty();
            let summary = "JSON API response".to_string();
            (content, content_ok, summary, 0)
        }
        PageType::Unknown => {
            let content = extracted_text.to_string();
            let content_ok = extracted_text.chars().count() >= MIN_CONTENT_OK_CHARS;
            let summary = build_summary(extracted_text, metadata);
            (content, content_ok, summary, 0)
        }
    }
}

// ---------------------------------------------------------------------------
// Link-list formatting (list/index pages)
// ---------------------------------------------------------------------------

/// Format classified links as a markdown `* [title](url)` list.
///
/// Links are resolved to absolute URLs against the page URL. The list
/// combines citations and navigation links (deduplicated by resolved URL).
/// If `max_links` is `0`, only a count is returned.
fn format_link_list(
    links: &ClassifiedLinks,
    page_url: &str,
    max_links: usize,
    metadata: &PageMetadata,
) -> String {
    if max_links == 0 {
        let count = links.citations.len() + links.navigation.len();
        return format!("({count} links found — listing suppressed)\n");
    }

    let base_url = Url::parse(page_url).ok();

    // Collect all links (citations first, then navigation), resolving to
    // absolute URLs.
    let mut entries: Vec<(String, String)> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for link in links.citations.iter().chain(links.navigation.iter()) {
        let resolved = resolve_url(&link.href, base_url.as_ref());
        if seen.contains(&resolved) {
            continue;
        }
        seen.insert(resolved.clone());
        entries.push((link.text.clone(), resolved));
    }

    let total = entries.len();
    let displayed = entries.len().min(max_links);

    let mut out = String::new();

    // Optional title header.
    if let Some(ref title) = metadata.title
        && !title.is_empty()
    {
        out.push_str(&format!("# {title}\n\n"));
    }

    for (text, url) in entries.iter().take(max_links) {
        let label = if text.is_empty() {
            url.clone()
        } else {
            text.clone()
        };
        out.push_str(&format!("* [{label}]({url})\n"));
    }

    if total > displayed {
        out.push_str(&format!("\n... and {} more links\n", total - displayed));
    }

    out
}

/// Resolve a potentially relative URL against the page's base URL.
///
/// Returns the original `href` unchanged if resolution fails.
fn resolve_url(href: &str, base: Option<&Url>) -> String {
    // Already absolute — return as-is.
    if href.starts_with("http://") || href.starts_with("https://") {
        return href.to_string();
    }

    if let Some(base) = base
        && let Ok(resolved) = base.join(href)
    {
        return resolved.to_string();
    }

    href.to_string()
}

// ---------------------------------------------------------------------------
// Honest reports (JS shells, auth walls, paywalls)
// ---------------------------------------------------------------------------

/// Format an honest report for a JavaScript-rendered shell page.
///
/// Includes any extracted text (usually minimal) for diagnostics, prefixed
/// with a clear notice that the page is JS-rendered.
fn format_js_shell_report(extracted_text: &str) -> String {
    let mut out = String::new();
    out.push_str("[JS-rendered page — no static content extracted]\n\n");
    if extracted_text.trim().is_empty() {
        out.push_str("(no text content found)");
    } else {
        out.push_str("Extracted text (minimal):\n");
        out.push_str(extracted_text);
    }
    out.push('\n');
    out
}

/// Format an honest report for an authentication-walled page.
fn format_auth_wall_report(extracted_text: &str) -> String {
    let mut out = String::new();
    out.push_str("[Authentication required — page content is behind a login wall]\n\n");
    if extracted_text.trim().is_empty() {
        out.push_str("(no visible content)");
    } else {
        out.push_str("Visible text (may be incomplete):\n");
        out.push_str(extracted_text);
    }
    out.push('\n');
    out
}

/// Format an honest report for a paywalled page.
fn format_paywall_report(extracted_text: &str) -> String {
    let mut out = String::new();
    out.push_str("[Paywall — content requires a subscription]\n\n");
    if extracted_text.trim().is_empty() {
        out.push_str("(no preview available)");
    } else {
        out.push_str("Preview text:\n");
        out.push_str(extracted_text);
    }
    out.push('\n');
    out
}

// ---------------------------------------------------------------------------
// Summary computation
// ---------------------------------------------------------------------------

/// Build a one-line summary from the extracted text and metadata.
///
/// Prefers the metadata description; falls back to the first sentence of the
/// extracted text, truncated to [`MAX_SUMMARY_LEN`] characters.
fn build_summary(extracted_text: &str, metadata: &PageMetadata) -> String {
    // Prefer metadata description.
    if let Some(ref desc) = metadata.description
        && !desc.trim().is_empty()
    {
        return truncate_summary(desc);
    }

    // Fall back to the first sentence of the extracted text.
    let text = extracted_text.trim();
    if text.is_empty() {
        return String::new();
    }

    // Find the first sentence boundary (. ! ?) followed by whitespace.
    let sentence_end = text
        .char_indices()
        .skip_while(|(_, c)| !".!?".contains(*c))
        .find(|(i, _)| {
            text[*i..]
                .chars()
                .nth(1)
                .is_some_and(|c| c.is_whitespace() || c == '\n')
        })
        .map(|(i, _)| i + 1);

    let first_sentence = match sentence_end {
        Some(end) => &text[..end],
        None => text,
    };

    truncate_summary(first_sentence.trim())
}

/// Truncate a string to [`MAX_SUMMARY_LEN`] characters, appending "…" if
/// truncated.
fn truncate_summary(s: &str) -> String {
    if s.chars().count() <= MAX_SUMMARY_LEN {
        return s.to_string();
    }
    let truncated: String = s.chars().take(MAX_SUMMARY_LEN).collect();
    format!("{truncated}…")
}

// ---------------------------------------------------------------------------
// LinkInfo helper
// ---------------------------------------------------------------------------

/// Count the total number of unique links on a page.
///
/// Returns the count of unique `href` values across all link categories.
/// This is a convenience function for diagnostics.
#[must_use]
pub fn count_unique_links(html: &str, page_url: &str, metadata: &PageMetadata) -> usize {
    let links = classify_links(html, page_url, metadata);
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for l in &links.citations {
        seen.insert(l.href.as_str());
    }
    for l in &links.navigation {
        seen.insert(l.href.as_str());
    }
    for l in &links.external {
        seen.insert(l.href.as_str());
    }
    seen.len()
}
