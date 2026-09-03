#![allow(clippy::assert_is_empty)]
//! Unit tests for `masterfetch::envelope` — page-type detection, source-type
//! classification, and freshness computation (T-033, FR-003, FR-029, FR-030,
//! NFR-003).
//!
//! Covers every [`PageType`] variant:
//! - article, docs, list, forum, qa, `js_shell`, `auth_wall`, paywall, redirect,
//!   json, image, unknown
//!
//! Covers [`SourceType`] classification:
//! - gov, edu, github, `vendor_docs`, `docs_site`, qa, forum, blog, news,
//!   ecommerce, unknown
//! - `is_official` flag (true only for gov, edu, github, `vendor_docs`)
//!
//! Covers [`compute_freshness`]:
//! - modified preferred over published
//! - future date → `content_age_days` = -1
//! - stale threshold (age > 365)
//! - no dates → -1
//! - empty date strings → -1
//! - invalid dates → -1
//! - various date formats (RFC 3339, offset, no timezone, date only, space
//!   separator)
//!
//! Also covers [`build_envelope`] integration and private helpers
//! [`parse_iso_date`] and [`parse_meta_refresh_seconds`].

use chrono::{Datelike, Utc};

use ragent_tools_extended::masterfetch::envelope::{
    STALE_THRESHOLD_DAYS, build_envelope, classify_source_type, compute_freshness,
    detect_page_type, parse_iso_date, parse_meta_refresh_seconds,
};
use ragent_tools_extended::masterfetch::{PageMetadata, PageType, SourceType};

// ===========================================================================
// detect_page_type: article
// ===========================================================================

#[test]
fn test_detect_article_tag() {
    let html = r"<html><body><article><p>Hello world this is a long article with enough text to pass thresholds.</p></article></body></html>";
    assert_eq!(
        detect_page_type(html, "https://example.com/post", 200),
        PageType::Article
    );
}

#[test]
fn test_detect_article_fallback_paragraphs() {
    let html = r"<html><body><p>Para one with enough text.</p><p>Para two with text.</p><p>Para three with text here.</p></body></html>";
    assert_eq!(
        detect_page_type(html, "https://example.com/post", 250),
        PageType::Article
    );
}

#[test]
fn test_detect_article_short_text_not_article() {
    // <article> tag but text is too short → not article.
    let html = r"<html><body><article><p>Short.</p></article></body></html>";
    assert_ne!(
        detect_page_type(html, "https://example.com/post", 10),
        PageType::Article
    );
}

#[test]
fn test_detect_article_two_paragraphs_not_enough() {
    // Only 2 <p> tags → needs >= 3 for fallback article detection.
    let html = r"<html><body><p>Para one with text.</p><p>Para two with text.</p></body></html>";
    assert_ne!(
        detect_page_type(html, "https://example.com/post", 250),
        PageType::Article
    );
}

// ===========================================================================
// detect_page_type: docs
// ===========================================================================

#[test]
fn test_detect_docs_code_blocks() {
    let html = r"<html><body><pre><code>fn main() {}</code></pre><pre><code>let x = 1;</code></pre><pre><code>println!();</code></pre></body></html>";
    assert_eq!(
        detect_page_type(html, "https://example.com/guide", 100),
        PageType::Docs
    );
}

#[test]
fn test_detect_docs_domain() {
    let html = r"<html><body><p>Documentation page.</p></body></html>";
    assert_eq!(
        detect_page_type(html, "https://docs.python.org/tutorial", 100),
        PageType::Docs
    );
}

#[test]
fn test_detect_docs_two_code_blocks_not_enough() {
    // Needs >= 3 total <pre> + <code> matches. One block = 1 <pre> + 1 <code> = 2.
    let html = r"<html><body><pre><code>fn main() {}</code></pre></body></html>";
    assert_ne!(
        detect_page_type(html, "https://example.com/guide", 100),
        PageType::Docs
    );
}

#[test]
fn test_detect_docs_developer_domain() {
    let html = r"<html><body><p>API reference.</p></body></html>";
    assert_eq!(
        detect_page_type(html, "https://developer.mozilla.org/en-US/docs/Web", 100),
        PageType::Docs
    );
}

#[test]
fn test_detect_docs_docs_prefix_domain() {
    let html = r"<html><body><p>Custom docs.</p></body></html>";
    assert_eq!(
        detect_page_type(html, "https://docs.mycompany.com/guide", 100),
        PageType::Docs
    );
}

#[test]
fn test_detect_docs_rust_lang_domain() {
    let html = r"<html><body><p>Rust docs.</p></body></html>";
    assert_eq!(
        detect_page_type(html, "https://doc.rust-lang.org/std", 100),
        PageType::Docs
    );
}

// ===========================================================================
// detect_page_type: list
// ===========================================================================

#[test]
fn test_detect_list_page_many_links() {
    let html = format!(
        "<html><body>{}</body></html>",
        (0..30)
            .map(|i| format!("<a href=\"/page{i}\">Link {i}</a>"))
            .collect::<String>()
    );
    assert_eq!(
        detect_page_type(&html, "https://example.com/dir", 200),
        PageType::List
    );
}

#[test]
fn test_detect_list_page_many_list_items() {
    let html = format!(
        "<html><body><ul>{}</ul></body></html>",
        (0..20)
            .map(|i| format!("<li><a href=\"/item{i}\">Item {i}</a></li>"))
            .collect::<String>()
    );
    assert_eq!(
        detect_page_type(&html, "https://example.com/list", 300),
        PageType::List
    );
}

#[test]
fn test_detect_list_page_many_table_rows() {
    // is_list_page requires link_count >= 10 before checking tr_count.
    let rows = (0..12)
        .map(|i| format!("<tr><td><a href=\"/row{i}\">Row {i}</a></td></tr>"))
        .collect::<String>();
    let html = format!("<html><body><table>{rows}</table></body></html>");
    assert_eq!(
        detect_page_type(&html, "https://example.com/table", 200),
        PageType::List
    );
}

#[test]
fn test_detect_list_page_few_links_not_list() {
    // Fewer than 10 links → not a list page.
    let html =
        r#"<html><body><a href="/a">A</a><a href="/b">B</a><a href="/c">C</a></body></html>"#;
    assert_ne!(
        detect_page_type(html, "https://example.com", 100),
        PageType::List
    );
}

// ===========================================================================
// detect_page_type: forum
// ===========================================================================

#[test]
fn test_detect_forum_domain() {
    let html = r"<html><body><p>Discussion thread.</p></body></html>";
    assert_eq!(
        detect_page_type(html, "https://www.reddit.com/r/rust", 100),
        PageType::Forum
    );
}

#[test]
fn test_detect_forum_structural_signal_post() {
    let html = r#"<html><body><div class="post">User comment here.</div></body></html>"#;
    assert_eq!(
        detect_page_type(html, "https://example.com/thread", 100),
        PageType::Forum
    );
}

