//! Tests for the `/research create` concept-extraction step (spec
//! researchcluster): the in-memory payload builder, the RESEARCH.md section
//! normalizer, and the `## Concepts` placement above Findings in both
//! document layouts.

use ragent_research::{
    ResearchDocument, ResearchItem, ResearchName, SourceBody, assemble_document,
    build_concepts_payload_from_bodies, concepts_section_for_research,
};
use std::collections::HashMap;

fn body(index: usize, title: &str, text: &str) -> SourceBody {
    SourceBody {
        index,
        kind: "web".to_string(),
        title: title.to_string(),
        path_or_url: format!("https://example.com/{index}"),
        relevance: String::new(),
        body: text.to_string(),
        published_at: None,
        author: None,
    }
}

#[test]
fn test_build_concepts_payload_from_bodies_headers_carry_combined_indices() {
    let bodies = vec![
        body(1, "First Source", "Alpha is important."),
        body(2, "Second Source", "Beta ties the sources together."),
    ];
    let payload = build_concepts_payload_from_bodies(&bodies, 64_000);
    assert!(payload.contains("--- [#1] First Source ---"), "{payload}");
    assert!(payload.contains("--- [#2] Second Source ---"), "{payload}");
    assert!(payload.contains("Alpha is important."));
    assert!(payload.contains("Beta ties the sources together."));
}

#[test]
fn test_build_concepts_payload_from_bodies_truncates_to_budget() {
    let bodies = vec![
        body(1, "Big One", &"a".repeat(500)),
        body(2, "Small Two", "tiny"),
    ];
    // Budget only fits the header plus a small slice of the first body.
    let payload = build_concepts_payload_from_bodies(&bodies, 60);
    assert!(payload.contains("[#1] Big One"), "{payload}");
    assert!(!payload.contains("[#2]"), "{payload}");
    assert!(payload.len() <= 60 + 80, "len={}", payload.len());
}

#[test]
fn test_concepts_section_for_research_strips_h1_and_demotes_headings() {
    let raw = "# Concepts\n\n## 1. Semantic Search\n\n**Definition:** retrieval over corpora.\n\n## 2. Retrieval Quality\n\n**Definition:** measured by recall.\n";
    let section = concepts_section_for_research(raw, &HashMap::new()).expect("sections exist");
    assert!(!section.contains("# Concepts"), "{section}");
    assert!(section.contains("### 1. Semantic Search"), "{section}");
    assert!(section.contains("### 2. Retrieval Quality"), "{section}");
}

#[test]
fn test_concepts_section_for_research_rewrites_web_refs_via_map() {
    // web-01 is the 5th combined source in this scenario.
    let mut map = HashMap::new();
    map.insert(1usize, 5usize);
    let raw = "# Concepts\n\n## 1. Theme\n\n- evidence here (web-01) and ([#7])\n";
    let section = concepts_section_for_research(raw, &map).expect("sections exist");
    assert!(
        section.contains("- evidence here ([#5]) and ([#7])"),
        "{section}"
    );
    assert!(!section.contains("web-"), "{section}");
}

#[test]
fn test_concepts_section_for_research_none_when_no_sections() {
    assert!(concepts_section_for_research("", &HashMap::new()).is_none());
    assert!(
        concepts_section_for_research("# Concepts\n\nno sections here", &HashMap::new()).is_none()
    );
}

fn sample_doc_with_concepts(concepts: Option<&str>) -> ResearchDocument {
    let name = ResearchName::new("concepts-placement").expect("name must validate");
    let mut item = ResearchItem::new(name, "Concepts Placement", "topic");
    item.set_queries(vec!["q1".to_string()]);
    ResearchDocument {
        item,
        summary: "Summary text.".to_string(),
        findings: vec!["Finding one.".to_string()],
        top_implications: Vec::new(),
        cross_references: Vec::new(),
        open_questions: Vec::new(),
        concepts: concepts.map(str::to_string),
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
        output_format: ragent_research::run_config::OutputFormat::Report,
        comparison_table: None,
        evaluation_scorecard: None,
        provider_stats: None,
    }
}

#[test]
fn test_report_layout_renders_concepts_above_findings() {
    let concepts =
        "### 1. Theme A\n\n**Definition:** first.\n\n### 2. Theme B\n\n**Definition:** second.";
    let doc = sample_doc_with_concepts(Some(concepts));
    let assembled = assemble_document(&doc);
    let concepts_pos = assembled
        .body
        .find("## Concepts")
        .expect("concepts present");
    let findings_pos = assembled
        .body
        .find("## Findings")
        .expect("findings present");
    assert!(
        concepts_pos < findings_pos,
        "## Concepts must render above ## Findings"
    );
    assert!(assembled.body.contains("### 1. Theme A"));
    assert!(assembled.body.contains("### 2. Theme B"));
}

#[test]
fn test_report_layout_omits_concepts_when_none() {
    let doc = sample_doc_with_concepts(None);
    let assembled = assemble_document(&doc);
    assert!(!assembled.body.contains("## Concepts"));
}

#[test]
fn test_imrad_layout_renders_concepts_above_findings() {
    let mut doc = sample_doc_with_concepts(Some("### 1. Theme A\n\n**Definition:** first."));
    doc.output_format = ragent_research::run_config::OutputFormat::Imrad;
    let assembled = assemble_document(&doc);
    let concepts_pos = assembled
        .body
        .find("### Concepts")
        .expect("concepts present");
    let findings_pos = assembled
        .body
        .find("### Findings")
        .expect("findings present");
    assert!(
        concepts_pos < findings_pos,
        "### Concepts must render above ### Findings in the IMRaD layout"
    );
}

#[test]
fn test_imrad_layout_omits_concepts_when_none() {
    let mut doc = sample_doc_with_concepts(None);
    doc.output_format = ragent_research::run_config::OutputFormat::Imrad;
    let assembled = assemble_document(&doc);
    assert!(!assembled.body.contains("### Concepts"));
}
