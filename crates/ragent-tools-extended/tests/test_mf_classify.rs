#![allow(clippy::assert_is_empty)]
//! Unit tests for `masterfetch::crawl::classify` — content-adaptive page
//! classification and extraction (T-018, FR-012, FR-029, NFR-003).
//!
//! Covers:
//! - Article detection → readability extraction, `content_ok=true`.
//! - Docs detection → readability extraction, `content_ok=true`.
//! - List/index detection → structured link list output.
//! - JS shell detection → honest report, `content_ok=false`.
//! - Auth wall detection → honest report, `content_ok=false`.
//! - Paywall detection → honest report, `content_ok=false`.
//! - Redirect detection → content from redirect target.
//! - Unknown fallback → extractor output.
//! - Summary computation (metadata description, first sentence).
//! - Link-list formatting (absolute URL resolution, dedup, `max_links` cap).
//! - Non-HTML content (raw passthrough, no classification).
//! - `count_unique_links` helper.

use ragent_tools_extended::masterfetch::PageType;
use ragent_tools_extended::masterfetch::crawl::classify::{
    ClassifyOptions, classify_and_extract, count_unique_links,
};
use ragent_tools_extended::masterfetch::extractor::{ExtractMethod, OutputFormat};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Generate article HTML with enough text to pass readability + article
/// classification thresholds.
fn article_html(title: &str, body: &str) -> String {
    format!(
        r#"<html><head><title>{title}</title>
<meta property="og:title" content="{title}">
<meta property="og:description" content="A test article about Rust.">
</head><body><article><p>{body}</p></article></body></html>"#
    )
}

/// Long body text that exceeds the 200-char article threshold.
const LONG_BODY: &str = "This is a substantial article with enough text to pass \
the readability threshold for article classification. It needs to be at least \
two hundred characters long to ensure the extractor produces meaningful output \
and the page type detector classifies it correctly as an article rather than a \
list or a JavaScript shell. This paragraph is intentionally long and detailed \
to meet that threshold comfortably without any ambiguity.";

/// Generate list-page HTML with many links and little text.
fn list_html(title: &str, link_count: usize) -> String {
    let mut links = String::new();
    for i in 0..link_count {
        links.push_str(&format!("<li><a href=\"/page/{i}\">Page {i}</a></li>\n"));
    }
    format!(
        r#"<html><head><title>{title}</title></head>
<body><nav><a href="/">Home</a></nav>
<ul>{links}</ul></body></html>"#
    )
}

/// Generate JS-shell HTML: large body, tiny text, JS-required signal.
fn js_shell_html() -> String {
    let padding = " ".repeat(4000);
    format!(
        r#"<html><head><title>App</title></head>
<body><div id="root">{padding}</div>
<noscript>You need to enable JavaScript to run this app.</noscript>
</body></html>"#
    )
}

/// Generate auth-wall HTML.
fn auth_wall_html() -> String {
    r#"<html><head><title>Login</title></head>
<body><div class="login-form"><h1>Sign In</h1>
<p>Please log in to continue.</p>
<form><input type="password"></form></div></body></html>"#
        .to_string()
}

/// Generate paywall HTML.
fn paywall_html() -> String {
    r#"<html><head><title>Article</title></head>
<body><article><p>This is a preview of the article.</p></article>
<div class="paywall">Subscribe to continue reading.</div></body></html>"#
        .to_string()
}

/// Generate redirect HTML (meta refresh, 0-second delay).
fn redirect_html(target: &str) -> String {
    format!(
        r#"<html><head><meta http-equiv="refresh" content="0;url={target}"></head>
<body>Redirecting...</body></html>"#
    )
}

// ---------------------------------------------------------------------------
// Article classification
// ---------------------------------------------------------------------------