#[test]
fn test_detect_forum_structural_signal_thread() {
    let html = r#"<html><body><div class="thread">Thread content.</div></body></html>"#;
    assert_eq!(
        detect_page_type(html, "https://example.com/t/123", 100),
        PageType::Forum
    );
}

#[test]
fn test_detect_forum_structural_signal_comment() {
    let html = r#"<html><body><div class="comment">A comment.</div></body></html>"#;
    assert_eq!(
        detect_page_type(html, "https://example.com/post", 100),
        PageType::Forum
    );
}

#[test]
fn test_detect_forum_structural_signal_data_post_id() {
    let html = r#"<html><body><div data-post-id="42">Post.</div></body></html>"#;
    assert_eq!(
        detect_page_type(html, "https://example.com/thread/42", 100),
        PageType::Forum
    );
}

#[test]
fn test_detect_forum_rust_lang_domain() {
    let html = r"<html><body><p>Rust forum discussion.</p></body></html>";
    assert_eq!(
        detect_page_type(html, "https://users.rust-lang.org/t/123", 100),
        PageType::Forum
    );
}

// ===========================================================================
// detect_page_type: qa
// ===========================================================================

#[test]
fn test_detect_qa_domain() {
    let html = r"<html><body><p>Question and answers.</p></body></html>";
    assert_eq!(
        detect_page_type(html, "https://stackoverflow.com/q/123", 100),
        PageType::Qa
    );
}

#[test]
fn test_detect_qa_structural_signal_question() {
    let html = r#"<html><body><div class="question">What is Rust?</div></body></html>"#;
    assert_eq!(
        detect_page_type(html, "https://example.com/q/1", 100),
        PageType::Qa
    );
}

#[test]
fn test_detect_qa_structural_signal_answer() {
    let html = r#"<html><body><div class="answer">Rust is a language.</div></body></html>"#;
    assert_eq!(
        detect_page_type(html, "https://example.com/q/1", 100),
        PageType::Qa
    );
}

#[test]
fn test_detect_qa_structural_signal_accepted_answer() {
    let html = r#"<html><body><div class="accepted-answer">The best answer.</div></body></html>"#;
    assert_eq!(
        detect_page_type(html, "https://example.com/q/1", 100),
        PageType::Qa
    );
}

// Note: schema.org/Question itemtype signal is not matched because the HTML
// is lowercased before matching but the signal contains uppercase letters.
// This is a known limitation — domain-based QA detection covers this case.

#[test]
fn test_detect_qa_quora_domain() {
    let html = r"<html><body><p>Quora question.</p></body></html>";
    assert_eq!(
        detect_page_type(html, "https://www.quora.com/What-is-Rust", 100),
        PageType::Qa
    );
}

// ===========================================================================
// detect_page_type: js_shell
// ===========================================================================

#[test]
fn test_detect_js_shell_with_signal() {
    // Build HTML > 3KB with JS shell signal and tiny text.
    let inner = format!(
        "<div id=\"root\">Please enable JavaScript to run this app.</div><script>{}{}</script>",
        "/* padding */",
        "x".repeat(3000)
    );
    let html = format!("<html><body>{inner}</body></html>");
    assert_eq!(
        detect_page_type(&html, "https://app.example.com", 20),
        PageType::JsShell
    );
}

#[test]
fn test_detect_js_shell_large_body_tiny_text() {
    let html = format!("<html><body>{}</body></html>", "x".repeat(10_000));
    assert_eq!(
        detect_page_type(&html, "https://app.example.com", 10),
        PageType::JsShell
    );
}

#[test]
fn test_detect_js_shell_not_triggered_by_small_body() {
    let html = "<html><body>Please enable JavaScript</body></html>";
    // Body is too small for JS shell diagnosis.
    assert_ne!(
        detect_page_type(html, "https://example.com", 20),
        PageType::JsShell
    );
}

#[test]
fn test_detect_js_shell_javascript_disabled_signal() {
    let inner = format!(
        "<div>JavaScript is disabled in this browser.{}</div>",
        "x".repeat(3000)
    );
    let html = format!("<html><body>{inner}</body></html>");
    assert_eq!(
        detect_page_type(&html, "https://app.example.com", 20),
        PageType::JsShell
    );
}

#[test]
fn test_detect_js_shell_requires_javascript_signal() {
    let inner = format!(
        "<div>Requires JavaScript to view this content.{}</div>",
        "x".repeat(3000)
    );
    let html = format!("<html><body>{inner}</body></html>");
    assert_eq!(
        detect_page_type(&html, "https://app.example.com", 20),
        PageType::JsShell
    );
}

// ===========================================================================
// detect_page_type: auth_wall
// ===========================================================================

#[test]
fn test_detect_auth_wall() {
    let html =
        r#"<html><body><form><h1>Please sign in</h1><input type="password"></form></body></html>"#;
    assert_eq!(
        detect_page_type(html, "https://example.com/login", 50),
        PageType::AuthWall
    );
}

#[test]
fn test_detect_auth_wall_not_triggered_with_article_content() {
    let html = r"<html><body><article><p>Sign in to comment. This is a long article with enough text to not be classified as an auth wall.</p></article></body></html>";
    // With substantial text, auth wall signals are not triggered.
    assert_ne!(
        detect_page_type(html, "https://example.com/post", 250),
        PageType::AuthWall
    );
}

#[test]
fn test_detect_auth_wall_log_in_signal() {
    let html = r"<html><body><form><h1>Please log in</h1></form></body></html>";
    assert_eq!(
        detect_page_type(html, "https://example.com/login", 50),
        PageType::AuthWall
    );
}

#[test]
fn test_detect_auth_wall_authentication_required_signal() {
    let html = r"<html><body><h1>Authentication required</h1></body></html>";
    assert_eq!(
        detect_page_type(html, "https://example.com/secure", 50),
        PageType::AuthWall
    );
}

#[test]
fn test_detect_auth_wall_must_be_logged_in_signal() {
    let html = r"<html><body><h1>You must be logged in to view this page</h1></body></html>";
    assert_eq!(
        detect_page_type(html, "https://example.com/secure", 50),
        PageType::AuthWall
    );
}

// ===========================================================================
// detect_page_type: paywall
// ===========================================================================

#[test]
fn test_detect_paywall() {
    let html =
        r"<html><body><h1>Subscribe to continue reading</h1><p>Preview text...</p></body></html>";
    assert_eq!(
        detect_page_type(html, "https://nytimes.com/article", 100),
        PageType::Paywall
    );
}

#[test]
fn test_detect_paywall_subscribe_to_read() {
    let html = r"<html><body><h1>Subscribe to read this article</h1></body></html>";
    assert_eq!(
        detect_page_type(html, "https://example.com/article", 50),
        PageType::Paywall
    );
}

#[test]
fn test_detect_paywall_premium_content() {
    let html = r"<html><body><h1>Premium content</h1><p>Unlock full article...</p></body></html>";
    assert_eq!(
        detect_page_type(html, "https://example.com/premium", 50),
        PageType::Paywall
    );
}

