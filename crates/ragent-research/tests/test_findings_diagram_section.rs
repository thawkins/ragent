//! rsearchdiag T-013 / FR-001 / FR-002 / FR-012.
//!
//! Integration test verifying that [`assemble_document`] emits the
//! `## Findings Relationship Diagram` section in every assembled
//! `RESEARCH.md`, positioned between `## Findings` and
//! `## In-Project Cross-References`, and that the addition does not remove or
//! reorder any of the existing FR-010 sections.
//!
//! - **FR-001** — the `## Findings Relationship Diagram` section is present.
//! - **FR-002** — the section sits immediately after `## Findings` and before
//!   `## In-Project Cross-References`.
//! - **FR-012** — every section in [`REQUIRED_SECTIONS`] is still present and
//!   appears in its canonical order; none were dropped or rearranged.

use ragent_research::OutputFormat;
use ragent_research::document::{REQUIRED_SECTIONS, ResearchDocument, assemble_document};
use ragent_research::item::ResearchItem;
use ragent_research::research_name::ResearchName;

/// Build a [`ResearchDocument`] with three findings whose
/// **Cross-reference / Dependencies** paragraphs reference each other so the
/// diagram contains nodes and at least one edge.
fn doc_with_findings() -> ResearchDocument {
    let name = ResearchName::new("diagram-section").expect("valid name");
    let item = ResearchItem::new(name, "Diagram Section Check", "FR-001 / FR-002 / FR-012");
    let findings = vec![
        "**Headline:** Root finding\n\n\
         **Observation:** the foundational finding others build on.\n\n\
         **Analysis:** a.\n\n\
         **Cross-reference / Dependencies:** No direct dependencies.\n\n\
         **Implication:** i."
            .to_string(),
        "**Headline:** Child finding\n\n\
         **Observation:** builds on the root finding [#1].\n\n\
         **Analysis:** b.\n\n\
         **Cross-reference / Dependencies:** Builds on Finding 1.\n\n\
         **Implication:** j."
            .to_string(),
        "**Headline:** Sibling finding\n\n\
         **Observation:** relates to the root finding.\n\n\
         **Analysis:** c.\n\n\
         **Cross-reference / Dependencies:** Relates to Finding 1.\n\n\
         **Implication:** k."
            .to_string(),
    ];
    ResearchDocument {
        item,
        summary: "Three findings with a dependency graph.".to_string(),
        findings,
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
        corpus_critic: None,
        gap_fetch: None,
        surgical_patch: None,
        cite_check: None,
        polish: None,
        readability_audit: None,
        template_body: None,
        decomposed_queries: Vec::new(),
        output_format: OutputFormat::Report,
    }
}

#[test]
fn assembled_document_contains_findings_relationship_diagram_section() {
    // FR-001: the `## Findings Relationship Diagram` section is present.
    let assembled = assemble_document(&doc_with_findings());
    assert!(
        assembled.body.contains("## Findings Relationship Diagram"),
        "assembled RESEARCH.md must contain the Findings Relationship Diagram section (FR-001): {}",
        assembled.body
    );

    // FR-003: the diagram is a Mermaid flowchart inside a fenced code block.
    assert!(
        assembled.body.contains("```mermaid"),
        "diagram must be a fenced Mermaid block (FR-003)"
    );
    assert!(
        assembled.body.contains("flowchart TD"),
        "diagram must use Mermaid flowchart TD syntax (FR-003)"
    );

    // FR-004: one node per finding (F1, F2, F3) with number — headline labels.
    assert!(assembled.body.contains("F1[\"1 — Root finding\"]"));
    assert!(assembled.body.contains("F2[\"2 — Child finding\"]"));
    assert!(assembled.body.contains("F3[\"3 — Sibling finding\"]"));

    // FR-006: a directed edge exists for the "Builds on Finding 1" dependency.
    assert!(
        assembled.body.contains("F2 --> F1"),
        "edge from referrer F2 to referenced F1 must be present (FR-006)"
    );
}

#[test]
fn diagram_section_is_between_findings_and_cross_references() {
    // FR-002: the `## Findings Relationship Diagram` section is emitted
    // immediately after `## Findings` and before `## In-Project
    // Cross-References`.
    let assembled = assemble_document(&doc_with_findings());
    let body = &assembled.body;

    let findings_idx = body
        .find("## Findings\n")
        .expect("## Findings heading must be present");
    let diagram_idx = body
        .find("## Findings Relationship Diagram")
        .expect("## Findings Relationship Diagram heading must be present");
    let xref_idx = body
        .find("## In-Project Cross-References")
        .expect("## In-Project Cross-References heading must be present");

    assert!(
        findings_idx < diagram_idx,
        "Findings Relationship Diagram must come AFTER ## Findings (FR-002)"
    );
    assert!(
        diagram_idx < xref_idx,
        "Findings Relationship Diagram must come BEFORE ## In-Project Cross-References (FR-002)"
    );

    // No other top-level `## ` section heading (other than the bold finding
    // sub-headings) appears between Findings and the diagram: the diagram is
    // the *immediate* next section.
    let between = &body[findings_idx + "## Findings\n".len()..diagram_idx];
    let unexpected_h2: Vec<&str> = between
        .lines()
        .filter(|line| line.starts_with("## ") && !line.starts_with("## **Finding"))
        .collect();
    assert!(
        unexpected_h2.is_empty(),
        "no top-level section heading may appear between ## Findings and ## Findings Relationship Diagram (FR-002): found: {unexpected_h2:?}"
    );
}

#[test]
fn all_required_sections_present_in_canonical_order() {
    // FR-012: adding the diagram section does not remove or reorder any of
    // the existing FR-010 sections. Every entry in REQUIRED_SECTIONS must
    // appear, in order, as a `## <name>` heading.
    let assembled = assemble_document(&doc_with_findings());
    let body = &assembled.body;

    let mut cursor = 0usize;
    for section in REQUIRED_SECTIONS {
        let heading = format!("## {section}");
        let found = body[cursor..].find(&heading).map(|p| cursor + p);
        let idx = found.unwrap_or_else(|| {
            panic!(
                "required section `## {section}` missing from assembled RESEARCH.md (FR-012): {body}"
            )
        });
        assert!(
            idx >= cursor,
            "section `## {section}` appeared out of order (FR-012)"
        );
        cursor = idx + heading.len();
    }

    // The original eight sections (excluding the new diagram section) must
    // still all be present by name — explicit guard against accidental
    // removal.
    for legacy in [
        "Topic",
        "Search Queries",
        "Executive Summary",
        "Top 10 Implications",
        "Findings",
        "In-Project Cross-References",
        "Open Questions",
        "References Index",
    ] {
        assert!(
            body.contains(&format!("## {legacy}")),
            "legacy FR-010 section `## {legacy}` must still be present (FR-012)"
        );
    }
}

#[test]
fn document_title_and_frontmatter_are_preserved() {
    // FR-012 sanity: the frontmatter and title block are unchanged by the
    // diagram addition.
    let assembled = assemble_document(&doc_with_findings());
    assert!(
        assembled.content.starts_with("---\n"),
        "document must start with a frontmatter block"
    );
    assert!(
        assembled.content.contains("# Title: Diagram Section Check"),
        "document title must be present and unchanged"
    );
}
