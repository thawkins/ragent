//! Tests for the diff-deck element model and the report splitter (spec
//! `diffdeck`: FR-003, FR-009, FR-019).
//!
//! Every case is a pure in-memory assertion over [`parse_report`]; no
//! fixture is written to disk.

use ragent_research::deck::{DeckElement, ParsedReport, parse_report};

/// A report shaped like the default report layout, with every deck element
/// populated and two unknown/non-element sections in between.
const FULL_REPORT: &str = "# Title: Rust Async Runtimes\n\n\
## Topic\n\nThe runtime landscape.\n\n\
## Search Queries\n\n- rust async runtime\n\n\
## Executive Summary\n\nEvery major runtime now ships a work-stealing scheduler [#1].\n\n\
## Concepts\n\n\
### 1. Ecosystem Breadth\n\n\
**Definition:** How much of the runtime landscape one runtime supports [#1].\n\n\
### 2. Structured Concurrency\n\n\
**Definition:** A task tree that cannot leak a child [#2].\n\n\
## Findings\n\n\
### **Finding 1** - Work stealing is now universal\n\n\
Observation text [#2].\n\n\
### **Finding 2** - io_uring adoption is uneven\n\n\
More observation text.\n\n\
## Findings Relationship Diagram\n\n\
```mermaid\nflowchart TD\n```\n\n\
## In-Project Cross-References\n\n\
| Path | Relevance |\n|------|-----------|\n\
| `crates/ragent-llm/src/x.rs` | retry policy |\n\
| `src/main.rs` | startup |\n\n\
## References Index\n\n\
| # | Type | Media | Language | Path/URL | Title | Author | Published | Relevance | Search tool | Engine | Captured |\n\
|---|------|-------|----------|----------|-------|--------|-----------|-----------|-------------|--------|----------|\n\
| 1 | web | page | English | https://example.com/a | A | - | - | high | mf_search | exa | 2026-01-01T00:00:00Z |\n\
| 2 | web | page | English | https://example.com/b | B | - | - | high | mf_search | exa | 2026-01-01T00:00:00Z |\n";

/// Parse a report that is expected to succeed.
fn parse_ok(text: &str) -> ParsedReport {
    match parse_report(text) {
        Ok(parsed) => parsed,
        Err(err) => panic!("expected the report to parse, got: {err}"),
    }
}

#[test]
fn test_deck_element_all_is_the_fr003_precedence() {
    let labels: Vec<&str> = DeckElement::ALL.iter().map(|e| e.label()).collect();
    assert_eq!(
        labels,
        vec![
            "Executive Summary",
            "Concepts",
            "Findings",
            "Cross-References",
            "References",
        ]
    );
    // The derived ordering must be the precedence order, because the render
    // stage sorts by it.
    let mut sorted = DeckElement::ALL;
    sorted.sort();
    assert_eq!(sorted, DeckElement::ALL);
}

#[test]
fn test_deck_element_from_name_accepts_labels_and_headings() {
    assert_eq!(
        DeckElement::from_name("Executive Summary"),
        Some(DeckElement::ExecutiveSummary)
    );
    assert_eq!(
        DeckElement::from_name("concepts"),
        Some(DeckElement::Concepts)
    );
    assert_eq!(
        DeckElement::from_name("Cross references"),
        Some(DeckElement::CrossReferences)
    );
    assert_eq!(
        DeckElement::from_name("In-Project Cross-References"),
        Some(DeckElement::CrossReferences)
    );
    assert_eq!(
        DeckElement::from_name("References Index"),
        Some(DeckElement::References)
    );
    assert_eq!(
        DeckElement::from_name("Top Implications"),
        None,
        "the deck no longer renders the report's implication list"
    );
    assert_eq!(DeckElement::from_name("Open Questions"), None);
    assert_eq!(DeckElement::from_name(""), None);
}

#[test]
fn test_parse_report_collects_every_element_in_precedence_order() {
    let parsed = parse_ok(FULL_REPORT);
    let order: Vec<DeckElement> = parsed.sections.iter().map(|s| s.element).collect();
    assert_eq!(order, DeckElement::ALL.to_vec());
    assert_eq!(
        parsed.title.as_deref(),
        Some("Rust Async Runtimes"),
        "the `# Title:` line feeds the cover slide"
    );
}

#[test]
fn test_parse_report_extracts_one_unit_per_item() {
    let parsed = parse_ok(FULL_REPORT);

    // Executive Summary: one paragraph unit.
    let summary = parsed.items(DeckElement::ExecutiveSummary);
    assert_eq!(summary.len(), 1);
    assert!(summary[0].text.starts_with("Every major runtime"));

    // Concepts: one unit per `### N. <concept>` block, keyed by the name.
    let concepts = parsed.items(DeckElement::Concepts);
    assert_eq!(concepts.len(), 2);
    assert_eq!(concepts[0].text, "Ecosystem Breadth");
    assert_eq!(concepts[0].key, "ecosystem breadth");
    assert_eq!(concepts[1].text, "Structured Concurrency");

    // Findings: one unit per `### **Finding N**` block, keyed by headline.
    let findings = parsed.items(DeckElement::Findings);
    assert_eq!(findings.len(), 2);
    assert_eq!(findings[0].key, "work stealing is now universal");
    assert_eq!(findings[0].text, "Work stealing is now universal");

    // Cross-References: one unit per table data row.
    let cross_refs = parsed.items(DeckElement::CrossReferences);
    assert_eq!(cross_refs.len(), 2);
    assert!(cross_refs[0].text.contains("crates/ragent-llm/src/x.rs"));

    // References: one unit per `[#N]` entry, keyed by normalised source URL.
    let references = parsed.items(DeckElement::References);
    assert_eq!(references.len(), 2);
    assert_eq!(references[0].key, "https://example.com/a");
    assert_eq!(references[1].key, "https://example.com/b");
    assert!(references[0].text.contains("https://example.com/a"));
}