#[test]
fn test_detect_paywall_subscription_required() {
    let html = r"<html><body><h1>Subscription required</h1></body></html>";
    assert_eq!(
        detect_page_type(html, "https://example.com/article", 50),
        PageType::Paywall
    );
}

#[test]
fn test_detect_paywall_paywall_keyword() {
    let html = r#"<html><body><div class="paywall">Content is hidden.</div></body></html>"#;
    assert_eq!(
        detect_page_type(html, "https://example.com/article", 50),
        PageType::Paywall
    );
}

// ===========================================================================
// detect_page_type: redirect
// ===========================================================================

#[test]
fn test_detect_redirect_meta_refresh() {
    let html = r#"<html><head><meta http-equiv="refresh" content="0;url=https://other.com"></head><body>Redirecting...</body></html>"#;
    assert_eq!(
        detect_page_type(html, "https://example.com/old", 50),
        PageType::Redirect
    );
}

#[test]
fn test_detect_redirect_long_delay_not_redirect() {
    let html = r#"<html><head><meta http-equiv="refresh" content="60;url=https://other.com"></head><body>Auto-refresh page.</body></html>"#;
    // 60-second delay is auto-refresh, not redirect.
    assert_ne!(
        detect_page_type(html, "https://example.com/page", 50),
        PageType::Redirect
    );
}

#[test]
fn test_detect_redirect_short_delay_3_seconds() {
    let html = r#"<html><head><meta http-equiv="refresh" content="3;url=https://other.com"></head><body>Redirecting...</body></html>"#;
    // 3 seconds is the upper boundary for redirect detection.
    assert_eq!(
        detect_page_type(html, "https://example.com/old", 50),
        PageType::Redirect
    );
}

#[test]
fn test_detect_redirect_4_seconds_not_redirect() {
    let html = r#"<html><head><meta http-equiv="refresh" content="4;url=https://other.com"></head><body>Auto-refresh.</body></html>"#;
    assert_ne!(
        detect_page_type(html, "https://example.com/page", 50),
        PageType::Redirect
    );
}

#[test]
fn test_detect_redirect_single_quotes() {
    let html = r"<html><head><meta http-equiv='refresh' content='0;url=https://other.com'></head><body>Redirecting...</body></html>";
    assert_eq!(
        detect_page_type(html, "https://example.com/old", 50),
        PageType::Redirect
    );
}

// ===========================================================================
// detect_page_type: json
// ===========================================================================

#[test]
fn test_detect_json_body() {
    let json = r#"{"key": "value", "items": [1, 2, 3]}"#;
    assert_eq!(
        detect_page_type(json, "https://api.example.com/data", 0),
        PageType::Json
    );
}

#[test]
fn test_detect_json_array_body() {
    let json = r#"[{"id": 1}, {"id": 2}]"#;
    assert_eq!(
        detect_page_type(json, "https://api.example.com/list", 0),
        PageType::Json
    );
}

#[test]
fn test_detect_json_with_whitespace_prefix() {
    let json = "   {\"key\": \"value\"}";
    assert_eq!(
        detect_page_type(json, "https://api.example.com/data", 0),
        PageType::Json
    );
}

#[test]
fn test_detect_json_not_triggered_by_html_starting_with_brace() {
    // HTML that starts with { but contains </html> should not be JSON.
    let html = r"{something}<html><body><p>Not JSON.</p></body></html>";
    assert_ne!(
        detect_page_type(html, "https://example.com", 50),
        PageType::Json
    );
}

// ===========================================================================
// detect_page_type: image
// ===========================================================================

#[test]
fn test_detect_image_page() {
    let html = r#"<html><body><img src="photo.jpg" alt="Photo"></body></html>"#;
    assert_eq!(
        detect_page_type(html, "https://example.com/photo", 50),
        PageType::Image
    );
}

#[test]
fn test_detect_image_multiple_images_not_image_page() {
    let html = r#"<html><body><img src="a.jpg"><img src="b.jpg"></body></html>"#;
    assert_ne!(
        detect_page_type(html, "https://example.com/gallery", 50),
        PageType::Image
    );
}

// ===========================================================================
// detect_page_type: unknown
// ===========================================================================

#[test]
fn test_detect_unknown_fallback() {
    let html = r"<html><body><p>Short.</p></body></html>";
    assert_eq!(
        detect_page_type(html, "https://example.com", 10),
        PageType::Unknown
    );
}

#[test]
fn test_detect_unknown_empty_body() {
    assert_eq!(
        detect_page_type("", "https://example.com", 0),
        PageType::Unknown
    );
}

// ===========================================================================
// detect_page_type: detection priority
// ===========================================================================

#[test]
fn test_json_detected_before_other_types() {
    // JSON body should be detected as JSON even if it contains "sign in".
    let json = r#"{"error": "sign in required"}"#;
    assert_eq!(
        detect_page_type(json, "https://api.example.com", 0),
        PageType::Json
    );
}

#[test]
fn test_redirect_detected_before_auth_wall() {
    // Meta refresh redirect should be detected before auth wall signals.
    let html = r#"<html><head><meta http-equiv="refresh" content="0;url=https://other.com"></head><body>Please sign in</body></html>"#;
    assert_eq!(
        detect_page_type(html, "https://example.com/old", 50),
        PageType::Redirect
    );
}

#[test]
fn test_auth_wall_detected_before_paywall() {
    // Auth wall should be detected before paywall signals.
    let html =
        r"<html><body><h1>Please sign in</h1><p>Subscribe to continue reading</p></body></html>";
    assert_eq!(
        detect_page_type(html, "https://example.com/locked", 50),
        PageType::AuthWall
    );
}

// ===========================================================================
// detect_page_type: UTF-8 boundary safety (regression for panic at 5000 bytes)
// ===========================================================================

#[test]
fn test_detect_page_type_multibyte_at_5000_byte_boundary() {
    // Regression: `head_len` previously returned `s.len().min(5000)`, which can
    // land inside a multi-byte UTF-8 sequence and panic on slicing. Build HTML
    // where byte 5000 falls in the middle of a 3-byte '…' (U+2026) character.
    let prefix = "x".repeat(4999);
    let html = format!("<html><body>{prefix}…sign in</body></html>");
    // Should not panic — classification result is not the point of this test.
    let _ = detect_page_type(&html, "https://example.com", 50);
}

#[test]
fn test_detect_page_type_multibyte_4byte_at_boundary() {
    // 4-byte emoji exactly straddling the 5000-byte cut.
    let prefix = "x".repeat(4998);
    let html = format!("<html><body>{prefix}😀sign in</body></html>");
    let _ = detect_page_type(&html, "https://example.com", 50);
}

// ===========================================================================
// classify_source_type: government
// ===========================================================================

#[test]
fn test_classify_gov() {
    let (st, official) = classify_source_type("https://www.gov.uk/policy");
    assert_eq!(st, SourceType::Gov);
    assert!(official);
}

#[test]
fn test_classify_gov_us() {
    let (st, official) = classify_source_type("https://www.nih.gov/grants");
    assert_eq!(st, SourceType::Gov);
    assert!(official);
}

