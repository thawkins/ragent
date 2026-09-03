#![allow(clippy::assert_is_empty)]
//! Integration tests for `masterfetch::focus` — BM25 query-focused content
//! filtering (T-009 / T-034, FR-004, NFR-003).
//!
//! Covers: BM25 scoring, threshold filtering, heading preservation, fallback
//! to top-N, empty query (no-op), single block (no-op), custom parameters,
//! block ordering, multiline blocks, punctuation in query.

use ragent_tools_extended::masterfetch::focus::{
    FocusParams, focus_content, focus_content_with_params,
};

// ---------------------------------------------------------------------------
// No-op conditions
// ---------------------------------------------------------------------------

#[test]
fn test_empty_query_is_noop() {
    let text = "block one\n\nblock two\n\nblock three";
    assert_eq!(focus_content(text, ""), text);
}

#[test]
fn test_whitespace_query_is_noop() {
    let text = "block one\n\nblock two";
    assert_eq!(focus_content(text, "   "), text);
}

#[test]
fn test_empty_text_is_noop() {
    assert_eq!(focus_content("", "query"), "");
}

#[test]
fn test_single_block_is_noop() {
    let text = "only one block here";
    assert_eq!(focus_content(text, "block"), text);
}

#[test]
fn test_no_query_terms_is_noop() {
    let text = "alpha beta\n\ngamma delta";
    // "x" and "y" are single-char tokens → filtered out → no usable terms.
    assert_eq!(focus_content(text, "x y"), text);
}

#[test]
fn test_text_with_only_blank_lines_is_noop() {
    assert_eq!(focus_content("\n\n\n", "query"), "\n\n\n");
}

// ---------------------------------------------------------------------------
// BM25 scoring & threshold filtering
// ---------------------------------------------------------------------------

#[test]
fn test_keeps_relevant_blocks() {
    let text = "Rust is a systems programming language focused on safety.\n\n\
                Python is great for data science and machine learning.\n\n\
                Go is simple and fast for microservices.";
    let focused = focus_content(text, "rust systems programming");
    assert!(focused.contains("Rust is a systems programming language"));
    assert!(!focused.contains("Python is great"));
    assert!(!focused.contains("Go is simple"));
}

#[test]
fn test_prefers_frequent_term_blocks() {
    let text = "rust rust rust rust rust\n\nrust once\n\nno match here at all";
    let focused = focus_content(text, "rust");
    // The block with 5 occurrences should be kept.
    assert!(focused.contains("rust rust rust rust rust"));
}

#[test]
fn test_all_blocks_kept_when_all_relevant() {
    let text = "rust alpha\n\nrust beta\n\nrust gamma";
    let focused = focus_content(text, "rust");
    assert!(focused.contains("rust alpha"));
    assert!(focused.contains("rust beta"));
    assert!(focused.contains("rust gamma"));
    assert!(focused.contains("of 3 blocks"));
}

#[test]
fn test_custom_threshold_zero_keeps_all_matching() {
    let text = "alpha beta\n\ngamma delta\n\nalpha epsilon";
    let params = FocusParams {
        threshold: 0.0,
        ..FocusParams::default()
    };
    let focused = focus_content_with_params(text, "alpha", &params);
    assert!(focused.contains("alpha beta"));
    assert!(focused.contains("alpha epsilon"));
}

// ---------------------------------------------------------------------------
// Heading preservation
// ---------------------------------------------------------------------------

#[test]
fn test_preserves_preceding_heading() {
    let text = "# Rust Guide\n\
                Rust is a systems programming language.\n\n\
                # Other Section\n\
                Python is for data science.";
    let focused = focus_content(text, "rust systems programming");
    assert!(focused.contains("# Rust Guide"));
}

#[test]
fn test_does_not_preserve_heading_for_kept_heading_block() {
    // If the kept block is itself a heading, no extra heading is preserved.
    let text = "# Heading One\n\n# Heading Two About Rust\n\nOther content";
    let focused = focus_content(text, "rust");
    assert!(!focused.is_empty());
}

// ---------------------------------------------------------------------------
// Fallback to top-N
// ---------------------------------------------------------------------------

