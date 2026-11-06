//! Tests for the diff-deck per-section item extraction (spec `diffdeck`:
//! FR-009).
//!
//! Each deck element is split into the item unit its row of the
//! element-to-unit mapping table names. Every case is a pure in-memory
//! assertion over [`parse_report`]; no fixture is written to disk.

use ragent_research::deck::{DeckElement, ParsedReport, parse_report};

/// Parse a report that is expected to succeed.
fn parse_ok(text: &str) -> ParsedReport {
    match parse_report(text) {
        Ok(parsed) => parsed,
        Err(err) => panic!("expected the report to parse, got: {err}"),
    }
}

/// The item texts of `element`.
fn texts(parsed: &ParsedReport, element: DeckElement) -> Vec<String> {
    parsed
        .items(element)
        .iter()
        .map(|item| item.text.clone())
        .collect()
}

#[test]
fn test_extract_executive_summary_takes_one_unit_per_paragraph() {
    let report = "## Executive Summary\n\n\
First paragraph of the summary [#1].\n\n\
Second paragraph, a separate unit.\n";
    let parsed = parse_ok(report);
    assert_eq!(
        texts(&parsed, DeckElement::ExecutiveSummary),
        vec![
            "First paragraph of the summary [#1].",
            "Second paragraph, a separate unit.",
        ]
    );
}

#[test]
fn test_extract_executive_summary_joins_a_wrapped_paragraph() {
    let report =
        "## Executive Summary\n\nA sentence that the renderer\nwrapped onto a second line.\n";
    let parsed = parse_ok(report);
    assert_eq!(
        texts(&parsed, DeckElement::ExecutiveSummary),
        vec!["A sentence that the renderer wrapped onto a second line."]
    );
}

#[test]
fn test_extract_executive_summary_skips_a_fenced_block() {
    let report = "## Executive Summary\n\nProse before.\n\n```text\nnot an item\n```\n";
    let parsed = parse_ok(report);
    assert_eq!(
        texts(&parsed, DeckElement::ExecutiveSummary),
        vec!["Prose before."]
    );
}

#[test]
fn test_extract_concepts_takes_one_unit_per_concept_block() {
    let report = "## Concepts\n\n\
### 1. Human Oversight\n\n\
**Definition:** Human oversight is the deliberate calibration of agent autonomy [#14].\n\n\
It works through approval gates and escalation rules.\n\n\
**Key Evidence:**\n\
- A governance checklist recommends human approval for spending money [#14].\n\n\
### 2. AI Infrastructure Financing\n\n\
**Definition:** AI infrastructure financing is the debt used to fund data centers [#47].\n";
    let parsed = parse_ok(report);
    assert_eq!(
        texts(&parsed, DeckElement::Concepts),
        vec!["Human Oversight", "AI Infrastructure Financing"],
        "a concept is named by its heading, without the ordinal"
    );
}

#[test]
fn test_extract_concepts_carries_the_definition_as_detail() {
    let report = "## Concepts\n\n\
### 1. Human Oversight\n\n\
**Definition:** The deliberate calibration of agent autonomy.\n\n\
**Key Evidence:**\n\n\
- A governance checklist [#14].\n";
    let parsed = parse_ok(report);
    let concept = parsed
        .items(DeckElement::Concepts)
        .first()
        .expect("the report declares one concept");
    assert!(
        concept
            .detail
            .iter()
            .any(|block| block == "The deliberate calibration of agent autonomy."),
        "the definition's prose is the concept's detail: {:?}",
        concept.detail
    );
    assert!(
        concept.sources.contains(&"14".to_string()),
        "a concept records the citations its evidence carries: {:?}",
        concept.sources
    );
}

#[test]
fn test_extract_concepts_without_sub_headings_falls_back_to_paragraphs() {
    // Older reports rendered the concept list as one paragraph per concept.
    let report = "## Concepts\n\n\
The first theme of the corpus.\n\n\
The second theme of the corpus.\n";
    let parsed = parse_ok(report);
    assert_eq!(
        texts(&parsed, DeckElement::Concepts),
        vec![
            "The first theme of the corpus.",
            "The second theme of the corpus."
        ]
    );
}

#[test]
fn test_extract_concepts_strips_a_bold_numbered_heading() {
    let report = "## Concepts\n\n### **3.** Runtime Agent Governance\n\nEnforced at runtime.\n";
    let parsed = parse_ok(report);
    assert_eq!(
        texts(&parsed, DeckElement::Concepts),
        vec!["Runtime Agent Governance"]
    );
}

