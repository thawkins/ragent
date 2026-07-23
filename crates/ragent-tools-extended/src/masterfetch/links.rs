//! Outgoing-link classification for masterfetch.
//!
//! Implements FR-007 and NFR-003.
//!
//! This module classifies the outgoing links on an HTML page into three
//! categories:
//!
//! - **Citations** — links inside the main-content area (`<article>`,
//!   `<main>`, `[role="main"]`).
//! - **Navigation** — links inside navigation elements (`<nav>`,
//!   `<header>`, `<footer>`, `<aside>`).
//! - **External** — links pointing to a different domain than the page URL.
//!
//! A link may appear in multiple lists: a content-area link to another domain
//! is both a citation and an external link.
//!
//! The module also computes a `primary_source` hint — the most likely
//! authoritative source for the page's content — derived from the canonical
//! URL, JSON-LD metadata, or a citation pointing at a known primary host
//! (e.g. `github.com`, `arxiv.org`, `doi.org`).
//!
//! All functions are pure — no network I/O — enabling unit tests without live
//! pages (NFR-003).

use super::PageMetadata;
use super::extractor::{HtmlToken, tokenize_tags};

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

/// Information about a single outgoing link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkInfo {
    /// The `href` URL as it appears in the HTML (may be relative).
    pub href: String,
    /// The link text content, trimmed.
    pub text: String,
    /// The `rel` attribute value if present (e.g. "nofollow", "author").
    pub rel: Option<String>,
}

/// Classified outgoing links from a page.
///
/// Returned by [`classify_links`]. Each list is deduplicated by `href`.
/// A link may appear in multiple lists (e.g. a content-area link to another
/// domain appears in both `citations` and `external`).
#[derive(Debug, Clone, Default)]
pub struct ClassifiedLinks {
    /// Links inside the main-content area (`<article>`, `<main>`).
    pub citations: Vec<LinkInfo>,
    /// Links inside navigation elements (`<nav>`, `<header>`, `<footer>`,
    /// `<aside>`).
    pub navigation: Vec<LinkInfo>,
    /// Links pointing to a different domain than the page URL.
    pub external: Vec<LinkInfo>,
    /// Hint for the most likely authoritative source URL.
    pub primary_source: Option<String>,
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// HTML elements that contain navigation links.
const NAV_ELEMENTS: &[&str] = &["nav", "header", "footer", "aside"];

/// HTML elements that contain main-content (citation) links.
const CONTENT_ELEMENTS: &[&str] = &["article", "main", "section"];

/// Known primary-host domains that are commonly authoritative sources.
///
/// When a citation link points to one of these domains, it is a strong
/// candidate for `primary_source`.
const KNOWN_PRIMARY_HOSTS: &[&str] = &[
    "github.com",
    "arxiv.org",
    "doi.org",
    "npmjs.com",
    "crates.io",
    "pypi.org",
    "developer.mozilla.org",
    "w3.org",
    "ietf.org",
    "ecma-international.org",
    "schema.org",
    "kernel.org",
    "gnu.org",
    "apache.org",
    "opensource.org",
];

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Classify outgoing links from an HTML page (FR-007).
///
/// Walks the HTML token stream, tracking whether the current position is
/// inside a navigation element or a content element, and classifies each
/// `<a href="...">` tag accordingly.
///
/// # Arguments
///
/// * `html` — the raw HTML response body.
/// * `page_url` — the final URL of the page (after redirects), used to
///   determine the page's domain for external-link classification.
/// * `metadata` — previously extracted page metadata, used to derive the
///   `primary_source` hint from the canonical URL.
///
/// # Returns
///
/// A [`ClassifiedLinks`] struct with deduplicated link lists and a
/// `primary_source` hint. Never panics — malformed HTML produces empty lists.
///
/// # Example
///
/// ```
/// use ragent_tools_extended::masterfetch::links::classify_links;
/// use ragent_tools_extended::masterfetch::PageMetadata;
///
/// let html = r#"<html><body>
/// <nav><a href="/home">Home</a></nav>
/// <article><a href="https://example.com/ref">Reference</a></article>
/// </body></html>"#;
/// let md = PageMetadata::default();
/// let links = classify_links(html, "https://mysite.com/page", &md);
/// assert!(links.navigation.iter().any(|l| l.text == "Home"));
/// assert!(links.citations.iter().any(|l| l.text == "Reference"));
/// ```
#[must_use]
pub fn classify_links(html: &str, page_url: &str, metadata: &PageMetadata) -> ClassifiedLinks {
    let page_host = extract_host(page_url);
    let tokens = tokenize_tags(html);

    let mut citations: Vec<LinkInfo> = Vec::new();
    let mut navigation: Vec<LinkInfo> = Vec::new();
    let mut external: Vec<LinkInfo> = Vec::new();

    // Stack of open element names to track context.
    let mut open_stack: Vec<String> = Vec::new();

    // Track the current <a> tag being processed (for text accumulation).
    let mut current_anchor: Option<CurrentAnchor> = None;

    for token in &tokens {
        match token {
            HtmlToken::OpenTag { name, attrs } => {
                open_stack.push((*name).to_string());

                // If this is an <a> tag, start tracking it.
                if *name == "a" {
                    let href = get_attr(attrs, "href").unwrap_or_default();
                    let rel = get_attr(attrs, "rel");
                    if !href.is_empty()
                        && !href.starts_with('#')
                        && !href.starts_with("javascript:")
                    {
                        current_anchor = Some(CurrentAnchor {
                            href,
                            rel,
                            text: String::new(),
                            depth_at_open: open_stack.len(),
                        });
                    }
                }
            }
            HtmlToken::CloseTag { name } => {
                // Pop the stack.
                if let Some(pos) = open_stack.iter().rposition(|n| n == name) {
                    open_stack.truncate(pos);
                }

                // If this closes an <a> tag, finalize the link.
                if *name == "a"
                    && let Some(anchor) = current_anchor.take()
                {
                    let link = LinkInfo {
                        href: anchor.href,
                        text: anchor.text.trim().to_string(),
                        rel: anchor.rel,
                    };
                    // Use the context stack at the time the <a> was opened.
                    // After popping <a>, the stack has depth_at_open-1 elements.
                    let context: Vec<&str> = open_stack
                        .iter()
                        .take(anchor.depth_at_open.saturating_sub(1))
                        .map(String::as_str)
                        .collect();
                    classify_and_add_with_context(
                        &link,
                        &page_host,
                        &context,
                        &mut citations,
                        &mut navigation,
                        &mut external,
                    );
                }
            }
            HtmlToken::Text(text) => {
                // Accumulate text into the current anchor if any.
                if let Some(ref mut anchor) = current_anchor {
                    anchor.text.push_str(text.trim());
                }
            }
            HtmlToken::SelfClosingTag { name, attrs } => {
                // Self-closing tags (e.g. <img/>, <br/>, <input/>) cannot wrap
                // text, so they never start an anchor. However, some HTML
                // serialisations emit <a/> as a self-closing tag with an href;
                // record such links with empty text.
                if *name == "a" {
                    let href = get_attr(attrs, "href").unwrap_or_default();
                    let rel = get_attr(attrs, "rel");
                    if !href.is_empty()
                        && !href.starts_with('#')
                        && !href.starts_with("javascript:")
                    {
                        let link = LinkInfo {
                            href,
                            text: String::new(),
                            rel,
                        };
                        let context: Vec<&str> = open_stack.iter().map(String::as_str).collect();
                        classify_and_add_with_context(
                            &link,
                            &page_host,
                            &context,
                            &mut citations,
                            &mut navigation,
                            &mut external,
                        );
                    }
                }
            }
        }
    }

    // Deduplicate each list by href.
    dedup_by_href(&mut citations);
    dedup_by_href(&mut navigation);
    dedup_by_href(&mut external);

    // Compute primary_source hint.
    let primary_source = compute_primary_source(metadata, &citations, &page_host);

    ClassifiedLinks {
        citations,
        navigation,
        external,
        primary_source,
    }
}

// ---------------------------------------------------------------------------
// Internal types and helpers
// ---------------------------------------------------------------------------

/// Tracks an in-progress `<a>` tag for text accumulation.
struct CurrentAnchor {
    href: String,
    rel: Option<String>,
    text: String,
    /// Stack depth at the time the `<a>` was opened (including the `<a>`
    /// itself). Used to reconstruct the containing-element context when the
    /// `</a>` is encountered.
    depth_at_open: usize,
}

/// Get an attribute value by name (case-insensitive).
fn get_attr(attrs: &[super::extractor::HtmlAttr], name: &str) -> Option<String> {
    attrs
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case(name))
        .map(|a| a.value.clone())
        .filter(|v| !v.is_empty())
}

