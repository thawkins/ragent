//! Integration tests for `masterfetch::extractor` — content extraction chain
//! (T-005, FR-002, NFR-003).
//!
//! Covers: format selection (markdown/html/text/raw), CSS selector narrowing,
//! readability → html2text → raw text chain, noise tag stripping, truncation,
//! non-HTML content, and error handling.

use ragent_tools_extended::masterfetch::extractor::{
    ExtractError, ExtractMethod, ExtractOptions, OutputFormat, extract,
};

// ---------------------------------------------------------------------------
// Helper: generate article-length HTML for readability tests
// ---------------------------------------------------------------------------

/// Build an HTML page with enough text to exceed the `MIN_READABILITY_CHARS`
/// threshold (500 chars).
fn article_html(body: &str) -> String {
    let long_text = body.repeat(20);
    format!(
        r"<html><head><title>Test Article</title></head>
<body>
<nav>Navigation links</nav>
<article>
<p>{long_text}</p>
</article>
<footer>Footer text</footer>
</body></html>"
    )
}

// ---------------------------------------------------------------------------
// Format selection
// ---------------------------------------------------------------------------

#[test]
fn test_format_raw_returns_html_unchanged() {
    let html = r"<html><body><p>Hello</p></body></html>";
    let opts = ExtractOptions {
        format: OutputFormat::Raw,
        ..Default::default()
    };
    let result = extract(html, "https://example.com", "text/html", &opts).unwrap();
    assert_eq!(result.content, html);
    assert_eq!(result.method, ExtractMethod::RawHtml);
    assert!(!result.is_truncated);
}

#[test]
fn test_format_text_strips_all_tags() {
    let html = r"<html><body><p>Hello <b>world</b></p><div>Foo</div></body></html>";
    let opts = ExtractOptions {
        format: OutputFormat::Text,
        ..Default::default()
    };
    let result = extract(html, "https://example.com", "text/html", &opts).unwrap();
    assert!(!result.content.contains('<'));
    assert!(!result.content.contains('>'));
    assert!(result.content.contains("Hello"));
    assert!(result.content.contains("world"));
    assert!(result.content.contains("Foo"));
    assert_eq!(result.method, ExtractMethod::StrippedText);
}

#[test]
fn test_format_html_strips_noise_tags() {
    let html = r"<html><head><script>alert(1)</script></head>
<body><nav>Nav</nav><p>Content</p><footer>Foot</footer></body></html>";
    let opts = ExtractOptions {
        format: OutputFormat::Html,
        ..Default::default()
    };
    let result = extract(html, "https://example.com", "text/html", &opts).unwrap();
    assert!(!result.content.contains("alert(1)"));
    assert!(!result.content.contains("Nav"));
    assert!(!result.content.contains("Foot"));
    assert!(result.content.contains("Content"));
    assert_eq!(result.method, ExtractMethod::CleanedHtml);
}

#[test]
fn test_format_markdown_default() {
    let html = article_html("This is a test paragraph with some content. ");
    let opts = ExtractOptions::default();
    let result = extract(&html, "https://example.com", "text/html", &opts).unwrap();
    assert!(!result.content.is_empty());
    // Should have used one of the extraction chain methods.
    assert!(
        result.method == ExtractMethod::Readability
            || result.method == ExtractMethod::Html2Text
            || result.method == ExtractMethod::RawText
    );
}

#[test]
fn test_format_from_str_defaults_to_markdown() {
    assert_eq!(
        OutputFormat::parse_format("markdown"),
        OutputFormat::Markdown
    );
    assert_eq!(OutputFormat::parse_format("html"), OutputFormat::Html);
    assert_eq!(OutputFormat::parse_format("text"), OutputFormat::Text);
    assert_eq!(OutputFormat::parse_format("raw"), OutputFormat::Raw);
    assert_eq!(
        OutputFormat::parse_format("unknown"),
        OutputFormat::Markdown
    );
    assert_eq!(OutputFormat::parse_format(""), OutputFormat::Markdown);
}

#[test]
fn test_format_display() {
    assert_eq!(OutputFormat::Markdown.to_string(), "markdown");
    assert_eq!(OutputFormat::Html.to_string(), "html");
    assert_eq!(OutputFormat::Text.to_string(), "text");
    assert_eq!(OutputFormat::Raw.to_string(), "raw");
}

// ---------------------------------------------------------------------------
// Non-HTML content
// ---------------------------------------------------------------------------

