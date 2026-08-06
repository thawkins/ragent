//! Unit tests for topic derivation helpers extracted from `session.rs`
//! (Milestone F-001).

#[path = "../src/session/topic.rs"]
mod topic;

use topic::{
    MAX_DERIVED_TOPIC_CHARS, clean_site_title, derive_topic_from_url_body, fuzzy_contains,
    split_glued_words,
};

#[test]
fn split_glued_words_splits_camel_case_and_acronyms() {
    assert_eq!(
        split_glued_words("HomepageArticlesLarge"),
        "Homepage Articles Large"
    );
    assert_eq!(split_glued_words("AIReasoning"), "AI Reasoning");
    assert_eq!(split_glued_words("RustAsync"), "Rust Async");
    // Lower/upper transitions only, not inside acronyms.
    assert_eq!(split_glued_words("URL"), "URL");
}

#[test]
fn clean_site_title_strips_site_brand_and_nav_prefixes() {
    assert_eq!(
        clean_site_title(
            "InfoQ HomepageArticlesLarge Concept Models: a Paradigm Shift in AI Reasoning"
        ),
        Some("Large Concept Models: a Paradigm Shift in AI Reasoning".into())
    );
    assert_eq!(
        clean_site_title("Medium | Articles | Rust Async Patterns"),
        Some("Rust Async Patterns".into())
    );
    assert_eq!(
        clean_site_title("Large Concept Models: a Paradigm Shift in AI Reasoning | InfoQ"),
        Some("Large Concept Models: a Paradigm Shift in AI Reasoning".into())
    );
}

#[test]
fn clean_site_title_rejects_generic_or_short_titles() {
    assert_eq!(clean_site_title("Example Site"), None);
    assert_eq!(clean_site_title("Home"), None);
    assert_eq!(clean_site_title("Articles"), None);
    assert_eq!(clean_site_title("OK"), None);
}

#[test]
fn derive_topic_prefers_cleaned_title_over_body() {
    let body = "Home About Contact\n\nThe actual article begins here with useful content."; // 10 words - kept
    let title = "InfoQ HomepageArticlesLarge Concept Models: a Paradigm Shift in AI Reasoning";
    assert_eq!(
        derive_topic_from_url_body(body, title, "https://example.com/article"),
        Some("Large Concept Models: a Paradigm Shift in AI Reasoning — The actual article begins here with useful content".into())
    );
}

#[test]
fn derive_topic_appends_description_and_skips_title_duplicate() {
    let body = "Large Concept Models: a Paradigm Shift in AI Reasoning are introduced in this article. They move generation from tokens to concepts, improving reasoning and explainability.";
    let title = "InfoQ HomepageArticlesLarge Concept Models: a Paradigm Shift in AI Reasoning";
    let topic = derive_topic_from_url_body(body, title, "https://example.com/article");
    assert!(
        topic
            .as_deref()
            .unwrap_or("")
            .starts_with("Large Concept Models: a Paradigm Shift in AI Reasoning — They move"),
        "expected title + body description, got {topic:?}"
    );
}

#[test]
fn derive_topic_falls_back_to_body_when_title_is_generic() {
    let body = "The Rust async model maps asynchronous operations onto lightweight futures.";
    let title = "Example Site";
    let topic = derive_topic_from_url_body(body, title, "https://example.com/article");
    assert!(
        topic
            .as_deref()
            .unwrap_or("")
            .contains("Rust async model maps asynchronous operations"),
        "expected body-derived topic, got {topic:?}"
    );
}

#[test]
fn derive_topic_description_truncates_long_sentences() {
    let body = "This is an extremely long introductory sentence that goes on and on and on in order to test that the derived topic description is truncated to a reasonable length without breaking in the middle of a word.";
    let title = "Some Article Title";
    let topic = derive_topic_from_url_body(body, title, "https://example.com/article").unwrap();
    assert!(
        topic.len() <= MAX_DERIVED_TOPIC_CHARS,
        "topic too long: {}",
        topic.len()
    );
    assert!(topic.starts_with("Some Article Title —"), "topic: {topic}");
}

#[test]
fn fuzzy_contains_detects_subsequence() {
    assert!(fuzzy_contains(
        "large concept models shift ai reasoning",
        "concept models shift"
    ));
    assert!(!fuzzy_contains(
        "large concept models",
        "concept models shift"
    ));
}
