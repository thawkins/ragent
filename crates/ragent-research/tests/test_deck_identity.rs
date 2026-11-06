//! Tests for the diff-deck item identity and the section-scoped index (spec
//! `diffdeck`: FR-009, NFR-001).
//!
//! Every case is a pure in-memory assertion over [`normalize_identity`] and
//! [`ElementIndex`]; no fixture is written to disk.

use ragent_research::deck::{
    DeckElement, DeckItem, ElementIndex, ParsedReport, normalize_identity, parse_report,
};

/// Build a minimal item for the index tests.
fn item(element: DeckElement, key: &str, text: &str) -> DeckItem {
    DeckItem {
        element,
        key: key.to_string(),
        text: text.to_string(),
        source_url: None,
        sources: Vec::new(),
        detail: Vec::new(),
    }
}

/// Parse a report that is expected to succeed.
fn parse_ok(text: &str) -> ParsedReport {
    match parse_report(text) {
        Ok(parsed) => parsed,
        Err(error) => panic!("expected a parse, got {error}"),
    }
}

#[test]
fn test_identity_strips_citation_markers() {
    assert_eq!(
        normalize_identity(DeckElement::ExecutiveSummary, "Runtime breadth grew [#1]."),
        "runtime breadth grew .",
        "a [#N] marker is removed whole"
    );
    assert_eq!(
        normalize_identity(DeckElement::CrossReferences, "Adoption rose [2] fast"),
        "adoption rose fast",
        "a bare [N] citation is removed too"
    );
    assert_eq!(
        normalize_identity(DeckElement::CrossReferences, "Is it ready [#12][#13] yet?"),
        "is it ready yet?",
        "consecutive citations are each removed"
    );
}

#[test]
fn test_identity_keeps_non_citation_brackets() {
    assert_eq!(
        normalize_identity(
            DeckElement::CrossReferences,
            "Which option [see note] wins?"
        ),
        "which option [see note] wins?",
        "a bracketed phrase that is not a citation stays"
    );
    assert_eq!(
        normalize_identity(DeckElement::CrossReferences, "A footnote [^1] reference"),
        "a footnote [^1] reference",
        "a footnote marker is not a numeric citation"
    );
    assert_eq!(
        normalize_identity(DeckElement::CrossReferences, "Unclosed [1 citation"),
        "unclosed [1 citation",
        "an unterminated bracket is text, not a citation"
    );
}

#[test]
fn test_identity_strips_markdown_emphasis_and_backticks() {
    assert_eq!(
        normalize_identity(DeckElement::CrossReferences, "**Tokio** keeps its lead"),
        "tokio keeps its lead"
    );
    assert_eq!(
        normalize_identity(DeckElement::CrossReferences, "__Tokio__ keeps its lead"),
        "tokio keeps its lead"
    );
    assert_eq!(
        normalize_identity(
            DeckElement::CrossReferences,
            "Is `io_uring` production ready?"
        ),
        "is iouring production ready?",
        "a code span loses its backticks and its underscore"
    );
    assert_eq!(
        normalize_identity(DeckElement::CrossReferences, "*smol* ~~is~~ small"),
        "smol is small",
        "single markers and strikethrough are dropped"
    );
}

#[test]
fn test_identity_collapses_whitespace_and_lowercases() {
    assert_eq!(
        normalize_identity(
            DeckElement::CrossReferences,
            "  src/main.rs   -   Startup\tpath\n"
        ),
        "src/main.rs - startup path",
        "a wrapped cross-reference and a one-line one compare equal"
    );
    assert_eq!(
        normalize_identity(DeckElement::ExecutiveSummary, "MiXeD   CaSe"),
        "mixed case"
    );
    assert_eq!(normalize_identity(DeckElement::Findings, "   "), "");
}

#[test]
fn test_identity_is_idempotent() {
    let raw = "**Runtime** breadth grew [#1].";
    let once = normalize_identity(DeckElement::ExecutiveSummary, raw);
    let twice = normalize_identity(DeckElement::ExecutiveSummary, &once);
    assert_eq!(once, twice, "normalising twice changes nothing");
}

#[test]
fn test_identity_keeps_url_brackets_for_a_reference() {
    assert_eq!(
        normalize_identity(DeckElement::References, "https://example.com/a[1]"),
        "https://example.com/a[1]",
        "a reference identity is its URL, so path brackets are kept"
    );
    assert_eq!(
        normalize_identity(DeckElement::References, "HTTPS://Example.COM/A"),
        "https://example.com/a",
        "a URL is lower-cased like every other identity"
    );
}

#[test]
fn test_identity_matches_a_renumbered_citation_between_reports() {
    let older = parse_ok(
        "## Executive Summary\n\nEvery runtime ships a scheduler [#1].\n\n\
## In-Project Cross-References\n\n- Tokio keeps its lead.\n",
    );
    let newer = parse_ok(
        "## Executive Summary\n\nEvery runtime ships a **scheduler** [#7].\n\n\
## In-Project Cross-References\n\n-   `Tokio`   keeps its lead.\n",
    );
    for element in [DeckElement::ExecutiveSummary, DeckElement::CrossReferences] {
        assert_eq!(
            older.items(element)[0].key,
            newer.items(element)[0].key,
            "{element:?} must match across a citation renumber and re-spacing"
        );
    }
}

