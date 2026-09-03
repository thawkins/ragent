#![allow(clippy::assert_is_empty)]
//! Integration tests for cross-locus reconcile and source tensions (T-009).
//!
//! Migrated from `crates/ragent-research/src/reconcile.rs` inline tests
//! per the project convention that all tests live in `tests/`.

use ragent_research::contradiction::{ContradictionClaim, ContradictionEdge, ContradictionGraph};
use ragent_research::locus::{Locus, LocusSet};
use ragent_research::reconcile::{TensionKind, build_cross_locus_reconcile, build_source_tensions};
use ragent_research::source::Source;
use std::path::PathBuf;

fn web_source(index: usize, body: &str) -> Source {
    Source::Web {
        url: format!("https://{index}.example"),
        title: format!("Source {index}"),
        captured_at: chrono::Utc::now(),
        published_at: None,
        body_path: PathBuf::new(),
        body: body.to_string(),
        relevance: "High".to_string(),
        search_tool: "mf_search".to_string(),
        search_engine: "duckduckgo".to_string(),
        content_type: None,
        page_type: None,
        media_type: "page".to_string(),
        language: None,
        oa_recovery: None,
        author: None,
    }
}

fn locus(label: &str, indices: &[usize]) -> Locus {
    Locus {
        keyword: label.to_lowercase(),
        label: label.to_string(),
        source_indices: indices.to_vec(),
        snippets: Vec::new(),
        mentions: indices.len(),
    }
}

fn edge(a: usize, b: usize, dimension: &str) -> ContradictionEdge {
    ContradictionEdge {
        claim_a: ContradictionClaim {
            text: "positive".into(),
            source_index: a,
            source_kind: "web".into(),
            source_path: format!("https://{a}.example"),
        },
        claim_b: ContradictionClaim {
            text: "negative".into(),
            source_index: b,
            source_kind: "web".into(),
            source_path: format!("https://{b}.example"),
        },
        dimension: dimension.into(),
        note: format!("opposing claims on {dimension}"),
        strength: 50,
    }
}

#[test]
fn reconcile_empty_when_fewer_than_two_loci() {
    let loci = LocusSet {
        loci: vec![locus("Performance", &[1, 2])],
    };
    let rec = build_cross_locus_reconcile(&loci, None, 2);
    assert!(rec.is_empty());
}

#[test]
fn reconcile_finds_shared_sources() {
    let loci = LocusSet {
        loci: vec![locus("Performance", &[1, 2, 3]), locus("Cost", &[2, 3, 4])],
    };
    let rec = build_cross_locus_reconcile(&loci, None, 4);
    assert_eq!(rec.pairs.len(), 1);
    assert_eq!(rec.pairs[0].shared_sources, 2);
    let mut got = rec.pairs[0].shared_source_indices.clone();
    got.sort_unstable();
    assert_eq!(got, vec![2, 3]);
    assert_eq!(rec.pairs[0].conflicting_edges, 0);
}

#[test]
fn reconcile_counts_conflicting_edges() {
    let mut graph = ContradictionGraph::empty();
    graph.add_edge(edge(2, 3, "Performance"));
    let loci = LocusSet {
        loci: vec![locus("Performance", &[1, 2, 3]), locus("Cost", &[2, 3, 4])],
    };
    let rec = build_cross_locus_reconcile(&loci, Some(&graph), 4);
    assert_eq!(rec.pairs.len(), 1);
    assert_eq!(rec.pairs[0].conflicting_edges, 1);
}

#[test]
fn reconcile_counts_conflicting_edges_by_shared_source_index() {
    // The old logic compared dimension labels to locus labels, so a
    // contradiction edge with a different dimension name than either locus
    // label was missed. The fix counts edges by shared source index only.
    let mut graph = ContradictionGraph::empty();
    graph.add_edge(edge(2, 5, "Reliability"));
    let loci = LocusSet {
        loci: vec![locus("Performance", &[1, 2, 3]), locus("Cost", &[2, 3, 4])],
    };
    let rec = build_cross_locus_reconcile(&loci, Some(&graph), 5);
    assert_eq!(rec.pairs.len(), 1);
    // Source #2 is shared between the two loci and appears in the edge,
    // so the edge should be counted even though "Reliability" does not
    // match either locus label.
    assert_eq!(rec.pairs[0].conflicting_edges, 1);
}

#[test]
fn reconcile_ignores_edges_with_no_shared_sources() {
    let mut graph = ContradictionGraph::empty();
    // Edge between sources 10 and 11, neither shared by the loci.
    graph.add_edge(edge(10, 11, "Performance"));
    let loci = LocusSet {
        loci: vec![locus("Performance", &[1, 2, 3]), locus("Cost", &[2, 3, 4])],
    };
    let rec = build_cross_locus_reconcile(&loci, Some(&graph), 4);
    assert_eq!(rec.pairs.len(), 1);
    assert_eq!(rec.pairs[0].conflicting_edges, 0);
}

#[test]
fn tensions_include_contradictions() {
    let mut graph = ContradictionGraph::empty();
    graph.add_edge(edge(1, 2, "Performance"));
    let sources = vec![web_source(1, "x"), web_source(2, "y")];
    let tensions = build_source_tensions(&LocusSet::empty(), Some(&graph), &sources);
    assert_eq!(tensions.tensions.len(), 1);
    assert_eq!(tensions.tensions[0].kind, TensionKind::Contradiction);
}

#[test]
fn tensions_flag_shallow_evidence() {
    let loci = LocusSet {
        loci: vec![locus("Performance", &[1])],
    };
    let sources = vec![web_source(1, "x")];
    let tensions = build_source_tensions(&loci, None, &sources);
    assert!(
        tensions
            .tensions
            .iter()
            .any(|t| t.kind == TensionKind::ShallowEvidence)
    );
}

#[test]
fn tensions_flag_isolated_sources() {
    let loci = LocusSet {
        loci: vec![locus("Performance", &[1, 2]), locus("Cost", &[2, 3])],
    };
    let sources = vec![web_source(1, "x"), web_source(2, "x"), web_source(3, "x")];
    let tensions = build_source_tensions(&loci, None, &sources);
    let isolated: Vec<_> = tensions
        .tensions
        .iter()
        .filter(|t| t.kind == TensionKind::IsolatedSource)
        .collect();
    assert_eq!(isolated.len(), 2);
    assert!(isolated.iter().any(|t| t.source_indices == vec![1]));
    assert!(isolated.iter().any(|t| t.source_indices == vec![3]));
}

#[test]
fn tensions_sort_contradictions_first() {
    let mut graph = ContradictionGraph::empty();
    graph.add_edge(edge(1, 2, "Performance"));
    let loci = LocusSet {
        loci: vec![locus("Performance", &[1])],
    };
    let sources = vec![web_source(1, "x"), web_source(2, "x")];
    let tensions = build_source_tensions(&loci, Some(&graph), &sources);
    assert_eq!(tensions.tensions[0].kind, TensionKind::Contradiction);
}
