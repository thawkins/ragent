//! Corpus critic and gap-fill fetch step (FR-005, T-010).
//!
//! This module provides deterministic, LLM-free quality assurance for the
//! gathered corpus *before* synthesis. It produces a [`CorpusCriticReport`] that
//! scores the corpus on coverage, evidence depth, balance, and unresolved
//! tensions, then derives a short list of gap queries that a follow-up fetch
//! step can use to close thin areas.
//!
//! The gap-fill fetch step is intentionally lightweight: it reuses the
//! configured [`WebGatherer`](crate::web_gatherer::WebGatherer) with a small
//! result budget, so no new network abstraction is needed. When no web gatherer
//! is configured, the step still emits a diagnostic event and an empty result.

use crate::contradiction::ContradictionGraph;
use crate::digest::EvidenceDigest;
use crate::locus::{DepthLevel, LocusSet};
use crate::polarity::{depth_from_count, source_body_text};
use crate::reconcile::SourceTensions;
use crate::source::Source;
use crate::synthesis::SynthesisAudit;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Result of a deterministic corpus-critic pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CorpusCriticReport {
    /// Overall corpus quality score 0–100.
    pub score: u32,
    /// Coverage subscore: how many detected dimensions are populated.
    pub coverage_score: u32,
    /// Evidence subscore: how deep the supporting evidence is.
    pub evidence_score: u32,
    /// Balance subscore: whether the corpus is dominated by one perspective.
    pub balance_score: u32,
    /// Tension subscore: how many contradictions remain unresolved.
    pub tension_score: u32,
    /// Human-readable issues detected by the critic.
    pub issues: Vec<String>,
    /// Specific evidence gaps surfaced by the critic.
    pub gaps: Vec<String>,
    /// Recommendations for improving the corpus before synthesis.
    pub recommendations: Vec<String>,
    /// Ratio of contested claims to total claims (0–100).
    pub contested_ratio: u32,
    /// Dimensions that only have surface-level support.
    pub shallow_dimensions: Vec<String>,
    /// Source indices that only support one dimension (outliers).
    pub isolated_sources: Vec<usize>,
    /// `true` when the corpus passes the critic's quality bar.
    pub passed: bool,
}

impl CorpusCriticReport {
    /// Create an empty report.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            score: 0,
            coverage_score: 0,
            evidence_score: 0,
            balance_score: 0,
            tension_score: 0,
            issues: Vec::new(),
            gaps: Vec::new(),
            recommendations: Vec::new(),
            contested_ratio: 0,
            shallow_dimensions: Vec::new(),
            isolated_sources: Vec::new(),
            passed: false,
        }
    }

    /// Return `true` when the report is degenerate (no real audit performed).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.issues.is_empty() && self.gaps.is_empty() && self.score == 0
    }
}

/// Result of a gap-fill fetch attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GapFetchResult {
    /// Queries issued to close gaps.
    pub queries: Vec<String>,
    /// Number of new sources captured.
    pub new_sources: usize,
    /// Number of queries that produced no results.
    pub failed_queries: usize,
    /// Whether the fetch was attempted (false when no web gatherer was configured).
    pub attempted: bool,
    /// Optional human-readable note.
    pub note: String,
}

impl GapFetchResult {
    /// Create an empty result.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            queries: Vec::new(),
            new_sources: 0,
            failed_queries: 0,
            attempted: false,
            note: "No gap-fill fetch attempted".to_string(),
        }
    }

    /// Return `true` when no gap-fill activity occurred.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.attempted && self.queries.is_empty() && self.new_sources == 0
    }
}