#[test]
fn test_article_detected_and_extracted() {
    let html = article_html("My Article", LONG_BODY);
    let result = classify_and_extract(
        &html,
        "https://example.com/post",
        "text/html",
        &ClassifyOptions::default(),
    );
    assert_eq!(result.page_type, PageType::Article);
    assert!(result.content_ok);
    assert!(!result.content.is_empty());
}

#[test]
fn test_article_summary_from_metadata_description() {
    let html = article_html("My Article", LONG_BODY);
    let result = classify_and_extract(
        &html,
        "https://example.com/post",
        "text/html",
        &ClassifyOptions::default(),
    );
    // The summary should come from og:description.
    assert_eq!(result.summary, "A test article about Rust.");
}

#[test]
fn test_article_title_extracted() {
    let html = article_html("My Article", LONG_BODY);
    let result = classify_and_extract(
        &html,
        "https://example.com/post",
        "text/html",
        &ClassifyOptions::default(),
    );
    assert_eq!(result.title.as_deref(), Some("My Article"));
}

#[test]
fn test_article_content_has_text() {
    let html = article_html("My Article", LONG_BODY);
    let result = classify_and_extract(
        &html,
        "https://example.com/post",
        "text/html",
        &ClassifyOptions::default(),
    );
    assert!(result.content.contains("substantial article"));
}

// ---------------------------------------------------------------------------
// Docs classification
// ---------------------------------------------------------------------------

#[test]
fn test_docs_detected_on_docs_domain() {
    // docs.rs is in VENDOR_DOCS_DOMAINS.
    let html = format!(
        r"<html><head><title>API Docs</title></head>
<body><main><p>{LONG_BODY}</p>
<pre><code>fn foo() -> i32 {{ 42 }}</code></pre>
<pre><code>fn bar() -> i32 {{ 99 }}</code></pre>
<pre><code>fn baz() -> i32 {{ 0 }}</code></pre>
</main></body></html>"
    );
    let result = classify_and_extract(
        &html,
        "https://docs.rs/crate/v1.0",
        "text/html",
        &ClassifyOptions::default(),
    );
    assert_eq!(result.page_type, PageType::Docs);
    assert!(result.content_ok);
}

// ---------------------------------------------------------------------------
// List/index classification
// ---------------------------------------------------------------------------

#[test]
fn test_list_detected_and_link_list_produced() {
    let html = list_html("Links Page", 30);
    let result = classify_and_extract(
        &html,
        "https://example.com/links",
        "text/html",
        &ClassifyOptions::default(),
    );
    assert_eq!(result.page_type, PageType::List);
    assert!(result.content_ok);
    // Content should contain markdown link format.
    assert!(result.content.contains("* ["));
}

#[test]
fn test_list_link_count_in_summary() {
    let html = list_html("Links Page", 25);
    let result = classify_and_extract(
        &html,
        "https://example.com/links",
        "text/html",
        &ClassifyOptions::default(),
    );
    assert!(result.summary.contains("links found"));
}

#[test]
fn test_list_links_resolved_to_absolute() {
    let html = list_html("Links Page", 15);
    let result = classify_and_extract(
        &html,
        "https://example.com/links",
        "text/html",
        &ClassifyOptions::default(),
    );
    // Relative hrefs like /page/0 should be resolved to absolute URLs.
    assert!(result.content.contains("https://example.com/page/"));
}

#[test]
fn test_list_title_in_header() {
    let html = list_html("My Links Page", 20);
    let result = classify_and_extract(
        &html,
        "https://example.com/links",
        "text/html",
        &ClassifyOptions::default(),
    );
    assert!(result.content.contains("# My Links Page"));
}

#[test]
fn test_list_max_links_cap() {
    let html = list_html("Links Page", 50);
    let opts = ClassifyOptions {
        max_links: 10,
        ..ClassifyOptions::default()
    };
    let result = classify_and_extract(&html, "https://example.com/links", "text/html", &opts);
    // Should show 10 links and a "more links" message.
    let link_lines = result
        .content
        .lines()
        .filter(|l| l.starts_with("* ["))
        .count();
    assert!(link_lines <= 10);
    assert!(result.content.contains("more links"));
}

