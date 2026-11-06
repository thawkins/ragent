//! Tests for the render stage (`render_deck`) of the deck pipeline
//! (spec `diffdeck`: FR-003, FR-005, FR-008, FR-010, FR-013, FR-015).
//!
//! Every case is a pure in-memory assertion over `render_deck`: no fixture is
//! written to disk and no clock is read, so a failure is always a rendering
//! rule and never an environment artefact.
//!
//! The cases are grouped by the requirement they pin:
//!
//! - FR-003 - one cover slide, element slides in the element precedence, one
//!   summary slide, all separated by `---`;
//! - FR-005 - the cover slide's title, generation date, coverage window,
//!   research id and the three counts;
//! - FR-008 - every item of a changed element, labelled, not only the deltas;
//! - FR-013 - the no-changes notice with the cover and summary still present;
//! - FR-010 - the summary slide's element count, per-status counts and names.

use chrono::NaiveDate;
use ragent_research::deck::{
    DeckElement, DeckRenderOptions, ItemStatus, NO_CHANGES_NOTICE, ParsedReport, diff_reports,
    parse_report, render_deck,
};

/// The generation date every case renders with, so output is comparable.
fn generated_on() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 3, 1).expect("2026-03-01 is a real date")
}

/// The render options a deck of the `2026-01-01` to `2026-02-01` pair uses.
fn options(title: Option<&str>) -> DeckRenderOptions {
    DeckRenderOptions {
        name: "graphindex".to_string(),
        generated_on: generated_on(),
        from: "2026-01-01".to_string(),
        to: "2026-02-01".to_string(),
        title: title.map(str::to_string),
        include_old: false,
        findings: Vec::new(),
        enrichment: ragent_research::deck_enrich::DeckEnrichment::default(),
        diffed_findings: Vec::new(),
    }
}

/// Parse a report that is expected to succeed.
fn parse_ok(text: &str) -> ParsedReport {
    match parse_report(text) {
        Ok(parsed) => parsed,
        Err(error) => panic!("the fixture must parse: {error}"),
    }
}

/// Render the deck of two report texts.
fn render(older: &str, newer: &str) -> String {
    let older = parse_ok(older);
    let newer = parse_ok(newer);
    let diff = diff_reports(&older, &newer);
    let title = newer.title.clone().or_else(|| older.title.clone());
    render_deck(&diff, &options(title.as_deref())).markdown
}

/// Render the deck of two report texts with `--include-old` (FR-015).
fn render_include_old(older: &str, newer: &str) -> String {
    let older = parse_ok(older);
    let newer = parse_ok(newer);
    let diff = diff_reports(&older, &newer);
    let title = newer.title.clone().or_else(|| older.title.clone());
    render_deck(&diff, &options(title.as_deref()).with_include_old(true)).markdown
}

/// The slides of a deck: the text split on the `---` rules.
fn slides(markdown: &str) -> Vec<String> {
    markdown
        .split("\n---\n")
        .map(|slide| slide.trim().to_string())
        .collect()
}

/// Whether `needle` appears as a whole line of `haystack`.
fn has_line(haystack: &str, needle: &str) -> bool {
    haystack.lines().any(|line| line.trim_end() == needle)
}

/// The element headings of a deck, in document order.
fn element_headings(markdown: &str) -> Vec<&'static str> {
    markdown
        .lines()
        .filter_map(|line| line.strip_prefix("## "))
        .filter_map(DeckElement::from_name)
        .map(DeckElement::label)
        .collect()
}

/// A report whose only changed content is a new cross-reference.
fn report_with_extra_question(extra: bool) -> String {
    let mut report = String::from(
        "# Title: Graph Indexing\n\n## In-Project Cross-References\n\n- `ship.rs` - release path\n",
    );
    if extra {
        report.push_str("\n- `recall.rs` - recall tuning\n");
    }
    report
}

// -- FR-003: slide structure and precedence ------------------------------

#[test]
fn test_render_deck_emits_a_cover_an_element_slide_and_a_summary() {
    let deck = render(
        &report_with_extra_question(false),
        &report_with_extra_question(true),
    );
    let slides = slides(&deck);
    assert_eq!(
        slides.len(),
        3,
        "cover + one element slide + summary:\n{deck}"
    );
    assert!(slides[0].starts_with("# "), "slide 0 is the cover:\n{deck}");
    assert!(
        slides[1].starts_with("## Cross-References"),
        "slide 1 is the changed element:\n{deck}"
    );
    assert!(
        slides[2].starts_with("## Summary"),
        "slide 2 is the summary:\n{deck}"
    );
}

