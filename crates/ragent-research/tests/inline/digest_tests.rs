//! Inline tests for `digest.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use crate::contradiction::ContradictionGraph;
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
fn digest_empty_when_fewer_than_two_sources() {
    let sources = vec![web_source(1, "Performance improves.")];
    let digest = build_evidence_digest(&sources, &LocusSet::empty(), &[], None);
    assert!(digest.is_empty());
    assert_eq!(digest.sources_scanned, 1);
}

#[test]
fn digest_detects_shared_dimension_and_marks_contested() {
    let sources = vec![
        web_source(1, "The drug improves performance and is safe."),
        web_source(
            2,
            "Performance worsens under load and adverse effects appear.",
        ),
    ];
    let digest = build_evidence_digest(&sources, &LocusSet::empty(), &[], None);
    let performance = digest
        .claims
        .iter()
        .find(|c| c.text == "Evidence on Performance")
        .expect("performance claim expected");
    assert_eq!(performance.support_count, 2);
    assert!(performance.contested);
    assert!(performance.note.contains("contested"));
}

#[test]
fn digest_uncontested_dimension_note() {
    let sources = vec![
        web_source(1, "Cost decreases with scale."),
        web_source(2, "Cost is lower at volume."),
    ];
    let digest = build_evidence_digest(&sources, &LocusSet::empty(), &[], None);
    let cost = digest
        .claims
        .iter()
        .find(|c| c.text == "Evidence on Cost")
        .expect("cost claim expected");
    assert!(!cost.contested);
    assert!(cost.note.contains("one direction"));
}

#[test]
fn digest_uses_graph_to_mark_contested() {
    let sources = vec![
        web_source(1, "Safety is excellent."),
        web_source(2, "Safety is poor."),
    ];
    let mut graph = ContradictionGraph::empty();
    graph.add_edge(crate::contradiction::ContradictionEdge {
        claim_a: crate::contradiction::ContradictionClaim::from_source(
            "claims safe",
            1,
            &sources[0],
        ),
        claim_b: crate::contradiction::ContradictionClaim::from_source(
            "claims unsafe",
            2,
            &sources[1],
        ),
        dimension: "safety".into(),
        note: "opposing safety claims".into(),
        strength: 80,
    });
    let digest = build_evidence_digest(&sources, &LocusSet::empty(), &[], Some(&graph));
    let safety = digest
        .claims
        .iter()
        .find(|c| c.text == "Evidence on Safety")
        .expect("safety claim expected");
    assert!(safety.contested);
}

#[test]
fn triple_draft_empty_when_digest_empty() {
    let draft = build_triple_draft(&EvidenceDigest::empty(), "topic");
    assert!(draft.is_empty());
}

#[test]
fn triple_draft_produces_three_candidates() {
    let digest = EvidenceDigest {
        claims: vec![
            DigestClaim {
                text: "Evidence on Performance".into(),
                source_indices: vec![1, 2, 3],
                support_count: 3,
                contested: false,
                note: "deep support".into(),
            },
            DigestClaim {
                text: "Evidence on Safety".into(),
                source_indices: vec![1, 2],
                support_count: 2,
                contested: true,
                note: "contested".into(),
            },
        ],
        sources_scanned: 3,
    };
    let draft = build_triple_draft(&digest, "AI coding agents");
    assert_eq!(draft.candidates.len(), 3);
    assert_eq!(draft.candidates[0].label, "A");
    assert_eq!(draft.candidates[1].label, "B");
    assert_eq!(draft.candidates[2].label, "C");
    assert!(draft.candidates[0].note.contains("consensus"));
    assert!(draft.candidates[1].note.contains("skeptical"));
    assert!(draft.candidates[2].note.contains("exploratory"));
}

#[test]
fn triple_draft_candidates_reference_sources() {
    let digest = EvidenceDigest {
        claims: vec![DigestClaim {
            text: "Evidence on Cost".into(),
            source_indices: vec![1, 2],
            support_count: 2,
            contested: false,
            note: "moderate support".into(),
        }],
        sources_scanned: 2,
    };
    let draft = build_triple_draft(&digest, "cloud costs");
    for c in &draft.candidates {
        assert!(!c.body.is_empty());
        assert!(!c.source_indices.is_empty());
    }
}

#[test]
fn digest_sorts_strongest_first() {
    let sources = vec![
        web_source(1, "Performance improves. Safety is uncertain."),
        web_source(2, "Performance is excellent. Safety needs study."),
        web_source(3, "Performance holds."),
        web_source(4, "Performance degrades."),
    ];
    let digest = build_evidence_digest(&sources, &LocusSet::empty(), &[], None);
    assert_eq!(digest.claims[0].text, "Evidence on Performance");
    assert!(
        digest.claims[0].support_count
            >= digest.claims.get(1).map(|c| c.support_count).unwrap_or(0)
    );
}