#[test]
fn test_list_max_links_zero_suppresses_listing() {
    let html = list_html("Links Page", 30);
    let opts = ClassifyOptions {
        max_links: 0,
        ..ClassifyOptions::default()
    };
    let result = classify_and_extract(&html, "https://example.com/links", "text/html", &opts);
    assert!(result.content.contains("listing suppressed"));
    assert!(!result.content.contains("* ["));
}

#[test]
fn test_list_link_count_field() {
    let html = list_html("Links Page", 30);
    let result = classify_and_extract(
        &html,
        "https://example.com/links",
        "text/html",
        &ClassifyOptions::default(),
    );
    // link_count should be > 0 (citations + navigation).
    assert!(result.link_count > 0);
}

#[test]
fn test_list_dedup_by_resolved_url() {
    // Two links pointing to the same resolved URL should only appear once.
    let mut links = String::new();
    for i in 1..=20 {
        links.push_str(&format!("<li><a href=\"/page/{i}\">Page {i}</a></li>\n"));
    }
    // Add a duplicate of page/1 with different text.
    links.push_str("<li><a href=\"/page/1\">First again</a></li>\n");
    let html = format!(
        r"<html><head><title>Dup</title></head>
<body><ul>{links}</ul></body></html>"
    );
    let result = classify_and_extract(
        &html,
        "https://example.com/list",
        "text/html",
        &ClassifyOptions::default(),
    );
    assert_eq!(result.page_type, PageType::List);
    // Count unique link lines.
    let link_lines = result
        .content
        .lines()
        .filter(|l| l.starts_with("* ["))
        .count();
    // Should have 20 unique URLs (page/1 appears twice but deduped).
    assert_eq!(link_lines, 20);
}

// ---------------------------------------------------------------------------
// JS shell classification
// ---------------------------------------------------------------------------

#[test]
fn test_js_shell_detected_content_not_ok() {
    let html = js_shell_html();
    let result = classify_and_extract(
        &html,
        "https://example.com/app",
        "text/html",
        &ClassifyOptions::default(),
    );
    assert_eq!(result.page_type, PageType::JsShell);
    assert!(!result.content_ok);
}

#[test]
fn test_js_shell_report_contains_notice() {
    let html = js_shell_html();
    let result = classify_and_extract(
        &html,
        "https://example.com/app",
        "text/html",
        &ClassifyOptions::default(),
    );
    assert!(result.content.contains("JS-rendered page"));
}

#[test]
fn test_js_shell_summary() {
    let html = js_shell_html();
    let result = classify_and_extract(
        &html,
        "https://example.com/app",
        "text/html",
        &ClassifyOptions::default(),
    );
    assert!(result.summary.contains("JavaScript"));
}

// ---------------------------------------------------------------------------
// Auth wall classification
// ---------------------------------------------------------------------------

#[test]
fn test_auth_wall_detected_content_not_ok() {
    let html = auth_wall_html();
    let result = classify_and_extract(
        &html,
        "https://example.com/secure",
        "text/html",
        &ClassifyOptions::default(),
    );
    assert_eq!(result.page_type, PageType::AuthWall);
    assert!(!result.content_ok);
}

#[test]
fn test_auth_wall_report_contains_notice() {
    let html = auth_wall_html();
    let result = classify_and_extract(
        &html,
        "https://example.com/secure",
        "text/html",
        &ClassifyOptions::default(),
    );
    assert!(result.content.contains("Authentication required"));
}

#[test]
fn test_auth_wall_summary() {
    let html = auth_wall_html();
    let result = classify_and_extract(
        &html,
        "https://example.com/secure",
        "text/html",
        &ClassifyOptions::default(),
    );
    assert!(result.summary.contains("Login"));
}

