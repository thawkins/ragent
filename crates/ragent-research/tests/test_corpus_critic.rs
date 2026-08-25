//! Integration tests for the corpus critic and gap-fill queries (T-010).
//!
//! Migrated from `crates/ragent-research/src/corpus_critic.rs` inline tests
//! per the project convention that all tests live in `tests/`.

use ragent_research::contradiction::{ContradictionClaim, ContradictionEdge, ContradictionGraph};
use ragent_research::corpus_critic::{CorpusCriticReport, build_corpus_critic, derive_gap_queries};
use ragent_research::digest::{DigestClaim, EvidenceDigest};
use ragent_research::locus::{Locus, LocusSet};
use ragent_research::reconcile::SourceTensions;
use ragent_research::source::Source;
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

#[test]
fn empty_sources_produces_degenerate_report() {
    let report = build_corpus_critic(
        &[],
        &LocusSet::empty(),
        &EvidenceDigest::empty(),
        &SourceTensions::empty(),
        None,
        None,
        "topic",
    );
    assert_eq!(report.score, 0);
    assert!(!report.passed);
    assert!(report.issues.iter().any(|i| i.contains("No sources")));
}

#[test]
fn shallow_dimension_penalizes_evidence_score() {
    let sources = vec![
        web_source(1, "Performance improves dramatically."),
        web_source(2, "Performance is mixed."),
    ];
    let loci = LocusSet {
        loci: vec![Locus {
            keyword: "performance".into(),
            label: "Performance".into(),
            source_indices: vec![1, 2],
            snippets: Vec::new(),
            mentions: 2,
        }],
    };
    let digest = EvidenceDigest {
        claims: vec![DigestClaim {
            text: "Evidence on Performance".into(),
            source_indices: vec![1, 2],
            support_count: 2,
            contested: false,
            note: "moderate support".into(),
        }],
        sources_scanned: 2,
    };
    let report = build_corpus_critic(
        &sources,
        &loci,
        &digest,
        &SourceTensions::empty(),
        None,
        None,
        "topic",
    );
    assert!(report.evidence_score < 100);
    assert!(
        report
            .shallow_dimensions
            .contains(&"Performance".to_string()),
        "Performance should be flagged as shallow: {:?}",
        report.shallow_dimensions
    );
}

#[test]
fn contradiction_lowers_tension_score() {
    let sources = vec![
        web_source(1, "The drug improves performance."),
        web_source(2, "Performance degrades under load."),
    ];
    let mut graph = ContradictionGraph::empty();
    graph.add_edge(ContradictionEdge {
        claim_a: ContradictionClaim::from_source("improves performance", 1, &sources[0]),
        claim_b: ContradictionClaim::from_source("degrades performance", 2, &sources[1]),
        dimension: "performance".into(),
        note: "opposing performance claims".into(),
        strength: 80,
    });
    let report = build_corpus_critic(
        &sources,
        &LocusSet::empty(),
        &EvidenceDigest::empty(),
        &SourceTensions::empty(),
        Some(&graph),
        None,
        "topic",
    );
    assert!(report.tension_score < 100);
    assert!(report.issues.iter().any(|i| i.contains("contradiction")));
}

#[test]
fn balance_score_flags_monoculture() {
    let sources = vec![
        web_source(1, "The treatment is safe and effective."),
        web_source(2, "Patients benefit greatly."),
        web_source(3, "Outcomes improve with use."),
    ];
    let report = build_corpus_critic(
        &sources,
        &LocusSet::empty(),
        &EvidenceDigest::empty(),
        &SourceTensions::empty(),
        None,
        None,
        "topic",
    );
    assert!(report.balance_score < 20);
    assert!(
        report.issues.iter().any(|i| i.contains("dominated")),
        "issues: {:?}",
        report.issues
    );
}

#[test]
fn balance_score_rewards_balanced_corpus() {
    let sources = vec![
        web_source(1, "The treatment is safe and effective."),
        web_source(2, "The treatment has serious adverse effects."),
    ];
    let report = build_corpus_critic(
        &sources,
        &LocusSet::empty(),
        &EvidenceDigest::empty(),
        &SourceTensions::empty(),
        None,
        None,
        "topic",
    );
    assert!(
        report.balance_score > 80,
        "balance_score: {}",
        report.balance_score
    );
}

#[test]
fn derive_gap_queries_includes_shallow_and_opposing() {
    let mut report = CorpusCriticReport::empty();
    report.shallow_dimensions = vec!["Cost".into(), "Safety".into()];
    let loci = LocusSet {
        loci: vec![Locus {
            keyword: "cost".into(),
            label: "Cost".into(),
            source_indices: vec![1],
            snippets: Vec::new(),
            mentions: 1,
        }],
    };
    let queries = derive_gap_queries(&report, &loci, "AI coding agents");
    assert!(!queries.is_empty());
    assert!(queries.iter().any(|q| q.contains("cost evidence")));
    assert!(queries.iter().any(|q| q.contains("opposing view")));
}
