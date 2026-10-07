//! Integration tests for the Corpus Quality Scoreboard insertion into the
//! report layout (`assemble_report_body`) at the FR-011 position
//! (spec `corpusAnalysis`, task T-003, requirements FR-011 / FR-012 plus the
//! scoreboard content requirements the placement exposes).

#[path = "support/scoreboard_fixture.rs"]
mod scoreboard_fixture;

use ragent_research::OutputFormat;
use ragent_research::document::assemble_document;
use ragent_research::synthesis::SynthesisAudit;

use scoreboard_fixture::{
    empty_doc, local_source, sample_cite_check, sample_contradiction_graph, sample_critic,
    sample_item, scoreboard_index, ts_2019, ts_2025, web_source,
};

#[test]
fn test_report_scoreboard_placed_after_title_before_topic() {
    let mut item = sample_item();
    item.sources = vec![web_source("example.com", "High", "body", None)];
    let doc = empty_doc(item, OutputFormat::Report);
    let assembled = assemble_document(&doc);

    let title_pos = assembled.body.find("# Title:").expect("title present");
    let sb_pos = scoreboard_index(&assembled.body);
    let topic_pos = assembled.body.find("## Topic").expect("Topic present");
    assert!(
        title_pos < sb_pos && sb_pos < topic_pos,
        "scoreboard must sit between the title ({title_pos}) and Topic ({topic_pos}), got {sb_pos}"
    );
}

#[test]
fn test_scoreboard_score_line_grade_and_meter_from_critic() {
    let mut item = sample_item();
    item.sources = vec![web_source("example.com", "High", "body", None)];
    let mut doc = empty_doc(item, OutputFormat::Report);
    doc.corpus_critic = Some(sample_critic());
    let assembled = assemble_document(&doc);

    let body = &assembled.body[assembled.body.find("# Title:").unwrap()..];
    let sb = &body[scoreboard_index(body)..body.find("## Topic").unwrap()];
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
fn test_scoreboard_critic_subscore_line() {
    let mut item = sample_item();
    item.sources = vec![web_source("example.com", "High", "body", None)];
    let mut doc = empty_doc(item, OutputFormat::Report);
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
fn test_scoreboard_synthesis_audit_fallback() {
    let mut item = sample_item();
    item.sources = vec![web_source("example.com", "High", "body", None)];
    let mut doc = empty_doc(item, OutputFormat::Report);
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
fn test_scoreboard_not_graded_when_no_scores() {
    let mut item = sample_item();
    item.sources = vec![web_source("example.com", "High", "body", None)];
    let doc = empty_doc(item, OutputFormat::Report);
    let assembled = assemble_document(&doc);

    let sb = &assembled.body
        [scoreboard_index(&assembled.body)..assembled.body.find("## Topic").unwrap()];
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
fn test_scoreboard_source_facts_line() {
    let mut item = sample_item();
    item.sources = vec![
        web_source("example.com", "Very high", "body", Some(ts_2019())),
        web_source("other.org", "Medium", "body", Some(ts_2025())),
        web_source("undated.io", "High", "body", None),
    ];
    let mut doc = empty_doc(item, OutputFormat::Report);
    doc.summary = "Claim [#1] and claim [#2] and claim [#3].".into();
    let assembled = assemble_document(&doc);

    let sb = &assembled.body
        [scoreboard_index(&assembled.body)..assembled.body.find("## Topic").unwrap()];
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
fn test_scoreboard_local_only_omits_domains_and_relevance() {
    let mut item = sample_item();
    item.sources = vec![local_source()];
    let doc = empty_doc(item, OutputFormat::Report);
    let assembled = assemble_document(&doc);

    let sb = &assembled.body
        [scoreboard_index(&assembled.body)..assembled.body.find("## Topic").unwrap()];
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
fn test_scoreboard_contradictions_and_citation_check_line() {
    let mut item = sample_item();
    item.sources = vec![web_source("example.com", "High", "It is mortal.", None)];
    let mut doc = empty_doc(item, OutputFormat::Report);
    doc.contradiction_graph = Some(sample_contradiction_graph());
    doc.cite_check = Some(sample_cite_check());
    let assembled = assemble_document(&doc);

    let sb = &assembled.body
        [scoreboard_index(&assembled.body)..assembled.body.find("## Topic").unwrap()];
    assert!(
        sb.contains("- Contradictions: 1 edges (strongest 78/100) | Citation check: passed"),
        "FR-009/FR-010 tension/citation line missing: {sb}"
    );
}

#[test]
fn test_scoreboard_omitted_for_empty_skeleton() {
    let doc = empty_doc(sample_item(), OutputFormat::Report);
    let assembled = assemble_document(&doc);
    assert!(
        !assembled.body.contains("## Corpus Quality Scoreboard"),
        "FR-001: scoreboard must not render when no artifact is available"
    );
}

#[test]
fn test_data_quality_summary_untouched_by_scoreboard() {
    // FR-012: with QA artifacts present, the detailed Data Quality & Consistency
    // section still renders in its original position (after Top 10 Implications,
    // before Open Questions), with the same heading and verdict line.
    let mut item = sample_item();
    item.sources = vec![web_source("example.com", "High", "body", None)];
    let mut doc = empty_doc(item, OutputFormat::Report);
    doc.corpus_critic = Some(sample_critic());
    let assembled = assemble_document(&doc);

    let implications_pos = assembled.body.find("## Top 10 Implications").unwrap();
    let dq_pos = assembled
        .body
        .find("## Data Quality & Consistency")
        .expect("FR-012: Data Quality & Consistency section must still render");
    let findings_pos = assembled.body.find("## Findings").unwrap();
    assert!(
        implications_pos < dq_pos && dq_pos < findings_pos,
        "FR-012: Data Quality placement changed (implications {implications_pos}, dq {dq_pos}, findings {findings_pos})"
    );
    assert!(
        assembled.body.contains("| Corpus critic | 74/100 (pass) |"),
        "FR-012: Data Quality critic row missing"
    );
}
