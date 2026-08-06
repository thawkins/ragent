//! Property-based tests for title cleaning and relevance scoring (Milestone H-004).
//!
//! Uses `proptest` to generate arbitrary inputs and verify invariants that
//! the hand-written unit tests can only spot-check:
//!
//! - `clean_title_text` never exceeds `MAX_WEB_SOURCE_TITLE_CHARS`.
//! - `truncate_title_words` never exceeds the requested `max_chars`.
//! - `normalize_query_terms` never contains stopwords or duplicates.
//! - `compute_relevance_label` always returns a label starting with one of
//!   the known prefixes and a consistent `retained` flag.

#[path = "../src/web_gatherer/title.rs"]
mod title;

#[path = "../src/web_gatherer/relevance.rs"]
mod relevance;

use proptest::prelude::*;
use relevance::{compute_relevance_label, normalize_query_terms};
use title::{MAX_WEB_SOURCE_TITLE_CHARS, clean_title_text, truncate_title_words};

/// Any string up to 500 chars (arbitrary "web page title" surrogate).
fn arb_title() -> impl Strategy<Value = String> {
    ".{0,500}"
}

/// Any non-empty string for `max_chars` between 1 and 200.
fn arb_max_chars() -> impl Strategy<Value = usize> {
    1usize..=200
}

proptest! {
    /// `clean_title_text` output never exceeds `MAX_WEB_SOURCE_TITLE_CHARS`
    /// Unicode scalar values, regardless of the input.
    #[test]
    fn prop_clean_title_text_within_max(s in arb_title()) {
        let out = clean_title_text(&s);
        prop_assert!(
            out.chars().count() <= MAX_WEB_SOURCE_TITLE_CHARS,
            "clean_title_text output was {} chars, max is {}",
            out.chars().count(),
            MAX_WEB_SOURCE_TITLE_CHARS
        );
    }

    /// `truncate_title_words` output never exceeds the requested `max_chars`,
    /// regardless of the input.
    #[test]
    fn prop_truncate_title_words_within_max(s in arb_title(), max in arb_max_chars()) {
        let out = truncate_title_words(&s, max);
        prop_assert!(
            out.chars().count() <= max,
            "truncate_title_words output was {} chars, max was {}",
            out.chars().count(),
            max
        );
    }

    /// `truncate_title_words` preserves short input unchanged (within budget).
    #[test]
    fn prop_truncate_preserves_short(s in ".{0,50}", max in 51usize..=200) {
        let out = truncate_title_words(&s, max);
        prop_assert_eq!(out, s);
    }

    /// `normalize_query_terms` never produces duplicates.
    #[test]
    fn prop_normalize_no_duplicates(query in arb_title()) {
        let terms = normalize_query_terms(&query);
        let unique: std::collections::HashSet<&String> = terms.iter().collect();
        prop_assert_eq!(unique.len(), terms.len());
    }

    /// `normalize_query_terms` never produces stopwords.
    #[test]
    fn prop_normalize_no_stopwords(query in arb_title()) {
        let terms = normalize_query_terms(&query);
        for t in &terms {
            let lower = t.to_lowercase();
            // Check against a representative subset of the stopword list used
            // in `relevance.rs`. We inline the key ones so the test is
            // self-contained.
            const STOPWORDS: &[&str] = &[
                "a", "an", "the", "is", "are", "was", "were", "be", "been", "being", "have", "has",
                "had", "do", "does", "did", "will", "would", "could", "should", "may", "might",
                "must", "can", "shall", "of", "in", "on", "at", "to", "for", "with", "from", "by",
                "about", "as", "and", "or", "but", "not", "no", "yes", "what", "which", "who",
                "when", "where", "why", "how", "this", "that", "these", "those", "i", "you", "he",
                "she", "it", "we", "they", "their", "there", "them", "his", "her", "its", "our",
                "your", "my", "me", "him", "us",
            ];
            prop_assert!(
                !STOPWORDS.contains(&lower.as_str()),
                "stopword '{}' should have been filtered out",
                t
            );
        }
    }

    /// `normalize_query_terms` output terms are always lowercase.
    #[test]
    fn prop_normalize_terms_lowercase(query in arb_title()) {
        let terms = normalize_query_terms(&query);
        for t in &terms {
            prop_assert_eq!(t, &t.to_lowercase());
        }
    }

    /// `compute_relevance_label` always returns a label starting with one of
    /// the known prefixes.
    #[test]
    fn prop_relevance_label_known_prefix(
        query in arb_title(),
        title in arb_title(),
        snippet in arb_title(),
        url in arb_title()
    ) {
        let (label, _) = compute_relevance_label(&query, &title, &snippet, &url);
        let known = [
            "Very high",
            "High",
            "Medium-high",
            "Medium",
            "Low",
            "Very low",
            "Match score unavailable",
        ];
        let ok = known.iter().any(|p| label.starts_with(p));
        prop_assert!(ok, "unknown label prefix: {label:?}");
    }

    /// `compute_relevance_label` returns `retained == false` for "Low" and
    /// "Very low" labels, and `true` otherwise.
    #[test]
    fn prop_relevance_retained_consistency(
        query in arb_title(),
        title in arb_title(),
        snippet in arb_title(),
        url in arb_title()
    ) {
        let (label, retained) = compute_relevance_label(&query, &title, &snippet, &url);
        let expected = !label.starts_with("Low") && !label.starts_with("Very low");
        prop_assert_eq!(retained, expected);
    }

    /// `compute_relevance_label` with an empty query always returns the
    /// "Match score unavailable" label and `retained == true`.
    #[test]
    fn prop_relevance_empty_query(
        title in arb_title(),
        snippet in arb_title(),
        url in arb_title()
    ) {
        let (label, retained) = compute_relevance_label("", &title, &snippet, &url);
        prop_assert_eq!(label, "Match score unavailable");
        prop_assert!(retained);
    }
}
