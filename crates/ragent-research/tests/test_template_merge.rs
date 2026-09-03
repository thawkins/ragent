//! FR-007 / T-006: when a `--template` is supplied, the template body is
//! merged with the standard `RESEARCH.md` sections — it must NOT replace the
//! Findings section or its five required labeled paragraphs.
//!
//! These tests exercise [`ragent_research::document::assemble_document`]
//! directly with a [`ResearchDocument`] that carries a `template_body`, and
//! assert that the assembled output contains BOTH the template's custom
//! content AND the canonical FR-010 sections (Topic, Summary, Findings,
//! Open Questions, References Index) with the Findings section retaining the
//! five required bold labels.

use ragent_research::document::{AssembledDocument, ResearchDocument, assemble_document};
use ragent_research::item::ResearchItem;
use ragent_research::research_name::ResearchName;

/// Build a minimal [`ResearchDocument`] with the supplied template body and
/// one finding that carries all five required labeled paragraphs.
fn doc_with_template(template_body: Option<&str>) -> ResearchDocument {
    let name = ResearchName::new("template-merge").expect("valid name");
    let item = ResearchItem::new(name, "Template Merge", "FR-007 template merge");
    // No sources needed for the assembly-level check.
    let finding = "**Headline:** Observation summary

**Observation:** the template provides extra context [#1].\n\n\
         **Analysis:** the standard finding structure is preserved.\n\n\
         **Cross-reference / Dependencies:** No direct dependencies.\n\n\
         **Implication:** templates augment, they do not replace."
        .to_string();
    ResearchDocument {
        item,
        summary: "Template-merge summary.".to_string(),
        findings: vec![finding],
        top_implications: Vec::new(),
        cross_references: Vec::new(),
        open_questions: vec!["Does the template survive alongside the standard sections?".into()],
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
        template_body: template_body.map(str::to_string),
        brief: None,
        decomposed_queries: Vec::new(),
        output_format: ragent_research::OutputFormat::Report,
        comparison_table: None,
        evaluation_scorecard: None,
    }
}

/// Extract the body (without frontmatter) from an [`AssembledDocument`].
fn body_of(assembled: &AssembledDocument) -> &str {
    &assembled.body
}

#[test]
fn template_is_merged_not_replacing_standard_sections() {
    let template = "# Custom Template\n\n{{title}} — {{topic}} ({{date}})\n\n## Background\n\nCustom background section from the template.\n";
    let doc = doc_with_template(Some(template));
    let assembled = assemble_document(&doc);
    let body = body_of(&assembled);

    // Template content is present (merged in).
    assert!(
        body.contains("Custom Template"),
        "template title should appear in the assembled body"
    );
    assert!(
        body.contains("Custom background section from the template."),
        "template custom section should appear in the assembled body"
    );
    // Placeholders were substituted (FR-020).
    assert!(
        !body.contains("{{title}}"),
        "template placeholders should be substituted, got: {body}"
    );

    // Standard FR-010 sections are STILL present (not replaced by the template).
    assert!(
        body.contains("## Topic"),
        "Topic section must survive merge"
    );
    assert!(
        body.contains("## Executive Summary"),
        "Summary section must survive merge"
    );
    assert!(
        body.contains("## Findings"),
        "Findings section must survive merge — template must not replace it"
    );
    assert!(
        body.contains("## Open Questions"),
        "Open Questions section must survive merge"
    );
    assert!(
        body.contains("## References Index"),
        "References Index section must survive merge"
    );

    // The five required labeled paragraphs remain mandatory inside Findings.
    assert!(body.contains("### **Finding 1** — Observation summary"));
    assert!(body.contains("**Observation:**"));
    assert!(body.contains("**Analysis:**"));
    assert!(body.contains("**Cross-reference / Dependencies:**"));
    assert!(body.contains("**Implication:**"));
}
#[test]
fn no_template_preserves_standard_sections_unchanged() {
    // Regression guard: the default (no template) path still produces all
    // standard sections and the five required finding labels.
    let doc = doc_with_template(None);
    let assembled = assemble_document(&doc);
    let body = body_of(&assembled);

    assert!(!body.contains("Custom Template"));
    assert!(body.contains("## Topic"));
    assert!(body.contains("## Executive Summary"));
    assert!(body.contains("## Findings"));
    assert!(body.contains("### **Finding 1** — Observation summary"));
    assert!(body.contains("**Observation:**"));
    assert!(body.contains("**Analysis:**"));
    assert!(body.contains("**Cross-reference / Dependencies:**"));
    assert!(body.contains("**Implication:**"));
    assert!(body.contains("## References Index"));
}
#[test]
fn template_with_custom_placeholder_section_is_populated_in_addition_to_findings() {
    // A template that defines a custom `## Background` section must appear
    // alongside (not instead of) the standard `## Findings` section.
    let template = "## Background\n\nTemplate-supplied background.\n";
    let doc = doc_with_template(Some(template));
    let assembled = assemble_document(&doc);
    let body = body_of(&assembled);

    assert!(
        body.contains("Template-supplied background."),
        "custom template section content should appear"
    );
    // The standard Findings section must still be present and must contain
    // the five required labeled paragraphs (FR-007: template augments, does
    // not replace).
    let findings_idx = body
        .find("## Findings")
        .expect("## Findings section must be present even with a template");
    let findings_section = &body[findings_idx..];
    assert!(findings_section.contains("### **Finding 1** — Observation summary"));
    assert!(findings_section.contains("**Observation:**"));
    assert!(findings_section.contains("**Analysis:**"));
    assert!(findings_section.contains("**Cross-reference / Dependencies:**"));
    assert!(findings_section.contains("**Implication:**"));
}
