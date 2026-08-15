//! Surgical patcher for draft revisions (FR-005, T-013).
//!
//! After the 4-critic audit identifies coverage, evidence, logic, and
//! readability problems, this module applies deterministic, minimal edits
//! to the synthesized [`AnalysisResult`] so the final `RESEARCH.md` reflects the
//! gaps explicitly rather than silently shipping a weak draft.
//!
//! Patches are intentionally surgical: they append missing findings, add
//! source-citation reminders, qualify the summary when contradictions are
//! unacknowledged, and surface readability issues as open questions. They do
//! not rewrite LLM prose wholesale; that remains the responsibility of the
//! synthesis step.

use crate::analysis::AnalysisResult;
use crate::corpus_critic::CorpusCriticReport;
use crate::synthesis::{CriticReport, SynthesisAudit};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// One deterministic patch operation considered by the patcher.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SurgicalPatch {
    /// Operation name, e.g. `append_finding`, `append_open_question`,
    /// `qualify_summary`.
    pub operation: String,
    /// Target field or dimension the patch addressed.
    pub target: String,
    /// Human-readable reason the patch was applied or skipped.
    pub reason: String,
    /// `true` when the patch changed the draft.
    pub applied: bool,
}

/// Result of running the surgical patcher.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PatchResult {
    /// Patches that were considered / applied.
    pub patches: Vec<SurgicalPatch>,
    /// The draft after patches have been applied. Not serialized into event
    /// JSON because [`AnalysisResult`] is not JSON-serializable; the patched
    /// counts below are surfaced instead.
    #[serde(skip)]
    pub patched_analysis: AnalysisResult,
    /// Overall audit score before patching.
    pub score_before: u32,
    /// Estimated overall score after patching.
    pub score_after: u32,
    /// Short note summarizing what changed.
    pub note: String,
    /// Number of findings in the patched analysis.
    pub patched_finding_count: usize,
    /// Number of implications in the patched analysis.
    pub patched_implication_count: usize,
    /// Number of open questions in the patched analysis.
    pub patched_open_question_count: usize,
}

impl PatchResult {
    /// Create an empty result.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            patches: Vec::new(),
            patched_analysis: AnalysisResult::default(),
            score_before: 0,
            score_after: 0,
            note: "No surgical patches applied".to_string(),
            patched_finding_count: 0,
            patched_implication_count: 0,
            patched_open_question_count: 0,
        }
    }

    /// Return `true` when no patches were considered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.patches.is_empty()
    }
}

/// Build a deterministic set of surgical patches from the synthesis audit and
/// corpus-critic report, returning both the patch list and the patched analysis.
#[must_use]
pub fn build_surgical_patches(
    audit: &SynthesisAudit,
    corpus_critic: &CorpusCriticReport,
    topic: &str,
    original: &AnalysisResult,
) -> PatchResult {
    let mut patched = original.clone();
    let mut patches: Vec<SurgicalPatch> = Vec::new();
    let mut seen_questions: HashSet<String> = HashSet::new();

    // If the audit has no critic reports, fall back to the corpus-critic gaps.
    if audit.critic_reports.is_empty() {
        for gap in &corpus_critic.gaps {
            apply_generic_gap_patch(
                &mut patched,
                topic,
                gap,
                &mut patches,
                &mut seen_questions,
                "corpus_critic",
                "corpus_critic",
            );
        }
        let score_after = (audit.overall_score + 5).min(100);
        return PatchResult {
            patches,
            patched_analysis: patched,
            score_before: audit.overall_score,
            score_after,
            note: "Applied corpus-critic driven patches (no synthesis audit available)".to_string(),
            patched_finding_count: 0,
            patched_implication_count: 0,
            patched_open_question_count: 0,
        };
    }

    for report in &audit.critic_reports {
        if report.passed {
            patches.push(SurgicalPatch {
                operation: "noop".to_string(),
                target: report.name.clone(),
                reason: format!("{} critic passed (score {})", report.name, report.score),
                applied: false,
            });
            continue;
        }
        match report.name.as_str() {
            "coverage" => apply_coverage_patches(
                &mut patched,
                topic,
                report,
                &mut patches,
                &mut seen_questions,
            ),
            "evidence" => apply_evidence_patches(
                &mut patched,
                topic,
                report,
                &mut patches,
                &mut seen_questions,
            ),
            "logic" => apply_logic_patches(
                &mut patched,
                topic,
                report,
                &mut patches,
                &mut seen_questions,
            ),
            "readability" => {
                apply_readability_patches(&mut patched, report, &mut patches, &mut seen_questions)
            }
            _ => {
                apply_generic_critic_patch(&mut patched, report, &mut patches, &mut seen_questions)
            }
        }
    }

    // Corpus-critic recommendations that were not already addressed by the
    // audit become open questions.
    for rec in &corpus_critic.recommendations {
        let q = format!("How should we address this recommendation: {rec}?");
        if seen_questions.insert(q.clone()) {
            patched.open_questions.push(q.clone());
            patches.push(SurgicalPatch {
                operation: "append_open_question".to_string(),
                target: "corpus_critic".to_string(),
                reason: rec.clone(),
                applied: true,
            });
        }
    }

    let applied_count = patches.iter().filter(|p| p.applied).count();
    let score_after = estimate_score_after(audit.overall_score, applied_count);
    let note = if applied_count == 0 {
        "No surgical patches were needed; the draft passed all critics.".to_string()
    } else {
        format!(
            "Applied {applied_count} surgical patch(es) to address failed critics; score estimate raised from {} to {}.",
            audit.overall_score, score_after
        )
    };

    PatchResult {
        patches,
        patched_analysis: patched.clone(),
        score_before: audit.overall_score,
        score_after,
        note,
        patched_finding_count: patched.findings.len(),
        patched_implication_count: patched.top_implications.len(),
        patched_open_question_count: patched.open_questions.len(),
    }
}

