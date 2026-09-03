//! Evidence digest and triple draft (FR-005, T-011).
//!
//! The digest is a deterministic, LLM-free summary of which claims in the
//! gathered corpus are well supported, which are contested, and which are thin.
//! It is intentionally heuristic: it re-uses the same polarity dimensions as the
//! contradiction graph and the same research dimensions as the locus analysis so
//! the report stays internally consistent.
//!
//! The triple draft then produces three candidate argumentative summaries from
//! the digest: a consensus-leaning draft, a skeptical draft, and a
//! exploratory/gap-aware draft. Like the digest, these are deterministic and
//! derived from the source bodies rather than an LLM.

use crate::contradiction::ContradictionGraph;
use crate::locus::{DepthInvestigation, DepthLevel, LocusSet};
use crate::polarity::{has_any_token, source_body_text};
use crate::source::Source;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// A single claim in the evidence digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DigestClaim {
    /// Short claim text (often derived from a dimension keyword).
    pub text: String,
    /// 1-based source indices that support or mention this claim.
    pub source_indices: Vec<usize>,
    /// Number of distinct sources that support the claim.
    pub support_count: usize,
    /// `true` when a contradiction-graph edge covers this claim.
    pub contested: bool,
    /// Human-readable note about the strength of the evidence.
    pub note: String,
}

/// Evidence digest produced from gathered sources, loci, depth investigation,
/// and the contradiction graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EvidenceDigest {
    /// Claims sorted from strongest to weakest support.
    pub claims: Vec<DigestClaim>,
    /// Number of sources that went into the digest.
    pub sources_scanned: usize,
}

impl EvidenceDigest {
    /// Create an empty digest.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            claims: Vec::new(),
            sources_scanned: 0,
        }
    }

    /// Return `true` when no claims were extracted.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.claims.is_empty()
    }
}

/// One candidate draft in the triple draft.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DraftCandidate {
    /// Candidate label: A, B, or C.
    pub label: String,
    /// Short argumentative body (a few sentences).
    pub body: String,
    /// Source indices the draft leans on.
    pub source_indices: Vec<usize>,
    /// Human-readable note about the angle of this draft.
    pub note: String,
}

/// Three deterministic candidate summaries produced from the digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TripleDraft {
    /// Candidate drafts, always A, B, C.
    pub candidates: Vec<DraftCandidate>,
}

impl TripleDraft {
    /// Create an empty triple draft.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            candidates: Vec::new(),
        }
    }

    /// Return `true` when no candidates are present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }
}

/// Research dimensions used by the digest. The values match the locus module
/// labels so the report is internally consistent.
const DIGEST_DIMENSIONS: &[(&str, &str)] = &[
    ("performance", "Performance"),
    ("cost", "Cost"),
    ("safety", "Safety"),
    ("efficacy", "Efficacy"),
    ("adoption", "Adoption"),
    ("mortality", "Mortality"),
    ("mechanism", "Mechanism"),
    ("risk", "Risk"),
    ("benefit", "Benefit"),
    ("side effects", "Side effects"),
    ("quality", "Quality"),
    ("usability", "Usability"),
    ("scalability", "Scalability"),
    ("reliability", "Reliability"),
    ("accessibility", "Accessibility"),
];

/// Polarity tokens used to detect contested claims. These mirror the
/// contradiction-graph module.
const POSITIVE_TOKENS: &[&str] = &[
    "improves",
    "benefits",
    "reduces risk",
    "decreases risk",
    "lowers risk",
    "protects against",
    "safe",
    "safer",
    "well tolerated",
    "few adverse effects",
    "reduces mortality",
    "lowers mortality",
    "decreases mortality",
    "improves survival",
    "reduces cost",
    "lowers cost",
    "decreases cost",
    "cheaper",
    "increases adoption",
    "higher adoption",
    "widespread adoption",
];

const NEGATIVE_TOKENS: &[&str] = &[
    "worsens",
    "increases risk",
    "raises risk",
    "harmful",
    "detrimental",
    "unsafe",
    "adverse effects",
    "side effects",
    "toxic",
    "increases mortality",
    "raises mortality",
    "higher mortality",
    "worse survival",
    "increases cost",
    "raises cost",
    "higher cost",
    "more expensive",
    "low adoption",
    "decreases adoption",
    "limited adoption",
    "declining adoption",
];

