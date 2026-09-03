//! Self-evaluation scorecard for research reports (FR-008, FR-015, FR-019).
//!
//! Provides a deterministic, LLM-free quality scorecard that scores the produced
//! report on five dimensions: quality, relevance, groundedness, completeness,
//! and structure. The scorecard is rendered as a Markdown section and appended
//! to the report when self-evaluation is enabled.

use crate::run_config::OutputFormat;
use crate::source::Source;
use serde::{Deserialize, Serialize};

/// Scores for the five self-evaluation dimensions plus an overall score.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EvaluationScorecard {
    /// Quality of the narrative: clarity, coherence, and usefulness.
    pub quality: u32,
    /// Relevance to the original topic/brief.
    pub relevance: u32,
    /// Groundedness: citations and source backing.
    pub groundedness: u32,
    /// Completeness: coverage of expected sections and source count.
    pub completeness: u32,
    /// Structure: presence of finding labels and readable formatting.
    pub structure: u32,
    /// Overall score (simple average of the five dimensions).
    pub overall: u32,
    /// Short rationale explaining the scores.
    pub rationale: String,
    /// Evaluation error, surfaced in the scorecard instead of being swallowed
    /// (FR-019).
    pub error: Option<String>,
}

impl EvaluationScorecard {
    /// Create a scorecard from individual dimension scores and a rationale.
    #[must_use]
    pub fn new(
        quality: u32,
        relevance: u32,
        groundedness: u32,
        completeness: u32,
        structure: u32,
        rationale: impl Into<String>,
    ) -> Self {
        let dims = [quality, relevance, groundedness, completeness, structure];
        let overall = dims.iter().sum::<u32>() / dims.len() as u32;
        Self {
            quality,
            relevance,
            groundedness,
            completeness,
            structure,
            overall,
            rationale: rationale.into(),
            error: None,
        }
    }

    /// Create a scorecard that records an evaluation failure (FR-019).
    #[must_use]
    pub fn failed(error: impl Into<String>) -> Self {
        Self {
            error: Some(error.into()),
            ..Self::default()
        }
    }

    /// Return `true` if the scorecard represents a failed evaluation.
    #[must_use]
    pub const fn is_failed(&self) -> bool {
        self.error.is_some()
    }
}

/// Render a scorecard as a Markdown section ready to append to RESEARCH.md.
#[must_use]
pub fn render_scorecard(scorecard: &EvaluationScorecard) -> String {
    let mut out = String::from("## Self-Evaluation Scorecard\n\n");
    if let Some(err) = &scorecard.error {
        out.push_str(&format!("_Evaluation could not be completed: {err}_\n\n"));
    }
    out.push_str("| Dimension | Score |\n");
    out.push_str("| --- | --- |\n");
    out.push_str(&format!("| Quality | {}/100 |\n", scorecard.quality));
    out.push_str(&format!("| Relevance | {}/100 |\n", scorecard.relevance));
    out.push_str(&format!(
        "| Groundedness | {}/100 |\n",
        scorecard.groundedness
    ));
    out.push_str(&format!(
        "| Completeness | {}/100 |\n",
        scorecard.completeness
    ));
    out.push_str(&format!("| Structure | {}/100 |\n", scorecard.structure));
    out.push_str(&format!(
        "| **Overall** | **{}/100** |\n",
        scorecard.overall
    ));
    out.push('\n');
    out.push_str(&format!(
        "**Rationale:** {}\n\n",
        scorecard.rationale.trim()
    ));
    out
}

