//! Integration tests for the Corpus Quality Scoreboard abbreviated-format and
//! local-only-run reductions (spec `corpusAnalysis`, task T-005, FR-013/FR-014).
//!
//! FR-013: `executive-summary`, `comparison-table`, and `source-bibliography`
//! documents omit the critic-subscore line and the tension/citation line,
//! keeping only the score line, meter bar, and source-facts block.
//! FR-014: local-only runs (zero web sources) omit the distinct-domain count
//! and the average-relevance figures from the source-facts line.

#[path = "support/scoreboard_fixture.rs"]
mod scoreboard_fixture;

use ragent_research::OutputFormat;
use ragent_research::contradiction::ContradictionGraph;
use ragent_research::document::assemble_document;

use scoreboard_fixture::{
    empty_doc, fully_populated, local_source, sample_cite_check, sample_critic, sample_item,
    scoreboard_slice, web_source,
};

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
    let mut item = sample_item();
    item.sources = vec![local_source()];
    let mut doc = empty_doc(item, OutputFormat::ExecutiveSummary);
    doc.corpus_critic = Some(sample_critic());
    doc.contradiction_graph = Some(ContradictionGraph::empty());
    doc.cite_check = Some(sample_cite_check());
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