#[test]
fn test_non_html_content_returns_raw() {
    let json_body = r#"{"key": "value"}"#;
    let opts = ExtractOptions::default();
    let result = extract(
        json_body,
        "https://example.com/api",
        "application/json",
        &opts,
    )
    .unwrap();
    assert_eq!(result.content, json_body);
    assert_eq!(result.method, ExtractMethod::RawHtml);
}

#[test]
fn test_xhtml_is_treated_as_html() {
    let html = r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>Test</p></body></html>"#;
    let opts = ExtractOptions {
        format: OutputFormat::Text,
        ..Default::default()
    };
    let result = extract(html, "https://example.com", "application/xhtml+xml", &opts).unwrap();
    assert!(result.content.contains("Test"));
    assert!(!result.content.contains('<'));
}

// ---------------------------------------------------------------------------
// Readability extraction chain
// ---------------------------------------------------------------------------

#[test]
fn test_readability_extracts_long_article() {
    let html = article_html("This is a substantial article paragraph that should be extracted. ");
    let opts = ExtractOptions::default();
    let result = extract(&html, "https://example.com", "text/html", &opts).unwrap();
    // Readability should succeed on article-length content.
    if result.method == ExtractMethod::Readability {
        assert!(result.title.is_some());
        assert!(result.content.contains("substantial"));
    }
    // Either way, content should not be empty.
    assert!(!result.content.is_empty());
}

#[test]
fn test_short_html_falls_back_to_html2text() {
    let html = r"<html><body><p>Short</p></body></html>";
    let opts = ExtractOptions::default();
    let result = extract(html, "https://example.com", "text/html", &opts).unwrap();
    // Short content should not trigger readability (under 500 chars).
    // It should fall to html2text or raw_text.
    assert!(result.method == ExtractMethod::Html2Text || result.method == ExtractMethod::RawText);
    assert!(result.content.contains("Short"));
}

#[test]
fn test_empty_html_returns_empty_content() {
    let html = "";
    let opts = ExtractOptions::default();
    let result = extract(html, "https://example.com", "text/html", &opts).unwrap();
    // Should not panic; content may be empty.
    assert!(result.content.is_empty() || result.content.trim().is_empty());
}

// ---------------------------------------------------------------------------
// CSS selector narrowing
// ---------------------------------------------------------------------------

#[test]
fn test_css_selector_by_tag() {
    let html = r"<html><body>
<nav>Navigation</nav>
<article><p>Article content here that is long enough to be meaningful.</p></article>
<footer>Footer</footer>
</body></html>";
    let opts = ExtractOptions {
        css_selector: Some("article".to_string()),
        ..Default::default()
    };
    let result = extract(html, "https://example.com", "text/html", &opts).unwrap();
    assert!(result.content.contains("Article content"));
    assert!(!result.content.contains("Navigation"));
    assert!(!result.content.contains("Footer"));
}

#[test]
fn test_css_selector_by_class() {
    let html = r#"<html><body>
<div class="sidebar">Sidebar content</div>
<div class="main"><p>Main content that is relevant.</p></div>
</body></html>"#;
    let opts = ExtractOptions {
        css_selector: Some(".main".to_string()),
        ..Default::default()
    };
    let result = extract(html, "https://example.com", "text/html", &opts).unwrap();
    assert!(result.content.contains("Main content"));
    assert!(!result.content.contains("Sidebar"));
}

#[test]
fn test_css_selector_by_id() {
    let html = r#"<html><body>
<div id="header">Header</div>
<div id="content"><p>The actual content.</p></div>
</body></html>"#;
    let opts = ExtractOptions {
        css_selector: Some("#content".to_string()),
        ..Default::default()
    };
    let result = extract(html, "https://example.com", "text/html", &opts).unwrap();
    assert!(result.content.contains("actual content"));
    assert!(!result.content.contains("Header"));
}

#[test]
fn test_css_selector_compound_tag_class() {
    let html = r#"<html><body>
<div class="other">Other</div>
<article class="post"><p>Post content.</p></article>
</body></html>"#;
    let opts = ExtractOptions {
        css_selector: Some("article.post".to_string()),
        ..Default::default()
    };
    let result = extract(html, "https://example.com", "text/html", &opts).unwrap();
    assert!(result.content.contains("Post content"));
    assert!(!result.content.contains("Other"));
}

#[test]
fn test_css_selector_compound_tag_id() {
    let html = r#"<html><body>
<div id="sidebar">Sidebar</div>
<main id="primary"><p>Primary content.</p></main>
</body></html>"#;
    let opts = ExtractOptions {
        css_selector: Some("main#primary".to_string()),
        ..Default::default()
    };
    let result = extract(html, "https://example.com", "text/html", &opts).unwrap();
    assert!(result.content.contains("Primary content"));
    assert!(!result.content.contains("Sidebar"));
}