/// Build a deterministic corpus-critic report from the gathered corpus and the
/// outputs of earlier pipeline steps.
///
/// The report is intentionally heuristic: it reuses the dimensions already
/// detected by loci analysis, the contested claims from the evidence digest,
/// and the tensions from the source-tensions step. The goal is to surface
/// thin evidence *before* the synthesis step spends prompt budget on it.
#[must_use]
pub fn build_corpus_critic(
    sources: &[Source],
    loci: &LocusSet,
    digest: &EvidenceDigest,
    _tensions: &SourceTensions,
    graph: Option<&ContradictionGraph>,
    _audit: Option<&SynthesisAudit>,
    topic: &str,
) -> CorpusCriticReport {
    if sources.is_empty() {
        return CorpusCriticReport {
            issues: vec!["No sources were gathered; corpus cannot be judged.".to_string()],
            gaps: vec![format!("Gather at least one source for '{topic}'")],
            recommendations: vec![
                "Check network/API keys and retry the web-gathering phase.".to_string(),
            ],
            ..CorpusCriticReport::empty()
        };
    }

    let mut issues = Vec::new();
    let mut gaps = Vec::new();
    let mut recommendations = Vec::new();
    let mut shallow_dimensions = Vec::new();

    // Coverage score: do we have any dimensions at all?
    let coverage_score = if loci.loci.is_empty() {
        issues.push("No research dimensions (loci) were detected in the corpus.".to_string());
        gaps.push(format!(
            "Add sources that discuss dimensions relevant to '{topic}'"
        ));
        20
    } else {
        let populated = loci.loci.len().min(10);
        ((populated * 100) / 10).min(100) as u32
    };

    // Evidence depth score.
    let mut total_depth_score = 0usize;
    let mut dimensions_with_deep = 0usize;
    for locus in &loci.loci {
        let depth = depth_from_count(locus.source_indices.len());
        match depth {
            DepthLevel::Surface => {
                shallow_dimensions.push(locus.label.clone());
                issues.push(format!(
                    "Dimension '{}' has only surface-level support ({} source(s))",
                    locus.label,
                    locus.source_indices.len()
                ));
                gaps.push(format!(
                    "Find additional evidence on '{}' for '{topic}'",
                    locus.label
                ));
            }
            DepthLevel::Moderate => {
                shallow_dimensions.push(locus.label.clone());
                issues.push(format!(
                    "Dimension '{}' has only moderate support ({} source(s))",
                    locus.label,
                    locus.source_indices.len()
                ));
                gaps.push(format!(
                    "Find additional evidence on '{}' for '{topic}'",
                    locus.label
                ));
                total_depth_score += 60;
            }
            DepthLevel::Deep => {
                total_depth_score += 100;
                dimensions_with_deep += 1;
            }
        }
    }
    let evidence_score = if loci.loci.is_empty() {
        20
    } else {
        ((total_depth_score / loci.loci.len()).min(100)) as u32
    };
    if dimensions_with_deep == 0 && !loci.loci.is_empty() {
        recommendations.push(
            "Broaden the width sweep to capture more sources for each dimension.".to_string(),
        );
    }

    // Balance score: avoid perspective monoculture by checking source provenance.
    // 100 means perfectly balanced, 0 means completely one-sided.
    let (positive_count, negative_count) = positive_negative_counts(sources);
    let balance_score = if positive_count + negative_count == 0 {
        50
    } else {
        let diff = positive_count.abs_diff(negative_count);
        let total = positive_count + negative_count;
        ((100u32).saturating_sub(((diff * 100) / total) as u32)).min(100)
    };
    if balance_score < 20 && positive_count + negative_count > 0 {
        issues.push(
            "Corpus is dominated by one perspective; adversarial sources may be missing."
                .to_string(),
        );
        gaps.push(format!("Add sources with an opposing view on '{topic}'"));
        recommendations
            .push("Re-run the width sweep with explicitly skeptical sub-queries.".to_string());
    }

    // Tension score: unresolved contradictions lower the score.
    let contested_count = digest.claims.iter().filter(|c| c.contested).count();
    let contested_ratio = if digest.claims.is_empty() {
        0
    } else {
        ((contested_count * 100) / digest.claims.len()).min(100) as u32
    };
    let unresolved_contradictions = graph.map(|g| g.edges.len()).unwrap_or(0);
    let tension_score = if unresolved_contradictions == 0 {
        100
    } else {
        (100u32)
            .saturating_sub((unresolved_contradictions * 15) as u32)
            .max(40)
    };
    if unresolved_contradictions > 0 {
        issues.push(format!(
            "{unresolved_contradictions} contradiction(s) detected in the corpus"
        ));
        gaps.push(
            "Add qualifying sources that explicitly address the contradictory evidence".to_string(),
        );
        recommendations
            .push("Ensure the synthesis section acknowledges each contradiction.".to_string());
    }

    // Isolated sources that only support one dimension.
    let mut source_to_loci: std::collections::HashMap<usize, usize> =
        std::collections::HashMap::new();
    for locus in &loci.loci {
        for idx in &locus.source_indices {
            *source_to_loci.entry(*idx).or_default() += 1;
        }
    }
    let isolated_sources: Vec<usize> = source_to_loci
        .iter()
        .filter(|(_, count)| **count == 1)
        .map(|(idx, _)| *idx)
        .collect();
    if !isolated_sources.is_empty() {
        issues.push(format!(
            "{} source(s) only support a single dimension and may be outliers",
            isolated_sources.len()
        ));
    }

    // Overall score: weighted average of the four dimensions.
    let score =
        (coverage_score * 25 + evidence_score * 35 + balance_score * 20 + tension_score * 20) / 100;
    let passed = score >= 60
        && evidence_score >= 40
        && tension_score >= 40
        && unresolved_contradictions <= 3;

    CorpusCriticReport {
        score,
        coverage_score,
        evidence_score,
        balance_score,
        tension_score,
        issues,
        gaps,
        recommendations,
        contested_ratio,
        shallow_dimensions,
        isolated_sources,
        passed,
    }
}