/// Apply patches for a failed coverage critic.
fn apply_coverage_patches(
    patched: &mut AnalysisResult,
    topic: &str,
    report: &CriticReport,
    patches: &mut Vec<SurgicalPatch>,
    seen: &mut HashSet<String>,
) {
    for issue in &report.issues {
        if let Some(dim) = extract_dimension(issue) {
            patched.findings.push(coverage_finding(topic, &dim));
            patches.push(SurgicalPatch {
                operation: "append_finding".to_string(),
                target: dim.clone(),
                reason: issue.clone(),
                applied: true,
            });
            let q = format!("What evidence exists for {dim} in {topic}?");
            if seen.insert(q.clone()) {
                patched.open_questions.push(q);
                patches.push(SurgicalPatch {
                    operation: "append_open_question".to_string(),
                    target: dim.clone(),
                    reason: issue.clone(),
                    applied: true,
                });
            }
        } else if seen.insert(issue.clone()) {
            patched.open_questions.push(issue.clone());
            patches.push(SurgicalPatch {
                operation: "append_open_question".to_string(),
                target: "coverage".to_string(),
                reason: issue.clone(),
                applied: true,
            });
        }
    }
    for gap in &report.gaps {
        let q = if let Some(dim) = extract_dimension(gap) {
            format!("Add a finding or source for '{dim}' to cover the gap: {gap}")
        } else {
            gap.clone()
        };
        if seen.insert(q.clone()) {
            patched.open_questions.push(q.clone());
            patches.push(SurgicalPatch {
                operation: "append_open_question".to_string(),
                target: "coverage".to_string(),
                reason: q,
                applied: true,
            });
        }
    }
}

/// Build a synthetic coverage finding for a missing dimension.
fn coverage_finding(topic: &str, dim: &str) -> String {
    format!(
        "**Headline:** Coverage gap on {dim}\n\n\
         **Observation:** The synthesis did not address the research dimension '{dim}' for '{topic}'.\n\n\
         **Analysis:** Without evidence on {dim}, the report may be incomplete or one-sided. Additional sources should be gathered and evaluated before drawing firm conclusions.\n\n\
         **Cross-reference / Dependencies:** —\n\n\
         **Implication:** Consider revising the synthesis to explicitly include {dim}."
    )
}