#[test]
fn test_extract_concepts_ignores_the_implications_and_questions_sections() {
    // The deck no longer renders the report's reader-facing narrative lists.
    let report = "## Top 10 Implications\n\n1. An implication.\n\n\
## Open Questions\n\n- A question?\n\n\
## Concepts\n\n### 1. A Concept\n\n**Definition:** Defined.\n";
    let parsed = parse_ok(report);
    assert_eq!(parsed.items(DeckElement::ExecutiveSummary), []);
    assert_eq!(parsed.items(DeckElement::Findings), []);
    assert_eq!(texts(&parsed, DeckElement::Concepts), vec!["A Concept"]);
}

#[test]
fn test_extract_cross_references_takes_one_unit_per_bullet() {
    let report = "## In-Project Cross-References\n\n\
- `crates/ragent-llm/src/retry.rs` - backoff policy\n\
- `src/main.rs` - startup path\n";
    let parsed = parse_ok(report);
    assert_eq!(
        texts(&parsed, DeckElement::CrossReferences),
        vec![
            "`crates/ragent-llm/src/retry.rs` - backoff policy",
            "`src/main.rs` - startup path",
        ]
    );
}

#[test]
fn test_extract_cross_references_takes_one_unit_per_table_data_row() {
    let report = "## In-Project Cross-References\n\n\
| Path | Relevance |\n\
|------|-----------|\n\
| `crates/a.rs` | retry |\n\
| `crates/b.rs` | |\n";
    let parsed = parse_ok(report);
    assert_eq!(
        texts(&parsed, DeckElement::CrossReferences),
        vec!["`crates/a.rs` - retry", "`crates/b.rs` - -"]
    );
}

#[test]
fn test_extract_findings_keys_each_block_by_its_headline() {
    let report = "## Findings\n\n\
### **Finding 1** - Work stealing is now universal\n\n\
**Observation:** Every runtime ships one [#2].\n\n\
### **Finding 2** - io_uring adoption is uneven\n\n\
Some text.\n";
    let parsed = parse_ok(report);
    let findings = parsed.items(DeckElement::Findings);
    assert_eq!(
        findings
            .iter()
            .map(|item| item.text.as_str())
            .collect::<Vec<_>>(),
        vec![
            "Work stealing is now universal",
            "io_uring adoption is uneven",
        ]
    );
    assert_eq!(findings[0].key, "work stealing is now universal");
}

#[test]
fn test_extract_findings_matches_the_headline_across_a_renumbering() {
    // FR-009: identity is the headline, so a finding that moved position still
    // matches between two reports.
    let older = parse_ok("## Findings\n\n### **Finding 1** - Alpha headline\n");
    let newer = parse_ok(
        "## Findings\n\n### **Finding 1** - Introduced headline\n\n\
### **Finding 2** - Alpha headline\n",
    );
    let older_key = older.items(DeckElement::Findings)[0].key.as_str();
    assert!(
        newer
            .items(DeckElement::Findings)
            .iter()
            .any(|item| item.key == older_key),
        "a renumbered finding must still match its older key"
    );
}

#[test]
fn test_extract_findings_accepts_the_plain_and_colon_heading_shapes() {
    let report = "## Findings\n\n\
### Finding 1: A colon-separated headline\n\n\
### Finding 2. A dot-separated headline\n\n\
### 3 - A bare numbered headline\n";
    let parsed = parse_ok(report);
    assert_eq!(
        texts(&parsed, DeckElement::Findings),
        vec![
            "A colon-separated headline",
            "A dot-separated headline",
            "A bare numbered headline",
        ]
    );
}

#[test]
fn test_extract_findings_falls_back_to_the_observation_headline() {
    // The heading carries no headline of its own; the assembler would derive one
    // from the observation, and the deck must agree with the report.
    let report = "## Findings\n\n\
### **Finding 1**\n\n\
Vector indexes accelerate similarity search across large corpora.\n";
    let parsed = parse_ok(report);
    // `make_headline_from_observation` trims the trailing sentence punctuation,
    // which is the headline the assembled report itself would carry.
    assert_eq!(
        texts(&parsed, DeckElement::Findings),
        vec!["Vector indexes accelerate similarity search across large corpora"]
    );
}