#[test]
fn test_classify_gov_us_domain() {
    let (st, official) = classify_source_type("https://www.example.gov.us/page");
    assert_eq!(st, SourceType::Gov);
    assert!(official);
}

#[test]
fn test_classify_gov_uk_subdomain() {
    let (st, official) = classify_source_type("https://api.service.gov.uk/data");
    assert_eq!(st, SourceType::Gov);
    assert!(official);
}

#[test]
fn test_classify_gov_bare_domain() {
    let (st, official) = classify_source_type("https://gov/policy");
    assert_eq!(st, SourceType::Gov);
    assert!(official);
}

// ===========================================================================
// classify_source_type: education
// ===========================================================================

#[test]
fn test_classify_edu() {
    let (st, official) = classify_source_type("https://www.mit.edu/research");
    assert_eq!(st, SourceType::Edu);
    assert!(official);
}

#[test]
fn test_classify_edu_cn() {
    let (st, official) = classify_source_type("https://www.tsinghua.edu.cn/en");
    assert_eq!(st, SourceType::Edu);
    assert!(official);
}

#[test]
fn test_classify_edu_au() {
    let (st, official) = classify_source_type("https://www.unimelb.edu.au/about");
    assert_eq!(st, SourceType::Edu);
    assert!(official);
}

#[test]
fn test_classify_ac_uk() {
    let (st, official) = classify_source_type("https://www.ox.ac.uk/admissions");
    assert_eq!(st, SourceType::Edu);
    assert!(official);
}

#[test]
fn test_classify_edu_bare_domain() {
    let (st, official) = classify_source_type("https://edu/page");
    assert_eq!(st, SourceType::Edu);
    assert!(official);
}

// ===========================================================================
// classify_source_type: github
// ===========================================================================

#[test]
fn test_classify_github() {
    let (st, official) = classify_source_type("https://github.com/user/repo");
    assert_eq!(st, SourceType::Github);
    assert!(official);
}

#[test]
fn test_classify_github_gist() {
    let (st, official) = classify_source_type("https://gist.github.com/user/123");
    assert_eq!(st, SourceType::Github);
    assert!(official);
}

#[test]
fn test_classify_github_io() {
    let (st, official) = classify_source_type("https://user.github.io/project");
    assert_eq!(st, SourceType::Github);
    assert!(official);
}

// ===========================================================================
// classify_source_type: vendor documentation
// ===========================================================================

#[test]
fn test_classify_vendor_docs() {
    let (st, official) = classify_source_type("https://docs.python.org/tutorial");
    assert_eq!(st, SourceType::VendorDocs);
    assert!(official);
}

#[test]
fn test_classify_vendor_docs_microsoft() {
    let (st, official) = classify_source_type("https://docs.microsoft.com/dotnet");
    assert_eq!(st, SourceType::VendorDocs);
    assert!(official);
}

#[test]
fn test_classify_vendor_docs_learn_microsoft() {
    let (st, official) = classify_source_type("https://learn.microsoft.com/azure");
    assert_eq!(st, SourceType::VendorDocs);
    assert!(official);
}

#[test]
fn test_classify_vendor_docs_mozilla() {
    let (st, official) = classify_source_type("https://developer.mozilla.org/docs");
    assert_eq!(st, SourceType::VendorDocs);
    assert!(official);
}

#[test]
fn test_classify_vendor_docs_aws() {
    let (st, official) = classify_source_type("https://docs.aws.amazon.com/s3");
    assert_eq!(st, SourceType::VendorDocs);
    assert!(official);
}

#[test]
fn test_classify_vendor_docs_kubernetes() {
    let (st, official) = classify_source_type("https://kubernetes.io/docs");
    assert_eq!(st, SourceType::VendorDocs);
    assert!(official);
}

#[test]
fn test_classify_vendor_docs_docs_rs() {
    let (st, official) = classify_source_type("https://docs.rs/serde/latest");
    assert_eq!(st, SourceType::VendorDocs);
    assert!(official);
}

// ===========================================================================
// classify_source_type: generic docs site (not official)
// ===========================================================================

#[test]
fn test_classify_docs_site_prefix() {
    let (st, official) = classify_source_type("https://docs.mycompany.com/guide");
    assert_eq!(st, SourceType::DocsSite);
    assert!(!official);
}

#[test]
fn test_classify_developer_prefix() {
    let (st, official) = classify_source_type("https://developer.mycompany.com/api");
    assert_eq!(st, SourceType::DocsSite);
    assert!(!official);
}

#[test]
fn test_classify_developers_prefix() {
    let (st, official) = classify_source_type("https://developers.mycompany.com/api");
    assert_eq!(st, SourceType::DocsSite);
    assert!(!official);
}

#[test]
fn test_classify_documentation_prefix() {
    let (st, official) = classify_source_type("https://documentation.mycompany.com/guide");
    assert_eq!(st, SourceType::DocsSite);
    assert!(!official);
}

// ===========================================================================
// classify_source_type: qa
// ===========================================================================

#[test]
fn test_classify_qa() {
    let (st, official) = classify_source_type("https://stackoverflow.com/q/123");
    assert_eq!(st, SourceType::Qa);
    assert!(!official);
}

#[test]
fn test_classify_qa_stackexchange() {
    let (st, official) = classify_source_type("https://math.stackexchange.com/q/456");
    assert_eq!(st, SourceType::Qa);
    assert!(!official);
}

#[test]
fn test_classify_qa_serverfault() {
    let (st, official) = classify_source_type("https://serverfault.com/q/789");
    assert_eq!(st, SourceType::Qa);
    assert!(!official);
}

#[test]
fn test_classify_qa_askubuntu() {
    let (st, official) = classify_source_type("https://askubuntu.com/q/101");
    assert_eq!(st, SourceType::Qa);
    assert!(!official);
}

// ===========================================================================
// classify_source_type: forum
// ===========================================================================

#[test]
fn test_classify_forum() {
    let (st, official) = classify_source_type("https://www.reddit.com/r/rust");
    assert_eq!(st, SourceType::Forum);
    assert!(!official);
}

#[test]
fn test_classify_forum_old_reddit() {
    let (st, official) = classify_source_type("https://old.reddit.com/r/rust");
    assert_eq!(st, SourceType::Forum);
    assert!(!official);
}

#[test]
fn test_classify_forum_elixirforum() {
    let (st, official) = classify_source_type("https://elixirforum.com/t/123");
    assert_eq!(st, SourceType::Forum);
    assert!(!official);
}

#[test]
fn test_classify_forum_groups_google() {
    let (st, official) = classify_source_type("https://groups.google.com/g/rust-lang");
    assert_eq!(st, SourceType::Forum);
    assert!(!official);
}

// ===========================================================================
// classify_source_type: blog
// ===========================================================================

#[test]
fn test_classify_blog() {
    let (st, official) = classify_source_type("https://medium.com/@user/post");
    assert_eq!(st, SourceType::Blog);
    assert!(!official);
}

