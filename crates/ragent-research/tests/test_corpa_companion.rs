//! Tests for the per-research `CORPA.md` companion document (spec
//! corpusAnalysis follow-up): the QA render sections that used to inline at
//! the bottom of `RESEARCH.md` now live in `CORPA.md`, and the references
//! table is copied there as a `## Sources Reference` section.

use chrono::Utc;
use ragent_research::{
    AssembledDocument, ContradictionClaim, ContradictionEdge, ContradictionGraph, OutputFormat,
    ResearchDocument, ResearchItem, ResearchName, Source, assemble_document, render_corpa_skeleton,
    render_skeleton,
};
use std::path::PathBuf;

fn sample_name() -> ResearchName {
    ResearchName::new("corpa-companion").expect("name must validate")
}

fn sample_item() -> ResearchItem {
    ResearchItem::new(
        sample_name(),
        "CORPA Companion",
        "companion document layout",
    )
}

fn base_document() -> ResearchDocument {
    ResearchDocument {
        item: sample_item(),
        summary: "Summary.".into(),
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
        corpus_critic: None,
        gap_fetch: None,
        surgical_patch: None,
        cite_check: None,
        polish: None,
        readability_audit: None,
        template_body: None,
        brief: None,
        decomposed_queries: Vec::new(),
        output_format: OutputFormat::Report,
        comparison_table: None,
        evaluation_scorecard: None,
    }
}

