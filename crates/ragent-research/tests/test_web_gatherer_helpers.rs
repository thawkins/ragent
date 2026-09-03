#![allow(clippy::assert_is_empty)]
//! Unit tests for web-gatherer title cleaning, relevance scoring, and
//! classification helpers extracted from `web_gatherer.rs` (Milestone F-002).

#[path = "../src/web_gatherer/title.rs"]
mod title;

#[path = "../src/web_gatherer/relevance.rs"]
mod relevance;

#[path = "../src/web_gatherer/classify.rs"]
mod classify;

use classify::{WebSourceKind, classify_web_source};
use relevance::normalize_query_terms;
use title::{
    MAX_WEB_SOURCE_TITLE_CHARS, clean_title_text, clean_web_source_title, truncate_title_words,
};

// ── title tests ───────────────────────────────────────────────────────────

#[test]
fn clean_title_strips_markdown_reference_links() {
    let out = clean_web_source_title("[Skip to main content][1]", "");
    assert!(
        out.is_empty(),
        "pure-noise title with empty fallback should be empty, got {out:?}"
    );
}

#[test]
fn clean_title_strips_markdown_links_but_keeps_text() {
    let out = clean_web_source_title("[DeepSeek V4 Pro][1] model card", "");
    assert_eq!(out, "DeepSeek V4 Pro model card");
}

#[test]
fn clean_title_strips_inline_markdown_links() {
    let out = clean_web_source_title("[DeepSeek](https://deepseek.com) overview", "");
    assert_eq!(out, "DeepSeek overview");
}

#[test]
fn clean_title_strips_leading_cookie_banner() {
    let long = "We use essential cookies to make our site work. With your consent, we may also use non-essential cookies to improve your site for you and your experience";
    let out = clean_web_source_title(long, "");
    assert!(
        out.chars().count() <= MAX_WEB_SOURCE_TITLE_CHARS,
        "got {} chars: {out}",
        out.chars().count()
    );
    assert!(!out.to_lowercase().contains("we use essential cookies"));
    assert!(out.ends_with('…'));
}

#[test]
fn clean_title_truncates_long_title_at_word_boundary() {
    let long = "This is a genuinely long and meaningful article title that goes well beyond the one hundred and twenty character cap so it must be truncated by the gatherer";
    let out = clean_web_source_title(long, "");
    assert!(
        out.chars().count() <= MAX_WEB_SOURCE_TITLE_CHARS,
        "got {} chars: {out}",
        out.chars().count()
    );
    assert!(out.ends_with('…'));
    assert!(!out.ends_with("… "));
}

#[test]
fn clean_title_falls_back_when_primary_is_noise() {
    let out = clean_web_source_title("[Skip to main content][1]", "Real Article Title");
    assert_eq!(out, "Real Article Title");
}

#[test]
fn clean_title_falls_back_when_primary_is_empty() {
    let out = clean_web_source_title("", "Hit Title");
    assert_eq!(out, "Hit Title");
}

#[test]
fn clean_title_preserves_short_meaningful_title() {
    let out = clean_web_source_title("A — resolved", "fallback");
    assert_eq!(out, "A — resolved");
}

#[test]
fn clean_title_returns_raw_fallback_when_both_reduce_to_empty() {
    let out = clean_web_source_title("[Skip to content][2]", "");
    assert!(
        out.is_empty(),
        "both-noise with empty fallback yields empty, got {out:?}"
    );
}

#[test]
fn clean_title_url_fallback_is_preserved() {
    let out = clean_web_source_title("", "https://example.com/deepseek-v4");
    assert_eq!(out, "https://example.com/deepseek-v4");
}

#[test]
fn strip_leading_noise_is_case_insensitive() {
    let out = clean_title_text("SKIP TO MAIN CONTENT: DeepSeek V4 Pro");
    assert_eq!(out, "DeepSeek V4 Pro");
}

#[test]
fn truncate_title_words_keeps_short_input_intact() {
    let out = truncate_title_words("short title", 120);
    assert_eq!(out, "short title");
}

#[test]
fn truncate_title_words_returns_empty_for_empty_input() {
    let out = truncate_title_words("", 120);
    assert!(out.is_empty());
}

// ── relevance tests ───────────────────────────────────────────────────────

#[test]
fn normalize_query_terms_deduplicates_and_drops_stopwords() {
    let terms = normalize_query_terms("What is the Rust async and Tokio runtime");
    assert!(terms.contains(&"rust".to_string()));
    assert!(terms.contains(&"async".to_string()));
    assert!(terms.contains(&"tokio".to_string()));
    assert!(terms.contains(&"runtime".to_string()));
    assert!(!terms.contains(&"what".to_string()));
    assert!(!terms.contains(&"is".to_string()));
    assert!(!terms.contains(&"the".to_string()));
    assert!(!terms.contains(&"and".to_string()));
    // Deduplicated: "tokio" appears once even though input had no duplicates.
    let tokio_count = terms.iter().filter(|t| *t == "tokio").count();
    assert_eq!(tokio_count, 1);
}

// ── classify tests ────────────────────────────────────────────────────────

#[test]
fn classify_web_source_detects_pdf_by_content_type() {
    assert_eq!(
        classify_web_source("https://example.com/doc", Some("application/pdf")),
        WebSourceKind::Pdf
    );
}

#[test]
fn classify_web_source_detects_pdf_by_extension() {
    assert_eq!(
        classify_web_source("https://example.com/doc.pdf", None),
        WebSourceKind::Pdf
    );
}

#[test]
fn classify_web_source_detects_youtube() {
    assert_eq!(
        classify_web_source("https://youtube.com/watch?v=abc", None),
        WebSourceKind::YouTube
    );
    assert_eq!(
        classify_web_source("https://youtu.be/abc", None),
        WebSourceKind::YouTube
    );
}

#[test]
fn classify_web_source_defaults_to_page() {
    assert_eq!(
        classify_web_source("https://example.com/article", None),
        WebSourceKind::Page
    );
}
