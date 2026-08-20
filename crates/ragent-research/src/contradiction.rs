//! Contradiction graph model and deterministic builder (FR-005, T-007).
//!
//! A [`ContradictionGraph`] captures pairs of source claims that are mutually
//! incompatible. The builder is intentionally deterministic and LLM-free: it
//! scans source bodies for a small set of polarity dimensions (e.g. "increases"
//! vs "decreases") and creates edges between sources that make opposite claims
//! about the same dimension keyword. The result is ranked by the number of
//! overlapping dimension tokens.
//!
//! This is a starting model. Future tasks can replace or augment the heuristic
//! with an LLM-based contradiction detector while keeping the same graph
//! structure.

use crate::source::Source;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single claim extracted from a source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContradictionClaim {
    /// Short claim text (often a sentence or phrase from the source body).
    pub text: String,
    /// 1-based index of the source in the research item reference list.
    pub source_index: usize,
    /// Source kind label (`web`, `local`, `spec`, `other`).
    pub source_kind: String,
    /// URL or project-relative path of the source.
    pub source_path: String,
}

impl ContradictionClaim {
    /// Create a claim from a source entry.
    #[must_use]
    pub fn from_source(text: impl Into<String>, index: usize, source: &Source) -> Self {
        let text = text.into();
        match source {
            Source::Web { url, title, .. } => Self {
                text,
                source_index: index,
                source_kind: "web".to_string(),
                source_path: format!("{} — {}", title, url),
            },
            Source::Local { path, .. } => Self {
                text,
                source_index: index,
                source_kind: "local".to_string(),
                source_path: path.clone(),
            },
            Source::Spec { spec_id, .. } => Self {
                text,
                source_index: index,
                source_kind: "spec".to_string(),
                source_path: format!("specs/{spec_id}/SPEC.md"),
            },
            Source::Other { label, .. } => Self {
                text,
                source_index: index,
                source_kind: "other".to_string(),
                source_path: label.clone(),
            },
        }
    }
}

/// One edge in the contradiction graph: two mutually incompatible claims.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContradictionEdge {
    /// The first claim.
    pub claim_a: ContradictionClaim,
    /// The second claim, opposing `claim_a`.
    pub claim_b: ContradictionClaim,
    /// Dimension keyword that triggered the contradiction (e.g. "mortality").
    pub dimension: String,
    /// Human-readable note describing the conflict.
    pub note: String,
    /// Strength score 0–100. Higher means more overlapping evidence.
    pub strength: u8,
}

/// A ranked set of contradictory source pairs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ContradictionGraph {
    /// Edges sorted from strongest to weakest.
    pub edges: Vec<ContradictionEdge>,
}

impl ContradictionGraph {
    /// Create an empty graph.
    #[must_use]
    pub fn empty() -> Self {
        Self { edges: Vec::new() }
    }

    /// Return `true` when the graph contains no edges.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    /// Add an edge and keep the list sorted by descending strength.
    pub fn add_edge(&mut self, edge: ContradictionEdge) {
        self.edges.push(edge);
        self.edges.sort_by(|a, b| {
            b.strength
                .cmp(&a.strength)
                .then(a.dimension.cmp(&b.dimension))
        });
    }

    /// Return all edges that involve a specific source index.
    #[must_use]
    pub fn edges_for_source(&self, index: usize) -> Vec<&ContradictionEdge> {
        self.edges
            .iter()
            .filter(|e| e.claim_a.source_index == index || e.claim_b.source_index == index)
            .collect()
    }
}

/// Polarity dimensions used by the deterministic builder.
///
/// Each entry contains:
/// - a dimension keyword (the subject being measured, e.g. "mortality");
/// - a positive token set (claims that the dimension goes up / is good);
/// - a negative token set (claims that the dimension goes down / is bad).
const POLARITY_DIMENSIONS: &[(&str, &[&str], &[&str])] = &[
    (
        "effect",
        &[
            "improves",
            "benefits",
            "reduces risk",
            "decreases risk",
            "lowers risk",
            "protects against",
        ],
        &[
            "worsens",
            "increases risk",
            "raises risk",
            "harmful",
            "detrimental",
        ],
    ),
    (
        "mortality",
        &[
            "reduces mortality",
            "lowers mortality",
            "decreases mortality",
            "improves survival",
        ],
        &[
            "increases mortality",
            "raises mortality",
            "higher mortality",
            "worse survival",
        ],
    ),
    (
        "performance",
        &[
            "improves performance",
            "faster",
            "better performance",
            "higher performance",
            "outperforms",
        ],
        &[
            "degrades performance",
            "slower",
            "worse performance",
            "lower performance",
        ],
    ),
    (
        "cost",
        &["reduces cost", "lowers cost", "decreases cost", "cheaper"],
        &[
            "increases cost",
            "raises cost",
            "higher cost",
            "more expensive",
        ],
    ),
    (
        "adoption",
        &[
            "increases adoption",
            "higher adoption",
            "widespread adoption",
            "growing adoption",
        ],
        &[
            "low adoption",
            "decreases adoption",
            "limited adoption",
            "declining adoption",
        ],
    ),
    (
        "safety",
        &["safe", "safer", "well tolerated", "few adverse effects"],
        &[
            "unsafe",
            "harmful",
            "adverse effects",
            "side effects",
            "toxic",
        ],
    ),
];

