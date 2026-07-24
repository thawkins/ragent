//! Integration tests for `masterfetch::links` — outgoing-link classification
//! (T-032, FR-007, NFR-003).
//!
//! Covers: citation classification, navigation classification, external
//! classification, `primary_source` hint, empty page, malformed HTML.

use ragent_tools_extended::masterfetch::PageMetadata;
use ragent_tools_extended::masterfetch::links::{ClassifiedLinks, LinkInfo, classify_links};

/// Helper: classify links from HTML with default metadata.
fn classify(html: &str, page_url: &str) -> ClassifiedLinks {
    classify_links(html, page_url, &PageMetadata::default())
}

/// Helper: check if a list contains a link with the given href.
fn has_href(list: &[LinkInfo], href: &str) -> bool {
    list.iter().any(|l| l.href == href)
}

// ---------------------------------------------------------------------------
// Citation classification (links inside main-content area)
// ---------------------------------------------------------------------------

#[test]
fn test_citation_in_article() {
    let html = r#"<html><body>
<article><a href="/post/1">Blog Post</a></article>
</body></html>"#;
    let links = classify(html, "https://example.com");
    assert!(has_href(&links.citations, "/post/1"));
    assert!(links.citations.iter().any(|l| l.text == "Blog Post"));
}

#[test]
fn test_citation_in_main() {
    let html = r#"<html><body>
<main><a href="/doc">Documentation</a></main>
</body></html>"#;
    let links = classify(html, "https://example.com");
    assert!(has_href(&links.citations, "/doc"));
}

#[test]
fn test_citation_in_section() {
    let html = r#"<html><body>
<section><a href="/sec">Section Link</a></section>
</body></html>"#;
    let links = classify(html, "https://example.com");
    assert!(has_href(&links.citations, "/sec"));
}

#[test]
fn test_link_in_body_without_context_defaults_to_citation() {
    let html = r#"<html><body><a href="/page">Page Link</a></body></html>"#;
    let links = classify(html, "https://example.com");
    assert!(has_href(&links.citations, "/page"));
}

// ---------------------------------------------------------------------------
// Navigation classification (links inside nav/header/footer/aside)
// ---------------------------------------------------------------------------

#[test]
fn test_navigation_in_nav() {
    let html = r#"<html><body>
<nav><a href="/home">Home</a></nav>
</body></html>"#;
    let links = classify(html, "https://example.com");
    assert!(has_href(&links.navigation, "/home"));
    assert!(!has_href(&links.citations, "/home"));
}

#[test]
fn test_navigation_in_header() {
    let html = r#"<html><body>
<header><a href="/about">About</a></header>
</body></html>"#;
    let links = classify(html, "https://example.com");
    assert!(has_href(&links.navigation, "/about"));
    assert!(!has_href(&links.citations, "/about"));
}

#[test]
fn test_navigation_in_footer() {
    let html = r#"<html><body>
<footer><a href="/privacy">Privacy</a></footer>
</body></html>"#;
    let links = classify(html, "https://example.com");
    assert!(has_href(&links.navigation, "/privacy"));
    assert!(!has_href(&links.citations, "/privacy"));
}

#[test]
fn test_navigation_in_aside() {
    let html = r#"<html><body>
<aside><a href="/sidebar">Sidebar</a></aside>
</body></html>"#;
    let links = classify(html, "https://example.com");
    assert!(has_href(&links.navigation, "/sidebar"));
    assert!(!has_href(&links.citations, "/sidebar"));
}

#[test]
fn test_nav_link_not_in_citations() {
    let html = r#"<html><body>
<nav><a href="/home">Home</a></nav>
<article><a href="/post">Post</a></article>
</body></html>"#;
    let links = classify(html, "https://example.com");
    assert!(has_href(&links.navigation, "/home"));
    assert!(!has_href(&links.citations, "/home"));
    assert!(has_href(&links.citations, "/post"));
    assert!(!has_href(&links.navigation, "/post"));
}

// ---------------------------------------------------------------------------
// External classification (off-domain links)
// ---------------------------------------------------------------------------

#[test]
fn test_external_link_classified() {
    let html = r#"<html><body>
<article><a href="https://other.com/page">External</a></article>
</body></html>"#;
    let links = classify(html, "https://example.com");
    assert!(has_href(&links.external, "https://other.com/page"));
}

#[test]
fn test_same_domain_link_not_external() {
    let html = r#"<html><body>
<a href="https://example.com/other">Same Domain</a>
</body></html>"#;
    let links = classify(html, "https://example.com");
    assert!(!has_href(&links.external, "https://example.com/other"));
}

#[test]
fn test_relative_link_not_external() {
    let html = r#"<html><body>
<a href="/relative">Relative</a>
</body></html>"#;
    let links = classify(html, "https://example.com");
    assert!(!has_href(&links.external, "/relative"));
}