/// Build a deterministic evidence digest from the gathered corpus.
///
/// The algorithm:
///
/// 1. For each research dimension, find all sources whose body mentions the
///    dimension keyword.
/// 2. Mark the claim as contested if any source in the set also contains both
///    a positive and a negative polarity token, or if the contradiction graph
///    contains an edge for the dimension keyword.
/// 3. Rank claims by distinct source count (deep > moderate > surface).
#[must_use]
pub fn build_evidence_digest(
    sources: &[Source],
    _loci: &LocusSet,
    depths: &[DepthInvestigation],
    graph: Option<&ContradictionGraph>,
) -> EvidenceDigest {
    if sources.len() < 2 {
        return EvidenceDigest {
            claims: Vec::new(),
            sources_scanned: sources.len(),
        };
    }

    let contested_dimensions: HashSet<&str> = graph
        .map(|g| {
            g.edges
                .iter()
                .map(|e| e.dimension.as_str())
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();

    let mut claims: Vec<DigestClaim> = Vec::new();

    // Pre-lowercase every source body once instead of re-lowercasing per
    // dimension (previously ~15x redundant work for the dimension scan plus
    // the positive/negative token scans below).
    let lowered_bodies: Vec<String> = sources
        .iter()
        .map(|s| source_body_text(s).to_lowercase())
        .collect();
    let positive_sources: Vec<bool> = lowered_bodies
        .iter()
        .map(|b| b.len() >= 20 && has_any_token(b, POSITIVE_TOKENS))
        .collect();
    let negative_sources: Vec<bool> = lowered_bodies
        .iter()
        .map(|b| b.len() >= 20 && has_any_token(b, NEGATIVE_TOKENS))
        .collect();

    for (keyword, label) in DIGEST_DIMENSIONS {
        let mut indices: Vec<usize> = Vec::new();
        for (idx, body) in lowered_bodies.iter().enumerate() {
            if body.len() < 20 {
                continue;
            }
            if body.contains(keyword) {
                indices.push(idx + 1);
            }
        }
        indices.sort_unstable();
        indices.dedup();
        if indices.is_empty() {
            continue;
        }
        let has_positive = positive_sources.iter().any(|b| *b);
        let has_negative = negative_sources.iter().any(|b| *b);
        let contested = contested_dimensions.contains(keyword) || (has_positive && has_negative);
        let depth = depth_for(keyword, depths);
        let note = if contested {
            format!(
                "{} support; evidence is contested ({}).",
                depth.as_str(),
                label
            )
        } else {
            format!(
                "{} support; evidence points in one direction ({}).",
                depth.as_str(),
                label
            )
        };
        claims.push(DigestClaim {
            text: format!("Evidence on {}", label),
            source_indices: indices.clone(),
            support_count: indices.len(),
            contested,
            note,
        });
    }

    // Rank by support count descending, then contested first so the reader sees
    // the strongest and most conflicted claims at the top.
    claims.sort_by(|a, b| {
        b.support_count
            .cmp(&a.support_count)
            .then(b.contested.cmp(&a.contested))
            .then(a.text.cmp(&b.text))
    });

    EvidenceDigest {
        claims,
        sources_scanned: sources.len(),
    }
}

/// Produce three deterministic candidate drafts from the digest.
///
/// - **Candidate A** (consensus) emphasizes the strongest, uncontested claims.
/// - **Candidate B** (skeptical) emphasizes contested claims and asks for more
///   evidence.
/// - **Candidate C** (exploratory) focuses on surface/moderate claims and calls
///   for targeted follow-up research.
#[must_use]
pub fn build_triple_draft(digest: &EvidenceDigest, topic: &str) -> TripleDraft {
    if digest.claims.is_empty() {
        return TripleDraft::empty();
    }

    let (contested, _uncontested): (Vec<&DigestClaim>, Vec<&DigestClaim>) =
        digest.claims.iter().partition(|c| c.contested);
    let strong: Vec<&DigestClaim> = digest
        .claims
        .iter()
        .filter(|c| c.support_count >= 3)
        .collect();
    let weak: Vec<&DigestClaim> = digest
        .claims
        .iter()
        .filter(|c| c.support_count < 3)
        .collect();
    let all: Vec<&DigestClaim> = digest.claims.iter().collect();

    let a_sources: Vec<&DigestClaim> = if strong.is_empty() {
        all.clone()
    } else {
        strong.clone()
    };
    let a = DraftCandidate {
        label: "A".into(),
        body: if strong.is_empty() {
            format!(
                "On the topic of \"{0}\", the gathered corpus is thin; no claim has broad support.",
                topic
            )
        } else {
            format!(
                "On \"{0}\", the strongest consensus view is: {1}. This rests on {2} sources with broad support.",
                topic,
                summary_of_claims(&strong),
                count_distinct_sources(&strong)
            )
        },
        source_indices: collect_indices(&a_sources),
        note: "consensus-leaning draft".into(),
    };

    let b_sources: Vec<&DigestClaim> = if contested.is_empty() {
        all.clone()
    } else {
        contested.clone()
    };
    let b = DraftCandidate {
        label: "B".into(),
        body: if contested.is_empty() {
            format!(
                "On \"{0}\" the evidence is largely one-directional; no major contradictions were detected, so the findings should be treated as provisional rather than proven.",
                topic
            )
        } else {
            format!(
                "A skeptical reading of \"{0}\" highlights contested evidence: {1}. These contradictions mean any conclusion should be qualified until more data resolves the conflict.",
                topic,
                summary_of_claims(&contested)
            )
        },
        source_indices: collect_indices(&b_sources),
        note: "skeptical / adversarial draft".into(),
    };

    let c_sources: Vec<&DigestClaim> = if weak.is_empty() {
        all.clone()
    } else {
        weak.clone()
    };
    let c = DraftCandidate {
        label: "C".into(),
        body: if weak.is_empty() {
            format!(
                "On \"{0}\" every detected claim has deep support; the main remaining task is to confirm the claims in real-world settings rather than gather more sources.",
                topic
            )
        } else {
            format!(
                "An exploratory framing of \"{0}\" focuses on under-supported dimensions: {1}. Targeted follow-up research here would most efficiently reduce uncertainty.",
                topic,
                summary_of_claims(&weak)
            )
        },
        source_indices: collect_indices(&c_sources),
        note: "exploratory / gap-aware draft".into(),
    };

    TripleDraft {
        candidates: vec![a, b, c],
    }
}
/// Look up the depth classification for a dimension keyword.
fn depth_for(keyword: &str, depths: &[DepthInvestigation]) -> DepthLevel {
    depths
        .iter()
        .find(|d| d.keyword == keyword)
        .map(|d| d.depth)
        .unwrap_or(DepthLevel::Surface)
}

/// Count distinct source indices across a slice of claims.
fn count_distinct_sources(claims: &[&DigestClaim]) -> usize {
    let mut set: HashSet<usize> = HashSet::new();
    for c in claims {
        for i in &c.source_indices {
            set.insert(*i);
        }
    }
    set.len()
}

/// Collect all source indices from a slice of claims, deduplicated and sorted.
fn collect_indices(claims: &[&DigestClaim]) -> Vec<usize> {
    let mut set: HashSet<usize> = HashSet::new();
    for c in claims {
        for i in &c.source_indices {
            set.insert(*i);
        }
    }
    let mut v: Vec<usize> = set.into_iter().collect();
    v.sort_unstable();
    v
}

/// Produce a short, comma-separated summary of claim labels for a draft body.
fn summary_of_claims(claims: &[&DigestClaim]) -> String {
    if claims.is_empty() {
        return "no claims".into();
    }
    let labels: Vec<String> = claims.iter().map(|c| c.text.clone()).collect();
    if labels.len() <= 3 {
        labels.join(", ")
    } else {
        format!("{} and {} others", labels[..3].join(", "), labels.len() - 3)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::assert_is_empty)]
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
}