#[test]
fn test_render_deck_orders_element_slides_by_element_precedence() {
    // The newer report introduces a change in the *late* elements and the older
    // report had content in an *early* one, so document order and element order
    // disagree unless the renderer enforces the precedence.
    let older = "## References Index\n\n\
| # | Type | Media | Language | Path/URL | Title | Author | Published | Relevance | Search tool | Engine | Captured |\n\
|---|------|-------|----------|----------|-------|--------|-----------|-----------|-------------|--------|----------|\n\
| [#1] | web | page | English | https://example.com/a | A | - | - | high | mf_search | exa | 2026-01-01T00:00:00Z |\n";
    let newer = format!(
        "## Executive Summary\n\nA changed summary.\n\n{}",
        older
            .replace("https://example.com/a", "https://example.com/b")
            .replace("| A |", "| B |")
    );
    let deck = render(older, &newer);
    assert_eq!(
        element_headings(&deck),
        vec!["Executive Summary", "References"],
        "element slides follow the FR-003 precedence:\n{deck}"
    );
}

#[test]
fn test_render_deck_never_emits_a_slide_for_an_unchanged_element() {
    // The Executive Summary never changes, so it produces no slide even though
    // the report carries it (FR-008 emits a slide only for a changed element).
    let older = "## Executive Summary\n\nThe same summary.\n\n## In-Project Cross-References\n\n- `ship.rs` - release path\n";
    let newer = "## Executive Summary\n\nThe same summary.\n\n## In-Project Cross-References\n\n- `recall.rs` - recall tuning\n";
    let deck = render(older, newer);
    assert_eq!(element_headings(&deck), vec!["Cross-References"], "{deck}");
}

// -- FR-005: the cover slide ---------------------------------------------

#[test]
fn test_render_deck_cover_shows_title_id_date_and_coverage_window() {
    let deck = render(
        &report_with_extra_question(false),
        &report_with_extra_question(true),
    );
    let cover = &slides(&deck)[0];
    assert!(cover.contains("Graph Indexing"), "{cover}");
    assert!(has_line(cover, "**Research id:** graphindex"), "{cover}");
    assert!(has_line(cover, "**Generated:** 2026-03-01"), "{cover}");
    assert!(
        has_line(cover, "**Coverage:** 2026-01-01 to 2026-02-01"),
        "{cover}"
    );
}

#[test]
fn test_render_deck_cover_counts_slides_changes_and_old_items() {
    // One change (the new question) and one unchanged item (the old question).
    let deck = render(
        &report_with_extra_question(false),
        &report_with_extra_question(true),
    );
    let cover = &slides(&deck)[0];
    assert!(has_line(cover, "| Element slides | 1 |"), "{cover}");
    assert!(has_line(cover, "| Change items | 1 |"), "{cover}");
    // The fixture holds two questions: the kept one and the added one, so the
    // unchanged count is the single question both reports carry.
    assert!(
        has_line(cover, "| Unchanged (OLD) items | 1 |"),
        "the OLD count is named after its tag:\n{cover}"
    );
}

#[test]
fn test_render_deck_cover_falls_back_to_an_untitled_title() {
    let deck = render(
        "## In-Project Cross-References\n\n- `ship.rs` - release path\n",
        "## In-Project Cross-References\n\n- `recall.rs` - recall tuning\n",
    );
    let cover = &slides(&deck)[0];
    assert!(cover.contains("Untitled research deck"), "{cover}");
}

#[test]
fn test_render_deck_cover_shows_the_legacy_baseline_stamp() {
    let older = parse_ok("## In-Project Cross-References\n\n- `ship.rs` - release path\n");
    let newer = parse_ok("## In-Project Cross-References\n\n- `recall.rs` - recall tuning\n");
    let options = DeckRenderOptions {
        from: "legacy".to_string(),
        ..options(None)
    };
    let deck = render_deck(&diff_reports(&older, &newer), &options);
    assert!(
        deck.markdown.contains("**Coverage:** legacy to 2026-02-01"),
        "{}",
        deck.markdown
    );
}

// -- FR-008: a slide is complete, not only its deltas ---------------------

#[test]
fn test_render_deck_emits_every_item_of_a_changed_element() {
    let deck = render(
        &report_with_extra_question(false),
        &report_with_extra_question(true),
    );
    let slide = &slides(&deck)[1];
    assert!(
        slide.contains("- `OLD` \\`ship.rs\\` - release path"),
        "{slide}"
    );
    assert!(
        slide.contains("- `NEW` \\`recall.rs\\` - recall tuning"),
        "{slide}"
    );
}

