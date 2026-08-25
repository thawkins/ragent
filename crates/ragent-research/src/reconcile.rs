//! Cross-locus reconcile and source tensions (FR-005, T-009).
//!
//! This module is intentionally deterministic and LLM-free. It takes the
//! [`LocusSet`](crate::locus::LocusSet) and
//! [`ContradictionGraph`](crate::contradiction::ContradictionGraph) produced by
//! earlier full-tier steps and derives two higher-level views of the corpus:
//!
//! 1. **Cross-locus reconcile** — which sources support *multiple* research
//!    dimensions at the same time, so the reader can see whether the same
//!    evidence underlies different claims.
//! 2. **Source tensions** — a combined view of direct contradictions, shallow
//!    dimensions, and isolated evidence, so gaps and conflicts are visible
//!    before synthesis.
//!
//! Like the contradiction graph and locus analysis, these are starting
//! heuristics. Later tasks can replace the scoring while keeping the same output
//! structures.

use crate::contradiction::ContradictionGraph;
use crate::locus::{DepthLevel, LocusSet};
use crate::polarity::depth_from_count;
use crate::source::Source;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// One reconciled pair of loci.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ReconcilePair {
    /// First locus label.
    pub locus_a: String,
    /// Second locus label.
    pub locus_b: String,
    /// 1-based source indices that mention *both* loci.
    pub shared_source_indices: Vec<usize>,
    /// Number of distinct sources that mention both loci.
    pub shared_sources: usize,
    /// Number of contradiction-graph edges that involve any shared source on
    /// either locus.
    pub conflicting_edges: usize,
    /// Human-readable note about the relationship.
    pub note: String,
}

/// Cross-locus reconciliation result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CrossLocusReconcile {
    /// Reconciled pairs, sorted from most shared evidence to least.
    pub pairs: Vec<ReconcilePair>,
    /// Number of sources scanned.
    pub sources_scanned: usize,
}

impl CrossLocusReconcile {
    /// Create an empty reconciliation result.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            pairs: Vec::new(),
            sources_scanned: 0,
        }
    }

    /// Return `true` when no pairs were found.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }
}

/// Type of source tension detected in the corpus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TensionKind {
    /// Two sources make opposing claims about the same dimension.
    Contradiction,
    /// A research dimension is only thinly supported.
    ShallowEvidence,
    /// A source supports only one locus and may be an outlier.
    #[default]
    IsolatedSource,
}

impl TensionKind {
    /// Human-readable label for report rendering.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Contradiction => "contradiction",
            Self::ShallowEvidence => "shallow evidence",
            Self::IsolatedSource => "isolated source",
        }
    }
}

/// One tension record in the source-tensions list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TensionRecord {
    /// What kind of tension this is.
    pub kind: TensionKind,
    /// Locus or dimension label involved.
    pub label: String,
    /// 1-based source indices involved.
    pub source_indices: Vec<usize>,
    /// Human-readable explanation.
    pub note: String,
}

/// Source-tensions summary for the corpus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SourceTensions {
    /// Tension records, sorted from most severe to least.
    pub tensions: Vec<TensionRecord>,
    /// Number of sources scanned.
    pub sources_scanned: usize,
}

impl SourceTensions {
    /// Create an empty tensions result.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            tensions: Vec::new(),
            sources_scanned: 0,
        }
    }

    /// Return `true` when no tensions were detected.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tensions.is_empty()
    }
}

