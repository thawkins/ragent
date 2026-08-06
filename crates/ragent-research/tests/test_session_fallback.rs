//! Unit tests for mechanical fallback content helpers extracted from
//! `session.rs` (Milestone F-001).

// Shims so that `crate::document` and `crate::source` resolve correctly
// when `fallback.rs` is compiled inside this test crate via `#[path]`.
pub use ragent_research::document;
pub use ragent_research::source;

#[path = "../src/session/fallback.rs"]
mod fallback;

use fallback::{body_excerpt, default_findings, default_open_questions, default_summary};
use ragent_research::source::{LocalSourceKind, Source};
use std::path::PathBuf;

fn web_source(title: &str, url: &str, body: &str) -> Source {
    Source::Web {
        published_at: None,
        url: url.into(),
        title: title.into(),
        captured_at: chrono::Utc::now(),
        body_path: PathBuf::from("sources/web-01.md"),
        relevance: String::new(),
        body: body.into(),
        search_tool: String::new(),
        search_engine: String::new(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
        language: None,
    }
}

fn local_source(path: &str, relevance: &str, body: &str) -> Source {
    Source::Local {
        path: path.into(),
        kind: LocalSourceKind::InProject,
        captured_at: chrono::Utc::now(),
        body_path: PathBuf::from("sources/local-01.md"),
        relevance: relevance.into(),
        body: body.into(),
    }
}

#[test]
fn default_summary_counts_each_source_type() {
    let s = vec![web_source("t", "u", ""), local_source("x.md", "r", "")];
    let out = default_summary(&s, "topic");
    assert!(out.contains("2 source(s)"));
    assert!(out.contains("1 web"));
    assert!(out.contains("1 local"));
    // Mechanical fallback must be transparent about its provenance.
    assert!(out.contains("No LLM analysis was applied"));
}

#[test]
fn default_summary_names_web_titles_and_local_paths() {
    let s = vec![
        web_source("Article A", "https://a", ""),
        web_source("Article B", "https://b", ""),
        local_source("src/lib.rs", "anchor", ""),
    ];
    let out = default_summary(&s, "topic");
    assert!(out.contains("**Web sources:**"));
    assert!(out.contains("Article A"));
    assert!(out.contains("Article B"));
    assert!(out.contains("**Local files:**"));
    assert!(out.contains("src/lib.rs"));
}

#[test]
fn default_summary_handles_empty_source_list() {
    let out = default_summary(&[], "topic");
    assert!(out.contains("No sources were captured"));
    assert!(!out.contains("No LLM analysis"));
}

#[test]
fn default_findings_handles_zero_sources() {
    let out = default_findings(&[], "x");
    assert_eq!(out.len(), 1);
    assert!(out[0].contains("No sources"));
    assert!(out[0].contains("**Headline:**"));
    assert!(out[0].contains("**Observation:**"));
    assert!(out[0].contains("No direct dependencies."));
}

#[test]
fn default_findings_include_source_citation_marker() {
    let s = vec![web_source(
        "Article A",
        "https://a",
        "Body of article A — talks about cargo workspaces and lockfiles.",
    )];
    let out = default_findings(&s, "topic");
    assert_eq!(out.len(), 1);
    assert!(
        out[0].contains("[#1]"),
        "mechanical finding should cite its source: {}",
        out[0]
    );
}

#[test]
fn default_findings_emits_per_source_with_excerpts() {
    let s = vec![
        web_source(
            "Article A",
            "https://a",
            "Body of article A — talks about cargo workspaces and lockfiles.",
        ),
        local_source(
            "src/lib.rs",
            "anchor file",
            "Excerpt — 2 keyword match(es)\n\n▶    1: fn main() { }",
        ),
        Source::Spec {
            spec_id: "foo".into(),
            captured_at: chrono::Utc::now(),
            relevance: "Foo spec".into(),
        },
    ];
    let out = default_findings(&s, "topic");
    // One finding per source.
    assert_eq!(out.len(), 3, "expected 3 findings, got {out:?}");
    // Each finding uses the five-paragraph structure (Headline + four required).
    for f in &out {
        assert!(
            f.contains("**Headline:**"),
            "missing Headline paragraph: {f}"
        );
        assert!(
            f.contains("**Observation:**"),
            "missing Observation paragraph: {f}"
        );
        assert!(
            f.contains("**Analysis:**"),
            "missing Analysis paragraph: {f}"
        );
        assert!(
            f.contains("**Cross-reference / Dependencies:**"),
            "missing Cross-reference paragraph: {f}"
        );
        assert!(
            f.contains("**Implication:**"),
            "missing Implication paragraph: {f}"
        );
    }
    // Web finding carries the title and excerpt.
    assert!(out[0].contains("Article A"));
    assert!(out[0].contains("cargo workspaces"));
    // Local finding carries the relevance note and excerpt, and references the web finding.
    assert!(out[1].contains("src/lib.rs"));
    assert!(out[1].contains("anchor file"));
    assert!(out[1].contains("Finding 1"));
    // Spec finding carries the id and references the local finding.
    assert!(out[2].contains("foo"));
    assert!(out[2].contains("Finding 2"));
}

#[test]
fn body_excerpt_respects_max_chars_and_counts_ellipsis() {
    let body = "word ".repeat(50);
    let excerpt = body_excerpt(&body, 200);
    assert!(
        excerpt.chars().count() <= 200,
        "excerpt must not exceed 200 chars, got {} chars",
        excerpt.chars().count()
    );
    assert!(
        excerpt.ends_with('…'),
        "truncated excerpt should end with ellipsis"
    );
}

#[test]
fn body_excerpt_strips_trailing_markdown_fences() {
    let body = "Real content line.\n```";
    let excerpt = body_excerpt(body, 200);
    assert!(!excerpt.contains("```"));
    assert!(excerpt.starts_with("Real content line"));
}

#[test]
fn body_excerpt_strips_leading_markdown_fences() {
    let body = "```text\nThis is the real content that should appear first.\n```";
    let excerpt = body_excerpt(body, 200);
    assert!(!excerpt.starts_with("`"));
    assert!(excerpt.starts_with("This is the real content"));
}

#[test]
fn default_findings_web_excerpt_does_not_exceed_limit() {
    let long_body = "a".repeat(500);
    let s = vec![web_source("Long Article", "https://a", &long_body)];
    let out = default_findings(&s, "topic");
    assert_eq!(out.len(), 1);
    let observation = &out[0];
    let obs_start = observation
        .find("**Observation:**")
        .expect("Observation paragraph");
    let obs_body = &observation[obs_start + "**Observation:**".len()..];
    let prefix = "states: \"";
    let start = obs_body.find(prefix).expect("quoted excerpt start") + prefix.len();
    let end = obs_body[start..].find("\" [#").expect("quoted excerpt end");
    let excerpt = &obs_body[start..start + end];
    assert!(
        excerpt.chars().count() <= 200,
        "web source excerpt must be at most 200 chars, got {}: {excerpt}",
        excerpt.chars().count()
    );
}

#[test]
fn default_findings_falls_back_to_metadata_when_body_is_empty() {
    let s = vec![web_source("Empty Page", "https://a", "")];
    let out = default_findings(&s, "topic");
    assert_eq!(out.len(), 1);
    assert!(out[0].contains("Empty Page"));
    assert!(out[0].contains("no body text was returned"));
    assert!(out[0].contains("**Headline:**"));
    assert!(out[0].contains("**Observation:**"));
    assert!(out[0].contains("No direct dependencies."));
}

#[test]
fn default_open_questions_suggests_re_run_with_llm() {
    let s = vec![Source::Spec {
        spec_id: "x".into(),
        captured_at: chrono::Utc::now(),
        relevance: String::new(),
    }];
    let out = default_open_questions(&s, "topic");
    assert!(out.iter().any(|q| q.contains("No web sources")));
    assert!(out.iter().any(|q| q.contains("No in-project files")));
    // Always suggest a re-run when no LLM analysis was applied.
    assert!(out.iter().any(|q| q.contains("Re-run")));
}

#[test]
fn default_open_questions_handles_empty_source_list() {
    let out = default_open_questions(&[], "topic");
    assert_eq!(out.len(), 1);
    assert!(out[0].contains("Why was nothing captured"));
}
