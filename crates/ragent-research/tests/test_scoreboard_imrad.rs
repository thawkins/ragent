//! Integration tests for the Corpus Quality Scoreboard insertion into the
//! IMRaD layout (`assemble_imrad_body`) at the FR-011 position
//! (spec `corpusAnalysis`, task T-004, requirements FR-011 / FR-012 plus the
//! scoreboard content requirements the placement exposes).
//!
//! Mirrors `test_scoreboard_report.rs` with `OutputFormat::Imrad` and
//! `## Abstract` as the first body section.

use chrono::{TimeZone, Utc};
use ragent_research::OutputFormat;
use ragent_research::cite_checker::CitationCheckResult;
use ragent_research::contradiction::{ContradictionClaim, ContradictionEdge, ContradictionGraph};
use ragent_research::corpus_critic::CorpusCriticReport;
use ragent_research::document::{ResearchDocument, assemble_document};
use ragent_research::item::ResearchItem;
use ragent_research::research_name::ResearchName;
use ragent_research::source::Source;
use ragent_research::synthesis::SynthesisAudit;
use std::path::PathBuf;

/// Build a minimal [`ResearchItem`] for tests.
fn sample_item() -> ResearchItem {
    let name = ResearchName::new("scoreboard-imrad").expect("valid name");
    ResearchItem::new(name, "Scoreboard Test", "scoreboard verification")
}

/// Build an empty IMRaD-layout [`ResearchDocument`].
fn empty_doc(item: ResearchItem) -> ResearchDocument {
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
        output_format: OutputFormat::Imrad,
        brief: None,
        comparison_table: None,
        evaluation_scorecard: None,
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

/// Corpus critic report pre-configured for the example in the spec (74, pass).
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

fn scoreboard_index(body: &str) -> usize {
    body.find("## Corpus Quality Scoreboard")
        .expect("scoreboard section must be present")
}

#[test]
fn test_imrad_scoreboard_placed_after_title_before_abstract() {
    let mut item = sample_item();
    item.sources = vec![web_source("example.com", "High", "body", None)];
    let doc = empty_doc(item);
    let assembled = assemble_document(&doc);

    let title_pos = assembled.body.find("# Title:").expect("title present");
    let sb_pos = scoreboard_index(&assembled.body);
    let abstract_pos = assembled
        .body
        .find("## Abstract")
        .expect("Abstract present");
    assert!(
        title_pos < sb_pos && sb_pos < abstract_pos,
        "scoreboard must sit between the title ({title_pos}) and Abstract ({abstract_pos}), got {sb_pos}"
    );
}

#[test]
fn test_imrad_scoreboard_score_line_grade_and_meter_from_critic() {
    let mut item = sample_item();
    item.sources = vec![web_source("example.com", "High", "body", None)];
    let mut doc = empty_doc(item);
    doc.corpus_critic = Some(sample_critic());
    let assembled = assemble_document(&doc);

    let body = &assembled.body[assembled.body.find("# Title:").unwrap()..];
    let sb = &body[scoreboard_index(body)..body.find("## Abstract").unwrap()];
    assert!(
        sb.contains("Quality: **74/100** - Grade B (Good)"),
        "score line missing from scoreboard: {sb}"
    );
    // FR-003: 20-cell meter in a fenced block.
    assert!(
        sb.contains("```\n[###############-----]  74/100\n```"),
        "meter block missing: {sb}"
    );
    // FR-016: ASCII only.
    assert!(sb.is_ascii(), "scoreboard must be ASCII-only: {sb}");
}

#[test]
fn test_imrad_scoreboard_critic_subscore_line() {
    let mut item = sample_item();
    item.sources = vec![web_source("example.com", "High", "body", None)];
    let mut doc = empty_doc(item);
    doc.corpus_critic = Some(sample_critic());
    let assembled = assemble_document(&doc);

    assert!(
        assembled
            .body
            .contains("- Critic: pass (coverage 60 | evidence 80 | balance 70 | tension 100)"),
        "critic subscore line missing: {}",
        &assembled.body[scoreboard_index(&assembled.body)..]
    );
}

#[test]
fn test_imrad_scoreboard_synthesis_audit_fallback() {
    let mut item = sample_item();
    item.sources = vec![web_source("example.com", "High", "body", None)];
    let mut doc = empty_doc(item);
    doc.synthesis_audit = Some(SynthesisAudit {
        overall_score: 82,
        recommendation: "proceed".into(),
        ..SynthesisAudit::empty()
    });
    let assembled = assemble_document(&doc);

    assert!(
        assembled
            .body
            .contains("Quality: **82/100** - Grade A (Excellent)"),
        "FR-006 audit-derived score line missing: {}",
        &assembled.body[scoreboard_index(&assembled.body)..]
    );
}

#[test]
fn test_imrad_scoreboard_not_graded_when_no_scores() {
    let mut item = sample_item();
    item.sources = vec![web_source("example.com", "High", "body", None)];
    let doc = empty_doc(item);
    let assembled = assemble_document(&doc);

    let sb = &assembled.body
        [scoreboard_index(&assembled.body)..assembled.body.find("## Abstract").unwrap()];
    assert!(
        sb.contains("Quality: Not graded"),
        "FR-007 not-graded line missing: {sb}"
    );
    assert!(
        !sb.contains("```"),
        "FR-007: meter block must not render when not graded: {sb}"
    );
}

#[test]
fn test_imrad_scoreboard_source_facts_line() {
    let mut item = sample_item();
    item.sources = vec![
        web_source(
            "example.com",
            "Very high",
            "body",
            Some(Utc.with_ymd_and_hms(2019, 5, 1, 0, 0, 0).unwrap()),
        ),
        web_source(
            "other.org",
            "Medium",
            "body",
            Some(Utc.with_ymd_and_hms(2025, 1, 15, 0, 0, 0).unwrap()),
        ),
        web_source("undated.io", "High", "body", None),
    ];
    let mut doc = empty_doc(item);
    doc.summary = "Claim [#1] and claim [#2] and claim [#3].".into();
    let assembled = assemble_document(&doc);

    let sb = &assembled.body
        [scoreboard_index(&assembled.body)..assembled.body.find("## Abstract").unwrap()];
    // 3 gathered, 3 cited in-range, 3 with bodies, 3 distinct domains,
    // average relevance (8+5+7)/3 = 6.7.
    assert!(
        sb.contains("- Sources: 3 gathered | 3 cited | 3 full text | 3 distinct domains | 6.7/8 average relevance"),
        "FR-004 source-facts line missing/malformed: {sb}"
    );
    assert!(
        sb.contains("- Cited date span: 2019-2025 (1 undated)"),
        "FR-004 cited date span missing: {sb}"
    );
}

#[test]
fn test_imrad_scoreboard_local_only_omits_domains_and_relevance() {
    let name = ResearchName::new("scoreboard-imrad-local").expect("valid name");
    let mut item = ResearchItem::new(name, "Scoreboard Test", "local only");
    item.sources = vec![Source::Local {
        path: "src/lib.rs".into(),
        kind: ragent_research::source::LocalSourceKind::InProject,
        captured_at: Utc::now(),
        body_path: PathBuf::from("sources/local-01.md"),
        relevance: "High".into(),
        body: "fn main() {}".into(),
    }];
    let doc = empty_doc(item);
    let assembled = assemble_document(&doc);

    let sb = &assembled.body
        [scoreboard_index(&assembled.body)..assembled.body.find("## Abstract").unwrap()];
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
        "source facts line malformed: {sb}"
    );
}