/// Apply patches for a failed evidence critic.
fn apply_evidence_patches(
    patched: &mut AnalysisResult,
    topic: &str,
    report: &CriticReport,
    patches: &mut Vec<SurgicalPatch>,
    seen: &mut HashSet<String>,
) {
    let needs_patch = report
        .issues
        .iter()
        .any(|i| i.contains("does not cite any source") || i.contains("out-of-range source"))
        || !report.gaps.is_empty();

    if needs_patch {
        patched.findings.push(evidence_finding(topic));
        patches.push(SurgicalPatch {
            operation: "append_finding".to_string(),
            target: "evidence".to_string(),
            reason: format!(
                "{} finding(s) lack valid source citations",
                report.issues.len()
            ),
            applied: true,
        });
        let q = format!("Which captured sources directly support the claims about {topic}?");
        if seen.insert(q.clone()) {
            patched.open_questions.push(q);
            patches.push(SurgicalPatch {
                operation: "append_open_question".to_string(),
                target: "evidence".to_string(),
                reason: "Missing source citations".to_string(),
                applied: true,
            });
        }
    }
    for gap in &report.gaps {
        let q = format!("Evidence gap: {gap}");
        if seen.insert(q.clone()) {
            patched.open_questions.push(q.clone());
            patches.push(SurgicalPatch {
                operation: "append_open_question".to_string(),
                target: "evidence".to_string(),
                reason: gap.clone(),
                applied: true,
            });
        }
    }
}

/// Build a synthetic finding that flags missing source citations.
fn evidence_finding(topic: &str) -> String {
    format!(
        "**Headline:** Source citation review required\n\n\
         **Observation:** Some findings about '{topic}' reference claims without matching source citations, or cite indices outside the captured source list.\n\n\
         **Analysis:** Every empirical claim should map to a captured source via `[#N]` citations. Unsupported statements weaken reproducibility and should be verified or removed.\n\n\
         **Cross-reference / Dependencies:** —\n\n\
         **Implication:** Before finalizing, add source references or flag claims as speculative."
    )
}

/// Apply patches for a failed logic critic.
fn apply_logic_patches(
    patched: &mut AnalysisResult,
    topic: &str,
    report: &CriticReport,
    patches: &mut Vec<SurgicalPatch>,
    seen: &mut HashSet<String>,
) {
    let mut dims: Vec<String> = report
        .issues
        .iter()
        .filter_map(|issue| {
            if issue.contains("Contradiction on '") {
                extract_dimension(issue)
            } else {
                None
            }
        })
        .collect();
    dims.sort_unstable();
    dims.dedup();

    if !dims.is_empty() {
        let qualification = format!(
            " Note: conflicting evidence exists on {}; see the Open Questions section for how the synthesis handles contradictory sources.",
            dims.join(", ")
        );
        patched.summary.push_str(&qualification);
        patches.push(SurgicalPatch {
            operation: "qualify_summary".to_string(),
            target: dims.join(", "),
            reason: "Unresolved contradictions detected by logic critic".to_string(),
            applied: true,
        });
        for dim in &dims {
            let q = format!("How should the contradiction on {dim} be resolved for {topic}?");
            if seen.insert(q.clone()) {
                patched.open_questions.push(q);
                patches.push(SurgicalPatch {
                    operation: "append_open_question".to_string(),
                    target: dim.clone(),
                    reason: "Unresolved contradiction".to_string(),
                    applied: true,
                });
            }
        }
    } else {
        patches.push(SurgicalPatch {
            operation: "noop".to_string(),
            target: "logic".to_string(),
            reason: "Logic critic failed but no contradictions could be extracted".to_string(),
            applied: false,
        });
        for gap in &report.gaps {
            let q = format!("Logic gap: {gap}");
            if seen.insert(q.clone()) {
                patched.open_questions.push(q.clone());
                patches.push(SurgicalPatch {
                    operation: "append_open_question".to_string(),
                    target: "logic".to_string(),
                    reason: gap.clone(),
                    applied: true,
                });
            }
        }
    }
}

/// Apply patches for a failed readability critic.
fn apply_readability_patches(
    patched: &mut AnalysisResult,
    report: &CriticReport,
    patches: &mut Vec<SurgicalPatch>,
    seen: &mut HashSet<String>,
) {
    if report.issues.is_empty() {
        patches.push(SurgicalPatch {
            operation: "noop".to_string(),
            target: "readability".to_string(),
            reason: "Readability critic did not report actionable issues".to_string(),
            applied: false,
        });
        return;
    }
    let q = "Can any findings be reformatted to include Headline, Observation, Analysis, Cross-reference, and Implication labels?".to_string();
    if seen.insert(q.clone()) {
        patched.open_questions.push(q);
        patches.push(SurgicalPatch {
            operation: "append_open_question".to_string(),
            target: "readability".to_string(),
            reason: report.issues.join("; "),
            applied: true,
        });
    }
}

