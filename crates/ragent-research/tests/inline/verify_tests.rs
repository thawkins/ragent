//! Inline tests for `verify.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use crate::source::Source;
use chrono::Utc;
use std::path::PathBuf;

fn web(url: &str, body: &str) -> Source {
    Source::Web {
        published_at: None,
        url: url.to_string(),
        title: "title".to_string(),
        captured_at: Utc::now(),
        body_path: PathBuf::from("sources/web-01.md"),
        body: body.to_string(),
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

fn state_with_sources(sources: Vec<Source>) -> ResearchState {
    let mut state = ResearchState::new("topic");
    for s in sources {
        state.add_source(s);
    }
    state
}

fn analysis_with_finding(finding: &str) -> AnalysisResult {
    AnalysisResult {
        summary: String::new(),
        findings: vec![finding.to_string()],
        top_implications: Vec::new(),
        cross_references: vec![],
        open_questions: vec![],
    }
}

#[tokio::test]
async fn verifier_passes_when_citation_supported() {
    let state = state_with_sources(vec![web("https://x", "async runtime scheduling details")]);
    let analysis = analysis_with_finding("Rust uses async runtimes for scheduling [#1].");
    let result = KeywordVerifier::new().verify(&state, Some(&analysis)).await;
    assert!(result.passed);
    assert_eq!(result.claims_supported, 1);
}

#[tokio::test]
async fn verifier_fails_when_source_body_does_not_support() {
    let state = state_with_sources(vec![web("https://x", "completely unrelated body")]);
    let analysis = analysis_with_finding("Rust uses async runtimes for scheduling [#1].");
    let result = KeywordVerifier::new().verify(&state, Some(&analysis)).await;
    assert!(!result.passed);
    assert!(result.issues.iter().any(|i| i.contains("does not support")));
}

#[tokio::test]
async fn verifier_fails_when_citation_missing() {
    let state = state_with_sources(vec![]);
    let analysis = analysis_with_finding("Rust uses async runtimes for scheduling [#1].");
    let result = KeywordVerifier::new().verify(&state, Some(&analysis)).await;
    assert!(!result.passed);
    assert!(result.issues.iter().any(|i| i.contains("does not exist")));
}
