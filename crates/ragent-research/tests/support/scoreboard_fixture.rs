//! Shared fixtures for the Corpus Quality Scoreboard integration tests.
//!
//! These helpers were previously copy-pasted across `test_scoreboard_report.rs`,
//! `test_scoreboard_imrad.rs`, and `test_scoreboard_reductions.rs`. They now have
//! a single definition here (code-audit T-606).
//!
//! The module is compiled into each test binary via
//! `#[path = "support/scoreboard_fixture.rs"] mod scoreboard_fixture;`, so only
//! the helpers a given test actually uses are flagged, and the others are
//! silenced by `#![allow(dead_code)]`.

#![allow(dead_code)]

use chrono::{TimeZone, Utc};
use ragent_research::OutputFormat;
use ragent_research::cite_checker::CitationCheckResult;
use ragent_research::contradiction::{ContradictionClaim, ContradictionEdge, ContradictionGraph};
use ragent_research::corpus_critic::CorpusCriticReport;
use ragent_research::document::ResearchDocument;
use ragent_research::item::ResearchItem;
use ragent_research::research_name::ResearchName;
use ragent_research::source::Source;
use std::path::PathBuf;

/// Build a minimal [`ResearchItem`] for tests.
pub fn sample_item() -> ResearchItem {
    let name = ResearchName::new("scoreboard-test").expect("valid name");
    ResearchItem::new(name, "Scoreboard Test", "scoreboard verification")
}

/// Build an empty [`ResearchDocument`] in the given output format.
pub fn empty_doc(item: ResearchItem, output_format: OutputFormat) -> ResearchDocument {
    ResearchDocument {
        item,
        summary: String::new(),
        findings: Vec::new(),
        top_implications: Vec::new(),
        cross_references: Vec::new(),
        open_questions: Vec::new(),
        concepts: None,
        contradiction_graph: None,
        loci: None,
        depth_investigation: None,
        evidence_digest: None,
        triple_draft: None,
        cross_locus_reconcile: None,
        source_tensions: None,
        synthesis_audit: None,
        template_body: None,
        corpus_critic: None,
        gap_fetch: None,
        surgical_patch: None,
        cite_check: None,
        polish: None,
        readability_audit: None,
        decomposed_queries: Vec::new(),
        output_format,
        brief: None,
        comparison_table: None,
        evaluation_scorecard: None,
        provider_stats: None,
    }
}

/// Build a `Source::Web` with controlled relevance, body, and date fields.
pub fn web_source(
    domain: &str,
    relevance: &str,
    body: &str,
    published_at: Option<chrono::DateTime<Utc>>,
) -> Source {
    Source::Web {
        url: format!("https://{domain}/article"),
        title: format!("Article on {domain}"),
        captured_at: Utc::now(),
        published_at,
        body_path: PathBuf::from("sources/web-01.md"),
        body: body.to_string(),
        relevance: relevance.to_string(),
        search_tool: "mf_search".into(),
        search_engine: "openalex".into(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
        language: None,
        oa_recovery: None,
        author: None,
    }
}

/// Corpus critic report pre-configured for the example in the spec (74, pass).
pub fn sample_critic() -> CorpusCriticReport {
    CorpusCriticReport {
        score: 74,
        coverage_score: 60,
        evidence_score: 80,
        balance_score: 70,
        tension_score: 100,
        issues: Vec::new(),
        gaps: Vec::new(),
        recommendations: Vec::new(),
        contested_ratio: 0,
        shallow_dimensions: Vec::new(),
        isolated_sources: Vec::new(),
        passed: true,
    }
}

/// Byte offset of the `## Corpus Quality Scoreboard` section start.
pub fn scoreboard_index(body: &str) -> usize {
    body.find("## Corpus Quality Scoreboard")
        .expect("scoreboard section must be present")
}

/// The scoreboard section as a string, up to the next `next_section` heading
/// (or end of body).
pub fn scoreboard_slice(body: &str, next_section: &str) -> String {
    let start = scoreboard_index(body);
    let end = body[start + 1..]
        .find(next_section)
        .map(|offset| start + 1 + offset)
        .unwrap_or(body.len());
    body[start..end].to_string()
}

/// An empty citation check, the "passed with no findings" result the
/// FR-010 scoreboard line renders.
pub fn sample_cite_check() -> CitationCheckResult {
    CitationCheckResult::empty()
}

/// A single in-project `Source::Local`, for the local-only reduction cases.
pub fn local_source() -> Source {
    Source::Local {
        path: "src/lib.rs".into(),
        kind: ragent_research::source::LocalSourceKind::InProject,
        captured_at: Utc::now(),
        body_path: PathBuf::from("sources/local-01.md"),
        relevance: "High".into(),
        body: "fn main() {}".into(),
    }
}

/// A contradiction graph with one 78/100 edge plus an empty citation check,
/// the combination the FR-009/FR-010 scoreboard line renders.
pub fn sample_contradiction_graph() -> ContradictionGraph {
    let claim_a = ContradictionClaim {
        text: "Claim A".into(),
        source_index: 1,
        source_kind: "web".into(),
        source_path: "https://example.com/article".into(),
    };
    let claim_b = ContradictionClaim {
        text: "Claim B".into(),
        source_index: 1,
        source_kind: "web".into(),
        source_path: "https://example.com/article".into(),
    };
    ContradictionGraph {
        edges: vec![ContradictionEdge {
            claim_a,
            claim_b,
            dimension: "mortality".into(),
            note: "conflicting claims".into(),
            strength: 78,
        }],
    }
}

/// All artifacts set, so a full-layout scoreboard would show every line.
pub fn fully_populated(doc: &mut ResearchDocument) {
    doc.corpus_critic = Some(sample_critic());
    doc.contradiction_graph = Some(sample_contradiction_graph());
    doc.cite_check = Some(CitationCheckResult::empty());
}

/// A `2019-05-01` UTC timestamp.
pub fn ts_2019() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2019, 5, 1, 0, 0, 0).unwrap()
}

/// A `2025-01-15` UTC timestamp.
pub fn ts_2025() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2025, 1, 15, 0, 0, 0).unwrap()
}
