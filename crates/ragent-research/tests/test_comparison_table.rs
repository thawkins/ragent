//! Integration tests for the dedicated `comparison-table` research artifact
//! (specs/opendeepresearch T-011, FR-014 / FR-016).
//!
//! These tests construct a [`ResearchDocument`] directly so no LLM call or
//! gathering pass is required.

use ragent_research::OutputFormat;
use ragent_research::document::{ResearchDocument, assemble_document};
use ragent_research::item::ResearchItem;
use ragent_research::research_name::ResearchName;

fn sample_item() -> ResearchItem {
    let name = ResearchName::new("comparison-table").expect("valid name");
    ResearchItem::new(name, "Comparison Table", "Compare A and B for speed")
}

fn comparison_doc(table: Option<&str>) -> ResearchDocument {
    let mut item = sample_item();
    item.output_format = Some(OutputFormat::ComparisonTable.as_str().to_string());
    item.add_source(ragent_research::source::Source::Web {
        url: "https://example.com/a".into(),
        title: "Source A".into(),
        captured_at: chrono::Utc::now(),
        published_at: None,
        body_path: std::path::PathBuf::from("sources/web-01.md"),
        body: "A is fast.".into(),
        relevance: String::new(),
        search_tool: "mf_search".into(),
        search_engine: "duckduckgo".into(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
        language: None,
        oa_recovery: None,
        author: None,
    });
    ResearchDocument {
        item,
        summary: "A and B were compared.".into(),
        findings: vec!["A is faster than B [#1].".into()],
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
        corpus_critic: None,
        gap_fetch: None,
        surgical_patch: None,
        cite_check: None,
        polish: None,
        readability_audit: None,
        template_body: None,
        brief: Some("Compare A and B.".into()),
        decomposed_queries: vec!["A speed".into(), "B speed".into()],
        output_format: OutputFormat::ComparisonTable,
        comparison_table: table.map(str::to_string),
        evaluation_scorecard: None,
    }
}

#[test]
fn comparison_table_format_renders_comparison_sections() {
    let table = "## Comparison Criteria\n\n- speed\n\n## Comparison Table\n\n| Entity | speed | Profile |\n| --- | --- | --- |\n| A | fast | A is fast. |\n| B | — | — |\n\n## Entity Profiles\n\n### A\n\nA is fast.\n\n### B\n\nNo data.\n";
    let doc = comparison_doc(Some(table));
    let assembled = assemble_document(&doc);

    assert!(
        assembled.body.contains("## Comparison Table"),
        "comparison-table format must contain a Comparison Table section:\n{}",
        assembled.body
    );
    assert!(
        assembled.body.contains("## Entity Profiles"),
        "comparison-table format must contain per-entity profiles:\n{}",
        assembled.body
    );
    assert!(
        assembled.body.contains("## Research Brief"),
        "comparison-table format must render the research brief:\n{}",
        assembled.body
    );
    assert!(
        assembled.body.contains("## Findings"),
        "comparison-table format must still include synthesized findings:\n{}",
        assembled.body
    );
    assert!(
        assembled.body.contains("## References Index"),
        "comparison-table format must include references index:\n{}",
        assembled.body
    );
}

#[test]
fn comparison_table_format_omits_search_queries_section() {
    // FR-014: the comparison-table artifact is shorter; it should not render
    // the full report's Search Queries section.
    let doc = comparison_doc(Some("pre-rendered table"));
    let assembled = assemble_document(&doc);
    assert!(
        !assembled.body.contains("## Search Queries"),
        "comparison-table artifact should not include Search Queries:\n{}",
        assembled.body
    );
}

#[test]
fn comparison_table_format_falls_back_when_no_table_provided() {
    let doc = comparison_doc(None);
    let assembled = assemble_document(&doc);
    assert!(
        assembled
            .body
            .contains("_(no comparison table was produced for this run)_"),
        "comparison-table format should render a fallback when no table is provided:\n{}",
        assembled.body
    );
}

#[test]
fn comparison_table_format_uses_abbreviated_scoreboard() {
    let doc = comparison_doc(Some("pre-rendered table"));
    let assembled = assemble_document(&doc);
    assert!(
        assembled.body.contains("## Corpus Quality Scoreboard"),
        "abbreviated scoreboard should still appear:\n{}",
        assembled.body
    );
    // FR-013: abbreviated formats omit the critic subscore line.
    let scoreboard = &assembled.body[..assembled
        .body
        .find("## Topic")
        .unwrap_or(assembled.body.len())];
    assert!(
        !scoreboard.contains("Critic:"),
        "abbreviated comparison-table scoreboard must omit critic subscore line:\n{scoreboard}"
    );
}

#[test]
fn comparison_table_format_frontmatter_persists_format() {
    let doc = comparison_doc(Some("pre-rendered table"));
    let assembled = assemble_document(&doc);
    assert!(
        assembled
            .frontmatter
            .contains("requested_format: comparison-table"),
        "frontmatter must persist the comparison-table format:\n{}",
        assembled.frontmatter
    );
}
