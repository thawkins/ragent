//! Integration tests for the Corpus Quality Scoreboard abbreviated-format and
//! local-only-run reductions (spec `corpusAnalysis`, task T-005, FR-013/FR-014).
//!
//! FR-013: `executive-summary`, `comparison-table`, and `source-bibliography`
//! documents omit the critic-subscore line and the tension/citation line,
//! keeping only the score line, meter bar, and source-facts block.
//! FR-014: local-only runs (zero web sources) omit the distinct-domain count
//! and the average-relevance figures from the source-facts line.

use chrono::Utc;
use ragent_research::OutputFormat;
use ragent_research::cite_checker::CitationCheckResult;
use ragent_research::contradiction::{ContradictionClaim, ContradictionEdge, ContradictionGraph};
use ragent_research::corpus_critic::CorpusCriticReport;
use ragent_research::document::{ResearchDocument, assemble_document};
use ragent_research::item::ResearchItem;
use ragent_research::research_name::ResearchName;
use ragent_research::source::Source;
use std::path::PathBuf;

/// Build a minimal [`ResearchItem`] for tests.
fn sample_item() -> ResearchItem {
    let name = ResearchName::new("scoreboard-reduct").expect("valid name");
    ResearchItem::new(name, "Scoreboard Test", "scoreboard verification")
}

/// Build an empty [`ResearchDocument`] with the given output format.
fn empty_doc(item: ResearchItem, output_format: OutputFormat) -> ResearchDocument {
    ResearchDocument {
        item,
        summary: String::new(),
        findings: Vec::new(),
        top_implications: Vec::new(),
        cross_references: Vec::new(),
        open_questions: Vec::new(),
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
    }
}

/// Build a `Source::Web` with controlled relevance, body, and date fields.
fn web_source(
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
        search_engine: "duckduckgo".into(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
        language: None,
        oa_recovery: None,
        author: None,
    }
}

