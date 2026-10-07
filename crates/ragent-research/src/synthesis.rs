//! Synthesis audit and 4-critic review (FR-005, T-012).
//!
//! This module provides deterministic, LLM-free quality assurance for the
//! research synthesis. It runs four internal critic subagents on the final
//! (or fallback) narrative and emits a structured [`SynthesisAudit`] that
//! the session can forward to the UI and persist in `RESEARCH.md`.
//!
//! The four critics are intentionally heuristic and mirror the dimensions
//! used elsewhere in the full-tier pipeline (loci, contradiction graph,
//! evidence digest) so the audit stays internally consistent:
//!
//! 1. **Coverage** - do the findings address every detected research dimension?
//! 2. **Logic** - are contradictions acknowledged rather than ignored?
//! 3. **Evidence** - does every finding cite at least one valid source index?
//! 4. **Readability** - are findings a reasonable length and structure?

use crate::analysis::AnalysisResult;
use crate::contradiction::ContradictionGraph;
use crate::digest::{EvidenceDigest, TripleDraft};
use crate::document::CrossReference;
use crate::locus::LocusSet;
use crate::source::Source;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::polarity::cited_indices;

/// A single critic subagent report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CriticReport {
    /// Critic name (coverage, logic, evidence, readability).
    pub name: String,
    /// Quality score 0-100.
    pub score: u32,
    /// Issues the critic detected.
    pub issues: Vec<String>,
    /// Evidence gaps the critic surfaced.
    pub gaps: Vec<String>,
    /// `true` when the critic considers the synthesis acceptable.
    pub passed: bool,
}

/// Structured result of the deterministic synthesis audit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SynthesisAudit {
    /// Short summary of the synthesis quality.
    pub summary: String,
    /// Findings evaluated by the audit (often a mirror of the final analysis).
    pub findings: Vec<String>,
    /// Top implications evaluated by the audit.
    pub top_implications: Vec<String>,
    /// Cross-references evaluated by the audit.
    pub cross_references: Vec<CrossReference>,
    /// Open questions evaluated by the audit.
    pub open_questions: Vec<String>,
    /// Reports from each of the four critic subagents.
    pub critic_reports: Vec<CriticReport>,
    /// Overall quality score 0-100 (average of the four critic scores).
    pub overall_score: u32,
    /// Human-readable recommendation (proceed / caution / revise).
    pub recommendation: String,
    /// Number of distinct sources actually cited across findings + implications.
    pub sources_used: usize,
}

impl SynthesisAudit {
    /// Create an empty audit.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            summary: String::new(),
            findings: Vec::new(),
            top_implications: Vec::new(),
            cross_references: Vec::new(),
            open_questions: Vec::new(),
            critic_reports: Vec::new(),
            overall_score: 0,
            recommendation: "No audit performed".to_string(),
            sources_used: 0,
        }
    }

    /// Return `true` when no critic reports were produced.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.critic_reports.is_empty()
    }
}

/// Build a deterministic synthesis audit from the gathered corpus and the
/// final analysis result.
///
/// `analysis` is `Some` when an LLM or fallback produced a final narrative; it
/// is `None` only in degenerate cases where the audit is built before
/// synthesis. The audit still runs when `analysis` is `None`, reporting empty
/// findings and zero coverage.
#[must_use]
pub fn build_synthesis_audit(
    sources: &[Source],
    _digest: &EvidenceDigest,
    _triple_draft: &TripleDraft,
    topic: &str,
    loci: &LocusSet,
    contradiction_graph: Option<&ContradictionGraph>,
    analysis: Option<&AnalysisResult>,
) -> SynthesisAudit {
    let empty = AnalysisResult::default();
    let analysis = analysis.unwrap_or(&empty);

    let coverage = coverage_critic(loci, analysis, sources.len());
    let logic = logic_critic(contradiction_graph, analysis);
    let evidence = evidence_critic(sources, analysis);
    let readability = readability_critic(analysis);

    let reports = vec![coverage, logic, evidence, readability];
    let overall_score = if analysis.findings.is_empty() {
        0
    } else {
        average_score(&reports)
    };
    let recommendation = build_recommendation(overall_score, &reports);
    let sources_used = count_distinct_cited_sources(
        &analysis.findings,
        &analysis.top_implications,
        sources.len(),
    );

    SynthesisAudit {
        summary: build_summary(topic, overall_score, &reports, sources_used, sources.len()),
        findings: analysis.findings.clone(),
        top_implications: analysis.top_implications.clone(),
        cross_references: analysis.cross_references.clone(),
        open_questions: analysis.open_questions.clone(),
        critic_reports: reports,
        overall_score,
        recommendation,
        sources_used,
    }
}