/// Check if any element in the context stack is a navigation element.
fn is_in_navigation(context: &[&str]) -> bool {
    context.iter().any(|tag| NAV_ELEMENTS.contains(tag))
}

/// Check if any element in the context stack is a content element.
#[allow(dead_code)]
fn is_in_content(context: &[&str]) -> bool {
    context.iter().any(|tag| CONTENT_ELEMENTS.contains(tag))
}

/// Classify a link with an explicit context stack and add to appropriate lists.
fn classify_and_add_with_context(
    link: &LinkInfo,
    page_host: &str,
    context: &[&str],
    citations: &mut Vec<LinkInfo>,
    navigation: &mut Vec<LinkInfo>,
    external: &mut Vec<LinkInfo>,
) {
    // External classification: different host than the page.
    let link_host = extract_host(&link.href);
    let is_external = !link_host.is_empty() && link_host != page_host;
    if is_external {
        external.push(link.clone());
    }

    // Area classification: navigation vs citation.
    if is_in_navigation(context) {
        navigation.push(link.clone());
    } else {
        // Default to citation if in content area or no specific context.
        citations.push(link.clone());
    }
}

/// Extract the host (lowercased) from a URL string.
///
/// Returns an empty string for relative URLs or invalid URLs.
fn extract_host(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_ascii_lowercase()))
        .unwrap_or_default()
}

/// Remove duplicate links by href, preserving first occurrence.
fn dedup_by_href(links: &mut Vec<LinkInfo>) {
    let mut seen = std::collections::HashSet::new();
    links.retain(|l| seen.insert(l.href.clone()));
}

/// Compute the `primary_source` hint.
///
/// Priority:
/// 1. Canonical URL from metadata.
/// 2. A citation pointing to a known primary host.
/// 3. The first external citation (if any).
fn compute_primary_source(
    metadata: &PageMetadata,
    citations: &[LinkInfo],
    page_host: &str,
) -> Option<String> {
    // 1. Canonical URL.
    if let Some(ref canonical) = metadata.canonical
        && !canonical.is_empty()
    {
        return Some(canonical.clone());
    }
    // 2. A citation pointing to a known primary host.
    for link in citations {
        let host = extract_host(&link.href);
        if !host.is_empty() && KNOWN_PRIMARY_HOSTS.contains(&host.as_str()) {
            return Some(link.href.clone());
        }
    }

    // 3. The first external citation (different domain from the page).
    for link in citations {
        let host = extract_host(&link.href);
        if !host.is_empty() && host != page_host {
            return Some(link.href.clone());
        }
    }

    None
}