#[test]
fn test_identity_does_not_match_a_reworded_item() {
    let older = parse_ok("## In-Project Cross-References\n\n- `ship.rs` - release path\n");
    let newer = parse_ok("## In-Project Cross-References\n\n- `ship.rs` - release path, revised\n");
    let index = ElementIndex::new(
        DeckElement::CrossReferences,
        older.items(DeckElement::CrossReferences),
    );
    assert_eq!(
        index.matching(&newer.items(DeckElement::CrossReferences)[0]),
        None,
        "a changed item is not the same item"
    );
}

#[test]
fn test_identity_key_matches_the_normalised_text() {
    let parsed = parse_ok("## In-Project Cross-References\n\n- **Bold** claim [#3].\n");
    let items = parsed.items(DeckElement::CrossReferences);
    assert_eq!(
        items[0].key,
        normalize_identity(DeckElement::CrossReferences, &items[0].text),
        "an item's stored key is exactly its normalised identity"
    );
}

#[test]
fn test_reference_identity_matches_across_a_renumbered_citation() {
    let table = |number: u32, title: &str| {
        format!(
            "## References Index\n\n\
| # | Type | Media | Language | Path/URL | Title |\n\
|---|------|-------|----------|----------|-------|\n\
| [#{number}] | web | page | English | https://example.com/a | {title} |\n"
        )
    };
    let older = parse_ok(&table(1, "Alpha"));
    let newer = parse_ok(&table(9, "Alpha revised"));
    let old_item = &older.items(DeckElement::References)[0];
    let new_item = &newer.items(DeckElement::References)[0];
    assert_eq!(
        old_item.key, new_item.key,
        "a source keeps its identity when its citation is renumbered"
    );
    let index = ElementIndex::new(
        DeckElement::References,
        older.items(DeckElement::References),
    );
    assert_eq!(
        index.matching(new_item),
        Some("alpha - https://example.com/a"),
        "the index resolves the newer source to the older identity"
    );
}

#[test]
fn test_index_is_section_scoped() {
    let questions = vec![item(
        DeckElement::CrossReferences,
        "will it ship",
        "Will it ship?",
    )];
    let index = ElementIndex::new(DeckElement::CrossReferences, &questions);
    let same_key_other_element = item(DeckElement::Concepts, "will it ship", "Will it ship?");
    assert!(
        index.matching(&same_key_other_element).is_none(),
        "an identical key in a different element never matches"
    );
    assert_eq!(
        index.matching(&questions[0]),
        Some("will it ship?"),
        "the same element matches"
    );
}

#[test]
fn test_index_lookup_is_by_identity_not_by_position() {
    let older = vec![
        item(DeckElement::CrossReferences, "alpha", "Alpha"),
        item(DeckElement::CrossReferences, "beta", "Beta"),
    ];
    let index = ElementIndex::new(DeckElement::CrossReferences, &older);
    // The newer report reordered its items; identity still finds them.
    let reordered = item(DeckElement::CrossReferences, "beta", "Beta");
    assert_eq!(index.matching(&reordered), Some("beta"));
    assert_eq!(index.len(), 2);
    assert!(!index.is_empty());
    assert_eq!(index.element(), Some(DeckElement::CrossReferences));
}

#[test]
fn test_index_returns_the_indexed_side_identity_for_an_update_check() {
    // Same key, different normalised text: the diff stage reads both strings to
    // tell `OLD` from `UPDATE` (FR-004) without re-normalising either.
    let older = vec![item(
        DeckElement::CrossReferences,
        "tokens cost less",
        "Tokens cost less now",
    )];
    let index = ElementIndex::new(DeckElement::CrossReferences, &older);
    let newer = item(
        DeckElement::CrossReferences,
        "tokens cost less",
        "Tokens cost far less now",
    );
    assert_eq!(index.matching(&newer), Some("tokens cost less now"));
    assert_ne!(
        index.matching(&newer),
        Some(normalize_identity(DeckElement::CrossReferences, &newer.text).as_str()),
        "the two identities differ, which is what makes the item an UPDATE"
    );
}

#[test]
fn test_index_keeps_the_first_of_a_duplicated_identity() {
    let duplicated = vec![
        item(
            DeckElement::CrossReferences,
            "src/main.rs",
            "src/main.rs - first",
        ),
        item(
            DeckElement::CrossReferences,
            "src/main.rs",
            "src/main.rs - second",
        ),
    ];
    let index = ElementIndex::new(DeckElement::CrossReferences, &duplicated);
    assert_eq!(index.len(), 1, "a repeated identity occupies one slot");
    assert_eq!(index.matching(&duplicated[1]), Some("src/main.rs - first"));
}

#[test]
fn test_index_of_no_items_is_empty() {
    let index = ElementIndex::new(DeckElement::References, &[]);
    assert!(index.is_empty());
    assert_eq!(index.len(), 0);
    assert_eq!(
        index.matching(&item(DeckElement::References, "x", "x")),
        None
    );
    let default = ElementIndex::default();
    assert_eq!(default.element(), None);
    assert!(default.is_empty());
}