// ---------------------------------------------------------------------------
// Paywall classification
// ---------------------------------------------------------------------------

#[test]
fn test_paywall_detected_content_not_ok() {
    let html = paywall_html();
    let result = classify_and_extract(
        &html,
        "https://example.com/article",
        "text/html",
        &ClassifyOptions::default(),
    );
    assert_eq!(result.page_type, PageType::Paywall);
    assert!(!result.content_ok);
}

#[test]
fn test_paywall_report_contains_notice() {
    let html = paywall_html();
    let result = classify_and_extract(
        &html,
        "https://example.com/article",
        "text/html",
        &ClassifyOptions::default(),
    );
    assert!(result.content.contains("Paywall"));
}

#[test]
fn test_paywall_summary() {
    let html = paywall_html();
    let result = classify_and_extract(
        &html,
        "https://example.com/article",
        "text/html",
        &ClassifyOptions::default(),
    );
    assert!(result.summary.contains("subscription"));
}

// ---------------------------------------------------------------------------
// Redirect classification
// ---------------------------------------------------------------------------

#[test]
fn test_redirect_detected() {
    let html = redirect_html("https://example.com/new-location");
    let result = classify_and_extract(
        &html,
        "https://example.com/old",
        "text/html",
        &ClassifyOptions::default(),
    );
    assert_eq!(result.page_type, PageType::Redirect);
}

#[test]
fn test_redirect_summary_mentions_redirect() {
    let html = redirect_html("https://example.com/new-location");
    let result = classify_and_extract(
        &html,
        "https://example.com/old",
        "text/html",
        &ClassifyOptions::default(),
    );
    assert!(result.summary.contains("Redirect"));
}

// ---------------------------------------------------------------------------
// Unknown / fallback
// ---------------------------------------------------------------------------

#[test]
fn test_unknown_page_with_minimal_html() {
    let html = r"<html><body><p>Hi</p></body></html>";
    let result = classify_and_extract(
        html,
        "https://example.com",
        "text/html",
        &ClassifyOptions::default(),
    );
    // Minimal text → content_ok is false.
    assert!(!result.content_ok);
}

#[test]
fn test_unknown_page_returns_extracted_text() {
    let html = r"<html><body><p>Hello world.</p></body></html>";
    let result = classify_and_extract(
        html,
        "https://example.com",
        "text/html",
        &ClassifyOptions::default(),
    );
    assert!(result.content.contains("Hello"));
}

// ---------------------------------------------------------------------------
// Non-HTML content
// ---------------------------------------------------------------------------

#[test]
fn test_non_html_content_passthrough() {
    let json = r#"{"key": "value", "items": [1, 2, 3]}"#;
    let result = classify_and_extract(
        json,
        "https://api.example.com/data",
        "application/json",
        &ClassifyOptions::default(),
    );
    // Non-HTML → returned as raw. Page type may be Json or Unknown.
    assert!(result.content.contains("key"));
}

#[test]
fn test_non_html_content_type_raw() {
    let plain = "This is plain text, not HTML.";
    let result = classify_and_extract(
        plain,
        "https://example.com/file.txt",
        "text/plain",
        &ClassifyOptions::default(),
    );
    assert!(result.content.contains("plain text"));
}

// ---------------------------------------------------------------------------
// Summary computation
// ---------------------------------------------------------------------------

#[test]
fn test_summary_from_first_sentence_when_no_metadata() {
    // HTML without og:description or meta description.
    let html = format!(
        r"<html><head><title>Test</title></head>
<body><article><p>{LONG_BODY}</p></article></body></html>"
    );
    let result = classify_and_extract(
        &html,
        "https://example.com/post",
        "text/html",
        &ClassifyOptions::default(),
    );
    // Summary should be the first sentence (ending at first period).
    assert!(result.summary.contains("substantial article"));
}

