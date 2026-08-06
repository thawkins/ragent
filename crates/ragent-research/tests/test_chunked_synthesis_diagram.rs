//! Milestone E-004: verify that diagrams and cross-references still render
//! correctly after chunked synthesis merges partial results.
//!
//! This test constructs an [`AnalysisResult`] that simulates the output of
//! [`merge_chunk_results`] — findings from multiple chunks, cross-references
//! from multiple chunks — and verifies that [`assemble_document`] produces a
//! valid `RESEARCH.md` with:
//!
//! - A `## Findings Relationship Diagram` section with nodes and edges.
//! - A `## In-Project Cross-References` section with all cross-references.
//! - All findings present and numbered contiguously.

use ragent_research::AnalysisResult;
use ragent_research::CrossReference;
use ragent_research::OutputFormat;
use ragent_research::analysis::merge_chunk_results;
use ragent_research::document::{REQUIRED_SECTIONS, ResearchDocument, assemble_document};
use ragent_research::item::ResearchItem;
use ragent_research::research_name::ResearchName;

/// Build a partial [`AnalysisResult`] simulating one chunk's LLM output.
fn chunk_result(
    summary: &str,
    findings: Vec<String>,
    cross_refs: Vec<CrossReference>,
) -> AnalysisResult {
    AnalysisResult {
        summary: summary.to_string(),
        findings,
        cross_references: cross_refs,
        open_questions: Vec::new(),
    }
}

/// Build a merged [`AnalysisResult`] from two simulated chunks, then assemble
/// a [`ResearchDocument`] from it and verify the diagram + cross-reference
/// sections render correctly.
#[test]
fn merged_chunk_results_produce_valid_diagram_and_cross_references() {
    // Chunk 1: findings 1-2 with a dependency edge, one cross-reference.
    let chunk1 = chunk_result(
        "Chunk 1 summary.",
        vec![
            "**Headline:** Root finding\n\n\
             **Observation:** the foundational finding [#1].\n\n\
             **Analysis:** a.\n\n\
             **Cross-reference / Dependencies:** No direct dependencies.\n\n\
             **Implication:** i."
                .to_string(),
            "**Headline:** Child finding\n\n\
             **Observation:** builds on the root finding [#2].\n\n\
             **Analysis:** b.\n\n\
             **Cross-reference / Dependencies:** Builds on Finding 1.\n\n\
             **Implication:** j."
                .to_string(),
        ],
        vec![CrossReference {
            path: "src/lib.rs".to_string(),
            relevance: "main entry".to_string(),
        }],
    );

    // Chunk 2: findings 1 (will be renumbered to 3) with a dependency on
    // Finding 1 from chunk 1, and a second cross-reference.
    let chunk2 = chunk_result(
        "Chunk 2 summary.",
        vec![
            "**Headline:** Third finding\n\n\
             **Observation:** relates to the root finding [#3].\n\n\
             **Analysis:** c.\n\n\
             **Cross-reference / Dependencies:** Relates to Finding 1.\n\n\
             **Implication:** k."
                .to_string(),
        ],
        vec![
            CrossReference {
                path: "src/lib.rs".to_string(),
                relevance: "main entry".to_string(),
            },
            CrossReference {
                path: "src/main.rs".to_string(),
                relevance: "entry point".to_string(),
            },
        ],
    );

    // Merge the two chunks — this is what the LLM engine does after chunked
    // synthesis (Milestone E-002).
    let merged = merge_chunk_results(&[chunk1, chunk2]);

    // The merged result should have 3 findings (2 + 1), renumbered 1-3.
    assert_eq!(merged.findings.len(), 3);

    // Cross-references should be deduplicated (src/lib.rs appears in both
    // chunks but should only appear once in the merged result).
    assert_eq!(merged.cross_references.len(), 2);

    // Build a ResearchDocument from the merged result and assemble it.
    let name = ResearchName::new("e004-diagram-check").expect("valid name");
    let item = ResearchItem::new(name, "E-004 Diagram Check", "Milestone E-004");
    let doc = ResearchDocument {
        item,
        summary: if merged.summary.is_empty() {
            "Merged summary.".to_string()
        } else {
            merged.summary
        },
        findings: merged.findings,
        cross_references: merged.cross_references,
        open_questions: merged.open_questions,
        template_body: None,
        decomposed_queries: Vec::new(),
        output_format: OutputFormat::Report,
    };
    let assembled = assemble_document(&doc);

    // E-004: the Findings Relationship Diagram section must be present.
    assert!(
        assembled.body.contains("## Findings Relationship Diagram"),
        "diagram section must be present after chunked merge: {}",
        assembled.body
    );

    // The diagram must contain Mermaid flowchart syntax.
    assert!(
        assembled.body.contains("```mermaid"),
        "Mermaid fence must be present after chunked merge"
    );
    assert!(
        assembled.body.contains("flowchart TD"),
        "flowchart TD must be present after chunked merge"
    );

    // All three findings should appear as nodes in the diagram.
    assert!(
        assembled.body.contains("F1["),
        "F1 node must be present in diagram after chunked merge"
    );
    assert!(
        assembled.body.contains("F2["),
        "F2 node must be present in diagram after chunked merge"
    );
    assert!(
        assembled.body.contains("F3["),
        "F3 node must be present in diagram after chunked merge"
    );

    // The dependency edge from F2 → F1 (from chunk 1) must survive the merge.
    assert!(
        assembled.body.contains("F2 --> F1"),
        "edge F2 --> F1 must survive chunked merge"
    );

    // E-004: the In-Project Cross-References section must contain both
    // deduplicated cross-references.
    assert!(
        assembled.body.contains("## In-Project Cross-References"),
        "cross-references section must be present after chunked merge"
    );
    assert!(
        assembled.body.contains("src/lib.rs"),
        "src/lib.rs cross-reference must be present after chunked merge"
    );
    assert!(
        assembled.body.contains("src/main.rs"),
        "src/main.rs cross-reference must be present after chunked merge"
    );

    // E-004: all REQUIRED_SECTIONS must still be present and in order.
    let section_positions: Vec<Option<usize>> = REQUIRED_SECTIONS
        .iter()
        .map(|section| assembled.body.find(section))
        .collect();
    for (i, pos) in section_positions.iter().enumerate() {
        assert!(
            pos.is_some(),
            "required section {} must be present after chunked merge",
            REQUIRED_SECTIONS[i]
        );
    }
    // Verify sections are in order.
    let mut last_pos = 0usize;
    for (i, pos) in section_positions.iter().enumerate() {
        let p = pos.unwrap();
        assert!(
            p >= last_pos,
            "required section {} at pos {} must come after previous section at pos {}",
            REQUIRED_SECTIONS[i],
            p,
            last_pos
        );
        last_pos = p;
    }
}