/// Group of sources that take the same polarity on a single dimension.
type PolarityGroup<'a> = Vec<(usize, &'a Source)>;
/// Map from dimension keyword to (positive group, negative group).
type DimensionClaims<'a> = HashMap<&'a str, (PolarityGroup<'a>, PolarityGroup<'a>)>;

/// Build a deterministic contradiction graph from the supplied sources.
///
/// The algorithm is intentionally lightweight and LLM-free:
///
/// 1. For each source body, scan for every polarity dimension.
/// 2. Record the source as making a "positive" or "negative" claim for that
///    dimension when a matching token is found.
/// 3. For every dimension, pair positive and negative sources. Each pair
///    becomes a [`ContradictionEdge`] whose strength is proportional to the
///    number of shared dimension keywords.
/// 4. Edges are deduplicated by sorted source-index pair and ranked by
///    strength.
///
/// Empty or very short source bodies (less than 20 bytes) are ignored to avoid
/// creating edges from placeholder text.
#[must_use]
pub fn build_contradiction_graph(sources: &[Source]) -> ContradictionGraph {
    if sources.len() < 2 {
        return ContradictionGraph::empty();
    }

    let lowercased_bodies: Vec<(usize, String, &Source)> = sources
        .iter()
        .enumerate()
        .map(|(idx, src)| {
            let body = source_body_text(src).to_lowercase();
            (idx, body, src)
        })
        .filter(|(_, body, _)| body.len() >= 20)
        .collect();

    if lowercased_bodies.len() < 2 {
        return ContradictionGraph::empty();
    }

    let mut dimension_claims: DimensionClaims<'_> = HashMap::new();

    for (idx, body, src) in &lowercased_bodies {
        for (dimension, positives, negatives) in POLARITY_DIMENSIONS {
            let has_positive = positives.iter().any(|token| body.contains(*token));
            let has_negative = negatives.iter().any(|token| body.contains(*token));
            if !has_positive && !has_negative {
                continue;
            }
            let entry = dimension_claims
                .entry(dimension)
                .or_insert_with(|| (Vec::new(), Vec::new()));
            // Source indices in the graph are 1-based to match the References
            // Index numbering used throughout RESEARCH.md.
            let one_based = idx + 1;
            if has_positive && !has_negative {
                entry.0.push((one_based, *src));
            } else if has_negative && !has_positive {
                entry.1.push((one_based, *src));
            }
        }
    }

    let mut graph = ContradictionGraph::empty();
    let mut seen_pairs: HashMap<(usize, usize), usize> = HashMap::new();

    for (dimension, (positives, negatives)) in &dimension_claims {
        if positives.is_empty() || negatives.is_empty() {
            continue;
        }
        for (a_idx, a_src) in positives {
            for (b_idx, b_src) in negatives {
                let key = if a_idx <= b_idx {
                    (*a_idx, *b_idx)
                } else {
                    (*b_idx, *a_idx)
                };
                let count = seen_pairs.entry(key).or_insert(0);
                *count += 1;

                // Strength: base 30 + 20 per overlapping dimension, capped at 100.
                let strength = (30u16 + 20u16 * (*count as u16)).min(100) as u8;
                graph.add_edge(ContradictionEdge {
                    claim_a: ContradictionClaim::from_source(
                        format!("Claims {} on {}", positive_label(dimension), dimension),
                        *a_idx,
                        a_src,
                    ),
                    claim_b: ContradictionClaim::from_source(
                        format!("Claims {} on {}", negative_label(dimension), dimension),
                        *b_idx,
                        b_src,
                    ),
                    dimension: (*dimension).to_string(),
                    note: format!(
                        "Source #{a_idx} and source #{b_idx} make opposing claims about {dimension}."
                    ),
                    strength,
                });
            }
        }
    }

    // Deduplicate: keep the strongest edge for each unique pair.
    let mut deduped = ContradictionGraph::empty();
    let mut best_by_pair: HashMap<(usize, usize), ContradictionEdge> = HashMap::new();
    for edge in &graph.edges {
        let key = if edge.claim_a.source_index <= edge.claim_b.source_index {
            (edge.claim_a.source_index, edge.claim_b.source_index)
        } else {
            (edge.claim_b.source_index, edge.claim_a.source_index)
        };
        if let Some(existing) = best_by_pair.get(&key) {
            if edge.strength > existing.strength {
                best_by_pair.insert(key, edge.clone());
            }
        } else {
            best_by_pair.insert(key, edge.clone());
        }
    }
    for edge in best_by_pair.into_values() {
        deduped.add_edge(edge);
    }
    deduped
}

