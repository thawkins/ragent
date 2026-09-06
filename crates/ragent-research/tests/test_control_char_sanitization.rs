//! Control-character sanitization tests for `assemble_document`.
//!
//! Verifies that control characters (C0/C1) in research document fields
//! (summary, findings, cross-references, open questions, queries) are
//! stripped before rendering into the `RESEARCH.md` body so the file is
//! never corrupted by binary garbage from model output or PDF extraction.

use ragent_research::OutputFormat;
use ragent_research::document::{CrossReference, ResearchDocument, assemble_document};
use ragent_research::item::ResearchItem;
use ragent_research::research_name::ResearchName;

/// Build a minimal [`ResearchItem`] for tests.
fn sample_item() -> ResearchItem {
    let name = ResearchName::new("ctrl-char-test").expect("valid name");
    ResearchItem::new(name, "Control Char Test", "sanitization verification")
}

/// Build a [`ResearchDocument`] with controlled fields, using the report layout.
fn sample_doc(item: ResearchItem) -> ResearchDocument {
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
        brief: None,
        corpus_critic: None,
        gap_fetch: None,
        surgical_patch: None,
        cite_check: None,
        polish: None,
        readability_audit: None,
        decomposed_queries: Vec::new(),
        output_format: OutputFormat::Report,
        comparison_table: None,
        evaluation_scorecard: None,
        provider_stats: None,
    }
}
#[test]
fn assemble_document_strips_control_chars_from_title_and_topic() {
    let item = ResearchItem::new(
        ResearchName::new("ctrl-char-test").expect("valid name"),
        "Title\x01with\x02ctrl",
        "Topic\x07with\x08ctrl",
    );
    let doc = sample_doc(item);

    let assembled = assemble_document(&doc);
    assert!(
        !assembled.body.contains('\x01'),
        "body must not contain 0x01"
    );
    assert!(
        !assembled.body.contains('\x02'),
        "body must not contain 0x02"
    );
    assert!(
        !assembled.body.contains('\x07'),
        "body must not contain 0x07"
    );
    assert!(
        !assembled.body.contains('\x08'),
        "body must not contain 0x08"
    );
}

#[test]
fn assemble_document_strips_control_chars_from_summary() {
    let doc = ResearchDocument {
        item: sample_item(),
        summary: "Summary with \x01 control \x02 chars".to_string(),
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
        brief: None,
        corpus_critic: None,
        gap_fetch: None,
        surgical_patch: None,
        cite_check: None,
        polish: None,
        readability_audit: None,
        decomposed_queries: Vec::new(),
        output_format: OutputFormat::Report,
        comparison_table: None,
        evaluation_scorecard: None,
        provider_stats: None,
    };
    let assembled = assemble_document(&doc);
    assert!(
        !assembled.body.contains('\x01') && !assembled.body.contains('\x02'),
        "body must not contain control chars from summary"
    );
    assert!(assembled.body.contains("Summary with  control  chars"));
}

#[test]
fn assemble_document_strips_control_chars_from_findings() {
    let doc = ResearchDocument {
        item: sample_item(),
        summary: String::new(),
        findings: vec!["Finding with \x01 ctrl \x02 chars".to_string()],
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
        brief: None,
        corpus_critic: None,
        gap_fetch: None,
        surgical_patch: None,
        cite_check: None,
        polish: None,
        readability_audit: None,
        decomposed_queries: Vec::new(),
        output_format: OutputFormat::Report,
        comparison_table: None,
        evaluation_scorecard: None,
        provider_stats: None,
    };
    let assembled = assemble_document(&doc);
    assert!(
        !assembled.body.contains('\x01') && !assembled.body.contains('\x02'),
        "body must not contain control chars from findings"
    );
    assert!(assembled.body.contains("Finding with  ctrl  chars"));
}

