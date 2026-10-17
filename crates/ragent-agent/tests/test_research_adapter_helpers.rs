//! Tests-only coverage for the pure research-adapter helpers
//! (`ragent_agent::research_adapter`): URL normalisation, the websearch output
//! parser, and the readability extractability probe.
//!
//! No network: `readability_extract_ok` parses the supplied HTML locally.

use ragent_agent::research_adapter::{
    normalize_github_blob_url, parse_websearch_output, readability_extract_ok,
};

#[test]
fn normalize_github_blob_url_rewrites_to_raw() {
    assert_eq!(
        normalize_github_blob_url("https://github.com/owner/repo/blob/main/src/lib.rs"),
        "https://raw.githubusercontent.com/owner/repo/main/src/lib.rs"
    );
    // The `www.` host is accepted too.
    assert_eq!(
        normalize_github_blob_url("https://www.github.com/o/r/blob/v1/a/b.rs"),
        "https://raw.githubusercontent.com/o/r/v1/a/b.rs"
    );
}

#[test]
fn normalize_github_blob_url_leaves_non_matches_alone() {
    // Non-blob GitHub URL (repo root).
    assert_eq!(
        normalize_github_blob_url("https://github.com/owner/repo"),
        "https://github.com/owner/repo"
    );
    // A blob directory listing (too few segments) is left unchanged.
    assert_eq!(
        normalize_github_blob_url("https://github.com/owner/repo/blob/main"),
        "https://github.com/owner/repo/blob/main"
    );
    // A non-GitHub host is left unchanged.
    assert_eq!(
        normalize_github_blob_url("https://gitlab.com/o/r/blob/main/x.rs"),
        "https://gitlab.com/o/r/blob/main/x.rs"
    );
    // A malformed URL is returned verbatim.
    assert_eq!(normalize_github_blob_url("not a url"), "not a url");
}

#[test]
fn parse_websearch_output_extracts_numbered_hits() {
    let output = "\
1. First Result Title
   https://example.com/one
   A short snippet for the first hit.

2. Second Result Title
   https://example.com/two
   Snippet line one.
   Snippet line two.
";
    let hits = parse_websearch_output(output);
    assert_eq!(hits.len(), 2, "two numbered hits: {hits:?}");
    assert_eq!(hits[0].title, "First Result Title");
    assert_eq!(hits[0].url, "https://example.com/one");
    assert!(hits[0].snippet.contains("short snippet"));
    assert_eq!(hits[0].search_tool, "websearch");
    assert_eq!(hits[0].search_engine, "tavily");

    assert_eq!(hits[1].title, "Second Result Title");
    // Multi-line snippets are joined with a single space.
    assert!(
        hits[1]
            .snippet
            .contains("Snippet line one. Snippet line two.")
    );
}

#[test]
fn parse_websearch_output_on_garbage_is_empty() {
    assert_eq!(parse_websearch_output("no results here").len(), 0);
    assert_eq!(parse_websearch_output("").len(), 0);
}

#[test]
fn readability_extract_ok_rejects_malformed_url() {
    assert!(!readability_extract_ok("<html></html>", "not a url"));
}

#[test]
fn readability_extract_ok_on_a_short_document_is_false() {
    // A page with essentially no readable text fails the minimum-length gate.
    let html = "<html><head><title>t</title></head><body><p>hi</p></body></html>";
    assert!(!readability_extract_ok(html, "https://example.com/short"));
}

#[test]
fn readability_extract_ok_on_a_full_article_is_true() {
    // A realistic article body long enough to pass the readability threshold.
    let body = "This is a sentence about Rust programming and testing. ".repeat(40);
    let html = format!(
        "<html><head><title>An Article</title></head><body><article><h1>Heading</h1><p>{body}</p></article></body></html>"
    );
    assert!(readability_extract_ok(&html, "https://example.com/article"));
}