#[test]
fn test_css_selector_no_match_returns_full_html() {
    let html = r"<html><body><p>Content</p></body></html>";
    let opts = ExtractOptions {
        css_selector: Some(".nonexistent".to_string()),
        ..Default::default()
    };
    let result = extract(html, "https://example.com", "text/html", &opts).unwrap();
    // Graceful degradation: should return content from full HTML.
    assert!(result.content.contains("Content"));
}

#[test]
fn test_css_selector_empty_returns_error() {
    let html = r"<html><body><p>Content</p></body></html>";
    let opts = ExtractOptions {
        css_selector: Some("   ".to_string()),
        ..Default::default()
    };
    let result = extract(html, "https://example.com", "text/html", &opts);
    assert!(matches!(result, Err(ExtractError::InvalidSelector(_))));
}

#[test]
fn test_css_selector_with_descendant_combinator_returns_error() {
    let html = r"<html><body><p>Content</p></body></html>";
    let opts = ExtractOptions {
        css_selector: Some("div p".to_string()),
        ..Default::default()
    };
    let result = extract(html, "https://example.com", "text/html", &opts);
    assert!(matches!(result, Err(ExtractError::InvalidSelector(_))));
}

#[test]
fn test_css_selector_with_child_combinator_returns_error() {
    let html = r"<html><body><p>Content</p></body></html>";
    let opts = ExtractOptions {
        css_selector: Some("div > p".to_string()),
        ..Default::default()
    };
    let result = extract(html, "https://example.com", "text/html", &opts);
    assert!(matches!(result, Err(ExtractError::InvalidSelector(_))));
}

#[test]
fn test_css_selector_with_multiple_selectors_returns_error() {
    let html = r"<html><body><p>Content</p></body></html>";
    let opts = ExtractOptions {
        css_selector: Some("div, p".to_string()),
        ..Default::default()
    };
    let result = extract(html, "https://example.com", "text/html", &opts);
    assert!(matches!(result, Err(ExtractError::InvalidSelector(_))));
}

#[test]
fn test_css_selector_nested_elements() {
    let html = r#"<html><body>
<article>
<h1>Title</h1>
<div class="content">
<p>Inner paragraph one.</p>
<p>Inner paragraph two.</p>
</div>
</article>
</body></html>"#;
    let opts = ExtractOptions {
        css_selector: Some("div.content".to_string()),
        ..Default::default()
    };
    let result = extract(html, "https://example.com", "text/html", &opts).unwrap();
    assert!(result.content.contains("Inner paragraph one"));
    assert!(result.content.contains("Inner paragraph two"));
    assert!(!result.content.contains("Title"));
}

// ---------------------------------------------------------------------------
// Noise tag stripping (format=html)
// ---------------------------------------------------------------------------

#[test]
fn test_noise_tags_stripped_from_html() {
    let html = r#"<html><head>
<script>var x = 1;</script>
<style>body { color: red; }</style>
</head><body>
<noscript>Enable JS</noscript>
<nav>Nav links</nav>
<aside>Sidebar</aside>
<footer>Footer</footer>
<iframe src="ad.html"></iframe>
<p>Real content</p>
</body></html>"#;
    let opts = ExtractOptions {
        format: OutputFormat::Html,
        ..Default::default()
    };
    let result = extract(html, "https://example.com", "text/html", &opts).unwrap();
    assert!(result.content.contains("Real content"));
    assert!(!result.content.contains("var x"));
    assert!(!result.content.contains("color: red"));
    assert!(!result.content.contains("Enable JS"));
    assert!(!result.content.contains("Nav links"));
    assert!(!result.content.contains("Sidebar"));
    assert!(!result.content.contains("Footer"));
    assert!(!result.content.contains("ad.html"));
}

#[test]
fn test_noise_tags_stripped_preserves_other_tags() {
    let html = r"<html><body>
<script>bad</script>
<h1>Heading</h1>
<p>Paragraph</p>
<ul><li>Item</li></ul>
</body></html>";
    let opts = ExtractOptions {
        format: OutputFormat::Html,
        ..Default::default()
    };
    let result = extract(html, "https://example.com", "text/html", &opts).unwrap();
    assert!(result.content.contains("<h1>Heading</h1>"));
    assert!(result.content.contains("<p>Paragraph</p>"));
    assert!(result.content.contains("<li>Item</li>"));
    assert!(!result.content.contains("bad"));
}

// ---------------------------------------------------------------------------
// Truncation
// ---------------------------------------------------------------------------