#[test]
fn test_summary_truncated_when_too_long() {
    let long_desc = "A".repeat(200);
    let html = format!(
        r#"<html><head>
<meta property="og:description" content="{long_desc}">
</head><body><article><p>{LONG_BODY}</p></article></body></html>"#
    );
    let result = classify_and_extract(
        &html,
        "https://example.com/post",
        "text/html",
        &ClassifyOptions::default(),
    );
    // Summary should be truncated to 120 chars + ellipsis.
    assert!(result.summary.ends_with('…'));
    assert!(result.summary.chars().count() <= 121); // 120 + ellipsis
}

#[test]
fn test_summary_empty_when_no_text_and_no_metadata() {
    let html = r"<html><head></head><body></body></html>";
    let result = classify_and_extract(
        html,
        "https://example.com",
        "text/html",
        &ClassifyOptions::default(),
    );
    assert!(result.summary.is_empty());
}

// ---------------------------------------------------------------------------
// Format options passthrough
// ---------------------------------------------------------------------------

#[test]
fn test_format_raw_passthrough() {
    let html = article_html("Test", LONG_BODY);
    let opts = ClassifyOptions {
        extract: ragent_tools_extended::masterfetch::extractor::ExtractOptions {
            format: OutputFormat::Raw,
            ..Default::default()
        },
        ..ClassifyOptions::default()
    };
    let result = classify_and_extract(&html, "https://example.com/post", "text/html", &opts);
    // Raw format returns HTML unchanged.
    assert!(result.content.contains("<html>"));
}

#[test]
fn test_format_text_strips_tags() {
    let html = article_html("Test", LONG_BODY);
    let opts = ClassifyOptions {
        extract: ragent_tools_extended::masterfetch::extractor::ExtractOptions {
            format: OutputFormat::Text,
            ..Default::default()
        },
        ..ClassifyOptions::default()
    };
    let result = classify_and_extract(&html, "https://example.com/post", "text/html", &opts);
    // Text format should not contain HTML tags.
    assert!(!result.content.contains("<article>"));
}

#[test]
fn test_extract_method_recorded() {
    let html = article_html("Test", LONG_BODY);
    let result = classify_and_extract(
        &html,
        "https://example.com/post",
        "text/html",
        &ClassifyOptions::default(),
    );
    // The method should be one of the valid ExtractMethod variants.
    assert_ne!(result.method, ExtractMethod::RawHtml);
}

// ---------------------------------------------------------------------------
// Link-list URL resolution edge cases
// ---------------------------------------------------------------------------

#[test]
fn test_absolute_urls_kept_as_is() {
    let mut links = String::new();
    for i in 1..=25 {
        links.push_str(&format!(
            "<li><a href=\"https://other.com/page{i}\">Page {i}</a></li>\n"
        ));
    }
    let html = format!(
        r"<html><head><title>Links</title></head>
<body><ul>{links}</ul></body></html>"
    );
    let result = classify_and_extract(
        &html,
        "https://example.com/links",
        "text/html",
        &ClassifyOptions::default(),
    );
    assert_eq!(result.page_type, PageType::List);
    assert!(result.content.contains("https://other.com/page1"));
}

#[test]
fn test_relative_urls_resolved() {
    let mut links = String::new();
    for i in 1..=25 {
        links.push_str(&format!("<li><a href=\"../page{i}\">Page {i}</a></li>\n"));
    }
    let html = format!(
        r"<html><head><title>Links</title></head>
<body><ul>{links}</ul></body></html>"
    );
    let result = classify_and_extract(
        &html,
        "https://example.com/sub/links",
        "text/html",
        &ClassifyOptions::default(),
    );
    assert_eq!(result.page_type, PageType::List);
    // ../page1 from https://example.com/sub/links → https://example.com/page1
    assert!(result.content.contains("https://example.com/page1"));
}

