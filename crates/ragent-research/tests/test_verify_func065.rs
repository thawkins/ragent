//! FUNC-065 regression tests for the keyword verifier semantics.

use ragent_research::analysis::AnalysisResult;
use ragent_research::state::ResearchState;
use ragent_research::verify::{KeywordVerifier, Verifier};

fn state() -> ResearchState {
    ResearchState::new("topic")
}

fn analysis_with_findings(findings: Vec<&str>) -> AnalysisResult {
    AnalysisResult {
        summary: String::new(),
        findings: findings.into_iter().map(str::to_string).collect(),
        top_implications: Vec::new(),
        cross_references: vec![],
        open_questions: vec![],
    }
}

#[tokio::test]
async fn func065_empty_analysis_is_not_a_pass() {
    let result = KeywordVerifier::new().verify(&state(), None).await;
    assert!(
        !result.passed,
        "empty analysis must not be reported as passed"
    );
    assert_eq!(result.claims_checked, 0);
}

#[tokio::test]
async fn func065_empty_findings_is_not_a_pass() {
    let empty = analysis_with_findings(vec![]);
    let result = KeywordVerifier::new().verify(&state(), Some(&empty)).await;
    assert!(!result.passed);
}

#[tokio::test]
async fn func065_uncited_finding_fails_verification() {
    let analysis = analysis_with_findings(vec!["This claim has no citation at all."]);
    let result = KeywordVerifier::new()
        .verify(&state(), Some(&analysis))
        .await;
    assert!(
        !result.passed,
        "an uncited finding must fail verification, issues: {:?}",
        result.issues
    );
    assert!(result.issues.iter().any(|i| i.contains("no citations")));
    assert_eq!(result.claims_checked, 1);
    assert_eq!(result.claims_supported, 0);
}