#[test]
fn test_truncation_at_max_content_chars() {
    let long_body = "A".repeat(1000);
    let html = format!("<html><body><p>{long_body}</p></body></html>");
    let opts = ExtractOptions {
        format: OutputFormat::Text,
        max_content_chars: 100,
        ..Default::default()
    };
    let result = extract(&html, "https://example.com", "text/html", &opts).unwrap();
    assert!(result.is_truncated);
    assert!(result.content.contains("[Content truncated]"));
}

#[test]
fn test_no_truncation_when_under_limit() {
    let html = r"<html><body><p>Short content</p></body></html>";
    let opts = ExtractOptions {
        format: OutputFormat::Text,
        max_content_chars: 40_000,
        ..Default::default()
    };
    let result = extract(html, "https://example.com", "text/html", &opts).unwrap();
    assert!(!result.is_truncated);
}

#[test]
fn test_max_content_chars_clamped_to_minimum() {
    // Setting max_content_chars below MIN_MAX_CONTENT_CHARS (500) should be
    // clamped up, so content under 500 chars should not be truncated.
    let html = r"<html><body><p>Short</p></body></html>";
    let opts = ExtractOptions {
        format: OutputFormat::Text,
        max_content_chars: 10, // below minimum
        ..Default::default()
    };
    let result = extract(html, "https://example.com", "text/html", &opts).unwrap();
    // The clamp ensures we don't truncate at 10 chars.
    // "Short" is 5 chars, well under the clamped 500 minimum.
    assert!(result.content.contains("Short"));
}

#[test]
fn test_truncation_preserves_char_boundary() {
    // Ensure truncation doesn't split a multi-byte UTF-8 character.
    let long_body = "日本語テスト".repeat(200);
    let html = format!("<html><body><p>{long_body}</p></body></html>");
    let opts = ExtractOptions {
        format: OutputFormat::Text,
        max_content_chars: 100,
        ..Default::default()
    };
    let result = extract(&html, "https://example.com", "text/html", &opts).unwrap();
    assert!(result.is_truncated);
    // The content should be valid UTF-8 (no panic = success).
    assert!(result.content.contains("[Content truncated]"));
}

// ---------------------------------------------------------------------------
// Total chars and metadata
// ---------------------------------------------------------------------------

#[test]
fn test_total_chars_reflects_pre_truncation_length() {
    let body = "B".repeat(600);
    let html = format!("<html><body><p>{body}</p></body></html>");
    let opts = ExtractOptions {
        format: OutputFormat::Text,
        max_content_chars: 100,
        ..Default::default()
    };
    let result = extract(&html, "https://example.com", "text/html", &opts).unwrap();
    assert!(result.total_chars > 100);
    assert!(result.is_truncated);
}

// ---------------------------------------------------------------------------
// Extract method display
// ---------------------------------------------------------------------------