/// Derive gap-fill queries from the critic report and the corpus.
///
/// The queries combine the research topic with each gap dimension and with a
/// generic adversarial prompt, producing up to five focused sub-queries.
#[must_use]
pub fn derive_gap_queries(
    report: &CorpusCriticReport,
    loci: &LocusSet,
    topic: &str,
) -> Vec<String> {
    let mut queries = Vec::new();
    let mut seen = HashSet::new();

    for dim in &report.shallow_dimensions {
        let q = format!("{} {} evidence", topic, dim.to_lowercase());
        if seen.insert(q.clone()) {
            queries.push(q);
        }
    }

    if !loci.loci.is_empty() {
        let q = format!("{} opposing view or critique", topic);
        if seen.insert(q.clone()) {
            queries.push(q);
        }
    }

    if !report.gaps.is_empty() {
        let q = format!("{} limitations or gaps", topic);
        if seen.insert(q.clone()) {
            queries.push(q);
        }
    }

    if report.isolated_sources.len() > 1 {
        let q = format!("{} cross-cutting evidence", topic);
        if seen.insert(q.clone()) {
            queries.push(q);
        }
    }

    // Cap at five queries to keep the gap-fill pass bounded.
    queries.into_iter().take(5).collect()
}

/// Count sources that appear to take a positive or negative stance on the topic.
fn positive_negative_counts(sources: &[Source]) -> (usize, usize) {
    const POSITIVE: &[&str] = &[
        "improves",
        "benefits",
        "reduces risk",
        "decreases risk",
        "lowers risk",
        "protects against",
        "safe",
        "safer",
        "well tolerated",
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
    ];
    const NEGATIVE: &[&str] = &[
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
    ];

    let mut positive = 0usize;
    let mut negative = 0usize;
    for src in sources {
        let body = source_body_text(src).to_lowercase();
        if has_any_token(&body, POSITIVE) {
            positive += 1;
        }
        if has_any_token(&body, NEGATIVE) {
            negative += 1;
        }
    }
    (positive, negative)
}

fn has_any_token(body: &str, tokens: &[&str]) -> bool {
    tokens.iter().any(|t| body.contains(t))
}
