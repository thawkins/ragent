//! Inline tests for `synthesis.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use crate::analysis::AnalysisResult;
use crate::contradiction::{ContradictionClaim, ContradictionEdge, ContradictionGraph};
use crate::locus::{Locus, LocusSet};
use crate::source::Source;
use std::path::PathBuf;

fn web_source(index: usize, body: &str) -> Source {
    Source::Web {
        url: format!("https://example.com/{index}"),
        title: format!("Source {index}"),
        captured_at: chrono::Utc::now(),
        published_at: None,
        body_path: PathBuf::new(),
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

fn valid_finding(n: usize) -> String {
    format!(
        "**Headline:** Headline {n}\n\n\
         **Observation:** observation [#{n}].\n\n\
         **Analysis:** analysis.\n\n\
         **Cross-reference / Dependencies:** No direct dependencies.\n\n\
         **Implication:** implication."
    )
}

#[test]
fn empty_audit_when_no_analysis() {
    let sources = vec![web_source(1, "body")];
    let audit = build_synthesis_audit(
        &sources,
        &EvidenceDigest::empty(),
        &TripleDraft::empty(),
        "topic",
        &LocusSet::empty(),
        None,
        None,
    );
    // No LLM analysis = empty findings, so evidence/readability critics score
    // low, but the four reports are still present.
    assert_eq!(audit.critic_reports.len(), 4);
    assert_eq!(audit.overall_score, 0);
    assert!(audit.recommendation.contains("Requires revision"));
}

#[test]
fn coverage_critic_scores_full_coverage() {
    let loci = LocusSet {
        loci: vec![Locus {
            keyword: "performance".into(),
            label: "Performance".into(),
            source_indices: vec![1],
            snippets: Vec::new(),
            mentions: 1,
        }],
    };
    let analysis = AnalysisResult {
        findings: vec![valid_finding(1).replace("observation", "Performance observation")],
        ..AnalysisResult::default()
    };
    let report = coverage_critic(&loci, &analysis, 1);
    assert_eq!(report.score, 100);
    assert!(report.passed);
}

#[test]
fn coverage_critic_penalizes_missing_locus() {
    let loci = LocusSet {
        loci: vec![Locus {
            keyword: "cost".into(),
            label: "Cost".into(),
            source_indices: vec![1],
            snippets: Vec::new(),
            mentions: 1,
        }],
    };
    let analysis = AnalysisResult {
        findings: vec![valid_finding(1).replace("observation", "Performance observation")],
        ..AnalysisResult::default()
    };
    let report = coverage_critic(&loci, &analysis, 1);
    assert!(report.score < 100);
    assert!(!report.passed);
    assert!(report.issues.iter().any(|i| i.contains("Cost")));
}

#[test]
fn logic_critic_detects_unacknowledged_contradiction() {
    let mut graph = ContradictionGraph::empty();
    let src1 = web_source(1, "improves performance");
    let src2 = web_source(2, "degrades performance");
    graph.add_edge(ContradictionEdge {
        claim_a: ContradictionClaim::from_source("better", 1, &src1),
        claim_b: ContradictionClaim::from_source("worse", 2, &src2),
        dimension: "performance".into(),
        note: "conflict".into(),
        strength: 80,
    });
    let analysis = AnalysisResult {
        findings: vec![valid_finding(1).replace("observation", "The system works well")],
        ..AnalysisResult::default()
    };
    let report = logic_critic(Some(&graph), &analysis);
    assert!(report.score < 100);
    assert!(report.issues.iter().any(|i| i.contains("performance")));
}

#[test]
fn logic_critic_passes_when_contradiction_acknowledged() {
    let mut graph = ContradictionGraph::empty();
    let src1 = web_source(1, "improves performance");
    let src2 = web_source(2, "degrades performance");
    graph.add_edge(ContradictionEdge {
        claim_a: ContradictionClaim::from_source("better", 1, &src1),
        claim_b: ContradictionClaim::from_source("worse", 2, &src2),
        dimension: "performance".into(),
        note: "conflict".into(),
        strength: 80,
    });
    let analysis = AnalysisResult {
        findings: vec![valid_finding(1).replace(
            "observation",
            "The evidence on performance is contradictory",
        )],
        ..AnalysisResult::default()
    };
    let report = logic_critic(Some(&graph), &analysis);
    assert_eq!(report.score, 100);
    assert!(report.passed);
}

#[test]
fn evidence_critic_flags_missing_citation() {
    let sources = vec![web_source(1, "body")];
    let analysis = AnalysisResult {
        findings: vec![valid_finding(1).replace("[#1]", "")],
        ..AnalysisResult::default()
    };
    let report = evidence_critic(&sources, &analysis);
    assert!(report.score < 100);
    assert!(report.issues.iter().any(|i| i.contains("does not cite")));
}

#[test]
fn evidence_critic_flags_out_of_range_citation() {
    let sources = vec![web_source(1, "body")];
    let analysis = AnalysisResult {
        findings: vec![valid_finding(1).replace("[#1]", "[#5]")],
        ..AnalysisResult::default()
    };
    let report = evidence_critic(&sources, &analysis);
    assert!(report.score < 100);
    assert!(report.issues.iter().any(|i| i.contains("out-of-range")));
}

#[test]
fn readability_critic_flags_missing_labels() {
    let analysis = AnalysisResult {
        findings: vec!["Just a plain paragraph. [#1]".to_string()],
        ..AnalysisResult::default()
    };
    let report = readability_critic(&analysis);
    assert!(report.score < 100);
    assert!(
        report
            .issues
            .iter()
            .any(|i| i.contains("missing required labels"))
    );
}

#[test]
fn readability_critic_passes_valid_finding() {
    let analysis = AnalysisResult {
        findings: vec![valid_finding(1)],
        ..AnalysisResult::default()
    };
    let report = readability_critic(&analysis);
    assert_eq!(report.score, 100);
    assert!(report.passed);
}

#[test]
fn overall_score_is_average() {
    let reports = vec![
        CriticReport {
            name: "a".into(),
            score: 100,
            issues: Vec::new(),
            gaps: Vec::new(),
            passed: true,
        },
        CriticReport {
            name: "b".into(),
            score: 50,
            issues: Vec::new(),
            gaps: Vec::new(),
            passed: false,
        },
    ];
    assert_eq!(average_score(&reports), 75);
}

#[test]
fn build_recommendation_gradations() {
    let reports = vec![CriticReport {
        name: "x".into(),
        score: 80,
        issues: vec!["minor".into()],
        gaps: Vec::new(),
        passed: true,
    }];
    assert!(build_recommendation(85, &reports).contains("Proceed"));
    assert!(build_recommendation(60, &reports).contains("caution"));
    assert!(build_recommendation(30, &reports).contains("Requires revision"));
}

#[test]
fn sources_used_counts_distinct_citations() {
    let findings = vec!["[#1] and [#2]".to_string(), "[#2] and [#3]".to_string()];
    let implications = vec!["[#1]".to_string()];
    assert_eq!(count_distinct_cited_sources(&findings, &implications, 3), 3);
    assert_eq!(count_distinct_cited_sources(&findings, &implications, 2), 2);
}

#[test]
fn evidence_critic_uses_one_based_finding_numbers() {
    let sources = vec![web_source(1, "body")];
    let analysis = AnalysisResult {
        findings: vec![
            valid_finding(1).replace("[#1]", "[#5]"),
            valid_finding(2).replace("[#2]", ""),
        ],
        ..AnalysisResult::default()
    };
    let report = evidence_critic(&sources, &analysis);
    assert!(
        report.issues.iter().any(|i| i.contains("Finding 1")),
        "expected 1-based finding numbers in {:?}",
        report.issues
    );
    assert!(
        !report.issues.iter().any(|i| i.contains("Finding 0")),
        "expected no 0-based finding numbers in {:?}",
        report.issues
    );
}

#[test]
fn readability_critic_uses_one_based_finding_numbers() {
    let analysis = AnalysisResult {
        findings: vec!["Just a plain paragraph. [#1]".to_string()],
        ..AnalysisResult::default()
    };
    let report = readability_critic(&analysis);
    assert!(
        report.issues.iter().any(|i| i.contains("Finding 1")),
        "expected 1-based finding numbers in {:?}",
        report.issues
    );
    assert!(
        !report.issues.iter().any(|i| i.contains("Finding 0")),
        "expected no 0-based finding numbers in {:?}",
        report.issues
    );
}
