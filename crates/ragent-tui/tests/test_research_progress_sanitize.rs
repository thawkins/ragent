//! Tests for research progress display sanitization.

#[path = "support/mod.rs"]
mod support;

use ragent_tui::app::sanitize_for_display;
use ragent_tui::research_progress::{decode_progress_event, encode_progress_event};

// =========================================================================
// sanitize_for_display
// =========================================================================

#[test]
fn test_sanitize_for_display_strips_ansi() {
    let input = "\x1b[31mred\x1b[0m text";
    assert_eq!(sanitize_for_display(input), "red text");
}

#[test]
fn test_sanitize_for_display_strips_control_chars() {
    let input = "hello\x00\x01\x02world\x07";
    assert_eq!(sanitize_for_display(input), "helloworld");
}

#[test]
fn test_sanitize_for_display_keeps_newlines_and_tabs() {
    let input = "line1\nline2\tcol";
    assert_eq!(sanitize_for_display(input), input);
}

// =========================================================================
// encode_progress_event sanitizes URL/title/error/body fields
// =========================================================================

#[test]
fn test_encode_progress_event_sanitizes_web_captured() {
    let url = "https://example.com\x1b[31m";
    let title = "Title with \x00control";
    let encoded = encode_progress_event(
        "run",
        "topic",
        &ragent_research::session::SessionEvent::WebCaptured {
            url: url.to_string(),
            title: title.to_string(),
            search_tool: String::new(),
            search_engine: String::new(),
            body_preview: String::new(),
            language: "UNKNOWN".to_string(),
            media_type: "page".to_string(),
            oa_recovery: None,
        },
    );
    let decoded = decode_progress_event(&encoded).expect("decode");
    assert!(
        !decoded.detail.contains('\u{1b}'),
        "ANSI escape should be stripped: {}",
        decoded.detail
    );
    assert!(
        !decoded.detail.contains('\u{0}'),
        "control char should be stripped: {}",
        decoded.detail
    );
    assert!(decoded.detail.contains("https://example.com"));
    assert!(decoded.detail.contains("Title with control"));
}

#[test]
fn test_encode_progress_event_sanitizes_fetch_failed() {
    let url = "https://bad\x00url";
    let error = "error\x07\x1b[31mmessage";
    let encoded = encode_progress_event(
        "run",
        "topic",
        &ragent_research::session::SessionEvent::WebFetchFailed {
            url: url.to_string(),
            error: error.to_string(),
        },
    );
    let decoded = decode_progress_event(&encoded).expect("decode");
    assert!(!decoded.detail.contains('\u{1b}'));
    assert!(!decoded.detail.contains('\u{0}'));
    assert!(!decoded.detail.contains('\u{7}'));
}

// =========================================================================
// render_markdown_to_ascii bypasses HTML pipeline for research progress
// =========================================================================

#[test]
fn test_render_markdown_to_ascii_bypasses_research_progress() {
    let mut app = support::make_app();
    let input = "[research] Research Progress — `run`\nTopic: topic\n\n  ✓ web     — captured https://example.com — Title";
    let output = app.render_markdown_to_ascii(input);
    assert_eq!(
        output, input,
        "research progress text should pass through unchanged: {output}"
    );
}

#[test]
fn test_render_markdown_to_ascii_sanitizes_passthrough_text() {
    let mut app = support::make_app();
    let input = "plain text with \x1b[31mANSI\x1b[0m and \x00control";
    let output = app.render_markdown_to_ascii(input);
    assert!(!output.contains('\u{1b}'));
    assert!(!output.contains('\u{0}'));
    assert!(output.contains("plain text with ANSI and control"));
}
