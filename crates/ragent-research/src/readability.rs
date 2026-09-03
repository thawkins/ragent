//! Polish and readability audit (FR-005, T-015).
//!
//! After the draft has passed citation verification, this module performs a
//! final deterministic polish pass on the narrative and then audits the
//! resulting readability. Both steps are intentionally LLM-free so they work
//! offline, run quickly, and remain fully reproducible for a given draft.
//!
//! The polish step removes control characters, normalizes whitespace, and
//! drops empty paragraphs. The audit step scores the polished draft on
//! structure and paragraph length and flags missing required labels.

use crate::analysis::AnalysisResult;
use crate::item::strip_control_chars;
use serde::{Deserialize, Serialize};

/// One deterministic change applied by the polish step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PolishChange {
    /// Field that was changed, e.g. `"summary"` or `"finding_2"`.
    pub field: String,
    /// Short human-readable description of what was done.
    pub description: String,
}

/// Result of the deterministic polish step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PolishResult {
    /// Individual changes applied to the draft.
    pub changes: Vec<PolishChange>,
    /// Number of control characters removed across all fields.
    pub control_chars_removed: usize,
    /// Number of trailing-whitespace / multiple-blank-line runs normalized.
    pub whitespace_normalized: usize,
    /// Number of empty findings, implications, or questions removed.
    pub empty_paragraphs_removed: usize,
    /// Human-readable summary of what changed.
    pub note: String,
}

impl PolishResult {
    /// Create a no-op polish result.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            changes: Vec::new(),
            control_chars_removed: 0,
            whitespace_normalized: 0,
            empty_paragraphs_removed: 0,
            note: "No polish changes were needed".to_string(),
        }
    }

    /// Return `true` when no changes were applied.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }
}

/// Structured outcome of the readability audit step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ReadabilityAudit {
    /// Readability score 0–100.
    pub score: u32,
    /// `true` when the audit passed the configured threshold (score >= 70 and
    /// at least one finding is present).
    pub passed: bool,
    /// Detected readability / structure issues.
    pub issues: Vec<String>,
    /// Recommendations for further improvement.
    pub recommendations: Vec<String>,
    /// Average finding length in characters after polishing.
    pub avg_finding_length: usize,
    /// Number of findings missing one or more required labels.
    pub missing_label_count: usize,
    /// Number of paragraphs longer than 1200 characters.
    pub long_paragraph_count: usize,
}

impl ReadabilityAudit {
    /// Create an empty audit.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            score: 0,
            passed: false,
            issues: Vec::new(),
            recommendations: Vec::new(),
            avg_finding_length: 0,
            missing_label_count: 0,
            long_paragraph_count: 0,
        }
    }

    /// Return `true` when the audit found no issues or recommendations.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.issues.is_empty() && self.recommendations.is_empty()
    }
}