// -- Individual critic subagents -------------------------------------------

/// Coverage critic: score based on how many detected loci appear in the final
/// findings / implications. Penalizes missing dimensions and emits them as
/// gaps.
fn coverage_critic(
    loci: &LocusSet,
    analysis: &AnalysisResult,
    _source_count: usize,
) -> CriticReport {
    let mut issues = Vec::new();
    let mut gaps = Vec::new();
    let mut covered = 0usize;

    if loci.loci.is_empty() {
        issues.push(
            "No research dimensions (loci) were detected, so coverage cannot be judged."
                .to_string(),
        );
    }

    let haystack = build_haystack(analysis);
    for locus in &loci.loci {
        let found = haystack.contains(&locus.keyword.to_lowercase())
            || haystack.contains(&locus.label.to_lowercase());
        if found {
            covered += 1;
        } else {
            let msg = format!(
                "Dimension '{}' is not addressed in the synthesis findings or implications",
                locus.label
            );
            issues.push(msg.clone());
            gaps.push(format!("Add evidence or a finding for '{}'", locus.label));
        }
    }

    let score = if loci.loci.is_empty() {
        50
    } else {
        ((covered * 100) / loci.loci.len()).min(100) as u32
    };
    let passed = score >= 60;

    CriticReport {
        name: "coverage".to_string(),
        score,
        issues,
        gaps,
        passed,
    }
}

/// Logic critic: penalizes unresolved contradictions. If the contradiction graph
/// contains edges and the findings do not acknowledge any of the disputed
/// dimensions, the score drops.
fn logic_critic(graph: Option<&ContradictionGraph>, analysis: &AnalysisResult) -> CriticReport {
    let mut issues = Vec::new();
    let mut gaps = Vec::new();

    let graph = match graph {
        Some(g) if !g.edges.is_empty() => g,
        _ => {
            return CriticReport {
                name: "logic".to_string(),
                score: 100,
                issues: vec![
                    "No contradictions detected; no logic conflicts to resolve.".to_string(),
                ],
                gaps: Vec::new(),
                passed: true,
            };
        }
    };

    let haystack = build_haystack(analysis);
    let mut unresolved = HashSet::new();
    for edge in &graph.edges {
        let dimension = edge.dimension.to_lowercase();
        let acknowledged = haystack.contains(&dimension)
            || haystack.contains("contradict")
            || haystack.contains("conflict")
            || haystack.contains("opposing");
        if !acknowledged {
            unresolved.insert(edge.dimension.clone());
        }
    }

    for dimension in &unresolved {
        issues.push(format!(
            "Contradiction on '{}' is not acknowledged in the synthesis",
            dimension
        ));
        gaps.push(format!(
            "Add a qualified finding that notes the conflicting evidence on '{}'",
            dimension
        ));
    }

    let score = if unresolved.is_empty() {
        100
    } else {
        ((100u32).saturating_sub((unresolved.len() * 20) as u32)).max(40)
    };
    let passed = unresolved.is_empty();

    CriticReport {
        name: "logic".to_string(),
        score,
        issues,
        gaps,
        passed,
    }
}

/// Evidence critic: every finding must cite at least one valid source index and
/// all cited indices must be within the available source range. Implications are
/// not required to cite but are checked for validity when they do.
fn evidence_critic(sources: &[Source], analysis: &AnalysisResult) -> CriticReport {
    let mut issues = Vec::new();
    let mut gaps = Vec::new();

    let mut total_findings = 0usize;
    let mut cited_findings = 0usize;

    for (idx, finding) in analysis.findings.iter().enumerate() {
        total_findings += 1;
        let display_idx = idx + 1;
        let indices = cited_indices(finding);
        if indices.is_empty() {
            issues.push(format!("Finding {display_idx} does not cite any source"));
            gaps.push(format!(
                "Add a supporting source citation to finding {display_idx}"
            ));
            continue;
        }
        // A finding counts as cited when at least one index is valid, but
        // every out-of-range citation is still reported: silently accepting a
        // mix of valid and out-of-range indices would hide broken references.
        if indices.iter().any(|n| *n <= sources.len()) {
            cited_findings += 1;
        }
        for n in indices.iter().filter(|n| **n > sources.len()) {
            issues.push(format!(
                "Finding {display_idx} cites out-of-range source #{n} (only {} sources available)",
                sources.len()
            ));
        }
    }

    let score = cited_findings
        .checked_mul(100)
        .and_then(|n| n.checked_div(total_findings))
        .map(|n| n.min(100) as u32)
        .unwrap_or(0);
    let passed = score >= 80 && issues.is_empty();

    CriticReport {
        name: "evidence".to_string(),
        score,
        issues,
        gaps,
        passed,
    }
}

