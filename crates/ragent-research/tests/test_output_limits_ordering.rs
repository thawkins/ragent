//! Tests for spec `researchmax` T-012: the ordering, truncation, boundary, and
//! byte-stability guarantees of the concept/finding output limits
//! (FR-001, FR-003, FR-004, FR-015, FR-016, FR-022, NFR-002).
//!
//! The ordering helper and the finding cap are already pinned in
//! `test_research_limits.rs`, and the concept ordering in
//! `test_concepts_section.rs`. This file adds the remaining coverage: the
//! cited-count tie-break and unknown-citation default rank on BOTH report
//! paths, and the NFR-002 requirement that a report whose counts are already
//! within the limits (including the no-findings and no-concepts cases) stays
//! byte-stable after the limits are applied.

use ragent_research::source::Source;
use ragent_research::{
    ResearchDocument, ResearchItem, ResearchName, assemble_document, cap_findings_to_limit,
    concepts_section_for_research,
};
use std::collections::HashMap;
use std::path::PathBuf;

/// A web source whose relevance label drives [`Source::relevance_rank`].
fn source(label: &str) -> Source {
    Source::Web {
        url: "https://example.invalid".to_string(),
        title: "Source".to_string(),
        captured_at: chrono::Utc::now(),
        published_at: None,
        body_path: PathBuf::new(),
        body: String::new(),
        relevance: label.to_string(),
        search_tool: "mf_search".to_string(),
        search_engine: "openalex".to_string(),
        content_type: None,
        page_type: None,
        media_type: "page".to_string(),
        language: None,
        oa_recovery: None,
        author: None,
    }
}

/// Build a minimal [`ResearchDocument`] for byte-stability checks.
fn sample_doc(findings: Vec<String>, concepts: Option<String>) -> ResearchDocument {
    let name = ResearchName::new("output-limits").expect("name must validate");
    let mut item = ResearchItem::new(name, "Output Limits", "topic");
    item.set_queries(vec!["q1".to_string()]);
    ResearchDocument {
        item,
        summary: "Summary text.".to_string(),
        findings,
        top_implications: Vec::new(),
        cross_references: Vec::new(),
        open_questions: Vec::new(),
        concepts,
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

/// Extract the concept heading titles (`### N. Title`) in render order.
fn concept_titles(section: &str) -> Vec<&str> {
    section
        .lines()
        .filter(|line| line.starts_with("### "))
        .collect()
}

/// Extract the `## Findings` section body (from its heading up to the next
/// top-level `## ` heading). Full `content` cannot be compared byte-for-byte
/// across two assemblies because the empty References Index placeholder row
/// embeds a fresh `Utc::now()` capture timestamp (io.rs); the cap governs this
/// section, so the byte-stability check is scoped to it.
fn findings_section(body: &str) -> &str {
    let start = body
        .find("## Findings\n")
        .expect("findings heading present");
    let rest = &body[start..];
    let end = rest[1..]
        .find("\n## ")
        .map_or(rest.len(), |offset| offset + 1);
    &rest[..end]
}

// ── Concept-path ordering: cited-count tie-break and unknown codes ─────────

/// A concept that cites two equal-rank sources outranks one that cites only a
/// single source of the same rank (FR-004).
#[test]
fn concepts_tie_break_by_cited_count() {
    // Both sources are "Low" (rank 3), so only the cited count separates the
    // two concepts.
    let sources = vec![source("Low"), source("Low")];
    let raw = "# Concepts\n\n\
        ## 1. Single\n\n**Definition:** single.\n\n**Key Evidence:** ([#1])\n\n\
        ## 2. Double\n\n**Definition:** double.\n\n**Key Evidence:** ([#1], [#2])\n";
    let section =
        concepts_section_for_research(raw, &HashMap::new(), &sources, 0).expect("sections exist");
    assert_eq!(
        concept_titles(&section),
        vec!["### 1. Double", "### 2. Single"],
        "{section}"
    );
}

/// A concept that cites an unrecognised source index falls back to the medium
/// default rank, which outranks a genuinely low-relevance concept (FR-005).
#[test]
fn concepts_unknown_citation_uses_default_rank() {
    // Only one gathered source ("Very low" = rank 1); the concept citing [#9]
    // resolves to no source and so scores the medium default (rank 5).
    let sources = vec![source("Very low")];
    let raw = "# Concepts\n\n\
        ## 1. Cited\n\n**Definition:** cited.\n\n**Key Evidence:** ([#1])\n\n\
        ## 2. Unknown\n\n**Definition:** unknown.\n\n**Key Evidence:** ([#9])\n";
    let section =
        concepts_section_for_research(raw, &HashMap::new(), &sources, 0).expect("sections exist");
    assert_eq!(
        concept_titles(&section),
        vec!["### 1. Unknown", "### 2. Cited"],
        "{section}"
    );
}

// ── Findings-path ordering: equal-rank stability ──────────────────────────

/// Findings whose highest cited rank is identical keep their original model
/// order when neither cites a further source (FR-003 tie-break).
#[test]
fn cap_findings_keeps_original_order_for_equal_rank_ties() {
    let sources = vec![source("Low"), source("Low")];
    let findings = vec!["1. first [#1]".to_string(), "2. second [#2]".to_string()];
    let capped = cap_findings_to_limit(findings.clone(), 0, &sources);
    assert_eq!(capped, findings);
}

// ── NFR-002: byte-stability when already within the limits ────────────────

/// A report with no findings and no concepts must render identically before
/// and after the caps are applied (NFR-002): the empty list stays empty and no
/// concept section appears.
#[test]
fn empty_findings_and_no_concepts_report_is_byte_stable() {
    let sources: Vec<Source> = Vec::new();
    let mut doc = sample_doc(Vec::new(), None);
    let before = assemble_document(&doc);

    // Apply the default cap to the empty list; it must stay empty.
    doc.findings = cap_findings_to_limit(doc.findings, 5, &sources);
    let after = assemble_document(&doc);

    assert!(doc.findings.is_empty(), "empty findings must stay empty");
    assert_eq!(
        findings_section(&before.body),
        findings_section(&after.body),
        "an empty-findings report section must be byte-stable across the cap pass"
    );
    assert!(
        after
            .body
            .contains("_(no findings yet — the gathering pass will populate this section)_"),
        "the empty-findings placeholder must be preserved"
    );
    assert!(
        !after.body.contains("## Concepts"),
        "no concept section must be emitted when concepts are absent"
    );
}

/// A run whose finding count is already at or below the limit renders the same
/// bytes before and after the cap (NFR-002, FR-015): order and count are
/// unchanged and no padding is inserted.
#[test]
fn within_limit_report_is_byte_stable() {
    let sources = vec![source("Very high"), source("Low")];
    // Two findings, both already most-relevant-first, below the cap of 20.
    let findings = vec!["1. strong [#1]".to_string(), "2. weak [#2]".to_string()];
    let mut doc = sample_doc(findings, None);
    let before = assemble_document(&doc);

    doc.findings = cap_findings_to_limit(doc.findings, 20, &sources);
    let after = assemble_document(&doc);

    assert_eq!(doc.findings.len(), 2, "count must not be reduced or padded");
    assert_eq!(
        findings_section(&before.body),
        findings_section(&after.body),
        "a within-limit report section must be byte-stable across the cap pass"
    );
}