/// Extract searchable body text from a source.
fn source_body_text(source: &Source) -> String {
    match source {
        Source::Web { body, .. } => body.clone(),
        Source::Local { body, .. } => body.clone(),
        Source::Spec { spec_id, .. } => spec_id.clone(),
        Source::Other { body, .. } => body.clone(),
    }
}

/// Human-readable positive direction label for a dimension.
fn positive_label(dimension: &str) -> &'static str {
    match dimension {
        "effect" => "benefit / risk reduction",
        "mortality" => "lower mortality",
        "performance" => "better performance",
        "cost" => "lower cost",
        "adoption" => "higher adoption",
        "safety" => "safer",
        _ => "positive",
    }
}

/// Human-readable negative direction label for a dimension.
fn negative_label(dimension: &str) -> &'static str {
    match dimension {
        "effect" => "harm / risk increase",
        "mortality" => "higher mortality",
        "performance" => "worse performance",
        "cost" => "higher cost",
        "adoption" => "lower adoption",
        "safety" => "less safe",
        _ => "negative",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn web_source(index: usize, url: &str, body: &str) -> Source {
        Source::Web {
            url: url.to_string(),
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

    #[test]
    fn graph_empty_when_fewer_than_two_sources() {
        let sources = vec![web_source(1, "https://a.example", " improves performance")];
        let graph = build_contradiction_graph(&sources);
        assert!(graph.is_empty());
    }

    #[test]
    fn graph_detects_opposing_performance_claims() {
        let sources = vec![
            web_source(
                1,
                "https://a.example",
                "The new system improves performance.",
            ),
            web_source(
                2,
                "https://b.example",
                "The legacy system degrades performance significantly.",
            ),
        ];
        let graph = build_contradiction_graph(&sources);
        assert_eq!(graph.edges.len(), 1);
        let edge = &graph.edges[0];
        assert_eq!(edge.dimension, "performance");
        assert!(edge.strength > 0);
        assert_eq!(edge.claim_a.source_index, 1);
        assert_eq!(edge.claim_b.source_index, 2);
    }

    #[test]
    fn graph_ignores_short_bodies() {
        let sources = vec![
            web_source(1, "https://a.example", "improves"),
            web_source(2, "https://b.example", "worsens"),
        ];
        let graph = build_contradiction_graph(&sources);
        assert!(graph.is_empty());
    }

    #[test]
    fn graph_filters_sources_with_both_polarities() {
        // A source claiming both positive and negative on the same dimension
        // is skipped because it does not clearly take one side.
        let sources = vec![
            web_source(
                1,
                "https://a.example",
                "The drug improves safety in adults but adverse effects in children make it less safe overall.",
            ),
            web_source(
                2,
                "https://b.example",
                "The drug is well tolerated and safe.",
            ),
        ];
        let graph = build_contradiction_graph(&sources);
        // Source 1 has both "safe" and "less safe" so it is not classified.
        assert!(graph.is_empty());
    }

    #[test]
    fn graph_ranks_stronger_edges_first() {
        let sources = vec![
            web_source(
                1,
                "https://a.example",
                "The intervention improves performance and reduces cost.",
            ),
            web_source(
                2,
                "https://b.example",
                "The intervention degrades performance and increases cost.",
            ),
        ];
        let graph = build_contradiction_graph(&sources);
        assert_eq!(graph.edges.len(), 1);
        assert!(graph.edges[0].strength > 30);
    }

    #[test]
    fn graph_can_lookup_edges_by_source_index() {
        let sources = vec![
            web_source(1, "https://a.example", "X improves performance."),
            web_source(2, "https://b.example", "X degrades performance."),
            web_source(3, "https://c.example", "X is neutral."),
        ];
        let graph = build_contradiction_graph(&sources);
        assert_eq!(graph.edges_for_source(1).len(), 1);
        assert_eq!(graph.edges_for_source(2).len(), 1);
        assert!(graph.edges_for_source(3).is_empty());
    }

    #[test]
    fn graph_deduplicates_pairs() {
        let sources = vec![
            web_source(
                1,
                "https://a.example",
                "X improves performance and reduces cost.",
            ),
            web_source(
                2,
                "https://b.example",
                "X degrades performance and increases cost.",
            ),
        ];
        // Two dimensions trigger, but only one unique pair should remain.
        let graph = build_contradiction_graph(&sources);
        let pairs: Vec<_> = graph
            .edges
            .iter()
            .map(|e| (e.claim_a.source_index, e.claim_b.source_index))
            .collect();
        assert_eq!(pairs.len(), 1);
    }
}