/// Readability critic: findings should not be empty, should contain all required
/// labels, and should not contain extremely long paragraphs.
fn readability_critic(analysis: &AnalysisResult) -> CriticReport {
    let mut issues = Vec::new();
    let mut gaps = Vec::new();

    let required = [
        "**Headline:**",
        "**Observation:**",
        "**Analysis:**",
        "**Cross-reference / Dependencies:**",
        "**Implication:**",
    ];

    if analysis.findings.is_empty() {
        issues.push("No findings were produced".to_string());
        gaps.push("Produce at least one structured finding".to_string());
        return CriticReport {
            name: "readability".to_string(),
            score: 0,
            issues,
            gaps,
            passed: false,
        };
    }

    let mut malformed = 0usize;
    for (idx, finding) in analysis.findings.iter().enumerate() {
        let display_idx = idx + 1;
        let missing: Vec<&str> = required
            .iter()
            .filter(|label| !finding.contains(**label))
            .copied()
            .collect();
        if !missing.is_empty() {
            malformed += 1;
            issues.push(format!(
                "Finding {display_idx} is missing required labels: {}",
                missing.join(", ")
            ));
        }
        let paragraphs: Vec<&str> = finding.split("\n\n").collect();
        for p in &paragraphs {
            if p.len() > 1200 {
                issues.push(format!(
                    "Finding {display_idx} contains a paragraph longer than 1200 characters"
                ));
                break;
            }
        }
    }

    let score = if malformed == 0 {
        100
    } else {
        ((100u32).saturating_sub((malformed * 15) as u32)).max(40)
    };
    let passed = malformed == 0;

    CriticReport {
        name: "readability".to_string(),
        score,
        issues,
        gaps,
        passed,
    }
}

// -- Helpers --------------------------------------------------------------

/// Concatenate all synthesis text into a single lower-case string for keyword
/// searches.
fn build_haystack(analysis: &AnalysisResult) -> String {
    let mut joined = String::new();
    for s in analysis
        .findings
        .iter()
        .chain(&analysis.top_implications)
        .chain(&analysis.open_questions)
    {
        joined.push_str(s);
        joined.push(' ');
    }
    for cr in &analysis.cross_references {
        joined.push_str(&cr.path);
        joined.push(' ');
        joined.push_str(&cr.relevance);
        joined.push(' ');
    }
    joined.to_lowercase()
}

/// Average of the four critic scores, rounded down.
fn average_score(reports: &[CriticReport]) -> u32 {
    if reports.is_empty() {
        return 0;
    }
    let sum: u32 = reports.iter().map(|r| r.score).sum();
    sum / reports.len() as u32
}

/// Build the human-readable recommendation from the overall score and reports.
fn build_recommendation(overall_score: u32, reports: &[CriticReport]) -> String {
    let top_issues: Vec<String> = reports
        .iter()
        .flat_map(|r| r.issues.iter().take(1).cloned())
        .collect();

    if overall_score >= 80 {
        "Proceed - the synthesis passes the deterministic 4-critic audit.".to_string()
    } else if overall_score >= 50 {
        let note = if top_issues.is_empty() {
            "some quality metrics are below the ideal range".to_string()
        } else {
            format!("issues: {}", top_issues.join("; "))
        };
        format!("Proceed with caution - {note}.")
    } else {
        let note = if top_issues.is_empty() {
            "quality is too low".to_string()
        } else {
            format!("critical issues: {}", top_issues.join("; "))
        };
        format!("Requires revision - {note}.")
    }
}

/// Build a short summary line for the audit.
fn build_summary(
    topic: &str,
    score: u32,
    reports: &[CriticReport],
    sources_used: usize,
    sources_available: usize,
) -> String {
    let critic_summary: Vec<String> = reports
        .iter()
        .map(|r| format!("{}={}", r.name, r.score))
        .collect();
    format!(
        "Synthesis audit for '{}' scored {}/100 across critics [{}]; {}/{} sources cited.",
        topic,
        score,
        critic_summary.join(" "),
        sources_used,
        sources_available
    )
}

/// Count distinct source indices cited across findings and implications that
/// are within the available source range.
fn count_distinct_cited_sources(
    findings: &[String],
    implications: &[String],
    source_count: usize,
) -> usize {
    let mut set: HashSet<usize> = HashSet::new();
    let haystack = findings
        .iter()
        .chain(implications.iter())
        .fold(String::new(), |mut acc, s| {
            acc.push_str(s);
            acc.push(' ');
            acc
        });
    for n in cited_indices(&haystack) {
        if n <= source_count {
            set.insert(n);
        }
    }
    set.len()
}

#[cfg(test)]
#[path = "../tests/inline/synthesis_tests.rs"]
mod tests;