/// Apply a generic patch for an unrecognized critic.
fn apply_generic_critic_patch(
    patched: &mut AnalysisResult,
    report: &CriticReport,
    patches: &mut Vec<SurgicalPatch>,
    seen: &mut HashSet<String>,
) {
    for gap in &report.gaps {
        let q = format!("{} critic gap: {gap}", report.name);
        if seen.insert(q.clone()) {
            patched.open_questions.push(q.clone());
            patches.push(SurgicalPatch {
                operation: "append_open_question".to_string(),
                target: report.name.clone(),
                reason: gap.clone(),
                applied: true,
            });
        }
    }
}

/// Apply a corpus-critic-driven patch when no synthesis audit is available.
fn apply_generic_gap_patch(
    patched: &mut AnalysisResult,
    topic: &str,
    gap: &str,
    patches: &mut Vec<SurgicalPatch>,
    seen: &mut HashSet<String>,
    source: &str,
    operation_prefix: &str,
) {
    if let Some(dim) = extract_dimension(gap) {
        patched.findings.push(coverage_finding(topic, &dim));
        patches.push(SurgicalPatch {
            operation: format!("{operation_prefix}_finding"),
            target: dim.clone(),
            reason: gap.to_string(),
            applied: true,
        });
    }
    let q = format!("Gap ({source}): {gap}");
    if seen.insert(q.clone()) {
        patched.open_questions.push(q.clone());
        patches.push(SurgicalPatch {
            operation: format!("{operation_prefix}_question"),
            target: source.to_string(),
            reason: gap.to_string(),
            applied: true,
        });
    }
}

/// Extract a dimension label from a gap/issue string such as
/// "Dimension 'Cost' is not addressed" or "Add evidence or a finding for 'Cost'".
fn extract_dimension(text: &str) -> Option<String> {
    for (open, close) in [("'", "'"), ("\"", "\"")] {
        if let Some(start) = text.find(open) {
            let rest = &text[start + open.len()..];
            if let Some(end) = rest.find(close) {
                let dim = rest[..end].trim();
                if !dim.is_empty() {
                    return Some(dim.to_string());
                }
            }
        }
    }
    // Fallback: "for '<dim>'" near the end of the string.
    if let Some(idx) = text.rfind("for '") {
        let rest = &text[idx + 5..];
        if let Some(end) = rest.find('\'') {
            let dim = rest[..end].trim();
            if !dim.is_empty() {
                return Some(dim.to_string());
            }
        }
    }
    None
}