#[test]
fn test_imrad_scoreboard_contradictions_and_citation_check_line() {
    let mut item = sample_item();
    item.sources = vec![web_source("example.com", "High", "It is mortal.", None)];
    let mut doc = empty_doc(item);
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
    let assembled = assemble_document(&doc);

    let sb = &assembled.body
        [scoreboard_index(&assembled.body)..assembled.body.find("## Abstract").unwrap()];
    assert!(
        sb.contains("- Contradictions: 1 edges (strongest 78/100) | Citation check: passed"),
        "FR-009/FR-010 tension/citation line missing: {sb}"
    );
}

#[test]
fn test_imrad_scoreboard_omitted_for_empty_skeleton() {
    let doc = empty_doc(sample_item());
    let assembled = assemble_document(&doc);
    assert!(
        !assembled.body.contains("## Corpus Quality Scoreboard"),
        "FR-001: scoreboard must not render when no artifact is available"
    );
}

#[test]
fn test_imrad_data_quality_summary_untouched_by_scoreboard() {
    // FR-012: with QA artifacts present, the detailed Data Quality & Consistency
    // subsection still renders inside Discussion, with the same heading and
    // verdict row. (The Contradiction Graph renders in the CORPA.md companion
    // payload; the graph artifact is supplied so the Data Quality metrics row
    // has data.)
    let mut item = sample_item();
    item.sources = vec![web_source("example.com", "High", "body", None)];
    let mut doc = empty_doc(item);
    doc.corpus_critic = Some(sample_critic());
    doc.contradiction_graph = Some(ContradictionGraph::empty());
    let assembled = assemble_document(&doc);

    let discussion_pos = assembled.body.find("## Discussion").unwrap();
    let dq_pos = assembled
        .body
        .find("### Data Quality & Consistency")
        .expect("FR-012: Data Quality & Consistency subsection must still render");
    // The detailed Contradiction Graph table moved to CORPA.md; RESEARCH.md
    // must no longer carry the subsection.
    let contradiction_pos = assembled
        .corpa
        .find("## Contradiction Graph")
        .expect("CORPA.md must carry the Contradiction Graph section");
    assert!(
        !assembled.body.contains("### Contradiction Graph"),
        "IMRaD RESEARCH.md must no longer carry the Contradiction Graph subsection"
    );
    assert!(
        discussion_pos < dq_pos,
        "FR-012: Data Quality placement changed (discussion {discussion_pos}, dq {dq_pos})"
    );
    assert!(
        !assembled.corpa.is_empty() && contradiction_pos > 0,
        "CORPA.md Contradiction Graph placement changed"
    );
    assert!(
        assembled.body.contains("| Corpus critic | 74/100 (pass) |"),
        "FR-012: Data Quality critic row missing"
    );
    assert!(
        assembled.corpa.contains("## Corpus Critic"),
        "CORPA.md must carry the Corpus Critic section"
    );
}
