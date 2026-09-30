//! Inline tests for `patcher.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use crate::analysis::AnalysisResult;
use crate::corpus_critic::CorpusCriticReport;
use crate::synthesis::{CriticReport, SynthesisAudit};

fn failing_coverage_report() -> CriticReport {
    CriticReport {
        name: "coverage".to_string(),
        score: 40,
        issues: vec![
            "Dimension 'Cost' is not addressed in the synthesis findings or implications"
                .to_string(),
        ],
        gaps: vec!["Add evidence or a finding for 'Cost'".to_string()],
        passed: false,
    }
}

fn failing_evidence_report() -> CriticReport {
    CriticReport {
        name: "evidence".to_string(),
        score: 50,
        issues: vec!["Finding 0 does not cite any source".to_string()],
        gaps: vec!["Add a supporting source citation to finding 0".to_string()],
        passed: false,
    }
}

fn passing_logic_report() -> CriticReport {
    CriticReport {
        name: "logic".to_string(),
        score: 100,
        issues: vec!["No contradictions detected; no logic conflicts to resolve.".to_string()],
        gaps: vec![],
        passed: true,
    }
}

fn sample_audit() -> SynthesisAudit {
    let mut audit = SynthesisAudit::empty();
    audit.overall_score = 60;
    audit.critic_reports = vec![
        failing_coverage_report(),
        failing_evidence_report(),
        passing_logic_report(),
    ];
    audit
}

#[test]
fn empty_audit_applies_corpus_critic_patches() {
    let original = AnalysisResult::default();
    let audit = SynthesisAudit::empty();
    let mut corpus = CorpusCriticReport::empty();
    corpus.gaps = vec!["Find additional evidence on 'Safety' for 'topic'".to_string()];
    let result = build_surgical_patches(&audit, &corpus, "topic", &original);
    assert!(
        result
            .patches
            .iter()
            .any(|p| p.applied && p.operation == "corpus_critic_finding"),
        "expected a corpus-critic finding patch"
    );
    assert!(
        result
            .patched_analysis
            .findings
            .iter()
            .any(|f| f.contains("Safety")),
        "patched analysis should contain a Safety coverage finding"
    );
}

#[test]
fn coverage_failure_adds_finding_and_question() {
    let original = AnalysisResult::default();
    let audit = sample_audit();
    let corpus = CorpusCriticReport::empty();
    let result = build_surgical_patches(&audit, &corpus, "topic", &original);
    assert!(
        result
            .patches
            .iter()
            .any(|p| p.operation == "append_finding" && p.target == "Cost"),
        "coverage patch should append a Cost finding"
    );
    assert!(
        result
            .patched_analysis
            .open_questions
            .iter()
            .any(|q| q.contains("Cost")),
        "coverage patch should add a Cost open question"
    );
}

#[test]
fn evidence_failure_adds_citation_reminder() {
    let original = AnalysisResult {
        findings: vec!["Some claim without citation.".to_string()],
        ..AnalysisResult::default()
    };
    let audit = sample_audit();
    let corpus = CorpusCriticReport::empty();
    let result = build_surgical_patches(&audit, &corpus, "topic", &original);
    assert!(
        result
            .patched_analysis
            .findings
            .iter()
            .any(|f| f.contains("Source citation review required")),
        "evidence patch should append a citation reminder finding"
    );
    assert!(
        result
            .patched_analysis
            .open_questions
            .iter()
            .any(|q| q.contains("sources")),
        "evidence patch should add a source open question"
    );
}

#[test]
fn passed_critic_emits_noop_patch() {
    let original = AnalysisResult::default();
    let mut audit = SynthesisAudit::empty();
    audit.overall_score = 100;
    audit.critic_reports = vec![passing_logic_report()];
    let corpus = CorpusCriticReport::empty();
    let result = build_surgical_patches(&audit, &corpus, "topic", &original);
    assert!(
        result
            .patches
            .iter()
            .any(|p| p.operation == "noop" && p.target == "logic"),
        "passing logic critic should produce a noop patch"
    );
}

#[test]
fn score_after_is_not_decreased() {
    let original = AnalysisResult::default();
    let audit = sample_audit();
    let corpus = CorpusCriticReport::empty();
    let result = build_surgical_patches(&audit, &corpus, "topic", &original);
    assert!(
        result.score_after >= result.score_before,
        "post-patch score should not drop"
    );
    assert!(
        result.score_after <= 100,
        "post-patch score should be capped at 100"
    );
}

#[test]
fn logic_failure_qualifies_summary() {
    let original = AnalysisResult {
        summary: "The treatment is effective.".to_string(),
        ..AnalysisResult::default()
    };
    let mut audit = SynthesisAudit::empty();
    audit.overall_score = 55;
    audit.critic_reports = vec![CriticReport {
        name: "logic".to_string(),
        score: 50,
        issues: vec!["Contradiction on 'safety' is not acknowledged in the synthesis".to_string()],
        gaps: vec![
            "Add a qualified finding that notes the conflicting evidence on 'safety'".to_string(),
        ],
        passed: false,
    }];
    let corpus = CorpusCriticReport::empty();
    let result = build_surgical_patches(&audit, &corpus, "topic", &original);
    assert!(
        result
            .patched_analysis
            .summary
            .contains("conflicting evidence"),
        "summary should be qualified with contradiction note"
    );
    assert!(
        result
            .patched_analysis
            .open_questions
            .iter()
            .any(|q| q.contains("safety")),
        "logic patch should add a safety contradiction question"
    );
}