#[test]
fn assemble_document_strips_control_chars_from_cross_references() {
    let doc = ResearchDocument {
        item: sample_item(),
        summary: String::new(),
        findings: Vec::new(),
        top_implications: Vec::new(),
        cross_references: vec![CrossReference {
            path: "src/lib.rs".to_string(),
            relevance: "Relevant\x01 because\x02 of X".to_string(),
        }],
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
        brief: None,
        corpus_critic: None,
        gap_fetch: None,
        surgical_patch: None,
        cite_check: None,
        polish: None,
        readability_audit: None,
        decomposed_queries: Vec::new(),
        output_format: OutputFormat::Report,
        comparison_table: None,
        evaluation_scorecard: None,
        provider_stats: None,
    };
    let assembled = assemble_document(&doc);
    assert!(
        !assembled.body.contains('\x01') && !assembled.body.contains('\x02'),
        "body must not contain control chars from cross-references"
    );
    assert!(assembled.body.contains("Relevant because of X"));
}

#[test]
fn assemble_document_strips_control_chars_from_open_questions() {
    let doc = ResearchDocument {
        item: sample_item(),
        summary: String::new(),
        findings: Vec::new(),
        top_implications: Vec::new(),
        cross_references: Vec::new(),
        open_questions: vec!["Question\x01 with \x02 ctrl".to_string()],
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
        brief: None,
        corpus_critic: None,
        gap_fetch: None,
        surgical_patch: None,
        cite_check: None,
        polish: None,
        readability_audit: None,
        decomposed_queries: Vec::new(),
        output_format: OutputFormat::Report,
        comparison_table: None,
        evaluation_scorecard: None,
        provider_stats: None,
    };
    let assembled = assemble_document(&doc);
    assert!(
        !assembled.body.contains('\x01') && !assembled.body.contains('\x02'),
        "body must not contain control chars from open questions"
    );
    assert!(assembled.body.contains("Question with  ctrl"));
}

#[test]
fn assemble_document_strips_control_chars_from_decomposed_queries() {
    let doc = ResearchDocument {
        item: sample_item(),
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
        brief: None,
        corpus_critic: None,
        gap_fetch: None,
        surgical_patch: None,
        cite_check: None,
        polish: None,
        readability_audit: None,
        decomposed_queries: vec!["query\x01 one".to_string(), "query\x02 two".to_string()],
        output_format: OutputFormat::Report,
        comparison_table: None,
        evaluation_scorecard: None,
        provider_stats: None,
    };
    let assembled = assemble_document(&doc);
    assert!(
        !assembled.body.contains('\x01') && !assembled.body.contains('\x02'),
        "body must not contain control chars from decomposed queries"
    );
    assert!(assembled.body.contains("query one"));
    assert!(assembled.body.contains("query two"));
}

#[test]
fn assemble_document_strips_control_chars_in_imrad_layout() {
    let doc = ResearchDocument {
        item: sample_item(),
        summary: "Abstract\x01 with ctrl".to_string(),
        findings: vec!["Finding\x02 with ctrl".to_string()],
        top_implications: Vec::new(),
        cross_references: vec![CrossReference {
            path: "src/main.rs".to_string(),
            relevance: "rel\x03evant".to_string(),
        }],
        open_questions: vec!["question\x01".to_string()],
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
        brief: None,
        corpus_critic: None,
        gap_fetch: None,
        surgical_patch: None,
        cite_check: None,
        polish: None,
        readability_audit: None,
        decomposed_queries: vec!["decomp\x02 query".to_string()],
        output_format: OutputFormat::Imrad,
        comparison_table: None,
        evaluation_scorecard: None,
        provider_stats: None,
    };
    let assembled = assemble_document(&doc);
    for byte in [0x01u8 as char, 0x02u8 as char, 0x03u8 as char] {
        assert!(
            !assembled.body.contains(byte),
            "IMRaD body must not contain control char {byte:?}"
        );
    }
    assert!(assembled.body.contains("Abstract with ctrl"));
    assert!(assembled.body.contains("Finding with ctrl"));
    assert!(assembled.body.contains("relevant"));
    assert!(assembled.body.contains("question"));
    assert!(assembled.body.contains("decomp query"));
}