#[test]
fn test_classify_blog_substack() {
    let (st, official) = classify_source_type("https://newsletter.substack.com/p/hello");
    assert_eq!(st, SourceType::Blog);
    assert!(!official);
}

#[test]
fn test_classify_blog_wordpress() {
    let (st, official) = classify_source_type("https://myblog.wordpress.com/2024/01");
    assert_eq!(st, SourceType::Blog);
    assert!(!official);
}

#[test]
fn test_classify_blog_dev_to() {
    let (st, official) = classify_source_type("https://dev.to/user/my-post");
    assert_eq!(st, SourceType::Blog);
    assert!(!official);
}

// ===========================================================================
// classify_source_type: news
// ===========================================================================

#[test]
fn test_classify_news() {
    let (st, official) = classify_source_type("https://www.bbc.com/news");
    assert_eq!(st, SourceType::News);
    assert!(!official);
}

#[test]
fn test_classify_news_cnn() {
    let (st, official) = classify_source_type("https://www.cnn.com/2024/article");
    assert_eq!(st, SourceType::News);
    assert!(!official);
}

#[test]
fn test_classify_news_reuters() {
    let (st, official) = classify_source_type("https://www.reuters.com/article");
    assert_eq!(st, SourceType::News);
    assert!(!official);
}

#[test]
fn test_classify_news_techcrunch() {
    let (st, official) = classify_source_type("https://techcrunch.com/2024/startup");
    assert_eq!(st, SourceType::News);
    assert!(!official);
}

// ===========================================================================
// classify_source_type: ecommerce
// ===========================================================================

#[test]
fn test_classify_ecommerce() {
    let (st, official) = classify_source_type("https://www.amazon.com/product");
    assert_eq!(st, SourceType::Ecommerce);
    assert!(!official);
}

#[test]
fn test_classify_ecommerce_ebay() {
    let (st, official) = classify_source_type("https://www.ebay.com/item/123");
    assert_eq!(st, SourceType::Ecommerce);
    assert!(!official);
}

#[test]
fn test_classify_ecommerce_shop_prefix() {
    let (st, official) = classify_source_type("https://shop.example.com/item");
    assert_eq!(st, SourceType::Ecommerce);
    assert!(!official);
}

#[test]
fn test_classify_ecommerce_store_prefix() {
    let (st, official) = classify_source_type("https://store.example.com/product");
    assert_eq!(st, SourceType::Ecommerce);
    assert!(!official);
}

// ===========================================================================
// classify_source_type: unknown / edge cases
// ===========================================================================

#[test]
fn test_classify_unknown() {
    let (st, official) = classify_source_type("https://example.com/page");
    assert_eq!(st, SourceType::Unknown);
    assert!(!official);
}

#[test]
fn test_classify_invalid_url() {
    let (st, official) = classify_source_type("not a url");
    assert_eq!(st, SourceType::Unknown);
    assert!(!official);
}

#[test]
fn test_classify_no_scheme() {
    let (st, official) = classify_source_type("example.com/page");
    assert_eq!(st, SourceType::Unknown);
    assert!(!official);
}

#[test]
fn test_classify_empty_url() {
    let (st, official) = classify_source_type("");
    assert_eq!(st, SourceType::Unknown);
    assert!(!official);
}

#[test]
fn test_classify_localhost() {
    let (st, official) = classify_source_type("http://localhost:8080");
    assert_eq!(st, SourceType::Unknown);
    assert!(!official);
}

// ===========================================================================
// classify_source_type: is_official flag summary
// ===========================================================================

#[test]
fn test_is_official_true_for_gov() {
    let (_, official) = classify_source_type("https://www.gov.uk");
    assert!(official);
}

#[test]
fn test_is_official_true_for_edu() {
    let (_, official) = classify_source_type("https://mit.edu");
    assert!(official);
}

#[test]
fn test_is_official_true_for_github() {
    let (_, official) = classify_source_type("https://github.com");
    assert!(official);
}

#[test]
fn test_is_official_true_for_vendor_docs() {
    let (_, official) = classify_source_type("https://docs.python.org");
    assert!(official);
}

#[test]
fn test_is_official_false_for_docs_site() {
    let (_, official) = classify_source_type("https://docs.mycompany.com");
    assert!(!official);
}

#[test]
fn test_is_official_false_for_news() {
    let (_, official) = classify_source_type("https://www.bbc.com");
    assert!(!official);
}

#[test]
fn test_is_official_false_for_blog() {
    let (_, official) = classify_source_type("https://medium.com");
    assert!(!official);
}

#[test]
fn test_is_official_false_for_ecommerce() {
    let (_, official) = classify_source_type("https://www.amazon.com");
    assert!(!official);
}

// ===========================================================================
// compute_freshness: no dates
// ===========================================================================

#[test]
fn test_freshness_no_dates() {
    let metadata = PageMetadata::default();
    let (age, stale) = compute_freshness(&metadata);
    assert_eq!(age, -1);
    assert!(!stale);
}

#[test]
fn test_freshness_empty_dates() {
    let metadata = PageMetadata {
        published_time: Some(String::new()),
        modified_time: Some(String::new()),
        ..Default::default()
    };
    let (age, stale) = compute_freshness(&metadata);
    assert_eq!(age, -1);
    assert!(!stale);
}

// ===========================================================================
// compute_freshness: published date only
// ===========================================================================

#[test]
fn test_freshness_published_date() {
    let metadata = PageMetadata {
        published_time: Some("2020-01-15T10:30:00Z".to_string()),
        ..Default::default()
    };
    let (age, stale) = compute_freshness(&metadata);
    assert!(age > 365, "age should be > 365 for a 2020 date: {age}");
    assert!(stale);
}

#[test]
fn test_freshness_recent_date_not_stale() {
    let recent = Utc::now() - chrono::Duration::days(30);
    let metadata = PageMetadata {
        published_time: Some(recent.to_rfc3339()),
        ..Default::default()
    };
    let (age, stale) = compute_freshness(&metadata);
    assert!(age > 20 && age < 40, "age should be ~30 days: {age}");
    assert!(!stale);
}

// ===========================================================================
// compute_freshness: modified preferred over published
// ===========================================================================