/// Build the cross-locus reconciliation from a [`LocusSet`] and optional
/// [`ContradictionGraph`].
///
/// For every distinct pair of loci, the function computes the intersection of
/// their supporting source indices. Pairs with at least two shared sources are
/// emitted so the report can show when the same evidence underpins multiple
/// dimensions. If a contradiction edge involves any of those shared sources, it
/// counts as a conflicting edge for the pair.
#[must_use]
pub fn build_cross_locus_reconcile(
    loci: &LocusSet,
    graph: Option<&ContradictionGraph>,
    sources_scanned: usize,
) -> CrossLocusReconcile {
    if loci.loci.len() < 2 {
        return CrossLocusReconcile {
            pairs: Vec::new(),
            sources_scanned,
        };
    }

    let mut pairs = Vec::new();
    for i in 0..loci.loci.len() - 1 {
        for j in i + 1..loci.loci.len() {
            let a = &loci.loci[i];
            let b = &loci.loci[j];
            let a_set: HashSet<usize> = a.source_indices.iter().copied().collect();
            let b_set: HashSet<usize> = b.source_indices.iter().copied().collect();
            let shared: Vec<usize> = a_set
                .intersection(&b_set)
                .copied()
                .collect::<Vec<_>>()
                .into_iter()
                .collect();
            if shared.len() < 2 {
                continue;
            }
            let shared_set: HashSet<usize> = shared.iter().copied().collect();
            let conflicting_edges = graph
                .map(|g| {
                    g.edges
                        .iter()
                        .filter(|e| {
                            shared_set.contains(&e.claim_a.source_index)
                                || shared_set.contains(&e.claim_b.source_index)
                        })
                        .count()
                })
                .unwrap_or(0);
            let shared_display = shared
                .iter()
                .map(|i| format!("#{i}"))
                .collect::<Vec<_>>()
                .join(", ");
            let note = if conflicting_edges > 0 {
                format!(
                    "{shared_display} sources support both dimensions; {conflicting_edges} related contradiction edge(s) detected."
                )
            } else {
                format!(
                    "{shared_display} sources support both dimensions with no related contradictions."
                )
            };
            pairs.push(ReconcilePair {
                locus_a: a.label.clone(),
                locus_b: b.label.clone(),
                shared_source_indices: shared.clone(),
                shared_sources: shared.len(),
                conflicting_edges,
                note,
            });
        }
    }

    pairs.sort_by(|a, b| {
        b.shared_sources
            .cmp(&a.shared_sources)
            .then(a.locus_a.cmp(&b.locus_a))
            .then(a.locus_b.cmp(&b.locus_b))
    });

    CrossLocusReconcile {
        pairs,
        sources_scanned,
    }
}

/// Build the source-tensions list from a [`LocusSet`], optional
/// [`ContradictionGraph`], and the original source list.
///
/// Tensions are derived from three heuristics:
///
/// 1. Every contradiction-graph edge becomes a `Contradiction` tension.
/// 2. Every locus whose depth is `Surface` or `Moderate` becomes a
///    `ShallowEvidence` tension.
/// 3. Every source that supports only one locus becomes an `IsolatedSource`
///    tension.
#[must_use]
pub fn build_source_tensions(
    loci: &LocusSet,
    graph: Option<&ContradictionGraph>,
    sources: &[Source],
) -> SourceTensions {
    let mut tensions = Vec::new();

    // 1. Contradictions from the graph.
    if let Some(graph) = graph {
        for edge in &graph.edges {
            tensions.push(TensionRecord {
                kind: TensionKind::Contradiction,
                label: edge.dimension.clone(),
                source_indices: vec![edge.claim_a.source_index, edge.claim_b.source_index],
                note: edge.note.clone(),
            });
        }
    }

    // 2. Shallow evidence: loci with few supporting sources.
    for locus in &loci.loci {
        let depth = depth_from_count(locus.source_indices.len());
        if depth != DepthLevel::Deep {
            tensions.push(TensionRecord {
                kind: TensionKind::ShallowEvidence,
                label: locus.label.clone(),
                source_indices: locus.source_indices.clone(),
                note: format!(
                    "{} evidence: only {} source(s) mention this dimension.",
                    depth.as_str(),
                    locus.source_indices.len()
                ),
            });
        }
    }

    // 3. Isolated sources: sources that support exactly one locus.
    let mut source_to_loci: HashMap<usize, Vec<String>> = HashMap::new();
    for locus in &loci.loci {
        for idx in &locus.source_indices {
            source_to_loci
                .entry(*idx)
                .or_default()
                .push(locus.label.clone());
        }
    }
    for (idx, labels) in &source_to_loci {
        if labels.len() == 1 {
            tensions.push(TensionRecord {
                kind: TensionKind::IsolatedSource,
                label: labels[0].clone(),
                source_indices: vec![*idx],
                note: format!(
                    "Source #{idx} only supports one dimension and may represent an outlier or niche view."
                ),
            });
        }
    }

    // Severity ordering: contradictions first, then shallow evidence, then
    // isolated sources. Within a kind, keep stable source-index order.
    tensions.sort_by(|a, b| {
        fn kind_rank(k: TensionKind) -> u8 {
            match k {
                TensionKind::Contradiction => 0,
                TensionKind::ShallowEvidence => 1,
                TensionKind::IsolatedSource => 2,
            }
        }
        kind_rank(a.kind)
            .cmp(&kind_rank(b.kind))
            .then(a.label.cmp(&b.label))
            .then(a.source_indices.cmp(&b.source_indices))
    });

    SourceTensions {
        tensions,
        sources_scanned: sources.len(),
    }
}