#[test]
fn test_render_deck_labels_every_item_with_one_status_tag() {
    let deck = render(
        "## In-Project Cross-References\n\n- `ship.rs` - release path\n- `fast.rs` - latency path\n",
        "## In-Project Cross-References\n\n- `ship.rs` - release path\n- `cheap.rs` - cost path\n- `new.rs` - a brand new item\n",
    );
    let slide = &slides(&deck)[1];
    let bullets: Vec<&str> = slide
        .lines()
        .filter(|line| line.starts_with("- `"))
        .collect();
    assert_eq!(
        bullets.len(),
        3,
        "three items in the newer report:\n{slide}"
    );
    for bullet in &bullets {
        let tags: Vec<&str> = ["NEW", "ADDED", "UPDATE", "OLD"]
            .into_iter()
            .filter(|tag| bullet.starts_with(&format!("- `{tag}` ")))
            .collect();
        assert_eq!(
            tags.len(),
            1,
            "exactly one tag on {bullet}: all => {tags:?}"
        );
    }
    assert!(
        slide.contains("- `OLD` \\`ship.rs\\` - release path"),
        "{slide}"
    );
    assert!(
        slide.contains("- `NEW` \\`cheap.rs\\` - cost path"),
        "{slide}"
    );
    assert!(
        slide.contains("- `NEW` \\`new.rs\\` - a brand new item"),
        "{slide}"
    );
}

#[test]
fn test_render_deck_distinguishes_a_rewritten_item_from_an_unchanged_one() {
    // The identity of a non-Findings element IS its normalised text (FR-009), so
    // a wholly rewritten item does not match and renders `NEW`; the unchanged
    // question stays `OLD` and still counts on the cover and in the summary even
    // though its own element produces no slide.
    let deck = render(
        "## Concepts\n\n### 1. Index Quality\n\n**Definition:** Index quality matters most.\n\n## In-Project Cross-References\n\n- `ship.rs` - release path\n",
        "## Concepts\n\n### 1. Chunking Strategy\n\n**Definition:** Chunking strategy matters most.\n\n## In-Project Cross-References\n\n- `ship.rs` - release path\n",
    );
    let all = slides(&deck);
    assert_eq!(all.len(), 3, "cover + one element slide + summary:\n{deck}");
    let concepts = &all[1];
    assert!(concepts.contains("## Chunking Strategy"), "{concepts}");
    assert!(concepts.contains("_`NEW`_"), "{concepts}");
    assert!(
        all[0].contains("| Unchanged (OLD) items | 1 |"),
        "the unchanged cross-reference is counted on the cover:\n{}",
        all[0]
    );
    let summary = all.last().map(String::as_str).unwrap_or_default();
    assert!(has_line(summary, "| OLD | 1 |"), "{summary}");
}

#[test]
fn test_render_deck_element_slide_header_counts_items_by_status() {
    let deck = render(
        &report_with_extra_question(false),
        &report_with_extra_question(true),
    );
    let slide = &slides(&deck)[1];
    assert!(
        slide.contains("NEW 1 - ADDED 0 - UPDATE 0 - OLD 1 - 2 item(s)"),
        "{slide}"
    );
}

// -- FR-013: the no-changes deck -----------------------------------------

#[test]
fn test_render_deck_renders_the_no_changes_notice_for_an_identical_pair() {
    let report = report_with_extra_question(true);
    let deck = render(&report, &report);
    assert!(has_line(&deck, "## No changes"), "{deck}");
    assert!(
        has_line(&deck, NO_CHANGES_NOTICE),
        "the exact FR-013 notice:\n{deck}"
    );
    assert_eq!(
        NO_CHANGES_NOTICE,
        "_No changes detected between the two reports._"
    );
}

#[test]
fn test_render_deck_no_changes_keeps_the_cover_and_the_summary() {
    let report = report_with_extra_question(true);
    let deck = render(&report, &report);
    let slides = slides(&deck);
    assert_eq!(slides.len(), 3, "cover + notice + summary:\n{deck}");
    assert!(slides[0].starts_with("# "), "{deck}");
    assert!(slides[1].starts_with("## No changes"), "{deck}");
    assert!(slides[2].starts_with("## Summary"), "{deck}");
}

#[test]
fn test_render_deck_no_changes_cover_reports_zero_slides_and_changes() {
    let report = report_with_extra_question(true);
    let deck = render(&report, &report);
    let cover = &slides(&deck)[0];
    assert!(has_line(cover, "| Element slides | 0 |"), "{cover}");
    assert!(has_line(cover, "| Change items | 0 |"), "{cover}");
    assert!(has_line(cover, "| Unchanged (OLD) items | 2 |"), "{cover}");
}

// -- FR-010: the closing summary slide -----------------------------------