/// Apply deterministic polish to the narrative fields of `analysis`.
///
/// The function mutates `analysis` in place and returns an auditable list of
/// changes. It is safe to call on an empty analysis.
pub fn polish_analysis(analysis: &mut AnalysisResult) -> PolishResult {
    let mut result = PolishResult::empty();

    // Summary.
    let polished_summary = polish_text(&analysis.summary);
    if polished_summary != analysis.summary {
        result.control_chars_removed += count_control_chars(&analysis.summary);
        result.whitespace_normalized += count_whitespace_fixes(&analysis.summary);
        analysis.summary = polished_summary;
        result.changes.push(PolishChange {
            field: "summary".to_string(),
            description: "normalized control chars and whitespace".to_string(),
        });
    }

    // Findings: drop empties and polish the rest.
    let mut kept_findings = Vec::new();
    for (idx, finding) in analysis.findings.iter().enumerate() {
        let polished = polish_text(finding);
        if polished.trim().is_empty() {
            result.empty_paragraphs_removed += 1;
            result.changes.push(PolishChange {
                field: format!("finding_{idx}"),
                description: "removed empty finding".to_string(),
            });
            continue;
        }
        if polished != *finding {
            result.control_chars_removed += count_control_chars(finding);
            result.whitespace_normalized += count_whitespace_fixes(finding);
            result.changes.push(PolishChange {
                field: format!("finding_{idx}"),
                description: "normalized control chars and whitespace".to_string(),
            });
        }
        kept_findings.push(polished);
    }
    analysis.findings = kept_findings;

    // Implications: drop empties and polish the rest.
    let mut kept_implications = Vec::new();
    for (idx, imp) in analysis.top_implications.iter().enumerate() {
        let polished = polish_text(imp);
        if polished.trim().is_empty() {
            result.empty_paragraphs_removed += 1;
            result.changes.push(PolishChange {
                field: format!("implication_{idx}"),
                description: "removed empty implication".to_string(),
            });
            continue;
        }
        if polished != *imp {
            result.control_chars_removed += count_control_chars(imp);
            result.whitespace_normalized += count_whitespace_fixes(imp);
            result.changes.push(PolishChange {
                field: format!("implication_{idx}"),
                description: "normalized control chars and whitespace".to_string(),
            });
        }
        kept_implications.push(polished);
    }
    analysis.top_implications = kept_implications;

    // Open questions: drop empties and polish the rest.
    let mut kept_questions = Vec::new();
    for (idx, q) in analysis.open_questions.iter().enumerate() {
        let polished = polish_text(q);
        if polished.trim().is_empty() {
            result.empty_paragraphs_removed += 1;
            result.changes.push(PolishChange {
                field: format!("question_{idx}"),
                description: "removed empty question".to_string(),
            });
            continue;
        }
        if polished != *q {
            result.control_chars_removed += count_control_chars(q);
            result.whitespace_normalized += count_whitespace_fixes(q);
            result.changes.push(PolishChange {
                field: format!("question_{idx}"),
                description: "normalized control chars and whitespace".to_string(),
            });
        }
        kept_questions.push(polished);
    }
    analysis.open_questions = kept_questions;

    if !result.is_empty() {
        result.note = format!(
            "Applied {} polish change(s): removed {} control character(s), normalized {} whitespace run(s), removed {} empty paragraph(s).",
            result.changes.len(),
            result.control_chars_removed,
            result.whitespace_normalized,
            result.empty_paragraphs_removed
        );
    }

    result
}

/// Remove control characters, trim trailing whitespace on each line, and
/// collapse consecutive blank lines to a single blank line.
fn polish_text(text: &str) -> String {
    let cleaned = strip_control_chars(text);
    let mut out = String::new();
    let mut blank_run = false;
    for line in cleaned.lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            if !blank_run {
                out.push('\n');
                blank_run = true;
            }
        } else {
            out.push_str(trimmed);
            out.push('\n');
            blank_run = false;
        }
    }
    out.trim().to_string()
}

/// Count non-whitespace control characters in `text`.
fn count_control_chars(text: &str) -> usize {
    text.chars()
        .filter(|c| c.is_control() && !c.is_whitespace())
        .count()
}

/// Approximate count of whitespace normalization opportunities: consecutive
/// blank lines and lines with trailing whitespace.
fn count_whitespace_fixes(text: &str) -> usize {
    let lines: Vec<&str> = text.lines().collect();
    let mut count = 0usize;
    let mut prev_blank = false;
    for line in &lines {
        let is_blank = line.trim().is_empty();
        if is_blank && prev_blank {
            count += 1;
        }
        if !is_blank && line.len() != line.trim_end().len() {
            count += 1;
        }
        prev_blank = is_blank;
    }
    count
}