/// Estimate the post-patch score from the pre-patch score and applied patch count.
fn estimate_score_after(before: u32, applied_count: usize) -> u32 {
    if applied_count == 0 {
        return before;
    }
    // Each applied patch is credited with a small improvement; cap at 100.
    let boost = (applied_count as u32 * 5).saturating_add(5);
    (before + boost).min(100)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::AnalysisResult;
    use crate::corpus_critic::CorpusCriticReport;
    use crate::synthesis::{CriticReport, SynthesisAudit};

    fn failing_coverage_report() -> CriticReport {
        CriticReport {
            name: "coverage".to_string(),
            score: 40,
            issues: vec![
                "Dimension 'Cost' is not addressed in the synthesis findings or implications"
                    .to_string(),
            ],
            gaps: vec!["Add evidence or a finding for 'Cost'".to_string()],
            passed: false,
        }
    }

    fn failing_evidence_report() -> CriticReport {
        CriticReport {
            name: "evidence".to_string(),
            score: 50,
            issues: vec!["Finding 0 does not cite any source".to_string()],
            gaps: vec!["Add a supporting source citation to finding 0".to_string()],
            passed: false,
        }
    }

    fn passing_logic_report() -> CriticReport {
        CriticReport {
            name: "logic".to_string(),
            score: 100,
            issues: vec!["No contradictions detected; no logic conflicts to resolve.".to_string()],
            gaps: vec![],
            passed: true,
        }
    }

    fn sample_audit() -> SynthesisAudit {
        let mut audit = SynthesisAudit::empty();
        audit.overall_score = 60;
        audit.critic_reports = vec![
            failing_coverage_report(),
            failing_evidence_report(),
            passing_logic_report(),
        ];
        audit
    }

    #[test]
    fn empty_audit_applies_corpus_critic_patches() {
        let original = AnalysisResult::default();
        let audit = SynthesisAudit::empty();
        let mut corpus = CorpusCriticReport::empty();
        corpus.gaps = vec!["Find additional evidence on 'Safety' for 'topic'".to_string()];
        let result = build_surgical_patches(&audit, &corpus, "topic", &original);
        assert!(
            result
                .patches
                .iter()
                .any(|p| p.applied && p.operation == "corpus_critic_finding"),
            "expected a corpus-critic finding patch"
        );
        assert!(
            result
                .patched_analysis
                .findings
                .iter()
                .any(|f| f.contains("Safety")),
            "patched analysis should contain a Safety coverage finding"
        );
    }

    #[test]
    fn coverage_failure_adds_finding_and_question() {
        let original = AnalysisResult::default();
        let audit = sample_audit();
        let corpus = CorpusCriticReport::empty();
        let result = build_surgical_patches(&audit, &corpus, "topic", &original);
        assert!(
            result
                .patches
                .iter()
                .any(|p| p.operation == "append_finding" && p.target == "Cost"),
            "coverage patch should append a Cost finding"
        );
        assert!(
            result
                .patched_analysis
                .open_questions
                .iter()
                .any(|q| q.contains("Cost")),
            "coverage patch should add a Cost open question"
        );
    }

    #[test]
    fn evidence_failure_adds_citation_reminder() {
        let original = AnalysisResult {
            findings: vec!["Some claim without citation.".to_string()],
            ..AnalysisResult::default()
        };
        let audit = sample_audit();
        let corpus = CorpusCriticReport::empty();
        let result = build_surgical_patches(&audit, &corpus, "topic", &original);
        assert!(
            result
                .patched_analysis
                .findings
                .iter()
                .any(|f| f.contains("Source citation review required")),
            "evidence patch should append a citation reminder finding"
        );
        assert!(
            result
                .patched_analysis
                .open_questions
                .iter()
                .any(|q| q.contains("sources")),
            "evidence patch should add a source open question"
        );
    }

    #[test]
    fn passed_critic_emits_noop_patch() {
        let original = AnalysisResult::default();
        let mut audit = SynthesisAudit::empty();
        audit.overall_score = 100;
        audit.critic_reports = vec![passing_logic_report()];
        let corpus = CorpusCriticReport::empty();
        let result = build_surgical_patches(&audit, &corpus, "topic", &original);
        assert!(
            result
                .patches
                .iter()
                .any(|p| p.operation == "noop" && p.target == "logic"),
            "passing logic critic should produce a noop patch"
        );
    }

    #[test]
    fn score_after_is_not_decreased() {
        let original = AnalysisResult::default();
        let audit = sample_audit();
        let corpus = CorpusCriticReport::empty();
        let result = build_surgical_patches(&audit, &corpus, "topic", &original);
        assert!(
            result.score_after >= result.score_before,
            "post-patch score should not drop"
        );
        assert!(
            result.score_after <= 100,
            "post-patch score should be capped at 100"
        );
    }

    #[test]
    fn logic_failure_qualifies_summary() {
        let original = AnalysisResult {
            summary: "The treatment is effective.".to_string(),
            ..AnalysisResult::default()
        };
        let mut audit = SynthesisAudit::empty();
        audit.overall_score = 55;
        audit.critic_reports = vec![CriticReport {
            name: "logic".to_string(),
            score: 50,
            issues: vec![
                "Contradiction on 'safety' is not acknowledged in the synthesis".to_string(),
            ],
            gaps: vec![
                "Add a qualified finding that notes the conflicting evidence on 'safety'"
                    .to_string(),
            ],
            passed: false,
        }];
        let corpus = CorpusCriticReport::empty();
        let result = build_surgical_patches(&audit, &corpus, "topic", &original);
        assert!(
            result
                .patched_analysis
                .summary
                .contains("conflicting evidence"),
            "summary should be qualified with contradiction note"
        );
        assert!(
            result
                .patched_analysis
                .open_questions
                .iter()
                .any(|q| q.contains("safety")),
            "logic patch should add a safety contradiction question"
        );
    }
}