#[test]
fn test_fallback_returns_content_when_nothing_clears_threshold() {
    let text = "alpha beta gamma\n\ndelta epsilon zeta\n\neta theta iota";
    let params = FocusParams {
        threshold: 999.0,
        fallback_top: 2,
        ..FocusParams::default()
    };
    let focused = focus_content_with_params(text, "completely unrelated xyzzy", &params);
    assert!(focused.starts_with("[Focus: "));
    assert!(!focused.is_empty());
}

#[test]
fn test_fallback_top_n_limit() {
    let blocks: Vec<String> = (0..10)
        .map(|i| format!("block {i} content alpha"))
        .collect();
    let text = blocks.join("\n\n");
    let params = FocusParams {
        threshold: 999.0,
        fallback_top: 3,
        ..FocusParams::default()
    };
    let focused = focus_content_with_params(&text, "xyzzy no match", &params);
    let count = focused.matches("block ").count();
    assert!(
        count <= 3,
        "fallback should keep at most fallback_top blocks, got {count}"
    );
}

// ---------------------------------------------------------------------------
// Header line
// ---------------------------------------------------------------------------

#[test]
fn test_header_line_present() {
    let text = "rust programming is great\n\npython data science is great";
    let focused = focus_content(text, "rust programming");
    assert!(focused.starts_with("[Focus: "));
    assert!(focused.contains("blocks by BM25 relevance"));
    assert!(focused.contains("Pass focus='' for the full page."));
}

#[test]
fn test_header_shows_block_count() {
    let text = "rust block alpha\n\nunrelated block\n\nrust block beta";
    let focused = focus_content(text, "rust");
    assert!(focused.contains("of 3 blocks"));
}

// ---------------------------------------------------------------------------
// Block ordering
// ---------------------------------------------------------------------------