/// Run a deterministic readability audit on the polished analysis.
///
/// The audit checks that findings exist, contain the five required labeled
/// paragraphs, and do not contain extremely long paragraphs. It returns a
/// score and a pass/fail verdict.
#[must_use]
pub fn audit_readability(analysis: &AnalysisResult) -> ReadabilityAudit {
    let required = [
        "**Headline:**",
        "**Observation:**",
        "**Analysis:**",
        "**Cross-reference / Dependencies:**",
        "**Implication:**",
    ];

    let mut issues = Vec::new();
    let mut recommendations = Vec::new();
    let mut missing_label_count = 0usize;
    let mut long_paragraph_count = 0usize;
    let mut total_finding_len = 0usize;

    if analysis.findings.is_empty() {
        issues.push("No findings were produced".to_string());
        recommendations.push("Produce at least one structured finding".to_string());
    }

    for (idx, finding) in analysis.findings.iter().enumerate() {
        total_finding_len += finding.len();
        let missing: Vec<&str> = required
            .iter()
            .filter(|label| !finding.contains(**label))
            .copied()
            .collect();
        if !missing.is_empty() {
            missing_label_count += 1;
            issues.push(format!(
                "Finding {idx} is missing required labels: {}",
                missing.join(", ")
            ));
        }
        let paragraphs: Vec<&str> = finding.split("\n\n").collect();
        for p in &paragraphs {
            if p.len() > 1200 {
                long_paragraph_count += 1;
                issues.push(format!(
                    "Finding {idx} contains a paragraph longer than 1200 characters"
                ));
                break;
            }
        }
    }

    let avg_finding_length = if analysis.findings.is_empty() {
        0
    } else {
        total_finding_len / analysis.findings.len()
    };

    if analysis.top_implications.is_empty() {
        issues.push("No implications were produced".to_string());
        recommendations.push("Derive at least one practical implication".to_string());
    }

    if analysis.open_questions.is_empty() {
        recommendations
            .push("Consider adding open questions for further investigation".to_string());
    }

    let empty_findings_penalty = if analysis.findings.is_empty() { 40 } else { 0 };
    let penalty = ((missing_label_count * 10) + (long_paragraph_count * 10))
        .min(60)
        .saturating_add(empty_findings_penalty);
    let score = 100u32.saturating_sub(penalty as u32);
    let passed = score > 70 && !analysis.findings.is_empty();

    ReadabilityAudit {
        score,
        passed,
        issues,
        recommendations,
        avg_finding_length,
        missing_label_count,
        long_paragraph_count,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::assert_is_empty)]
    use super::*;
    use crate::analysis::AnalysisResult;

    #[test]
    fn polish_removes_control_chars_and_empty_paragraphs() {
        let mut analysis = AnalysisResult {
            summary: "Summary\x01 text\n\n\n".to_string(),
            findings: vec![
                "Finding\x02 one.".to_string(),
                "\x03   ".to_string(),
                "Finding three.".to_string(),
            ],
            top_implications: vec!["   ".to_string(), "Implication".to_string()],
            cross_references: Vec::new(),
            open_questions: vec!["Question\n\n\n".to_string()],
        };
        let result = polish_analysis(&mut analysis);
        assert!(!result.is_empty());
        assert_eq!(result.control_chars_removed, 2); // two control chars remain in original fields
        assert_eq!(result.empty_paragraphs_removed, 2); // empty finding + empty implication
        assert!(!analysis.summary.contains('\x01'));
        assert_eq!(analysis.findings.len(), 2);
        assert_eq!(analysis.top_implications.len(), 1);
        assert!(!analysis.open_questions[0].contains("\n\n\n"));
    }

    #[test]
    fn audit_passes_well_formed_analysis() {
        let analysis = AnalysisResult {
            summary: "Summary".to_string(),
            findings: vec![format!(
                "**Headline:** H\n\n\
                 **Observation:** O\n\n\
                 **Analysis:** A\n\n\
                 **Cross-reference / Dependencies:** C\n\n\
                 **Implication:** I"
            )],
            top_implications: vec!["Imp".to_string()],
            cross_references: Vec::new(),
            open_questions: vec!["Q".to_string()],
        };
        let audit = audit_readability(&analysis);
        assert!(audit.passed);
        assert_eq!(audit.score, 100);
        assert!(audit.issues.is_empty());
    }

    #[test]
    fn audit_fails_empty_analysis() {
        let analysis = AnalysisResult::default();
        let audit = audit_readability(&analysis);
        assert!(!audit.passed);
        assert!(
            audit.score < 70,
            "empty analysis should score below passing threshold: {}",
            audit.score
        );
        assert!(!audit.issues.is_empty());
    }

    #[test]
    fn audit_flags_missing_labels_and_long_paragraphs() {
        let analysis = AnalysisResult {
            summary: "Summary".to_string(),
            findings: vec![
                "**Headline:** H\n\n**Observation:** O".to_string(),
                "a".repeat(1300),
            ],
            top_implications: vec!["Imp".to_string()],
            cross_references: Vec::new(),
            open_questions: Vec::new(),
        };
        let audit = audit_readability(&analysis);
        assert!(!audit.passed);
        assert!(audit.missing_label_count > 0);
        assert!(audit.long_paragraph_count > 0);
    }
}