#[test]
fn test_extract_findings_skips_a_block_with_no_headline_and_no_body() {
    let report = "## Findings\n\n### **Finding 1**\n\n_(no observation recorded)_\n";
    let parsed = parse_ok(report);
    assert_eq!(parsed.items(DeckElement::Findings), []);
}

#[test]
fn test_extract_findings_ignores_body_before_the_first_block() {
    let report = "## Findings\n\nStray prose before any finding heading.\n\n\
### **Finding 1** - A real headline\n";
    let parsed = parse_ok(report);
    assert_eq!(
        texts(&parsed, DeckElement::Findings),
        vec!["A real headline"]
    );
}

#[test]
fn test_extract_references_takes_one_unit_per_table_entry() {
    let report = "## References Index\n\n\
| # | Type | Media | Language | Path/URL | Title | Author | Published | Relevance | Search tool | Engine | Captured |\n\
|---|------|-------|----------|----------|-------|--------|-----------|-----------|-------------|--------|----------|\n\
| [#1] | web | page | English | https://example.com/a | Alpha | - | - | high | mf_search | exa | 2026-01-01T00:00:00Z |\n\
| [#2] | web | page | English | https://example.com/b | - | - | - | high | mf_search | exa | 2026-01-01T00:00:00Z |\n";
    let parsed = parse_ok(report);
    let references = parsed.items(DeckElement::References);
    assert_eq!(references.len(), 2);
    assert_eq!(
        references[0].key, "https://example.com/a",
        "a reference is keyed by its normalised source URL, not its citation"
    );
    assert_eq!(references[1].key, "https://example.com/b");
    assert_eq!(
        references[0].source_url.as_deref(),
        Some("https://example.com/a")
    );
    assert_eq!(
        references[0].text, "Alpha - https://example.com/a",
        "a titled reference reads title then URL"
    );
    assert_eq!(
        references[1].text, "https://example.com/b",
        "an untitled reference reads as its URL"
    );
}

#[test]
fn test_extract_references_ignores_a_row_without_a_number() {
    let report = "## References Index\n\n\
| # | Path/URL | Title |\n\
|---|----------|-------|\n\
| - | https://example.com/a | Alpha |\n";
    let parsed = parse_ok(report);
    assert_eq!(parsed.items(DeckElement::References), []);
}

#[test]
fn test_extract_items_keeps_document_order_within_a_section() {
    let report = "## In-Project Cross-References\n\n\
- `zeta.rs` - zebra\n- `alpha.rs` - anvil\n- `mike.rs` - marble\n";
    let parsed = parse_ok(report);
    assert_eq!(
        texts(&parsed, DeckElement::CrossReferences),
        vec![
            "`zeta.rs` - zebra",
            "`alpha.rs` - anvil",
            "`mike.rs` - marble"
        ],
        "items stay in document order; only sections are reordered"
    );
}

#[test]
fn test_extract_imrad_findings_use_the_same_units() {
    let report = "## Results\n\n### Findings\n\n\
### **Finding 1** - Nested headline one\n\n\
### **Finding 2** - Nested headline two\n";
    let parsed = parse_ok(report);
    assert_eq!(
        texts(&parsed, DeckElement::Findings),
        vec!["Nested headline one", "Nested headline two"]
    );
}

#[test]
fn test_extract_strips_control_characters_from_every_unit() {
    let report = "## In-Project Cross-References\n\n- `x.rs` - a note\u{7} with a bell\n";
    let parsed = parse_ok(report);
    let references = parsed.items(DeckElement::CrossReferences);
    assert_eq!(references.len(), 1);
    assert!(
        !references[0].text.contains('\u{7}'),
        "a parsed item never carries a control character: {:?}",
        references[0].text
    );
}

#[test]
fn test_extract_placeholder_units_produce_no_item() {
    let report = "## Executive Summary\n\n_(no executive summary recorded yet)_\n\n\
## Findings\n\n_(no findings yet - the gathering pass will populate this section)_\n\n\
## References Index\n\n_(no sources captured yet)_\n";
    let parsed = parse_ok(report);
    assert_eq!(parsed.items(DeckElement::ExecutiveSummary), []);
    assert_eq!(parsed.items(DeckElement::Findings), []);
    assert_eq!(parsed.items(DeckElement::References), []);
}
