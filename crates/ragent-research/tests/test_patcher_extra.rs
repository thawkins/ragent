//! Additional pure-logic coverage for `patcher.rs`: the readability critic
//! arm, the generic (unknown-critic) arm, the `PatchResult` accessors, and the
//! corpus-critic recommendation -> open-question path.

use ragent_research::analysis::AnalysisResult;
use ragent_research::corpus_critic::CorpusCriticReport;
use ragent_research::patcher::{PatchResult, SurgicalPatch, build_surgical_patches};
use ragent_research::synthesis::{CriticReport, SynthesisAudit};

#[test]
fn patch_result_empty_and_is_empty() {
    let empty = PatchResult::empty();
    assert!(empty.is_empty());
    assert_eq!(empty.score_before, 0);
    assert_eq!(empty.score_after, 0);
    assert_eq!(empty.patches, [] as [ragent_research::SurgicalPatch; 0]);
}

#[test]
fn readability_failure_appends_reformat_question() {
    let mut audit = SynthesisAudit::empty();
    audit.overall_score = 50;
    audit.critic_reports = vec![CriticReport {
        name: "readability".to_string(),
        score: 40,
        issues: vec!["Finding paragraphs are too long".to_string()],
        gaps: vec![],
        passed: false,
    }];
    let result = build_surgical_patches(
        &audit,
        &CorpusCriticReport::empty(),
        "topic",
        &AnalysisResult::default(),
    );
    assert!(
        result
            .patches
            .iter()
            .any(|p| p.operation == "append_open_question" && p.target == "readability"),
        "readability patch should be recorded"
    );
    assert!(
        result
            .patched_analysis
            .open_questions
            .iter()
            .any(|q| q.contains("Headline, Observation")),
        "readability issue should become a reformat question"
    );
    assert!(result.score_after >= result.score_before);
}

#[test]
fn readability_without_issues_is_noop() {
    let mut audit = SynthesisAudit::empty();
    audit.overall_score = 90;
    audit.critic_reports = vec![CriticReport {
        name: "readability".to_string(),
        score: 60,
        issues: vec![],
        gaps: vec![],
        passed: false,
    }];
    let result = build_surgical_patches(
        &audit,
        &CorpusCriticReport::empty(),
        "topic",
        &AnalysisResult::default(),
    );
    assert!(
        result
            .patches
            .iter()
            .any(|p| p.operation == "noop" && p.target == "readability"),
        "readability with no issues should be a noop patch"
    );
}

#[test]
fn unknown_critic_gaps_become_open_questions() {
    let mut audit = SynthesisAudit::empty();
    audit.overall_score = 55;
    audit.critic_reports = vec![CriticReport {
        name: "novelty".to_string(),
        score: 40,
        issues: vec![],
        gaps: vec!["Clarify the novelty claim".to_string()],
        passed: false,
    }];
    let result = build_surgical_patches(
        &audit,
        &CorpusCriticReport::empty(),
        "topic",
        &AnalysisResult::default(),
    );
    assert!(
        result
            .patches
            .iter()
            .any(|p| p.operation == "append_open_question" && p.target == "novelty"),
        "unknown critic gap should become an open question"
    );
    assert!(
        result
            .patched_analysis
            .open_questions
            .iter()
            .any(|q| q.contains("novelty critic gap")),
        "question should name the critic"
    );
}

#[test]
fn corpus_critic_recommendations_become_open_questions() {
    let mut audit = SynthesisAudit::empty();
    audit.overall_score = 80;
    // A passing critic so the audit has reports (not the empty-audit branch).
    audit.critic_reports = vec![CriticReport {
        name: "logic".to_string(),
        score: 100,
        issues: vec![],
        gaps: vec![],
        passed: true,
    }];
    let mut corpus = CorpusCriticReport::empty();
    corpus.recommendations = vec!["Broaden the source base".to_string()];

    let result = build_surgical_patches(&audit, &corpus, "topic", &AnalysisResult::default());
    assert!(
        result
            .patches
            .iter()
            .any(|p| p.target == "corpus_critic" && p.reason.contains("Broaden")),
        "corpus recommendation should be recorded as a patch"
    );
    assert!(
        result
            .patched_analysis
            .open_questions
            .iter()
            .any(|q| q.contains("Broaden the source base")),
        "recommendation should become an open question"
    );
    // Counts are surfaced from the patched analysis.
    assert_eq!(
        result.patched_open_question_count,
        result.patched_analysis.open_questions.len()
    );
}

#[test]
fn corpus_critic_gap_without_audit_extracts_dimension() {
    let mut corpus = CorpusCriticReport::empty();
    corpus.gaps = vec!["Add evidence or a finding for 'Cost'".to_string()];
    let result = build_surgical_patches(
        &SynthesisAudit::empty(),
        &corpus,
        "topic",
        &AnalysisResult::default(),
    );
    assert!(
        result
            .patched_analysis
            .findings
            .iter()
            .any(|f| f.contains("Cost")),
        "dimension in quotes should drive a coverage finding"
    );
    assert!(result.note.contains("corpus-critic"));
}

#[test]
fn surgical_patch_serde_round_trip() {
    let patch = SurgicalPatch {
        operation: "append_finding".to_string(),
        target: "Cost".to_string(),
        reason: "gap".to_string(),
        applied: true,
    };
    let json = serde_json::to_string(&patch).expect("serializable");
    let back: SurgicalPatch = serde_json::from_str(&json).expect("deserializable");
    assert_eq!(back, patch);
}

#[test]
fn applied_count_zero_gives_clean_note() {
    let mut audit = SynthesisAudit::empty();
    audit.overall_score = 100;
    audit.critic_reports = vec![CriticReport {
        name: "coverage".to_string(),
        score: 100,
        issues: vec![],
        gaps: vec![],
        passed: true,
    }];
    let result = build_surgical_patches(
        &audit,
        &CorpusCriticReport::empty(),
        "topic",
        &AnalysisResult::default(),
    );
    assert!(
        result.note.contains("No surgical patches were needed"),
        "note should reflect a clean pass: {}",
        result.note
    );
    assert_eq!(result.score_after, result.score_before);
}