/// Heuristically evaluate a research report from its assembled content and
/// source list. No LLM call is required, so the scorecard is deterministic and
/// cheap to produce.
#[must_use]
pub fn evaluate_report(
    topic: &str,
    brief: Option<&str>,
    summary: &str,
    findings: &[String],
    sources: &[Source],
    output_format: &OutputFormat,
) -> EvaluationScorecard {
    let query = brief.unwrap_or(topic);
    if query.trim().is_empty() {
        return EvaluationScorecard::failed("no topic or brief provided for evaluation");
    }

    let query_lower = query.to_lowercase();
    let query_words: Vec<&str> = query_lower
        .split_whitespace()
        .filter(|w| w.len() > 3)
        .collect();

    let body_text = format!("{}\n{}", summary, findings.join("\n")).to_lowercase();

    // Quality: summary + findings presence and length.
    let quality = {
        let mut score = 20u32;
        if !summary.trim().is_empty() {
            score += 20;
        }
        let total_finding_len: usize = findings.iter().map(|f| f.len()).sum();
        let avg_finding_len = if findings.is_empty() {
            0
        } else {
            total_finding_len / findings.len()
        };
        if !findings.is_empty() {
            score += 20;
        }
        if avg_finding_len > 80 {
            score += 20;
        }
        if findings.iter().any(|f| f.contains("**Headline:**")) {
            score += 20;
        }
        score.min(100)
    };

    // Relevance: overlap between query words and body text.
    let relevance = if query_words.is_empty() {
        50
    } else {
        let matches = query_words
            .iter()
            .filter(|w| body_text.contains(*w))
            .count();
        let ratio = (matches as f64 / query_words.len() as f64).min(1.0);
        ratio.mul_add(80.0, 20.0) as u32
    };

    // Groundedness: presence of citation markers and sources.
    let citation_count = std::iter::once(summary)
        .chain(findings.iter().map(|s| s.as_str()))
        .flat_map(|s| s.matches("[#"))
        .count();
    let groundedness = {
        let mut score = 20u32;
        if !sources.is_empty() {
            score += 30;
        }
        if sources.len() >= 3 {
            score += 20;
        }
        if citation_count > 0 {
            score += 30;
        }
        score.min(100)
    };

    // Completeness: expected content for the format plus source count.
    let completeness = {
        let mut score = 20u32;
        if !summary.trim().is_empty() {
            score += 20;
        }
        if !findings.is_empty() {
            score += 20;
        }
        match output_format {
            OutputFormat::ComparisonTable => {
                if body_text.contains("## comparison table") {
                    score += 10;
                }
                if body_text.contains("## entity profiles") {
                    score += 10;
                }
            }
            OutputFormat::Imrad => {
                if body_text.contains("## abstract") || body_text.contains("## introduction") {
                    score += 20;
                }
            }
            _ => {
                score += 20;
            }
        }
        if sources.len() >= 3 {
            score += 10;
        }
        if sources.len() >= 5 {
            score += 10;
        }
        score.min(100)
    };

    // Structure: finding labels and readability markers.
    let structure = {
        let mut score = 20u32;
        let labels = [
            "**Headline:**",
            "**Observation:**",
            "**Analysis:**",
            "**Implication:**",
        ];
        for label in labels {
            if findings.iter().any(|f| f.contains(label)) {
                score += 15;
            }
        }
        if findings.iter().any(|f| f.contains("**Sources:**")) {
            score += 20;
        }
        score.min(100)
    };

    let rationale = format!(
        "Evaluated against topic '{topic}': {findings_count} findings, {sources_count} sources, {citation_count} citation markers. Quality reflects narrative depth; relevance measures topic overlap; groundedness depends on citations; completeness checks expected content and sources; structure checks finding labels.",
        findings_count = findings.len(),
        sources_count = sources.len(),
    );

    EvaluationScorecard::new(
        quality,
        relevance,
        groundedness,
        completeness,
        structure,
        rationale,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn web_source(url: &str) -> Source {
        Source::Web {
            url: url.into(),
            title: "Source".into(),
            captured_at: chrono::Utc::now(),
            published_at: None,
            body_path: std::path::PathBuf::from("sources/web-01.md"),
            body: "body".into(),
            relevance: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "duckduckgo".into(),
            content_type: None,
            page_type: None,
            media_type: "page".into(),
            language: None,
            oa_recovery: None,
            author: None,
        }
    }

    #[test]
    fn failed_when_no_topic_or_brief() {
        let card = evaluate_report("", None, "", &[], &[], &OutputFormat::Report);
        assert!(card.is_failed());
        assert!(card.error.as_deref().unwrap().contains("no topic"));
    }

    #[test]
    fn groundedness_rewards_citations_and_sources() {
        let finding = "Rust async is useful [#1].".to_string();
        let sources = vec![web_source("https://example.com")];
        let card = evaluate_report(
            "Rust async",
            None,
            "Summary.",
            &[finding],
            &sources,
            &OutputFormat::Report,
        );
        assert!(card.groundedness >= 50);
        assert!(card.overall > 0);
    }

    #[test]
    fn structure_rewards_finding_labels() {
        let finding = "**Headline:** Important\n\n**Observation:** it works.".to_string();
        let card = evaluate_report(
            "Rust async",
            None,
            "Summary.",
            &[finding],
            &[],
            &OutputFormat::Report,
        );
        assert!(card.structure > 20);
    }

    #[test]
    fn render_scorecard_includes_all_dimensions() {
        let card = EvaluationScorecard::new(80, 70, 90, 60, 75, "Looks good.");
        let rendered = render_scorecard(&card);
        assert!(rendered.contains("## Self-Evaluation Scorecard"));
        assert!(rendered.contains("| Quality | 80/100 |"));
        assert!(rendered.contains("| **Overall** | **75/100** |"));
        assert!(rendered.contains("Looks good."));
    }

    #[test]
    fn render_failed_scorecard_surfaces_error() {
        let card = EvaluationScorecard::failed("something went wrong");
        let rendered = render_scorecard(&card);
        assert!(rendered.contains("something went wrong"));
    }
}