#[test]
fn test_external_link_also_in_citations() {
    let html = r#"<html><body>
<article><a href="https://other.com/ref">Ref</a></article>
</body></html>"#;
    let links = classify(html, "https://example.com");
    // Should be in both citations (content area) and external (different domain).
    assert!(has_href(&links.citations, "https://other.com/ref"));
    assert!(has_href(&links.external, "https://other.com/ref"));
}

#[test]
fn test_external_in_navigation_too() {
    let html = r#"<html><body>
<nav><a href="https://social.com/share">Share</a></nav>
</body></html>"#;
    let links = classify(html, "https://example.com");
    assert!(has_href(&links.navigation, "https://social.com/share"));
    assert!(has_href(&links.external, "https://social.com/share"));
}

#[test]
fn test_subdomain_is_external() {
    let html = r#"<html><body>
<a href="https://blog.example.com/post">Blog Subdomain</a>
</body></html>"#;
    let links = classify(html, "https://example.com");
    // Different host (blog.example.com vs example.com) = external.
    assert!(has_href(&links.external, "https://blog.example.com/post"));
}

// ---------------------------------------------------------------------------
// Link text extraction
// ---------------------------------------------------------------------------

#[test]
fn test_link_text_extracted() {
    let html = r#"<article><a href="/p">Click here</a></article>"#;
    let links = classify(html, "https://example.com");
    assert_eq!(links.citations[0].text, "Click here");
}

#[test]
fn test_link_text_trimmed() {
    let html = r#"<article><a href="/p">  Spaced Text  </a></article>"#;
    let links = classify(html, "https://example.com");
    assert_eq!(links.citations[0].text, "Spaced Text");
}

#[test]
fn test_link_with_nested_tags_text() {
    let html = r#"<article><a href="/p">Read <b>this</b> post</a></article>"#;
    let links = classify(html, "https://example.com");
    // Text from nested tags should be concatenated.
    assert!(links.citations[0].text.contains("Read"));
    assert!(links.citations[0].text.contains("this"));
    assert!(links.citations[0].text.contains("post"));
}

// ---------------------------------------------------------------------------
// rel attribute
// ---------------------------------------------------------------------------

#[test]
fn test_rel_attribute_captured() {
    let html = r#"<article><a href="/p" rel="nofollow">Link</a></article>"#;
    let links = classify(html, "https://example.com");
    assert_eq!(links.citations[0].rel.as_deref(), Some("nofollow"));
}

#[test]
fn test_no_rel_returns_none() {
    let html = r#"<article><a href="/p">Link</a></article>"#;
    let links = classify(html, "https://example.com");
    assert!(links.citations[0].rel.is_none());
}

// ---------------------------------------------------------------------------
// Anchor and fragment links skipped
// ---------------------------------------------------------------------------

#[test]
fn test_anchor_links_skipped() {
    let html = r##"<article><a href="#section">Jump to section</a></article>"##;
    let links = classify(html, "https://example.com");
    assert!(links.citations.is_empty());
}

#[test]
fn test_javascript_links_skipped() {
    let html = r#"<article><a href="javascript:void(0)">Click</a></article>"#;
    let links = classify(html, "https://example.com");
    assert!(links.citations.is_empty());
}

// ---------------------------------------------------------------------------
// Deduplication
// ---------------------------------------------------------------------------

#[test]
fn test_duplicate_hrefs_deduplicated() {
    let html = r#"<article>
<a href="/same">First</a>
<a href="/same">Second</a>
</article>"#;
    let links = classify(html, "https://example.com");
    assert_eq!(links.citations.len(), 1);
    assert_eq!(links.citations[0].text, "First");
}

#[test]
fn test_different_hrefs_not_deduplicated() {
    let html = r#"<article>
<a href="/a">A</a>
<a href="/b">B</a>
</article>"#;
    let links = classify(html, "https://example.com");
    assert_eq!(links.citations.len(), 2);
}

// ---------------------------------------------------------------------------
// primary_source hint
// ---------------------------------------------------------------------------

#[test]
fn test_primary_source_from_canonical() {
    let html = r#"<article><a href="/ref">Ref</a></article>"#;
    let mut md = PageMetadata::default();
    md.canonical = Some("https://original.com/source".to_string());
    let links = classify_links(html, "https://example.com", &md);
    assert_eq!(
        links.primary_source.as_deref(),
        Some("https://original.com/source")
    );
}

#[test]
fn test_primary_source_from_known_host_citation() {
    let html = r#"<article>
<a href="https://github.com/user/repo">GitHub</a>
</article>"#;
    let links = classify(html, "https://example.com");
    assert_eq!(
        links.primary_source.as_deref(),
        Some("https://github.com/user/repo")
    );
}

#[test]
fn test_primary_source_from_arxiv_citation() {
    let html = r#"<article>
<a href="https://arxiv.org/abs/2024.12345">Paper</a>
</article>"#;
    let links = classify(html, "https://example.com");
    assert_eq!(
        links.primary_source.as_deref(),
        Some("https://arxiv.org/abs/2024.12345")
    );
}