#[test]
fn test_render_deck_summary_lists_the_element_count_and_names() {
    let newer = "## Executive Summary\n\nA changed summary.\n\n## In-Project Cross-References\n\n- `recall.rs` - recall tuning\n";
    let deck = render(
        "## In-Project Cross-References\n\n- `ship.rs` - release path\n",
        newer,
    );
    let all = slides(&deck);
    assert_eq!(
        all.len(),
        4,
        "cover + two element slides + summary:\n{deck}"
    );
    let summary = all.last().map(String::as_str).unwrap_or_default();
    assert!(has_line(summary, "**Element slides:** 2"), "{summary}");
    assert!(
        has_line(summary, "**Elements:** Executive Summary, Cross-References"),
        "the names follow the element precedence:\n{summary}"
    );
}

#[test]
fn test_render_deck_summary_counts_every_status() {
    let deck = render(
        "## In-Project Cross-References\n\n- `ship.rs` - release path\n- `fast.rs` - latency path\n",
        "## In-Project Cross-References\n\n- `ship.rs` - release path\n- `cheap.rs` - cost path\n",
    );
    let summary = &slides(&deck)[2];
    assert!(has_line(summary, "| NEW | 1 |"), "{summary}");
    assert!(has_line(summary, "| ADDED | 0 |"), "{summary}");
    assert!(has_line(summary, "| UPDATE | 0 |"), "{summary}");
    assert!(has_line(summary, "| OLD | 1 |"), "{summary}");
}

#[test]
fn test_render_deck_summary_reports_no_elements_for_an_unchanged_pair() {
    let report = report_with_extra_question(true);
    let deck = render(&report, &report);
    let summary = &slides(&deck)[2];
    assert!(has_line(summary, "**Element slides:** 0"), "{summary}");
    assert!(
        has_line(summary, "**Elements:** none (no changes)"),
        "{summary}"
    );
    assert!(has_line(summary, "| OLD | 2 |"), "{summary}");
}

// -- FR-015: --include-old ------------------------------------------------

#[test]
fn test_render_deck_include_old_emits_a_slide_for_a_no_change_element() {
    let report = report_with_extra_question(true);
    let deck = render_include_old(&report, &report);
    assert_eq!(
        element_headings(&deck),
        vec!["Cross-References"],
        "the unchanged element gets a slide under --include-old:\n{deck}"
    );
    assert_eq!(deck.matches(NO_CHANGES_NOTICE).count(), 0, "{deck}");
    let cover = &slides(&deck)[0];
    assert!(has_line(cover, "| Element slides | 1 |"), "{cover}");
}

#[test]
fn test_render_deck_include_old_does_not_add_an_empty_element() {
    // A section with no item is not an element that "contains only OLD items".
    let older = "## In-Project Cross-References\n\n- `ship.rs` - release path\n\n\
## Concepts\n\n### 1. Index Quality\n\n**Definition:** Index quality matters.\n";
    let newer = "## In-Project Cross-References\n\n- `ship.rs` - release path\n";
    let deck = render_include_old(older, newer);
    assert_eq!(element_headings(&deck), vec!["Cross-References"], "{deck}");
}

// -- NFR-002 / FR-021: purity --------------------------------------------

#[test]
fn test_render_deck_is_deterministic_for_the_same_inputs() {
    let older = report_with_extra_question(false);
    let newer = report_with_extra_question(true);
    assert_eq!(render(&older, &newer), render(&older, &newer));
}

#[test]
fn test_render_deck_reports_the_rendered_elements() {
    let older = parse_ok(&report_with_extra_question(false));
    let newer = parse_ok(&report_with_extra_question(true));
    let document = render_deck(&diff_reports(&older, &newer), &options(None));
    assert_eq!(document.elements, vec![DeckElement::CrossReferences]);
    assert_eq!(
        document.markdown.matches("\n---\n").count(),
        2,
        "two rules separate three slides:\n{}",
        document.markdown
    );
}

#[test]
fn test_render_deck_counts_no_change_items_as_old_in_the_diff() {
    // A guard against the cover and summary disagreeing with the diff itself.
    let older = parse_ok(&report_with_extra_question(false));
    let newer = parse_ok(&report_with_extra_question(true));
    let diff = diff_reports(&older, &newer);
    let document = render_deck(&diff, &options(None));
    let summary = slides(&document.markdown).pop().unwrap_or_default();
    for status in [
        ItemStatus::New,
        ItemStatus::Added,
        ItemStatus::Update,
        ItemStatus::Old,
    ] {
        assert!(
            has_line(
                &summary,
                &format!("| {} | {} |", status.tag(), diff.count(status))
            ),
            "{summary}"
        );
    }
}
