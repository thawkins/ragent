//! Claim-to-source verification pass (T-010, FR-002, FR-013).
//!
//! A [`Verifier`] checks that the claims produced by the synthesis step are
//! traceable to at least one captured source. The default
//! [`KeywordVerifier`] is a deterministic, explainable baseline: every
//! citation `[#N]` in a finding must point to a source whose body overlaps
//! with the finding's content.

use crate::analysis::AnalysisResult;
use crate::source::Source;
use crate::state::ResearchState;
use async_trait::async_trait;
use regex::Regex;
use std::collections::HashSet;
use std::sync::OnceLock;

static CITATION_RE: OnceLock<Regex> = OnceLock::new();

fn citation_re() -> &'static Regex {
    CITATION_RE.get_or_init(|| Regex::new(r"\[#(\d+)\]").expect("valid citation regex"))
}

/// Result of a verification pass.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VerificationResult {
    /// `true` when every checked claim had source support.
    pub passed: bool,
    /// Human-readable issues for any failed checks.
    pub issues: Vec<String>,
    /// Number of claims that were checked.
    pub claims_checked: usize,
    /// Number of claims that passed the check.
    pub claims_supported: usize,
}

/// Abstraction over claim verifiers.
#[async_trait]
pub trait Verifier: Send + Sync {
    /// Verify the claims in `analysis` against `state.sources`.
    async fn verify(
        &self,
        state: &ResearchState,
        analysis: Option<&AnalysisResult>,
    ) -> VerificationResult;
}

/// Deterministic verifier that requires:
///
/// - every `[#N]` citation to resolve to a known source;
/// - the cited source body to share at least one content word with the finding
///   (a cheap proxy for "the source actually supports the claim").
///
/// The implementation is intentionally simple so it works without an LLM and
/// produces transparent, reproducible diagnostics.
#[derive(Debug, Default, Clone, Copy)]
pub struct KeywordVerifier;

impl KeywordVerifier {
    /// Create a new keyword verifier.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
    /// Extract citation indices from a finding body.
    fn cited_indices(finding: &str) -> Vec<usize> {
        let mut out: Vec<usize> = citation_re()
            .captures_iter(finding)
            .filter_map(|cap| cap[1].parse().ok())
            .filter(|n| *n > 0)
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Extract content words from a text, lowercased and deduplicated.
    /// Filters out common stop words and tokens shorter than 4 characters
    /// so the overlap check is a better proxy for genuine source support
    /// (FUNC-ANL-07).
    fn words(text: &str) -> Vec<String> {
        let mut words: Vec<String> = text
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() >= 4 && !is_stopword_lc(w))
            .map(std::string::ToString::to_string)
            .collect();
        words.sort_unstable();
        words.dedup();
        words
    }

    /// Check whether `finding` is supported by `source`.
    /// Requires at least two non-trivial content-word overlaps so that a
    /// single shared common word does not cause a false pass (FUNC-ANL-07).
    fn supported_by(finding: &str, source: &Source) -> bool {
        let f_words = Self::words(finding);
        let Some(body) = source.body() else {
            return false;
        };
        let s_words: HashSet<String> = Self::words(body).into_iter().collect();
        let overlap = f_words.iter().filter(|w| s_words.contains(*w)).count();
        // Require at least 2 shared content words, or 1 when the finding
        // has very few content words total (short findings).
        let min_overlap = if f_words.len() <= 2 { 1 } else { 2 };
        overlap >= min_overlap
    }
}

/// Common English stop words used as a content-word filter in the verifier.
/// Words shorter than 4 chars are already filtered by `words()`, so only
/// longer function words are listed here.
fn is_stopword_lc(word: &str) -> bool {
    matches!(
        word,
        "this"
            | "that"
            | "these"
            | "those"
            | "what"
            | "which"
            | "who"
            | "when"
            | "where"
            | "why"
            | "how"
            | "will"
            | "would"
            | "could"
            | "should"
            | "must"
            | "have"
            | "been"
            | "being"
            | "from"
            | "they"
            | "them"
            | "their"
            | "there"
            | "your"
            | "our"
    )
}