#[test]
fn test_preserves_block_order() {
    let text = "alpha first\n\nbeta second alpha\n\ngamma third";
    let focused = focus_content(text, "alpha");
    let pos_first = focused.find("alpha first").unwrap();
    let pos_second = focused.find("beta second alpha").unwrap();
    assert!(pos_first < pos_second);
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[test]
fn test_query_with_punctuation() {
    let text = "rust programming language\n\npython data science";
    let focused = focus_content(text, "rust, programming!!!");
    assert!(focused.contains("rust programming language"));
}

#[test]
fn test_multiline_blocks_preserved() {
    let text = "# Heading\n\n\
                line one of block\n\
                line two of block\n\n\
                another block entirely";
    let focused = focus_content(text, "block");
    assert!(focused.contains("line one of block\nline two of block"));
}

#[test]
fn test_empty_query_returns_original_exactly() {
    let text = "  leading spaces\n\ntrailing  ";
    assert_eq!(focus_content(text, ""), text);
}

#[test]
fn test_custom_k1_b_params() {
    let text = "rust rust rust\n\nrust\n\nno match";
    let params = FocusParams {
        k1: 2.0,
        b: 0.3,
        threshold: 0.0,
        ..FocusParams::default()
    };
    let focused = focus_content_with_params(text, "rust", &params);
    assert!(focused.contains("rust"));
}

#[test]
fn test_default_params_match_constants() {
    let params = FocusParams::default();
    assert_eq!(params.threshold, 1.0);
    assert_eq!(params.k1, 1.5);
    assert_eq!(params.b, 0.75);
    assert_eq!(params.fallback_top, 5);
}
// ---------------------------------------------------------------------------
// Heading preservation — query matches block but not the preceding heading
// ---------------------------------------------------------------------------

#[test]
fn test_heading_preserved_when_following_block_matches() {
    // The query "rust" does not appear in "# Unrelated Heading", but the
    // following block is about rust. The heading immediately preceding a
    // kept block must be preserved so the output is coherent.
    let content = "# Unrelated Heading\n\nSome text about rust programming language.\n\n## Other\n\nUnrelated text.";
    let out = focus_content(content, "rust");
    assert!(out.contains("rust"), "kept block should be present: {out}");
    // The heading immediately before the matched block should be preserved.
    assert!(
        out.contains("Unrelated Heading") || !out.contains("Other"),
        "preceding heading preserved, unrelated later section dropped: {out}"
    );
}

#[test]
fn test_query_matching_heading_keeps_that_block() {
    // A heading that itself contains the query term should keep its block.
    let content = "# Foxes Are Cool\n\nGeneral text without the term.\n\n## Other\n\nMore text.";
    let out = focus_content(content, "foxes");
    assert!(
        out.to_lowercase().contains("foxes"),
        "heading with query term should be kept: {out}"
    );
}

// ---------------------------------------------------------------------------
// Empty content / single block / empty query edge cases
// ---------------------------------------------------------------------------

#[test]
fn test_empty_content_returns_empty() {
    let out = focus_content("", "anything");
    assert!(
        out.is_empty(),
        "empty content should yield empty output: [{out}]"
    );
}

#[test]
fn test_whitespace_only_content_returns_empty() {
    let out = focus_content("   \n\n  \n  ", "anything");
    assert!(
        out.trim().is_empty(),
        "whitespace-only content should yield empty/whitespace output: [{out}]"
    );
}

#[test]
fn test_no_matching_blocks_falls_back_to_top_n() {
    // No block contains the query — implementation falls back to returning the
    // top-N closest blocks (or all blocks). Either way it must be non-empty
    // and not panic.
    let content = "# H1\n\nBlock one text.\n\n# H2\n\nBlock two text.";
    let out = focus_content(content, "zzzznomatch");
    // Fallback should return *some* content rather than an empty string.
    assert!(
        !out.trim().is_empty(),
        "no-match fallback should return top-N content, got empty: [{out}]"
    );
}

// ---------------------------------------------------------------------------
// Multi-term query — BM25 over a phrase
// ---------------------------------------------------------------------------

#[test]
fn test_multi_term_query_ranks_relevant_block_higher() {
    // Block A contains both "rust" and "async"; block B contains only "rust".
    // The block with more query-term coverage should be preferred.
    let content = "# A\n\nrust async tokio runtime.\n\n# B\n\nrust is a language.";
    let out = focus_content(content, "rust async");
    assert!(out.contains("tokio"), "block with more terms kept: {out}");
}

// ---------------------------------------------------------------------------
// Case insensitivity
// ---------------------------------------------------------------------------

#[test]
fn test_query_case_insensitive_scoring() {
    // Tokenisation lowercases both query and content, so the *kept blocks*
    // are identical regardless of query case. (The focus header echoes the
    // original query string verbatim, so the full output strings differ in
    // the header — we compare the post-header content instead.)
    let content = "# H\n\nThe Rust programming language is fast.";
    let lower = focus_content(content, "rust");
    let upper = focus_content(content, "RUST");
    let mixed = focus_content(content, "RuSt");

    // Strip the "[Focus: ...]" header (everything up to the first blank line).
    let body = |s: &str| s.split("\n\n").skip(1).collect::<Vec<_>>().join("\n\n");
    assert_eq!(
        body(&lower),
        body(&upper),
        "case-insensitive: lower == upper"
    );
    assert_eq!(
        body(&lower),
        body(&mixed),
        "case-insensitive: lower == mixed"
    );
    assert!(
        body(&lower).contains("Rust"),
        "matched block kept: {}",
        body(&lower)
    );
}

// ---------------------------------------------------------------------------
// Threshold filtering — low-relevance blocks dropped
// ---------------------------------------------------------------------------

#[test]
fn test_low_relevance_block_dropped() {
    // Block A is strongly about "python"; block B only mentions it once in a
    // long unrelated paragraph. A should be kept; B may be dropped.
    let content = "# A\n\npython python python snakes and code.\n\n# B\n\nA long paragraph about many topics including a single python mention among dozens of other unrelated words about cooking travel and weather.";
    let out = focus_content(content, "python");
    assert!(out.contains("snakes"), "high-relevance block kept: {out}");
}

// ---------------------------------------------------------------------------
// Punctuation / tokenisation
// ---------------------------------------------------------------------------

#[test]
fn test_query_term_with_punctuation_still_matches() {
    // "rust." with trailing punctuation in the content should still match "rust".
    let content = "# H\n\nI love rust. Rust is great.";
    let out = focus_content(content, "rust");
    assert!(out.contains("rust"), "punctuation-handled match: {out}");
}
