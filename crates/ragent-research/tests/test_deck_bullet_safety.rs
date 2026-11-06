//! Tests for the Markdown safety of emitted deck bullets (spec `diffdeck`:
//! FR-022).
//!
//! FR-022 has two halves, and a bullet can satisfy one while breaking the other:
//!
//! - **escaping** - `|`, backticks, and a leading `#` must not break the slide
//!   structure, because a finding is model-generated text and may contain any of
//!   them;
//! - **capping** - a bullet is capped at 200 characters with a trailing
//!   ellipsis, and the cap applies to the *escaped* text so an escape sequence
//!   can never push a bullet past the limit.
//!
//! Each case drives the whole pipeline (`parse_report` -> `diff_reports` ->
//! `render_deck`) rather than a private helper, so the assertions hold for the
//! bullets a reader actually sees in the deck file. Every fixture adds one item
//! and never removes one, so the item is `NEW` and its element always produces a
//! slide.

use chrono::NaiveDate;
use ragent_research::deck::{
    CONDENSE_MAX_CHARS, DeckRenderOptions, ParsedReport, diff_reports, parse_report, render_deck,
};

/// The trailing marker a truncated bullet ends with (FR-022).
const ELLIPSIS: &str = "...";

/// Parse a report that is expected to succeed.
fn parse_ok(text: &str) -> ParsedReport {
    match parse_report(text) {
        Ok(parsed) => parsed,
        Err(error) => panic!("the fixture must parse: {error}"),
    }
}

/// The render options for the `2026-01-01` to `2026-02-01` pair.
fn options() -> DeckRenderOptions {
    DeckRenderOptions {
        name: "bulletfixture".to_string(),
        generated_on: NaiveDate::from_ymd_opt(2026, 3, 1).expect("2026-03-01 is a real date"),
        from: "2026-01-01".to_string(),
        to: "2026-02-01".to_string(),
        title: Some("Bullet Fixture".to_string()),
        include_old: false,
        findings: Vec::new(),
        enrichment: ragent_research::deck_enrich::DeckEnrichment::default(),
        diffed_findings: Vec::new(),
    }
}

/// Render the deck of the `older` / `newer` pair.
fn deck(older: &str, newer: &str) -> String {
    let older = parse_ok(older);
    let newer = parse_ok(newer);
    render_deck(&diff_reports(&older, &newer), &options()).markdown
}

/// The bullet lines of `markdown` that carry `status`.
///
/// An item bullet is ``- `TAG` text``, so a line is matched on the status tag
/// followed by a space: `- \`NEW\` `.
fn bullets(markdown: &str, status: &str) -> Vec<String> {
    let prefix = format!("- `{status}` ");
    markdown
        .lines()
        .filter(|line| line.starts_with(&prefix))
        .map(str::to_string)
        .collect()
}

/// The item text of a rendered bullet, with its status tag removed.
fn bullet_text(bullet: &str) -> &str {
    match bullet
        .strip_prefix("- `")
        .and_then(|rest| rest.split_once("` "))
    {
        Some((_, text)) => text,
        None => panic!("an item bullet is `- `TAG` text`, got: {bullet}"),
    }
}

/// A report whose `## In-Project Cross-References` list holds one bullet, which
/// is where a model-generated sentence with Markdown-significant characters can
/// reach a slide.
fn implication_report(body: &str) -> String {
    format!(
        "# Bullet Fixture\n\n## Executive Summary\n\nA deck fixture for bullet safety.\n\n\
         ## In-Project Cross-References\n\n- {body}\n"
    )
}

/// The baseline every bullet case diffs against: one cross-reference and one
/// finding, neither of which a newer report repeats.
const BASELINE: &str = "# Bullet Fixture\n\n## Executive Summary\n\nA deck fixture for bullet safety.\n\n\
                        ## In-Project Cross-References\n\n- `base.rs` - a baseline reference\n\n\
                        ## Findings\n\n### **Finding 1** - A baseline finding\n\nA baseline body. [#1]\n";