#[async_trait]
impl Verifier for KeywordVerifier {
    async fn verify(
        &self,
        state: &ResearchState,
        analysis: Option<&AnalysisResult>,
    ) -> VerificationResult {
        let findings = analysis.map(|a| a.findings.as_slice()).unwrap_or(&[]);
        if findings.is_empty() {
            return VerificationResult {
                passed: true,
                issues: vec!["no findings to verify".to_string()],
                claims_checked: 0,
                claims_supported: 0,
            };
        }

        let mut issues = Vec::new();
        let mut checked = 0usize;
        let mut supported = 0usize;

        for (idx, finding) in findings.iter().enumerate() {
            let indices = Self::cited_indices(finding);
            if indices.is_empty() {
                issues.push(format!("Finding {} has no citations", idx + 1));
                continue;
            }
            checked += 1;
            let mut finding_supported = true;
            for n in indices {
                let Some(source) = state.sources.get(n - 1) else {
                    issues.push(format!(
                        "Finding {} cites source [#{n}] which does not exist",
                        idx + 1
                    ));
                    finding_supported = false;
                    continue;
                };
                if !Self::supported_by(finding, source) {
                    issues.push(format!(
                        "Finding {} cites source [#{n}] but the source body does not support the claim",
                        idx + 1
                    ));
                    finding_supported = false;
                }
            }
            if finding_supported {
                supported += 1;
            }
        }

        let passed = checked > 0 && supported == checked;
        VerificationResult {
            passed,
            issues,
            claims_checked: checked,
            claims_supported: supported,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::Source;
    use chrono::Utc;
    use std::path::PathBuf;

    fn web(url: &str, body: &str) -> Source {
        Source::Web {
            published_at: None,
            url: url.to_string(),
            title: "title".to_string(),
            captured_at: Utc::now(),
            body_path: PathBuf::from("sources/web-01.md"),
            body: body.to_string(),
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

    fn state_with_sources(sources: Vec<Source>) -> ResearchState {
        let mut state = ResearchState::new("topic");
        for s in sources {
            state.add_source(s);
        }
        state
    }

    fn analysis_with_finding(finding: &str) -> AnalysisResult {
        AnalysisResult {
            summary: String::new(),
            findings: vec![finding.to_string()],
            top_implications: Vec::new(),
            cross_references: vec![],
            open_questions: vec![],
        }
    }

    #[tokio::test]
    async fn verifier_passes_when_citation_supported() {
        let state = state_with_sources(vec![web("https://x", "async runtime scheduling details")]);
        let analysis = analysis_with_finding("Rust uses async runtimes for scheduling [#1].");
        let result = KeywordVerifier::new().verify(&state, Some(&analysis)).await;
        assert!(result.passed);
        assert_eq!(result.claims_supported, 1);
    }

    #[tokio::test]
    async fn verifier_fails_when_source_body_does_not_support() {
        let state = state_with_sources(vec![web("https://x", "completely unrelated body")]);
        let analysis = analysis_with_finding("Rust uses async runtimes for scheduling [#1].");
        let result = KeywordVerifier::new().verify(&state, Some(&analysis)).await;
        assert!(!result.passed);
        assert!(result.issues.iter().any(|i| i.contains("does not support")));
    }

    #[tokio::test]
    async fn verifier_fails_when_citation_missing() {
        let state = state_with_sources(vec![]);
        let analysis = analysis_with_finding("Rust uses async runtimes for scheduling [#1].");
        let result = KeywordVerifier::new().verify(&state, Some(&analysis)).await;
        assert!(!result.passed);
        assert!(result.issues.iter().any(|i| i.contains("does not exist")));
    }
}