#[test]
fn test_parse_report_ignores_placeholders_and_non_element_sections() {
    let report = "## Executive Summary\n\n\
_(no executive summary recorded yet - run a gathering pass to populate)_\n\n\
## Concepts\n\n\
_(no concepts yet - the concept pass will populate this section)_\n\n\
## Findings\n\n_(no findings yet)_\n";
    let parsed = parse_ok(report);
    assert_eq!(parsed.items(DeckElement::ExecutiveSummary), []);
    assert_eq!(parsed.items(DeckElement::Concepts), []);
    assert_eq!(parsed.items(DeckElement::Findings), []);
    // The report's own standard sections are not heading drift.
    assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
}

#[test]
fn test_parse_report_records_a_warning_for_an_unknown_section() {
    let report = "## Topic\n\nT\n\n## Widget Metrics\n\nstuff\n\n## Findings\n\n\
### **Finding 1** - A headline\n";
    let parsed = parse_ok(report);
    assert_eq!(parsed.sections.len(), 1);
    assert_eq!(parsed.sections[0].element, DeckElement::Findings);
    assert!(
        parsed.warnings.iter().any(|w| w.contains("Widget Metrics")),
        "an unknown section must be recorded: {:?}",
        parsed.warnings
    );
}

#[test]
fn test_parse_report_refuses_a_report_with_no_recognised_section() {
    let err = parse_report("# Title: Nothing useful\n\n## Widget Metrics\n\nstuff\n")
        .expect_err("a report with no recognised section must be refused");
    let message = err.to_string();
    assert!(
        message.contains("no recognised top-level section"),
        "{message}"
    );
    assert!(message.contains("Executive Summary"), "{message}");
    assert!(message.contains("References Index"), "{message}");
}

#[test]
fn test_parse_report_refuses_empty_text() {
    assert!(parse_report("").is_err());
    assert!(parse_report("   \n\n").is_err());
}

#[test]
fn test_parse_report_reads_imrad_findings_subsection() {
    // The IMRaD layout nests the findings block under `## Results`.
    let report = "## Results\n\n### Findings\n\n### **Finding 1** - Nested still works\n";
    let parsed = parse_ok(report);
    let findings = parsed.items(DeckElement::Findings);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].key, "nested still works");
}

#[test]
fn test_parse_report_warns_on_a_duplicate_element_section() {
    let report = "## In-Project Cross-References\n\n- First.\n\n\
## In-Project Cross-References\n\n- Second.\n";
    let parsed = parse_ok(report);
    let questions = parsed.items(DeckElement::CrossReferences);
    assert_eq!(questions.len(), 1, "the first section wins");
    assert_eq!(questions[0].text, "First.");
    assert!(
        parsed.warnings.iter().any(|w| w.contains("duplicate")),
        "{:?}",
        parsed.warnings
    );
}

/// FR-026: the IMRaD layout nests `### Concepts` under `## Results`, so the
/// concepts reach the deck from either layout.
#[test]
fn test_parse_report_reads_imrad_concepts_subsection() {
    let report = "\
## Results

### Concepts

### 1. Runtime Breadth

**Definition:** How much of the landscape one runtime covers.
";
    let parsed = parse_ok(report);
    let concepts = parsed.items(DeckElement::Concepts);
    assert_eq!(concepts.len(), 1, "{concepts:?}");
    assert_eq!(concepts[0].text, "Runtime Breadth");
    assert!(
        concepts[0]
            .detail
            .iter()
            .any(|block| block == "How much of the landscape one runtime covers."),
        "the definition's prose reaches the slide: {:?}",
        concepts[0].detail
    );
}

/// A report that carries only the reader-facing narrative the deck dropped -
/// `Top Implications` and `Open Questions` - has no deck element at all, so it
/// is refused rather than producing an empty deck.
#[test]
fn test_parse_report_refuses_a_report_with_only_implications_and_questions() {
    let err = parse_report(
        "## Top 10 Implications\n\n1. An implication.\n\n## Open Questions\n\n- A question?\n",
    )
    .expect_err("no deck element means no deck");
    assert!(
        err.to_string().contains("no recognised top-level section"),
        "{err}"
    );
}

/// FR-004: a prose or listicle item records the `[#N]` citations its text
/// carries, which is what lets the diff assign `ADDED`.
///
/// Findings carry a `**Sources:**` block; every other element has its citations
/// inline, so without this the older report's cited-source set is empty for
/// those elements and an unmatched item can never be `ADDED`.
#[test]
fn test_parse_report_records_inline_citations_as_sources() {
    let text = "## Concepts\n\n\
### 1. First Concept\n\nCited [#18][#19][#18].\n\n\
### 2. Second Concept\n\nNo citation here.\n";

    let parsed = parse_ok(text);
    let items = parsed.items(DeckElement::Concepts);

    assert_eq!(items.len(), 2, "got {items:?}");
    assert_eq!(
        items[0].sources,
        vec!["18".to_string(), "19".to_string()],
        "citations must be recorded once each, in order"
    );
    assert!(
        items[1].sources.is_empty(),
        "an uncited item records no source: {:?}",
        items[1].sources
    );
}
