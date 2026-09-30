//! Inline tests for `cite_checker.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use crate::source::{LocalSourceKind, Source};
use chrono::Utc;
use std::path::PathBuf;

fn web_source(body: &str) -> Source {
    Source::Web {
        url: "https://example.com".into(),
        title: "Example".into(),
        captured_at: Utc::now(),
        published_at: None,
        body_path: PathBuf::from("sources/web-01.md"),
        body: body.into(),
        relevance: String::new(),
        search_tool: String::new(),
        search_engine: String::new(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
        language: None,
        oa_recovery: None,
        author: None,
    }
}

fn empty_web_source() -> Source {
    web_source("")
}

fn spec_source() -> Source {
    Source::Spec {
        spec_id: "hyperresearch".into(),
        captured_at: Utc::now(),
        relevance: String::new(),
    }
}

#[test]
fn no_citations_passes_empty() {
    let result = check_citations(
        "summary without citations",
        &["finding without citation".into()],
        &[],
        &[],
        &[web_source("body")],
    );
    assert!(result.passed);
    assert!(result.gate_open);
    assert_eq!(result.checked, 0);
    assert!(result.failed_claims.is_empty());
}

#[test]
fn valid_citation_with_body_passes() {
    let result = check_citations(
        "Claim supported by [#1].",
        &[],
        &[],
        &[],
        &[web_source("body text")],
    );
    assert!(result.passed);
    assert!(result.gate_open);
    assert_eq!(result.checked, 1);
}

#[test]
fn citation_to_unknown_source_fails_and_closes_gate() {
    let result = check_citations(
        "Claim supported by [#2].",
        &[],
        &[],
        &[],
        &[web_source("body text")],
    );
    assert!(!result.passed);
    assert!(!result.gate_open);
    assert_eq!(result.checked, 1);
    assert_eq!(result.failed_claims.len(), 1);
    assert!(result.failed_claims[0].contains("CITATION_VERIFICATION_FAILED"));
    assert!(result.issues[0].contains("unknown source index"));
}

#[test]
fn citation_to_empty_body_source_fails() {
    let result = check_citations(
        "Claim supported by [#1].",
        &[],
        &[],
        &[],
        &[empty_web_source()],
    );
    assert!(!result.passed);
    assert!(!result.gate_open);
    assert_eq!(result.failed_claims.len(), 1);
    assert!(result.failed_claims[0].contains("no captured body"));
}

#[test]
fn spec_citation_passes_without_body() {
    let result = check_citations("Claim tied to spec [#1].", &[], &[], &[], &[spec_source()]);
    assert!(result.passed);
    assert!(result.gate_open);
    assert_eq!(result.checked, 1);
}

#[test]
fn multiple_failures_accumulate() {
    let result = check_citations(
        "Summary [#1] and [#3].",
        &["Finding [#2]".into()],
        &[],
        &[],
        &[empty_web_source()],
    );
    assert!(!result.passed);
    assert_eq!(result.checked, 3);
    assert_eq!(result.failed_claims.len(), 3);
}

#[test]
fn checks_all_text_fields() {
    let sources = vec![web_source("evidence")];
    let result = check_citations(
        "Summary [#1].",
        &["Finding [#1]".into()],
        &["Implication [#1]".into()],
        &["Question [#1]?".into()],
        &sources,
    );
    assert!(result.passed);
    assert_eq!(result.checked, 4);
}

#[test]
fn local_source_with_body_passes() {
    let local = Source::Local {
        path: "src/lib.rs".into(),
        kind: LocalSourceKind::InProject,
        captured_at: Utc::now(),
        body_path: PathBuf::from("sources/local-01.md"),
        body: "fn main() {}".into(),
        relevance: String::new(),
    };
    let result = check_citations("Code reference [#1].", &[], &[], &[], &[local]);
    assert!(result.passed);
}