#[test]
fn test_primary_source_from_first_external_citation() {
    let html = r#"<article>
<a href="https://random.com/post">Random</a>
</article>"#;
    let links = classify(html, "https://example.com");
    // No canonical, no known host → first external citation.
    assert_eq!(
        links.primary_source.as_deref(),
        Some("https://random.com/post")
    );
}

#[test]
fn test_primary_source_canonical_takes_priority() {
    let html = r#"<article>
<a href="https://github.com/user/repo">GitHub</a>
</article>"#;
    let mut md = PageMetadata::default();
    md.canonical = Some("https://canonical.com/page".to_string());
    let links = classify_links(html, "https://example.com", &md);
    assert_eq!(
        links.primary_source.as_deref(),
        Some("https://canonical.com/page")
    );
}

#[test]
fn test_primary_source_none_when_no_citations_or_canonical() {
    let html = r#"<nav><a href="/home">Home</a></nav>"#;
    let links = classify(html, "https://example.com");
    assert!(links.primary_source.is_none());
}

#[test]
fn test_primary_source_none_when_only_same_domain_citations() {
    let html = r#"<article><a href="/internal">Internal</a></article>"#;
    let links = classify(html, "https://example.com");
    assert!(links.primary_source.is_none());
}

// ---------------------------------------------------------------------------
// Empty / malformed HTML
// ---------------------------------------------------------------------------

#[test]
fn test_empty_html_returns_empty_lists() {
    let links = classify("", "https://example.com");
    assert!(links.citations.is_empty());
    assert!(links.navigation.is_empty());
    assert!(links.external.is_empty());
    assert!(links.primary_source.is_none());
}

#[test]
fn test_html_with_no_links_returns_empty_lists() {
    let html = r"<html><body><p>No links here</p></body></html>";
    let links = classify(html, "https://example.com");
    assert!(links.citations.is_empty());
    assert!(links.navigation.is_empty());
    assert!(links.external.is_empty());
}

#[test]
fn test_malformed_html_does_not_panic() {
    let html = r#"<article><a href="/p">Unclosed</body></html>"#;
    let links = classify(html, "https://example.com");
    // Should not panic; may or may not extract the link.
    let _ = links;
}

#[test]
fn test_link_with_empty_href_skipped() {
    let html = r#"<article><a href="">Empty</a></article>"#;
    let links = classify(html, "https://example.com");
    assert!(links.citations.is_empty());
}

// ---------------------------------------------------------------------------
// Complex page
// ---------------------------------------------------------------------------

#[test]
fn test_complex_page_classification() {
    let html = r#"<html><body>
<header>
<nav>
<a href="/home">Home</a>
<a href="/about">About</a>
</nav>
</header>
<main>
<article>
<p>See <a href="https://github.com/repo">the repo</a> and
<a href="https://other.com/ref">the reference</a>.</p>
<a href="/related">Related post</a>
</article>
</main>
<footer>
<a href="/privacy">Privacy Policy</a>
<a href="https://social.com/share">Share</a>
</footer>
</body></html>"#;
    let links = classify(html, "https://example.com");

    // Navigation: Home, About, Privacy, Share.
    assert!(has_href(&links.navigation, "/home"));
    assert!(has_href(&links.navigation, "/about"));
    assert!(has_href(&links.navigation, "/privacy"));
    assert!(has_href(&links.navigation, "https://social.com/share"));

    // Citations: github repo, other ref, related post.
    assert!(has_href(&links.citations, "https://github.com/repo"));
    assert!(has_href(&links.citations, "https://other.com/ref"));
    assert!(has_href(&links.citations, "/related"));

    // External: github, other, social.
    assert!(has_href(&links.external, "https://github.com/repo"));
    assert!(has_href(&links.external, "https://other.com/ref"));
    assert!(has_href(&links.external, "https://social.com/share"));

    // primary_source: github is a known host.
    assert_eq!(
        links.primary_source.as_deref(),
        Some("https://github.com/repo")
    );
}

// ---------------------------------------------------------------------------
// Nested context (nav inside article, article inside main)
// ---------------------------------------------------------------------------

#[test]
fn test_nav_inside_article_treated_as_navigation() {
    let html = r#"<article>
<nav><a href="/toc">Table of Contents</a></nav>
<p>Content <a href="/ref">Reference</a></p>
</article>"#;
    let links = classify(html, "https://example.com");
    // The <nav> inside <article> should still classify its links as navigation.
    assert!(has_href(&links.navigation, "/toc"));
    // The link outside <nav> but inside <article> should be a citation.
    assert!(has_href(&links.citations, "/ref"));
}

#[test]
fn test_article_inside_main() {
    let html = r#"<main><article><a href="/p">Post</a></article></main>"#;
    let links = classify(html, "https://example.com");
    assert!(has_href(&links.citations, "/p"));
}