#[test]
fn test_empty_link_text_uses_url_as_label() {
    let mut links = String::new();
    for i in 1..=25 {
        links.push_str(&format!("<li><a href=\"/page{i}\"></a></li>\n"));
    }
    let html = format!(
        r"<html><head><title>Links</title></head>
<body><ul>{links}</ul></body></html>"
    );
    let result = classify_and_extract(
        &html,
        "https://example.com/links",
        "text/html",
        &ClassifyOptions::default(),
    );
    assert_eq!(result.page_type, PageType::List);
    // Empty text links should use the URL as the label.
    assert!(result.content.contains("https://example.com/page1]"));
}

// ---------------------------------------------------------------------------
// count_unique_links helper
// ---------------------------------------------------------------------------

#[test]
fn test_count_unique_links_basic() {
    let html = r#"<html><body>
<a href="/a">A</a>
<a href="/b">B</a>
<a href="/a">A again</a>
<a href="/c">C</a>
</body></html>"#;
    let metadata = ragent_tools_extended::masterfetch::PageMetadata::default();
    let count = count_unique_links(html, "https://example.com", &metadata);
    assert_eq!(count, 3); // /a, /b, /c (deduped)
}

#[test]
fn test_count_unique_links_empty_page() {
    let html = r"<html><body></body></html>";
    let metadata = ragent_tools_extended::masterfetch::PageMetadata::default();
    let count = count_unique_links(html, "https://example.com", &metadata);
    assert_eq!(count, 0);
}

// ---------------------------------------------------------------------------
// Default options
// ---------------------------------------------------------------------------

#[test]
fn test_default_options_work() {
    let html = article_html("Test", LONG_BODY);
    let result = classify_and_extract(
        &html,
        "https://example.com/post",
        "text/html",
        &ClassifyOptions::default(),
    );
    assert!(result.content_ok);
}

// ---------------------------------------------------------------------------
// Integration with focus (conceptual — classify output can be focused)
// ---------------------------------------------------------------------------

#[test]
fn test_classify_then_focus_article() {
    use ragent_tools_extended::masterfetch::focus::focus_content;

    let html = article_html("Test", LONG_BODY);
    let result = classify_and_extract(
        &html,
        "https://example.com/post",
        "text/html",
        &ClassifyOptions::default(),
    );
    // The classify output can be passed to focus_content for BM25 filtering.
    let focused = focus_content(&result.content, "article");
    assert!(!focused.is_empty());
}

// ---------------------------------------------------------------------------
// CrawlPage compatibility
// ---------------------------------------------------------------------------

#[test]
fn test_result_fields_populate_crawl_page() {
    let html = article_html("Test", LONG_BODY);
    let result = classify_and_extract(
        &html,
        "https://example.com/post",
        "text/html",
        &ClassifyOptions::default(),
    );

    // Verify all fields needed for CrawlPage are present.
    let _page_type = result.page_type;
    let _content_ok = result.content_ok;
    let _summary = &result.summary;
    let _content = &result.content;
    assert!(result.content_ok);
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[test]
fn test_empty_html() {
    let result = classify_and_extract(
        "",
        "https://example.com",
        "text/html",
        &ClassifyOptions::default(),
    );
    assert!(!result.content_ok);
}

#[test]
fn test_malformed_html_does_not_panic() {
    let html = r"<html><head><title>Broken<body><p>unclosed";
    let result = classify_and_extract(
        html,
        "https://example.com",
        "text/html",
        &ClassifyOptions::default(),
    );
    // Should not panic; content_ok depends on extracted text.
    let _ = result.content_ok;
}

#[test]
fn test_very_large_html_does_not_panic() {
    let body = format!("<p>{}</p>", "A".repeat(100_000));
    let html = format!(
        r"<html><head><title>Big</title></head>
<body><article>{body}</article></body></html>"
    );
    let result = classify_and_extract(
        &html,
        "https://example.com/big",
        "text/html",
        &ClassifyOptions::default(),
    );
    assert_eq!(result.page_type, PageType::Article);
}
