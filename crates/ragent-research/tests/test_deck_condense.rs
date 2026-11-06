//! Tests for the diff-deck bullet condenser (spec `diffdeck`: FR-022).
//!
//! Every case is a pure in-memory assertion over [`condense_bullet`]; no fixture
//! is written to disk.

use ragent_research::deck::{CONDENSE_MAX_CHARS, DeckElement, DeckItem, condense_bullet};

/// Build an item of `element` carrying `text`.
fn item(element: DeckElement, text: &str) -> DeckItem {
    DeckItem {
        element,
        key: String::new(),
        text: text.to_string(),
        source_url: None,
        sources: Vec::new(),
        detail: Vec::new(),
    }
}

/// Build a Cross-References item, a plain bullet-bearing element.
fn cross_reference(text: &str) -> DeckItem {
    item(DeckElement::CrossReferences, text)
}

/// The part of `bullet` before its trailing ellipsis, or the whole bullet.
fn bullet_head(bullet: &str) -> &str {
    bullet.strip_suffix("...").unwrap_or(bullet)
}

#[test]
fn test_condense_bullet_keeps_only_the_first_sentence() {
    let bullet = condense_bullet(&cross_reference(
        "Index quality matters more than model size. Chunking strategy dominates recall.",
    ));
    assert_eq!(bullet, "Index quality matters more than model size.");
}

#[test]
fn test_condense_bullet_keeps_an_item_without_a_terminator_whole() {
    let bullet = condense_bullet(&cross_reference("How much does chunk overlap matter"));
    assert_eq!(bullet, "How much does chunk overlap matter");
}

#[test]
fn test_condense_bullet_does_not_end_a_sentence_inside_an_abbreviation() {
    let bullet = condense_bullet(&cross_reference(
        "e.g. this stays one sentence. This does not.",
    ));
    assert_eq!(bullet, "e.g. this stays one sentence.");
}

#[test]
fn test_condense_bullet_keeps_a_decimal_from_ending_the_sentence() {
    let bullet = condense_bullet(&cross_reference(
        "Precision rose to 0.91 on the test set. Next.",
    ));
    assert_eq!(bullet, "Precision rose to 0.91 on the test set.");
}

#[test]
fn test_condense_bullet_keeps_a_question_mark_only() {
    let bullet = condense_bullet(&cross_reference(
        "Does it scale? Every measurement says no further than 10M rows.",
    ));
    assert_eq!(bullet, "Does it scale?");
}

#[test]
fn test_condense_bullet_collapses_whitespace_runs_to_one_line() {
    let bullet = condense_bullet(&item(
        DeckElement::ExecutiveSummary,
        "  A wrapped   paragraph\n\twith   a newline and a tab.  ",
    ));
    assert_eq!(bullet, "A wrapped paragraph with a newline and a tab.");
    assert!(!bullet.contains('\n'));
}

#[test]
fn test_condense_bullet_escapes_a_pipe_backtick_and_leading_hash() {
    // The hostile item from the spec's TC-009: the pipe must not start a table,
    // the backtick must not open a code span, the hash must not open a heading.
    let bullet = condense_bullet(&cross_reference("Pipe | and `backtick` are escaped"));
    assert_eq!(bullet, "Pipe \\| and \\`backtick\\` are escaped");
}

#[test]
fn test_condense_bullet_escapes_a_leading_hash() {
    let bullet = condense_bullet(&cross_reference("# a leading hash cannot open a heading"));
    assert_eq!(bullet, "\\# a leading hash cannot open a heading");
}

#[test]
fn test_condense_bullet_escapes_only_a_leading_hash() {
    let bullet = condense_bullet(&cross_reference("C# and F# keep their inner hashes"));
    assert_eq!(bullet, "C# and F# keep their inner hashes");
}

#[test]
fn test_condense_bullet_escapes_the_escape_character() {
    let bullet = condense_bullet(&cross_reference("a \\| b"));
    assert_eq!(bullet, "a \\\\\\| b");
}

#[test]
fn test_condense_bullet_leaves_ordinary_emphasis_markup_intact() {
    let bullet = condense_bullet(&cross_reference("**Bold** and *italic* survive."));
    assert_eq!(bullet, "**Bold** and *italic* survive.");
}

#[test]
fn test_condense_bullet_caps_a_long_bullet_with_an_ellipsis() {
    let text = "word ".repeat(80);
    let bullet = condense_bullet(&cross_reference(&text));
    assert!(bullet.ends_with("..."));
    let head = bullet_head(&bullet);
    assert!(head.chars().count() <= CONDENSE_MAX_CHARS);
    // The cut is on a word boundary, so no word is split in half.
    assert!(head.chars().all(|c| c.is_ascii_lowercase() || c == ' '));
    assert!(head.ends_with("word"));
}

#[test]
fn test_condense_bullet_does_not_cap_a_bullet_at_the_limit() {
    let text = "a".repeat(CONDENSE_MAX_CHARS);
    let bullet = condense_bullet(&cross_reference(&text));
    assert_eq!(bullet, text);
}

#[test]
fn test_condense_bullet_caps_after_escaping_so_the_escaped_bullet_stays_in_the_cap() {
    // Every character doubles when escaped; the cap is applied to the escaped
    // text so the emitted bullet cannot outgrow the slide.
    let text = "|".repeat(CONDENSE_MAX_CHARS + 50);
    let bullet = condense_bullet(&cross_reference(&text));
    assert!(bullet.ends_with("..."));
    let head = bullet_head(&bullet);
    assert!(head.chars().count() <= CONDENSE_MAX_CHARS);
    assert!(head.chars().all(|c| c == '\\' || c == '|'));
}

#[test]
fn test_condense_bullet_hard_truncates_a_single_over_long_word() {
    let text = "x".repeat(CONDENSE_MAX_CHARS + 40);
    let bullet = condense_bullet(&cross_reference(&text));
    assert_eq!(bullet_head(&bullet).chars().count(), CONDENSE_MAX_CHARS);
    assert!(bullet.ends_with("..."));
}

#[test]
fn test_condense_bullet_falls_back_when_a_sentence_has_no_words() {
    // A leading `...` has no alphanumeric content; the whole text is kept so the
    // bullet is never reduced to bare punctuation.
    let bullet = condense_bullet(&cross_reference("... and then the real content"));
    assert_eq!(bullet, "... and then the real content");
}

#[test]
fn test_condense_bullet_condenses_a_finding_headline() {
    let bullet = condense_bullet(&item(
        DeckElement::Findings,
        "Vector indexes accelerate similarity search",
    ));
    assert_eq!(bullet, "Vector indexes accelerate similarity search");
}

#[test]
fn test_condense_bullet_condenses_a_reference_row() {
    let bullet = condense_bullet(&item(
        DeckElement::References,
        "Indexing Survey - https://example.org/indexing-survey",
    ));
    assert_eq!(
        bullet,
        "Indexing Survey - https://example.org/indexing-survey"
    );
}
