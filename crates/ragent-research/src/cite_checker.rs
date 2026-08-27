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

use crate::polarity::citation_re;

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

    let citation_re = citation_re();

    let mut examine = |text: &str, context: &str| {
        for cap in citation_re.captures_iter(text) {
            let index: usize = match cap[1].parse() {
                Ok(n) if n > 0 => n,
                _ => continue,
            };
            checked += 1;

            let source_label = format!("[#{}]", index);
            let source = sources.get(index - 1);

            match source {
                None => {
                    let claim = format!(
                        "CITATION_VERIFICATION_FAILED: {source_label} in {context} references an unknown source (only {} source(s) available).",
                        sources.len()
                    );
                    failed_claims.push(claim.clone());
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
                        failed_claims.push(claim.clone());
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
mod tests {
    use super::*;
    use crate::source::{LocalSourceKind, Source};
    use chrono::Utc;
    use std::path::PathBuf;

    fn web_source(body: &str) -> Source {
        Source::Web {
            url: "https://example.com".into(),
            title: "Example".into(),
            captured_at: Utc::now(),
            published_at: None,
            body_path: PathBuf::from("sources/web-01.md"),
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

    fn empty_web_source() -> Source {
        web_source("")
    }

    fn spec_source() -> Source {
        Source::Spec {
            spec_id: "hyperresearch".into(),
            captured_at: Utc::now(),
            relevance: String::new(),
        }
    }

    #[test]
    fn no_citations_passes_empty() {
        let result = check_citations(
            "summary without citations",
            &["finding without citation".into()],
            &[],
            &[],
            &[web_source("body")],
        );
        assert!(result.passed);
        assert!(result.gate_open);
        assert_eq!(result.checked, 0);
        assert!(result.failed_claims.is_empty());
    }

    #[test]
    fn valid_citation_with_body_passes() {
        let result = check_citations(
            "Claim supported by [#1].",
            &[],
            &[],
            &[],
            &[web_source("body text")],
        );
        assert!(result.passed);
        assert!(result.gate_open);
        assert_eq!(result.checked, 1);
    }

    #[test]
    fn citation_to_unknown_source_fails_and_closes_gate() {
        let result = check_citations(
            "Claim supported by [#2].",
            &[],
            &[],
            &[],
            &[web_source("body text")],
        );
        assert!(!result.passed);
        assert!(!result.gate_open);
        assert_eq!(result.checked, 1);
        assert_eq!(result.failed_claims.len(), 1);
        assert!(result.failed_claims[0].contains("CITATION_VERIFICATION_FAILED"));
        assert!(result.issues[0].contains("unknown source index"));
    }

    #[test]
    fn citation_to_empty_body_source_fails() {
        let result = check_citations(
            "Claim supported by [#1].",
            &[],
            &[],
            &[],
            &[empty_web_source()],
        );
        assert!(!result.passed);
        assert!(!result.gate_open);
        assert_eq!(result.failed_claims.len(), 1);
        assert!(result.failed_claims[0].contains("no captured body"));
    }

    #[test]
    fn spec_citation_passes_without_body() {
        let result = check_citations("Claim tied to spec [#1].", &[], &[], &[], &[spec_source()]);
        assert!(result.passed);
        assert!(result.gate_open);
        assert_eq!(result.checked, 1);
    }

    #[test]
    fn multiple_failures_accumulate() {
        let result = check_citations(
            "Summary [#1] and [#3].",
            &["Finding [#2]".into()],
            &[],
            &[],
            &[empty_web_source()],
        );
        assert!(!result.passed);
        assert_eq!(result.checked, 3);
        assert_eq!(result.failed_claims.len(), 3);
    }

    #[test]
    fn checks_all_text_fields() {
        let sources = vec![web_source("evidence")];
        let result = check_citations(
            "Summary [#1].",
            &["Finding [#1]".into()],
            &["Implication [#1]".into()],
            &["Question [#1]?".into()],
            &sources,
        );
        assert!(result.passed);
        assert_eq!(result.checked, 4);
    }

    #[test]
    fn local_source_with_body_passes() {
        let local = Source::Local {
            path: "src/lib.rs".into(),
            kind: LocalSourceKind::InProject,
            captured_at: Utc::now(),
            body_path: PathBuf::from("sources/local-01.md"),
            body: "fn main() {}".into(),
            relevance: String::new(),
        };
        let result = check_citations("Code reference [#1].", &[], &[], &[], &[local]);
        assert!(result.passed);
    }
}
