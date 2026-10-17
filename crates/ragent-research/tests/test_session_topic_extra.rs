//! Extra pure-logic coverage for `session/topic.rs` helpers: the body/title
//! derivation paths, fragment cleaning, nav/noise rejection, and truncation.
//!
//! The source module is included directly via `#[path]` (the same technique the
//! existing `test_session_topic.rs` uses) so the `pub(crate)` free functions are
//! callable without widening visibility.

#[path = "../src/session/topic.rs"]
mod topic;

use topic::{
    MAX_DERIVED_TOPIC_CHARS, clean_site_title, clean_topic_fragment, collapse_whitespace,
    derive_topic_description, derive_topic_from_body, derive_topic_from_url_body,
    truncate_at_char_boundary,
};

#[test]
fn derive_topic_from_body_selects_first_substantive_sentence() {
    let body = "Short.\n\n# Real Heading Here\nMore text follows this.";
    assert_eq!(derive_topic_from_body(body), "Real Heading Here");
    // Empty and all-noise bodies yield an empty string.
    assert_eq!(derive_topic_from_body("   "), String::new());
    assert_eq!(
        derive_topic_from_body("Skip to main content"),
        String::new()
    );
}

#[test]
fn derive_topic_from_body_honours_heading_word_floor() {
    // A markdown heading with three words qualifies even below the 4-word floor.
    assert_eq!(
        derive_topic_from_body("# Three Word Heading"),
        "Three Word Heading"
    );
}

#[test]
fn derive_topic_description_returns_first_long_fragment() {
    let body = "A first sentence that is quite long indeed and descriptive.";
    let desc = derive_topic_description(body, None).expect("description");
    assert!(desc.starts_with("A first sentence"));
    assert_eq!(derive_topic_description("", None), None);
}

#[test]
fn derive_topic_description_skips_fragments_matching_title() {
    let title = "rust async runtimes explained";
    let body = "Rust async runtimes explained in depth here. Async runtimes multiplex many tasks onto few threads.";
    // First fragment echoes the title and must be skipped; the second is used.
    let desc = derive_topic_description(body, Some(title)).expect("description");
    assert!(desc.contains("multiplex"), "got {desc:?}");
}

#[test]
fn derive_topic_from_url_body_falls_back_to_body_without_title() {
    let body = "The quick brown fox jumps over the lazy dog repeatedly.";
    let topic = derive_topic_from_url_body(body, "Home", "https://example.com/x");
    assert!(
        topic.as_deref().unwrap_or("").contains("quick brown fox"),
        "got {topic:?}"
    );
    // No usable title and no body -> None.
    assert_eq!(derive_topic_from_url_body("", "Home", "u"), None);
}

#[test]
fn clean_topic_fragment_strips_markup_and_reference_indices() {
    assert_eq!(
        clean_topic_fragment("### Heading Text Here"),
        "Heading Text Here"
    );
    assert_eq!(
        clean_topic_fragment("* Bullet Item Text"),
        "Bullet Item Text"
    );
    assert_eq!(
        clean_topic_fragment("+ Plus Prefixed Text"),
        "Plus Prefixed Text"
    );
    assert_eq!(
        clean_topic_fragment("Topic with reference [12]"),
        "Topic with reference"
    );
    assert_eq!(
        clean_topic_fragment("  collapsed   whitespace  "),
        "collapsed whitespace"
    );
}

#[test]
fn collapse_and_truncate_helpers() {
    assert_eq!(collapse_whitespace("  a \n b\t c "), "a b c");
    assert_eq!(truncate_at_char_boundary("hello", 10), "hello");
    // The helper cuts on a UTF-8 char boundary; the returned slice never
    // exceeds the requested char count.
    let cut = truncate_at_char_boundary("ab\u{2014}cd", 3);
    assert!(cut.chars().count() <= 3, "cut = {cut:?}");
    assert!(cut.is_char_boundary(cut.len()));
    let long = "x".repeat(MAX_DERIVED_TOPIC_CHARS + 50);
    let truncated = truncate_at_char_boundary(&long, MAX_DERIVED_TOPIC_CHARS);
    assert!(truncated.chars().count() <= MAX_DERIVED_TOPIC_CHARS);
    assert!(truncated.chars().count() >= MAX_DERIVED_TOPIC_CHARS - 1);
}

#[test]
fn clean_site_title_handles_separator_segments_and_noise() {
    // Trailing brand after a separator wins by word count.
    assert_eq!(
        clean_site_title("Understanding Memory Safety | Tech Blog"),
        Some("Understanding Memory Safety".to_string())
    );
    // A single-segment title with navigation noise is rejected.
    assert_eq!(clean_site_title("Subscribe to our newsletter"), None);
    assert_eq!(clean_site_title(""), None);
}