#[test]
fn test_freshness_modified_preferred_over_published() {
    // Modified is more recent → should use modified.
    let metadata = PageMetadata {
        published_time: Some("2010-01-01T00:00:00Z".to_string()),
        modified_time: Some("2024-01-01T00:00:00Z".to_string()),
        ..Default::default()
    };
    let (age, _stale) = compute_freshness(&metadata);
    // Verify the age reflects the modified date, not the published date.
    let now = Utc::now();
    let modified_date = chrono::DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let published_date = chrono::DateTime::parse_from_rfc3339("2010-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let modified_age = (now - modified_date).num_days();
    let published_age = (now - published_date).num_days();
    assert!(
        (age - modified_age).abs() <= 1,
        "age {age} should match modified date age {modified_age}, not published date age {published_age}"
    );
}

#[test]
fn test_freshness_modified_only() {
    let metadata = PageMetadata {
        modified_time: Some("2020-06-15T00:00:00Z".to_string()),
        ..Default::default()
    };
    let (age, stale) = compute_freshness(&metadata);
    assert!(
        age > 365,
        "age should be > 365 for a 2020 modified date: {age}"
    );
    assert!(stale);
}

#[test]
fn test_freshness_published_ignored_when_modified_present() {
    // When modified_time is empty but published_time is set, should use published.
    let metadata = PageMetadata {
        published_time: Some("2020-01-01T00:00:00Z".to_string()),
        modified_time: Some(String::new()),
        ..Default::default()
    };
    let (age, stale) = compute_freshness(&metadata);
    assert!(
        age > 365,
        "should fall back to published when modified is empty: {age}"
    );
    assert!(stale);
}

// ===========================================================================
// compute_freshness: future date → -1
// ===========================================================================

#[test]
fn test_freshness_future_date() {
    let future = Utc::now() + chrono::Duration::days(30);
    let metadata = PageMetadata {
        published_time: Some(future.to_rfc3339()),
        ..Default::default()
    };
    let (age, stale) = compute_freshness(&metadata);
    assert_eq!(age, -1);
    assert!(!stale);
}

#[test]
fn test_freshness_future_modified_date() {
    let future = Utc::now() + chrono::Duration::days(365);
    let metadata = PageMetadata {
        modified_time: Some(future.to_rfc3339()),
        ..Default::default()
    };
    let (age, stale) = compute_freshness(&metadata);
    assert_eq!(age, -1);
    assert!(!stale);
}

#[test]
fn test_freshness_future_published_past_modified() {
    // Published is in the future, modified is in the past.
    // Should prefer modified → age > 0, not -1.
    let future = Utc::now() + chrono::Duration::days(30);
    let past = Utc::now() - chrono::Duration::days(100);
    let metadata = PageMetadata {
        published_time: Some(future.to_rfc3339()),
        modified_time: Some(past.to_rfc3339()),
        ..Default::default()
    };
    let (age, _stale) = compute_freshness(&metadata);
    assert!(
        age > 90 && age < 110,
        "should use past modified date, not future published: {age}"
    );
}

// ===========================================================================
// compute_freshness: stale threshold
// ===========================================================================

#[test]
fn test_freshness_stale_threshold() {
    // Exactly at 365 days → not stale (stale is > 365).
    let date = Utc::now() - chrono::Duration::days(365);
    let metadata = PageMetadata {
        published_time: Some(date.to_rfc3339()),
        ..Default::default()
    };
    let (age, stale) = compute_freshness(&metadata);
    assert_eq!(age, 365);
    assert!(!stale); // stale is > 365, not >= 365
}

#[test]
fn test_freshness_stale_threshold_plus_one() {
    let date = Utc::now() - chrono::Duration::days(366);
    let metadata = PageMetadata {
        published_time: Some(date.to_rfc3339()),
        ..Default::default()
    };
    let (age, stale) = compute_freshness(&metadata);
    assert_eq!(age, 366);
    assert!(stale);
}

#[test]
fn test_freshness_stale_threshold_constant() {
    assert_eq!(STALE_THRESHOLD_DAYS, 365);
}

#[test]
fn test_freshness_one_day_old_not_stale() {
    let date = Utc::now() - chrono::Duration::days(1);
    let metadata = PageMetadata {
        published_time: Some(date.to_rfc3339()),
        ..Default::default()
    };
    let (age, stale) = compute_freshness(&metadata);
    assert_eq!(age, 1);
    assert!(!stale);
}

#[test]
fn test_freshness_zero_days_old() {
    let date = Utc::now();
    let metadata = PageMetadata {
        published_time: Some(date.to_rfc3339()),
        ..Default::default()
    };
    let (age, stale) = compute_freshness(&metadata);
    assert_eq!(age, 0);
    assert!(!stale);
}

// ===========================================================================
// compute_freshness: date format variants
// ===========================================================================

#[test]
fn test_freshness_date_only() {
    let metadata = PageMetadata {
        published_time: Some("2020-06-15".to_string()),
        ..Default::default()
    };
    let (age, stale) = compute_freshness(&metadata);
    assert!(age > 365, "age should be > 365 for a 2020 date: {age}");
    assert!(stale);
}

#[test]
fn test_freshness_no_timezone() {
    let metadata = PageMetadata {
        published_time: Some("2020-01-15T10:30:00".to_string()),
        ..Default::default()
    };
    let (age, stale) = compute_freshness(&metadata);
    assert!(age > 365);
    assert!(stale);
}

#[test]
fn test_freshness_space_separator() {
    let metadata = PageMetadata {
        published_time: Some("2020-01-15 10:30:00".to_string()),
        ..Default::default()
    };
    let (age, stale) = compute_freshness(&metadata);
    assert!(age > 365);
    assert!(stale);
}

#[test]
fn test_freshness_with_offset() {
    let metadata = PageMetadata {
        published_time: Some("2020-01-15T10:30:00+02:00".to_string()),
        ..Default::default()
    };
    let (age, stale) = compute_freshness(&metadata);
    assert!(age > 365);
    assert!(stale);
}

// ===========================================================================
// compute_freshness: invalid dates
// ===========================================================================

#[test]
fn test_freshness_invalid_date() {
    let metadata = PageMetadata {
        published_time: Some("not a date".to_string()),
        ..Default::default()
    };
    let (age, stale) = compute_freshness(&metadata);
    assert_eq!(age, -1);
    assert!(!stale);
}

#[test]
fn test_freshness_whitespace_only_date() {
    let metadata = PageMetadata {
        published_time: Some("   ".to_string()),
        ..Default::default()
    };
    let (age, stale) = compute_freshness(&metadata);
    assert_eq!(age, -1);
    assert!(!stale);
}

// ===========================================================================
// build_envelope: integration
// ===========================================================================

#[test]
fn test_build_envelope_article() {
    let html = r"<html><body><article><p>Article text that is long enough to pass the threshold.</p></article></body></html>";
    let metadata = PageMetadata::default();
    let envelope = build_envelope(html, "https://example.com/post", &metadata, true, 200);
    assert_eq!(envelope.page_type, PageType::Article);
    assert!(envelope.content_ok);
    assert_eq!(envelope.content_age_days, -1); // no dates
}

#[test]
fn test_build_envelope_gov_source() {
    let html =
        r"<html><body><article><p>Government article with enough text.</p></article></body></html>";
    let metadata = PageMetadata::default();
    let envelope = build_envelope(html, "https://www.gov.uk/policy", &metadata, true, 200);
    assert_eq!(envelope.source_type, SourceType::Gov);
    assert!(envelope.is_official);
}

#[test]
fn test_build_envelope_with_freshness() {
    let html = r"<html><body><article><p>Old article with enough text.</p></article></body></html>";
    let metadata = PageMetadata {
        published_time: Some("2010-01-01T00:00:00Z".to_string()),
        ..Default::default()
    };
    let envelope = build_envelope(html, "https://example.com/old", &metadata, true, 200);
    assert!(envelope.content_age_days > 365);
    assert!(envelope.is_stale);
}

#[test]
fn test_build_envelope_summary_empty_by_default() {
    let html = r"<html><body><article><p>Text.</p></article></body></html>";
    let metadata = PageMetadata::default();
    let envelope = build_envelope(html, "https://example.com", &metadata, true, 200);
    assert!(envelope.summary.is_empty());
}

// ===========================================================================
// build_envelope: next_action suggestions
// ===========================================================================

#[test]
fn test_build_envelope_js_shell_next_action() {
    let html = format!(
        "<html><body>Please enable JavaScript to run this app.{}</body></html>",
        "x".repeat(4000)
    );
    let metadata = PageMetadata::default();
    let envelope = build_envelope(&html, "https://app.example.com", &metadata, false, 10);
    assert_eq!(envelope.page_type, PageType::JsShell);
    assert!(!envelope.content_ok);
    assert!(envelope.next_action.contains("JavaScript"));
}

#[test]
fn test_build_envelope_auth_wall_next_action() {
    let html = r"<html><body><form><h1>Please sign in</h1></form></body></html>";
    let metadata = PageMetadata::default();
    let envelope = build_envelope(html, "https://example.com/login", &metadata, false, 50);
    assert_eq!(envelope.page_type, PageType::AuthWall);
    assert!(envelope.next_action.contains("login"));
}

#[test]
fn test_build_envelope_paywall_next_action() {
    let html = r"<html><body><h1>Subscribe to continue reading</h1></body></html>";
    let metadata = PageMetadata::default();
    let envelope = build_envelope(html, "https://example.com/article", &metadata, false, 50);
    assert_eq!(envelope.page_type, PageType::Paywall);
    assert!(
        envelope.next_action.contains("paywalled") || envelope.next_action.contains("free source")
    );
}

#[test]
fn test_build_envelope_list_next_action() {
    let html = format!(
        "<html><body>{}</body></html>",
        (0..30)
            .map(|i| format!("<a href=\"/p{i}\">L{i}</a>"))
            .collect::<String>()
    );
    let metadata = PageMetadata::default();
    let envelope = build_envelope(&html, "https://example.com/dir", &metadata, true, 200);
    assert_eq!(envelope.page_type, PageType::List);
    assert!(envelope.next_action.contains("linked URLs"));
}

#[test]
fn test_build_envelope_redirect_next_action_content_ok() {
    let html = r#"<html><head><meta http-equiv="refresh" content="0;url=https://other.com"></head><body>Redirecting...</body></html>"#;
    let metadata = PageMetadata::default();
    let envelope = build_envelope(html, "https://example.com/old", &metadata, true, 50);
    assert_eq!(envelope.page_type, PageType::Redirect);
    // When content_ok is true, redirect next_action mentions "redirect target".
    assert!(envelope.next_action.contains("redirect"));
}

#[test]
fn test_build_envelope_redirect_next_action_content_not_ok() {
    let html = r#"<html><head><meta http-equiv="refresh" content="0;url=https://other.com"></head><body>Redirecting...</body></html>"#;
    let metadata = PageMetadata::default();
    let envelope = build_envelope(html, "https://example.com/old", &metadata, false, 50);
    assert_eq!(envelope.page_type, PageType::Redirect);
    assert!(envelope.next_action.contains("redirect") || envelope.next_action.contains("follow"));
}

#[test]
fn test_build_envelope_article_no_next_action_when_content_ok() {
    let html = r"<html><body><article><p>Article text that is long enough to pass the threshold.</p></article></body></html>";
    let metadata = PageMetadata::default();
    let envelope = build_envelope(html, "https://example.com/post", &metadata, true, 200);
    assert_eq!(envelope.page_type, PageType::Article);
    // Article with content_ok=true has no special next_action.
    assert!(envelope.next_action.is_empty());
}

#[test]
fn test_build_envelope_article_next_action_when_content_not_ok() {
    let html = r"<html><body><article><p>Article text that is long enough to pass the threshold.</p></article></body></html>";
    let metadata = PageMetadata::default();
    let envelope = build_envelope(html, "https://example.com/post", &metadata, false, 200);
    assert_eq!(envelope.page_type, PageType::Article);
    // Article with content_ok=false → generic failure message.
    assert!(!envelope.next_action.is_empty());
    assert!(
        envelope.next_action.contains("try a different URL")
            || envelope.next_action.contains("try a different source")
    );
}

// ===========================================================================
// build_envelope: combined signals
// ===========================================================================

#[test]
fn test_build_envelope_gov_article_with_freshness() {
    let html = r"<html><body><article><p>Government article with enough text to pass the threshold here.</p></article></body></html>";
    let recent = Utc::now() - chrono::Duration::days(30);
    let metadata = PageMetadata {
        published_time: Some(recent.to_rfc3339()),
        ..Default::default()
    };
    let envelope = build_envelope(html, "https://www.gov.uk/policy", &metadata, true, 200);
    assert_eq!(envelope.page_type, PageType::Article);
    assert_eq!(envelope.source_type, SourceType::Gov);
    assert!(envelope.is_official);
    assert!(envelope.content_ok);
    assert!(!envelope.is_stale); // 30 days old is not stale
}

#[test]
fn test_build_envelope_docs_with_stale_content() {
    let html = r"<html><body><pre><code>fn main() {}</code></pre><pre><code>let x = 1;</code></pre><pre><code>println!();</code></pre></body></html>";
    let metadata = PageMetadata {
        published_time: Some("2010-01-01T00:00:00Z".to_string()),
        ..Default::default()
    };
    let envelope = build_envelope(html, "https://docs.python.org/guide", &metadata, true, 100);
    assert_eq!(envelope.page_type, PageType::Docs);
    assert_eq!(envelope.source_type, SourceType::VendorDocs);
    assert!(envelope.is_official);
    assert!(envelope.is_stale);
}

#[test]
fn test_build_envelope_forum_with_freshness() {
    let html = r#"<html><body><div class="post">Discussion about Rust.</div></body></html>"#;
    let recent = Utc::now() - chrono::Duration::days(10);
    let metadata = PageMetadata {
        published_time: Some(recent.to_rfc3339()),
        ..Default::default()
    };
    let envelope = build_envelope(html, "https://www.reddit.com/r/rust", &metadata, true, 100);
    assert_eq!(envelope.page_type, PageType::Forum);
    assert_eq!(envelope.source_type, SourceType::Forum);
    assert!(!envelope.is_stale);
    assert!(envelope.content_age_days > 8 && envelope.content_age_days < 12);
}

#[test]
fn test_build_envelope_qa_unknown_source() {
    let html = r#"<html><body><div class="question">What is Rust?</div></body></html>"#;
    let metadata = PageMetadata::default();
    let envelope = build_envelope(html, "https://example.com/q/1", &metadata, true, 100);
    assert_eq!(envelope.page_type, PageType::Qa);
    assert_eq!(envelope.source_type, SourceType::Unknown);
    assert!(!envelope.is_official);
}

#[test]
fn test_build_envelope_json_source() {
    let json = r#"{"data": [1, 2, 3]}"#;
    let metadata = PageMetadata::default();
    let envelope = build_envelope(json, "https://api.example.com/data", &metadata, true, 0);
    assert_eq!(envelope.page_type, PageType::Json);
    assert_eq!(envelope.source_type, SourceType::Unknown);
}

#[test]
fn test_build_envelope_image_source() {
    let html = r#"<html><body><img src="photo.jpg" alt="Photo"></body></html>"#;
    let metadata = PageMetadata::default();
    let envelope = build_envelope(html, "https://example.com/photo", &metadata, true, 50);
    assert_eq!(envelope.page_type, PageType::Image);
}

#[test]
fn test_build_envelope_unknown_page_type() {
    let html = r"<html><body><p>Short.</p></body></html>";
    let metadata = PageMetadata::default();
    let envelope = build_envelope(html, "https://example.com", &metadata, true, 10);
    assert_eq!(envelope.page_type, PageType::Unknown);
}

// ===========================================================================
// parse_iso_date
// ===========================================================================

#[test]
fn test_parse_iso_date_rfc3339() {
    let dt = parse_iso_date("2024-01-15T10:30:00Z");
    assert!(dt.is_some());
    assert_eq!(dt.unwrap().year(), 2024);
}

#[test]
fn test_parse_iso_date_with_offset() {
    let dt = parse_iso_date("2024-01-15T10:30:00+02:00");
    assert!(dt.is_some());
}

#[test]
fn test_parse_iso_date_no_timezone() {
    let dt = parse_iso_date("2024-01-15T10:30:00");
    assert!(dt.is_some());
}

#[test]
fn test_parse_iso_date_date_only() {
    let dt = parse_iso_date("2024-01-15");
    assert!(dt.is_some());
    assert_eq!(dt.unwrap().year(), 2024);
}

#[test]
fn test_parse_iso_date_invalid() {
    assert!(parse_iso_date("not a date").is_none());
    assert!(parse_iso_date("").is_none());
}

#[test]
fn test_parse_iso_date_whitespace_only() {
    assert!(parse_iso_date("   ").is_none());
}

#[test]
fn test_parse_iso_date_space_separator() {
    let dt = parse_iso_date("2024-01-15 10:30:00");
    assert!(dt.is_some());
}

#[test]
fn test_parse_iso_date_with_fractional_seconds() {
    let dt = parse_iso_date("2024-01-15T10:30:00.123Z");
    assert!(dt.is_some());
}

#[test]
fn test_parse_iso_date_negative_timezone() {
    let dt = parse_iso_date("2024-01-15T10:30:00-05:00");
    assert!(dt.is_some());
}

#[test]
fn test_parse_iso_date_trims_whitespace() {
    let dt = parse_iso_date("  2024-01-15T10:30:00Z  ");
    assert!(dt.is_some());
}

// ===========================================================================
// parse_meta_refresh_seconds
// ===========================================================================

#[test]
fn test_parse_meta_refresh_seconds_with_url() {
    assert_eq!(parse_meta_refresh_seconds("0;url=https://other.com"), 0);
    assert_eq!(parse_meta_refresh_seconds("3;url=/page"), 3);
}

#[test]
fn test_parse_meta_refresh_seconds_no_url() {
    assert_eq!(parse_meta_refresh_seconds("5"), 5);
}

#[test]
fn test_parse_meta_refresh_seconds_invalid() {
    assert_eq!(parse_meta_refresh_seconds("abc"), 999);
}

#[test]
fn test_parse_meta_refresh_seconds_empty() {
    assert_eq!(parse_meta_refresh_seconds(""), 999);
}

#[test]
fn test_parse_meta_refresh_seconds_boundary_zero() {
    assert_eq!(parse_meta_refresh_seconds("0"), 0);
}

#[test]
fn test_parse_meta_refresh_seconds_boundary_three() {
    assert_eq!(parse_meta_refresh_seconds("3"), 3);
}

#[test]
fn test_parse_meta_refresh_seconds_four_is_not_redirect() {
    // 4 seconds → 4, which is > 3, so not a redirect.
    assert_eq!(parse_meta_refresh_seconds("4"), 4);
}

// ===========================================================================
// Display trait
// ===========================================================================

#[test]
fn test_page_type_display() {
    assert_eq!(PageType::Article.to_string(), "article");
    assert_eq!(PageType::Docs.to_string(), "docs");
    assert_eq!(PageType::List.to_string(), "list");
    assert_eq!(PageType::Forum.to_string(), "forum");
    assert_eq!(PageType::Qa.to_string(), "qa");
    assert_eq!(PageType::JsShell.to_string(), "js_shell");
    assert_eq!(PageType::AuthWall.to_string(), "auth_wall");
    assert_eq!(PageType::Paywall.to_string(), "paywall");
    assert_eq!(PageType::Redirect.to_string(), "redirect");
    assert_eq!(PageType::Image.to_string(), "image");
    assert_eq!(PageType::Json.to_string(), "json");
    assert_eq!(PageType::Unknown.to_string(), "unknown");
}

#[test]
fn test_source_type_display() {
    assert_eq!(SourceType::VendorDocs.to_string(), "vendor_docs");
    assert_eq!(SourceType::OfficialDocs.to_string(), "official_docs");
    assert_eq!(SourceType::News.to_string(), "news");
    assert_eq!(SourceType::Blog.to_string(), "blog");
    assert_eq!(SourceType::Forum.to_string(), "forum");
    assert_eq!(SourceType::Qa.to_string(), "qa");
    assert_eq!(SourceType::Gov.to_string(), "gov");
    assert_eq!(SourceType::Edu.to_string(), "edu");
    assert_eq!(SourceType::Github.to_string(), "github");
    assert_eq!(SourceType::DocsSite.to_string(), "docs_site");
    assert_eq!(SourceType::Ecommerce.to_string(), "ecommerce");
    assert_eq!(SourceType::Unknown.to_string(), "unknown");
}

// ===========================================================================
// PageType Default
// ===========================================================================

#[test]
fn test_page_type_default_is_unknown() {
    assert_eq!(PageType::default(), PageType::Unknown);
}

#[test]
fn test_source_type_default_is_unknown() {
    assert_eq!(SourceType::default(), SourceType::Unknown);
}

// ===========================================================================
// PageMetadata Default
// ===========================================================================

#[test]
fn test_page_metadata_default_all_none() {
    let md = PageMetadata::default();
    assert!(md.title.is_none());
    assert!(md.description.is_none());
    assert!(md.site_name.is_none());
    assert!(md.og_type.is_none());
    assert!(md.image.is_none());
    assert!(md.canonical.is_none());
    assert!(md.lang.is_none());
    assert!(md.published_time.is_none());
    assert!(md.modified_time.is_none());
    assert!(md.author.is_none());
}
