//! Citation checker with failure gate (FR-005, T-014).
//!
//! Before the final `RESEARCH.md` is written, this module scans the draft
//! summary, findings, implications, and open questions for `[#N]` citations
//! and verifies that each cited source exists in the gathered corpus and has
//! usable content. Any unsupported citation is flagged with the
//! `CITATION_VERIFICATION_FAILED` marker and blocks the report from shipping.
//!
//! The checker is intentionally deterministic and does not call an LLM. It
//! relies on the source vault invariant (FR-004): every cited claim must map
//! to a source that is present in the final `sources` list and, for non-spec
//! sources, carries a non-empty captured body.

use crate::source::Source;
use serde::{Deserialize, Serialize};

/// Outcome of the deterministic cite-check pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CitationCheckResult {
    /// `true` when every checked citation was supported by a source.
    pub passed: bool,
    /// Number of citation markers examined.
    pub checked: usize,
    /// Failed claims, each prefixed with `CITATION_VERIFICATION_FAILED`.
    pub failed_claims: Vec<String>,
    /// Human-readable issue strings for each failure.
    pub issues: Vec<String>,
    /// `true` when the failure gate is open (i.e. the report may ship).
    /// This is identical to `passed` for the strict implementation.
    pub gate_open: bool,
}

impl CitationCheckResult {
    /// Create a passing empty result.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            passed: true,
            checked: 0,
            failed_claims: Vec::new(),
            issues: Vec::new(),
            gate_open: true,
        }
    }

    /// Return `true` when no citations were checked.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.checked == 0
    }
}

/// Verify every `[#N]` citation in the assembled narrative.
///
/// A citation passes when:
///
/// - the index `N` is in range (`1 <= N <= sources.len()`),
/// - the referenced source exists, and
/// - the source has a non-empty body (web/local/other) or is a spec reference
///   (which has no body by design but still counts as a valid cross-reference).
///
/// The returned [`CitationCheckResult::passed`] is `false` as soon as any
/// citation fails. The failure gate (`gate_open`) is closed in the same case,
/// preventing the session from writing the report.
#[must_use]
pub fn check_citations(
    summary: &str,
    findings: &[String],
    implications: &[String],
    open_questions: &[String],
    sources: &[Source],
) -> CitationCheckResult {
    let mut checked = 0usize;
    let mut failed_claims: Vec<String> = Vec::new();
    let mut issues: Vec<String> = Vec::new();

    let mut examine = |text: &str, context: &str| {
        for index in crate::polarity::cited_indices(text) {
            checked += 1;

            let source_label = format!("[#{}]", index);
            let source = sources.get(index - 1);

            match source {
                None => {
                    let claim = format!(
                        "CITATION_VERIFICATION_FAILED: {source_label} in {context} references an unknown source (only {} source(s) available).",
                        sources.len()
                    );
                    failed_claims.push(claim);
                    issues.push(format!(
                        "{source_label} in {context}: unknown source index {index}"
                    ));
                }
                Some(src) => {
                    let valid = match src {
                        Source::Spec { .. } => true,
                        _ => src.has_body(),
                    };
                    if !valid {
                        let claim = format!(
                            "CITATION_VERIFICATION_FAILED: {source_label} in {context} points to a source with no captured body.",
                        );
                        failed_claims.push(claim);
                        issues.push(format!(
                            "{source_label} in {context}: source {index} has no captured body"
                        ));
                    }
                }
            }
        }
    };

    examine(summary, "summary");
    for (i, f) in findings.iter().enumerate() {
        examine(f, &format!("finding {}", i + 1));
    }
    for (i, imp) in implications.iter().enumerate() {
        examine(imp, &format!("implication {}", i + 1));
    }
    for (i, q) in open_questions.iter().enumerate() {
        examine(q, &format!("open question {}", i + 1));
    }

    let passed = failed_claims.is_empty();
    CitationCheckResult {
        passed,
        checked,
        failed_claims,
        issues,
        gate_open: passed,
    }
}

#[cfg(test)]
#[path = "../tests/inline/cite_checker_tests.rs"]
mod tests;