// -- FR-022: escaping ------------------------------------------------------

#[test]
fn test_deck_bullet_escapes_a_pipe_so_it_cannot_start_a_table() {
    let markdown = deck(
        BASELINE,
        &implication_report("Lexical | dense retrieval beats either alone."),
    );

    let items = bullets(&markdown, "NEW");
    assert_eq!(items.len(), 1, "the new item is rendered once");
    let text = bullet_text(&items[0]);
    assert!(
        text.contains("Lexical \\| dense"),
        "the pipe must be escaped so it cannot start a table: {text}"
    );
    assert!(
        !text.contains("Lexical | dense"),
        "an unescaped pipe could split the bullet into table cells: {text}"
    );
}

#[test]
fn test_deck_bullet_escapes_a_backtick_so_it_cannot_open_a_code_span() {
    let markdown = deck(
        BASELINE,
        &implication_report("Run `cargo test` before landing a change."),
    );

    let items = bullets(&markdown, "NEW");
    assert_eq!(items.len(), 1, "the new item is rendered once");
    let text = bullet_text(&items[0]);
    assert!(
        text.contains("\\`cargo test\\`"),
        "backticks must be escaped so they cannot open a code span: {text}"
    );
    assert!(
        !text.contains("`cargo test`"),
        "an unescaped backtick pair would render as code: {text}"
    );
}

#[test]
fn test_deck_bullet_escapes_a_leading_hash_so_it_cannot_create_a_heading() {
    let markdown = deck(
        BASELINE,
        &implication_report("# Not a heading, just an implication."),
    );

    let items = bullets(&markdown, "NEW");
    assert_eq!(items.len(), 1, "the new item is rendered once");
    let text = bullet_text(&items[0]);
    assert!(
        text.starts_with("\\# "),
        "a leading hash must be escaped so it cannot open a heading: {text}"
    );
    assert!(
        !text.starts_with("# "),
        "an unescaped leading hash would create a heading: {text}"
    );
}

#[test]
fn test_deck_bullet_escapes_only_a_leading_hash() {
    let markdown = deck(
        BASELINE,
        &implication_report("Issue #42 tracks the regression."),
    );

    let items = bullets(&markdown, "NEW");
    assert_eq!(items.len(), 1, "the new item is rendered once");
    let text = bullet_text(&items[0]);
    assert!(
        text.contains("Issue #42"),
        "a hash inside a sentence is ordinary text and stays unescaped: {text}"
    );
}

#[test]
fn test_deck_bullet_escapes_a_hostile_finding_headline() {
    // A finding's identity is its headline (FR-009), so the headline reaches
    // the slide and must be escaped on the same rules as any other bullet.
    let newer = "# Bullet Fixture\n\n## Executive Summary\n\nA deck fixture for bullet safety.\n\n\
                 ## Findings\n\n### **Finding 1** - Pipes | and `ticks` in a headline\n\n\
                 A fresh body. [#1]\n";
    let markdown = deck(BASELINE, newer);

    let items = bullets(&markdown, "NEW");
    assert!(
        items
            .iter()
            .any(|item| bullet_text(item).contains("Pipes \\| and \\`ticks\\`")),
        "the finding headline must be escaped: {items:?}"
    );
}

#[test]
fn test_deck_bullet_escapes_the_escape_character_itself() {
    // A report that already contains `\|` must not lose its backslash and
    // leave a bare pipe on the slide.
    let markdown = deck(BASELINE, &implication_report("Already \\| escaped."));

    let items = bullets(&markdown, "NEW");
    assert_eq!(items.len(), 1, "the new item is rendered once");
    let text = bullet_text(&items[0]);
    assert!(
        text.contains("\\\\|"),
        "an existing backslash must be escaped rather than dropped: {text}"
    );
}