#[test]
fn test_extract_method_display() {
    assert_eq!(ExtractMethod::Readability.to_string(), "readability");
    assert_eq!(ExtractMethod::Html2Text.to_string(), "html2text");
    assert_eq!(ExtractMethod::RawText.to_string(), "raw_text");
    assert_eq!(ExtractMethod::RawHtml.to_string(), "raw_html");
    assert_eq!(ExtractMethod::CleanedHtml.to_string(), "cleaned_html");
    assert_eq!(ExtractMethod::StrippedText.to_string(), "stripped_text");
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[test]
fn test_malformed_html_does_not_panic() {
    let html = r"<html><body><p>Unclosed paragraph<div>Nested</body></html>";
    let opts = ExtractOptions::default();
    let result = extract(html, "https://example.com", "text/html", &opts);
    assert!(result.is_ok());
}

#[test]
fn test_html_with_only_script_tags() {
    let html = r"<html><head><script>alert(1)</script></head><body></body></html>";
    let opts = ExtractOptions::default();
    let result = extract(html, "https://example.com", "text/html", &opts).unwrap();
    // Should produce some content (even if empty) without panicking.
    assert!(result.content.is_empty() || !result.content.contains("alert"));
}

#[test]
fn test_css_selector_with_text_format() {
    let html = r#"<html><body>
<div id="main"><p>Main text content here.</p></div>
<div id="nav">Navigation text</div>
</body></html>"#;
    let opts = ExtractOptions {
        format: OutputFormat::Text,
        css_selector: Some("#main".to_string()),
        ..Default::default()
    };
    let result = extract(html, "https://example.com", "text/html", &opts).unwrap();
    assert!(result.content.contains("Main text content"));
    assert!(!result.content.contains("Navigation"));
    assert!(!result.content.contains('<'));
    assert_eq!(result.method, ExtractMethod::StrippedText);
}

#[test]
fn test_css_selector_with_html_format() {
    let html = r#"<html><body>
<div id="content">
<script>noise()</script>
<p>Good content</p>
</div>
</body></html>"#;
    let opts = ExtractOptions {
        format: OutputFormat::Html,
        css_selector: Some("#content".to_string()),
        ..Default::default()
    };
    let result = extract(html, "https://example.com", "text/html", &opts).unwrap();
    assert!(result.content.contains("Good content"));
    assert!(!result.content.contains("noise()"));
    assert_eq!(result.method, ExtractMethod::CleanedHtml);
}

#[test]
fn test_css_selector_with_raw_format_ignores_selector() {
    // format=raw should return the full HTML regardless of css_selector.
    let html = r#"<html><body><div id="x">Content</div></body></html>"#;
    let opts = ExtractOptions {
        format: OutputFormat::Raw,
        css_selector: Some("#x".to_string()),
        ..Default::default()
    };
    let result = extract(html, "https://example.com", "text/html", &opts).unwrap();
    assert_eq!(result.content, html);
    assert_eq!(result.method, ExtractMethod::RawHtml);
}

#[test]
fn test_self_closing_tags_preserved_in_html_format() {
    let html = r#"<html><body><p>Text<br/>Line<img src="x.png"/>More</p></body></html>"#;
    let opts = ExtractOptions {
        format: OutputFormat::Html,
        ..Default::default()
    };
    let result = extract(html, "https://example.com", "text/html", &opts).unwrap();
    // br and img are self-closing/void — should appear in output.
    assert!(result.content.contains("Text"));
    assert!(result.content.contains("More"));
}

#[test]
fn test_text_format_collapses_whitespace() {
    let html = r"<html><body>
<p>  Multiple    spaces   </p>
<div>
\n\n\nNewlines\n\n\n</div>
</body></html>";
    let opts = ExtractOptions {
        format: OutputFormat::Text,
        ..Default::default()
    };
    let result = extract(html, "https://example.com", "text/html", &opts).unwrap();
    // Should not contain multiple consecutive spaces.
    assert!(!result.content.contains("  "));
}

// ---------------------------------------------------------------------------
// Panic-isolation regression
// ---------------------------------------------------------------------------

/// Regression test for the html2text overflow panic seen on real mdBook pages.
///
/// The `rust-book.cs.brown.edu` HTML triggers a debug-mode
/// `attempt to subtract with overflow` panic inside `html2text` 0.16.7's
/// `text_renderer.rs:509` when used as the html2text fallback. The extractor
/// must catch (or never expose) that panic and degrade to raw text rather than
/// crashing the calling web gatherer.
#[test]
fn test_html2text_overflow_panic_degrades_to_raw_text() {
    let html = include_str!("fixtures/rust_book_brown.html");
    let opts = ExtractOptions::default();
    let result = extract(html, "https://rust-book.cs.brown.edu/", "text/html", &opts);

    // Must not propagate the panic.
    let result = result.expect("extractor should return Ok on a panic-prone page");

    // Readability produces very short text on this page, so we should end up
    // with a non-empty fallback (either html2text or raw text). The exact method
    // depends on whether the inner html2text catch_unwind succeeded; what
    // matters is that the result is usable and the process survived.
    assert!(
        !result.content.is_empty(),
        "fallback content should not be empty"
    );
    assert!(
        result.method == ExtractMethod::Html2Text || result.method == ExtractMethod::RawText,
        "expected fallback method, got {:?}",
        result.method
    );
}

// ---------------------------------------------------------------------------
// Regression: html2text `flush_word` subtraction overflow (panic log
// 20260820-092513). A table cell content wider than the render width used
// to panic with "attempt to subtract with overflow" in debug builds.
// ---------------------------------------------------------------------------

#[test]
fn test_wide_table_cell_does_not_panic() {
    let wide_a = "a".repeat(160);
    let wide_b = "b".repeat(160);
    let html = format!(
        "<html><body>\
         <p>before</p>\
         <table width=\"100%\">\
         <tr><td>{wide_a} {wide_b}</td></tr>\
         </table>\
         <p>after</p>\
         </body></html>"
    );
    let opts = ExtractOptions::default();
    let result = extract(&html, "https://example.com", "text/html", &opts).unwrap();
    assert!(result.content.contains("before"));
    assert!(result.content.contains("aaaa"));
    assert!(result.content.contains("after"));
}