fn web_source(index: usize) -> Source {
    Source::Web {
        url: format!("https://example.test/{index}"),
        title: format!("Source {index}"),
        captured_at: Utc::now(),
        published_at: None,
        body_path: PathBuf::new(),
        body: format!("body {index}"),
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

#[test]
fn test_corpa_carries_qa_sections_that_have_artifacts() {
    // Only the contradiction graph artifact is supplied, so CORPA.md renders
    // its section and omits the ones whose artifacts were not produced
    // (light-tier runs keep the companion file short).
    let mut doc = base_document();
    doc.item.add_source(web_source(1));
    doc.item.add_source(web_source(2));

    let mut graph = ContradictionGraph::empty();
    graph.add_edge(ContradictionEdge {
        claim_a: ContradictionClaim::from_source("claims better", 1, &doc.item.sources[0]),
        claim_b: ContradictionClaim::from_source("claims worse", 2, &doc.item.sources[1]),
        dimension: "performance".into(),
        note: "opposing performance claims".into(),
        strength: 70,
    });
    doc.contradiction_graph = Some(graph);
    let AssembledDocument { body, corpa, .. } = assemble_document(&doc);

    for section in [
        "## Contradiction Graph",
        "## Loci Analysis",
        "## Depth Investigation",
        "## Cross-Locus Reconcile",
        "## Source Tensions",
        "## Synthesis Audit",
        "## Corpus Critic",
    ] {
        assert!(
            !body.contains(section),
            "RESEARCH.md must not carry `{section}` anymore"
        );
    }
    assert!(
        corpa.contains("## Contradiction Graph"),
        "CORPA.md must carry the produced artifact section"
    );
    assert!(
        !corpa.contains("## Loci Analysis")
            && !corpa.contains("## Depth Investigation")
            && !corpa.contains("## Cross-Locus Reconcile")
            && !corpa.contains("## Source Tensions")
            && !corpa.contains("## Synthesis Audit")
            && !corpa.contains("## Corpus Critic"),
        "CORPA.md must omit sections without artifacts: {corpa}"
    );

    let cg = corpa.find("## Contradiction Graph").expect("graph section");
    assert!(
        corpa[cg..].contains("opposing performance claims"),
        "edge note must render in CORPA.md"
    );
}

#[test]
fn test_corpa_sources_reference_copies_references_table() {
    let mut doc = base_document();
    doc.item.add_source(web_source(1));
    doc.item.add_source(web_source(2));

    let AssembledDocument { body, corpa, .. } = assemble_document(&doc);

    assert!(
        corpa.contains("## Sources Reference\n\n"),
        "CORPA.md must carry a Sources Reference section:\n{corpa}"
    );
    // The references table rows resolve in both documents.
    for idx in 1..=2 {
        let row = format!("| {idx} | web |");
        assert!(
            corpa.contains(&row),
            "CORPA.md sources reference missing row {idx}"
        );
        assert!(
            body.contains(&row),
            "RESEARCH.md references index row {idx}"
        );
    }
    assert!(
        corpa.contains("## Sources Reference") && corpa.contains("https://example.test/1"),
        "CORPA.md sources reference must list the gathered sources"
    );
}

#[tokio::test]
async fn test_corpa_manager_create_writes_skeleton_file() {
    let tmp = tempfile::TempDir::new().unwrap();
    let manager = ragent_research::ResearchManager::new(tmp.path());
    let item = manager
        .create("corpa-skel", "CORPA Skeleton Item", "skeleton layout")
        .await
        .expect("create succeeds");
    assert_eq!(item.name.as_str(), "corpa-skel");

    let corpa_path = tmp.path().join("corpa-skel").join("CORPA.md");
    let corpa = std::fs::read_to_string(&corpa_path).expect("CORPA.md written at create");
    assert!(corpa.starts_with("# Corpus Analysis Companion"));
    assert!(corpa.contains("## Sources Reference"));
    assert!(corpa.contains("No sources captured"));
    assert!(!corpa.contains("## Contradiction Graph"));
}

#[tokio::test]
async fn test_corpa_manager_write_document_updates_companion() {
    let tmp = tempfile::TempDir::new().unwrap();
    let manager = ragent_research::ResearchManager::new(tmp.path());
    let name = ResearchName::new("corpa-write").unwrap();
    manager
        .create("corpa-write", "CORPA Write", "write path")
        .await
        .expect("create succeeds");

    let mut doc = base_document();
    doc.item = ResearchItem::new(name, "CORPA Write", "write path");
    doc.item.add_source(web_source(1));
    let mut graph = ContradictionGraph::empty();
    graph.add_edge(ContradictionEdge {
        claim_a: ContradictionClaim::from_source("claims better", 1, &doc.item.sources[0]),
        claim_b: ContradictionClaim::from_source("claims worse", 1, &doc.item.sources[0]),
        dimension: "performance".into(),
        note: "opposing performance claims".into(),
        strength: 60,
    });
    doc.contradiction_graph = Some(graph);

    manager.write_document(&doc).await.expect("write succeeds");

    // RESEARCH.md no longer carries the moved QA sections.
    let research = std::fs::read_to_string(tmp.path().join("corpa-write").join("RESEARCH.md"))
        .expect("RESEARCH.md written");
    assert!(!research.contains("## Contradiction Graph"));
    assert!(!research.contains("## Source Tensions"));

    // CORPA.md carries the moved sections and the Sources Reference copy.
    let corpa = std::fs::read_to_string(tmp.path().join("corpa-write").join("CORPA.md"))
        .expect("CORPA.md written");
    assert!(corpa.contains("## Contradiction Graph"));
    assert!(corpa.contains("opposing performance claims"));
    assert!(corpa.contains("## Sources Reference"));
    assert!(corpa.contains("https://example.test/1"));
}
#[test]
fn test_corpa_imrad_and_report_share_identical_companion_body() {
    let mut report = base_document();
    report.item.add_source(web_source(1));
    let mut imrad = base_document();
    imrad.item = report.item.clone();
    imrad.output_format = OutputFormat::Imrad;

    let report_corpa = assemble_document(&report).corpa;
    let imrad_corpa = assemble_document(&imrad).corpa;
    assert_eq!(
        report_corpa, imrad_corpa,
        "CORPA.md must be layout-independent"
    );
}

#[test]
fn test_corpa_skeleton_matches_document_format() {
    let report = render_skeleton(
        &sample_name(),
        "CORPA Skeleton",
        "topic",
        OutputFormat::Report,
    );
    assert!(report.contains("## References Index"));

    let corpa = render_corpa_skeleton(
        &sample_name(),
        "CORPA Skeleton",
        "topic",
        OutputFormat::Report,
    );
    assert!(corpa.starts_with("# Corpus Analysis Companion"));
    assert!(corpa.contains("## Sources Reference"));
    assert!(!corpa.contains("## Contradiction Graph"));
}
