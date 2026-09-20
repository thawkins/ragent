//! Unit tests for web-gatherer title cleaning, relevance scoring, and
//! classification helpers extracted from `web_gatherer.rs` (Milestone F-002).

#[path = "../src/web_gatherer/title.rs"]
mod title;

#[path = "../src/web_gatherer/relevance.rs"]
mod relevance;

#[path = "../src/web_gatherer/classify.rs"]
mod classify;

use classify::{WebSourceKind, classify_web_source};
use relevance::{PreparedQuery, compute_relevance_label, normalize_query_terms, term_matches};
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

// ── morphological term-matching tests ─────────────────────────────────────

#[test]
fn term_matches_literal_substring() {
    assert!(term_matches("loop", "agentic loop architecture"));
    assert!(!term_matches("loop", "agentic workflow"));
}

#[test]
fn term_matches_plural_variant() {
    // Query says "loops", title says "loop".
    assert!(term_matches("loops", "agentic loop architecture"));
    // Query says "loop", title says "loops".
    assert!(term_matches("loop", "the loops that drive agents"));
    // "queries" -> "query".
    assert!(term_matches("queries", "query planning"));
    // "branches" strips -es directly to "branch".
    assert!(term_matches("branches", "git branch naming"));
}

#[test]
fn term_matches_derivational_variant() {
    // "agentic" -> "agent": queries using adjective form match agent nouns.
    assert!(term_matches("agentic", "AI agent frameworks"));
    assert!(term_matches("agent", "agentic workflows explained"));
    // "engineering" -> "engine".
    assert!(term_matches("engineering", "prompt engine design"));
    // "prompting" -> "prompt".
    assert!(term_matches("prompting", "prompt engineering guide"));
    // "running" -> "run" (doubled consonant).
    assert!(term_matches("running", "run a loop"));
}

#[test]
fn term_matches_no_false_positive_for_unrelated_terms() {
    // "rate" must not match "iterate" via a bogus stem.
    assert!(!term_matches("rate", "iteration speed"));
    // A short stem must not appear inside an unrelated longer word's
    // substring space unless genuinely present.
    assert!(!term_matches("quantum", "classical computing"));
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

// ── relevance-label tests ─────────────────────────────────────────────────

#[test]
fn relevance_label_rescues_two_title_term_hits_on_verbose_queries() {
    // Reproduces the 2026-09-13 gather-log exclusion: the decomposed
    // sub-query has 6 terms, the on-topic title matches only 2 ("agent",
    // "loops") for ratio 0.33 — below the 0.35 Medium floor but clearly
    // topical via the title signal.
    let (label, retained) = compute_relevance_label(
        "how to write goals and configure AI agent loops",
        "Designing agentic loops",
        "Patterns for structuring autonomous agent systems.",
        "https://simonwillison.net/2025/Sep/30/designing-agentic-loops",
    );
    assert!(
        retained,
        "two title-term hits must be retained, got {label}"
    );
    assert!(label.starts_with("Medium"));
}

#[test]
fn relevance_label_title_rescue_requires_two_distinct_title_terms() {
    // Only one query term in the title: the rescue must NOT fire, so a
    // genuinely unrelated page stays rejected.
    let (_, retained) = compute_relevance_label(
        "agentic loop programming recent articles 2025",
        "Semantic Role Labeling: A Systematical Survey",
        "We survey semantic role labeling methods.",
        "https://arxiv.org/html/2502.08660v1",
    );
    assert!(!retained, "single-title-term hit must stay rejected");
}

#[test]
fn relevance_label_high_ratio_still_labels_as_before() {
    let (label, retained) = compute_relevance_label(
        "rust async runtime",
        "Rust async runtime guide",
        "Tokio and async Rust performance runtime",
        "https://example.com/tokio",
    );
    assert!(retained);
    assert!(label.starts_with("High"), "got {label}");
}

#[test]
fn relevance_label_low_ratio_without_title_signal_stays_rejected() {
    let (_, retained) = compute_relevance_label(
        "quantum computing error correction",
        "buy shoes and gadgets here",
        "discount sneakers and phone cases",
        "https://shop.example",
    );
    assert!(!retained);
}

// ── prepared-query equivalence tests (PERF-068) ──────────────────────────

#[test]
fn prepared_query_matches_compute_relevance_label() {
    let query = "how to configure and operate agentic AI loops";
    let prepared = PreparedQuery::new(query);
    let cases = [
        (
            "Designing agentic loops",
            "Patterns for structuring autonomous agent systems.",
            "https://simonwillison.net/2025/Sep/30/designing-agentic-loops",
        ),
        (
            "What is an agentic loop? (And how to build one)",
            "A practical guide to building agentic loops in production.",
            "https://www.inngest.com/blog/agent-loop-architecture",
        ),
        (
            "Semantic Role Labeling: A Systematical Survey",
            "We survey semantic role labeling methods.",
            "https://arxiv.org/html/2502.08660v1",
        ),
        ("", "no title at all", "https://example.com/empty-title"),
    ];
    for (title, snippet, url) in cases {
        assert_eq!(
            prepared.label(title, snippet, url),
            compute_relevance_label(query, title, snippet, url),
            "prepared vs one-shot mismatch for title {title:?}"
        );
    }
}

#[test]
fn prepared_query_empty_query_is_match_unavailable() {
    let prepared = PreparedQuery::new("");
    let (label, retained) = prepared.label("anything", "anything", "https://example.com");
    assert_eq!(label, "Match score unavailable");
    assert!(retained);
}

#[test]
fn prepared_query_handles_uppercase_url_without_changing_result() {
    // The Cow lowercase path must still match a mixed-case URL exactly as the
    // unconditional-to_lowercase path did.
    let query = "rust async";
    let prepared = PreparedQuery::new(query);
    assert_eq!(
        prepared.label("unrelated", "unrelated", "HTTPS://EXAMPLE.COM/RUST-ASYNC"),
        compute_relevance_label(
            query,
            "unrelated",
            "unrelated",
            "HTTPS://EXAMPLE.COM/RUST-ASYNC"
        ),
    );
    assert_eq!(
        prepared.label("unrelated", "unrelated", "HTTPS://EXAMPLE.COM/RUST-ASYNC"),
        compute_relevance_label(
            query,
            "unrelated",
            "unrelated",
            "https://example.com/rust-async"
        ),
    );
}