// -- FR-022: the 200-character cap ----------------------------------------

#[test]
fn test_deck_bullet_caps_a_long_item_with_a_trailing_ellipsis() {
    let long = format!("{}and much more.", "word ".repeat(120));
    let markdown = deck(BASELINE, &implication_report(&long));

    let items = bullets(&markdown, "NEW");
    assert_eq!(items.len(), 1, "the new item is rendered once");
    let text = bullet_text(&items[0]);
    assert!(
        text.ends_with(ELLIPSIS),
        "a truncated bullet ends in an ellipsis: {text}"
    );
    // The ellipsis marks the cut and is appended after it, so the rendered head
    // is inside the cap and the whole bullet is inside it plus the marker.
    let head = text
        .strip_suffix(ELLIPSIS)
        .expect("the bullet was truncated");
    assert!(
        head.chars().count() <= CONDENSE_MAX_CHARS,
        "the cut text is capped at {CONDENSE_MAX_CHARS} characters, got {}: {text}",
        head.chars().count()
    );
    assert!(
        text.chars().count() <= CONDENSE_MAX_CHARS + ELLIPSIS.chars().count(),
        "a capped bullet stays inside the cap plus its ellipsis, got {}: {text}",
        text.chars().count()
    );
}

#[test]
fn test_deck_bullet_does_not_cap_an_item_at_exactly_the_limit() {
    // The cap is `<= 200`, so a bullet of exactly the limit is kept whole: the
    // trailing ellipsis marks truncation and must not appear otherwise.
    let body = format!("{} end", "a".repeat(CONDENSE_MAX_CHARS - 4));
    let markdown = deck(BASELINE, &implication_report(&body));

    let items = bullets(&markdown, "NEW");
    assert_eq!(items.len(), 1, "the new item is rendered once");
    let text = bullet_text(&items[0]);
    assert_eq!(
        text.chars().count(),
        CONDENSE_MAX_CHARS,
        "a bullet of exactly the limit is kept whole: {text}"
    );
    assert!(
        !text.ends_with(ELLIPSIS),
        "an untruncated bullet carries no ellipsis: {text}"
    );
}

#[test]
fn test_deck_bullet_caps_after_escaping_so_the_escaped_bullet_stays_in_the_cap() {
    // Every pipe doubles in length when escaped, so measuring the cap on the
    // unescaped text would let the rendered bullet exceed the limit.
    let pipes = format!("{} end.", "a | b ".repeat(60));
    let markdown = deck(BASELINE, &implication_report(&pipes));

    let items = bullets(&markdown, "NEW");
    assert_eq!(items.len(), 1, "the new item is rendered once");
    let text = bullet_text(&items[0]);
    assert!(
        text.contains("\\|"),
        "the case must actually exercise escaping: {text}"
    );
    assert!(
        text.chars().count() <= CONDENSE_MAX_CHARS,
        "the escaped bullet must stay inside the cap, got {}: {text}",
        text.chars().count()
    );
    assert!(
        text.ends_with(ELLIPSIS),
        "a capped bullet ends in an ellipsis: {text}"
    );
}

#[test]
fn test_deck_bullet_cap_is_counted_in_characters_not_bytes() {
    // A multi-byte character counts as one, so a bullet of exactly the limit in
    // characters is kept whole even though it is longer in bytes.
    let body = format!("{} end", "\u{e9}".repeat(CONDENSE_MAX_CHARS - 4));
    let markdown = deck(BASELINE, &implication_report(&body));

    let items = bullets(&markdown, "NEW");
    assert_eq!(items.len(), 1, "the new item is rendered once");
    let text = bullet_text(&items[0]);
    assert_eq!(
        text.chars().count(),
        CONDENSE_MAX_CHARS,
        "the cap is a character count, not a byte count"
    );
    assert!(
        !text.ends_with(ELLIPSIS),
        "a bullet at the limit in characters is not truncated: {text}"
    );
}