/// Corpus critic report pre-configured for the spec example (74, pass).
fn sample_critic() -> CorpusCriticReport {
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

fn scoreboard_slice(body: &str, next_section: &str) -> String {
    let start = body
        .find("## Corpus Quality Scoreboard")
        .expect("scoreboard section must be present");
    let end = body[start + 1..]
        .find(next_section)
        .map(|offset| start + 1 + offset)
        .unwrap_or(body.len());
    body[start..end].to_string()
}

/// All artifacts set, so a full-layout scoreboard would show every line.
fn fully_populated(doc: &mut ResearchDocument) {
    doc.corpus_critic = Some(sample_critic());
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
    doc.contradiction_graph = Some(ContradictionGraph {
        edges: vec![ContradictionEdge {
            claim_a,
            claim_b,
            dimension: "mortality".into(),
            note: "conflicting claims".into(),
            strength: 78,
        }],
    });
    doc.cite_check = Some(CitationCheckResult::empty());
}

#[test]
fn test_abbreviated_executive_summary_omits_critic_and_tension_lines() {
    let mut item = sample_item();
    item.sources = vec![web_source("example.com", "High", "body", None)];
    let mut doc = empty_doc(item, OutputFormat::ExecutiveSummary);
    fully_populated(&mut doc);
    let assembled = assemble_document(&doc);

    // Non-Imrad formats all use the report layout, so the scoreboard is
    // followed by the shared `## Topic` section regardless of format.
    let sb = scoreboard_slice(&assembled.body, "## Topic");
    assert!(
        sb.contains("Quality: **74/100** - Grade B (Good)"),
        "FR-013 abbreviated score line missing: {sb}"
    );
    assert!(
        sb.contains("```\n[###############-----]  74/100\n```"),
        "FR-013 abbreviated meter block missing: {sb}"
    );
    assert!(
        sb.contains("- Sources: 1 gathered | 0 cited | 1 full text"),
        "FR-013 abbreviated source-facts line missing: {sb}"
    );
    assert!(
        !sb.contains("Critic:"),
        "FR-013: critic subscore line must be omitted: {sb}"
    );
    assert!(
        !sb.contains("Contradictions:"),
        "FR-013: contradiction count must be omitted: {sb}"
    );
    assert!(
        !sb.contains("Citation check:"),
        "FR-013: citation-check status must be omitted: {sb}"
    );
    assert!(sb.is_ascii(), "scoreboard must remain ASCII-only: {sb}");
}

#[test]
fn test_all_abbreviated_formats_reduce_and_full_formats_do_not() {
    for (format, _) in [
        (OutputFormat::ExecutiveSummary, "## Topic"),
        (OutputFormat::ComparisonTable, "## Topic"),
        (OutputFormat::SourceBibliography, "## Topic"),
    ] {
        let mut item = sample_item();
        item.sources = vec![web_source("example.com", "High", "body", None)];
        let mut doc = empty_doc(item, format);
        fully_populated(&mut doc);
        let body = assemble_document(&doc).body;
        let sb = scoreboard_slice(&body, "## Topic");

        let reduced = !sb.contains("Critic:")
            && !sb.contains("Contradictions:")
            && !sb.contains("Citation check:");
        assert!(
            reduced,
            "FR-013 violated for {}: scoreboard not reduced: {sb}",
            format.as_str()
        );
    }

    // Report and IMRaD keep the full scoreboard for the same input.
    for format in [OutputFormat::Report, OutputFormat::Imrad] {
        let mut item = sample_item();
        item.sources = vec![web_source("example.com", "High", "body", None)];
        let mut doc = empty_doc(item, format);
        fully_populated(&mut doc);
        let sb = scoreboard_slice(&assemble_document(&doc).body, "## Findings");
        assert!(
            sb.contains("- Critic: pass (coverage 60 | evidence 80 | balance 70 | tension 100)"),
            "full layout {format:?} must keep the critic line: {sb}"
        );
        assert!(
            sb.contains("- Contradictions: 1 edges (strongest 78/100) | Citation check: passed"),
            "full layout {format:?} must keep the tension line: {sb}"
        );
    }
}

#[test]
fn test_imrad_keeps_critic_and_tension_lines_despite_artifacts() {
    let mut item = sample_item();
    item.sources = vec![web_source("example.com", "High", "body", None)];
    let mut doc = empty_doc(item, OutputFormat::Imrad);
    fully_populated(&mut doc);
    let sb = scoreboard_slice(&assemble_document(&doc).body, "## Findings");
    assert!(
        sb.contains("- Critic: pass (coverage 60 | evidence 80 | balance 70 | tension 100)"),
        "IMRaD must keep the critic subscore line: {sb}"
    );
    assert!(
        sb.contains("- Contradictions: 1 edges (strongest 78/100) | Citation check: passed"),
        "IMRaD must keep the tension/citation line: {sb}"
    );
}

#[test]
fn test_abbreviated_local_only_reduces_lines_and_facts() {
    let name = ResearchName::new("scoreboard-local-abbrev").expect("valid name");
    let mut item = ResearchItem::new(name, "Scoreboard Test", "local only");
    item.sources = vec![Source::Local {
        path: "src/lib.rs".into(),
        kind: ragent_research::source::LocalSourceKind::InProject,
        captured_at: Utc::now(),
        body_path: PathBuf::from("sources/local-01.md"),
        relevance: "High".into(),
        body: "fn main() {}".into(),
    }];
    let mut doc = empty_doc(item, OutputFormat::ExecutiveSummary);
    doc.corpus_critic = Some(sample_critic());
    doc.contradiction_graph = Some(ContradictionGraph::empty());
    doc.cite_check = Some(CitationCheckResult::empty());
    let assembled = assemble_document(&doc);
    let sb = scoreboard_slice(&assembled.body, "## Topic");
    assert!(
        !sb.contains("Critic:")
            && !sb.contains("Contradictions:")
            && !sb.contains("Citation check:"),
        "FR-013: lines must be suppressed in abbreviated layout: {sb}"
    );
    assert!(
        !sb.contains("distinct domains"),
        "FR-014: domains must be omitted for local-only runs: {sb}"
    );
    assert!(
        !sb.contains("average relevance"),
        "FR-014: average relevance must be omitted for local-only runs: {sb}"
    );
    assert!(
        sb.contains("- Sources: 1 gathered | 0 cited | 1 full text"),
        "abbreviated local-only source facts malformed: {sb}"
    );
    assert!(
        sb.contains("Quality: **74/100** - Grade B (Good)")
            && sb.contains("[###############-----]  74/100"),
        "abbreviated local-only scoreboard must still show score and meter: {sb}"
    );
}
