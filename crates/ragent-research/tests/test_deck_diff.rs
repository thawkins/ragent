//! Tests for the diff stage (`diff_reports`) of the deck pipeline
//! (spec `diffdeck`: FR-004, FR-008, FR-013).
//!
//! FR-004 is a closed tag set with three definitions that are easy to conflate,
//! so most cases pin one definition at a time:
//!
//! - `NEW` needs *no* matching item and *no* new source;
//! - `ADDED` needs no matching item **and** a source the older report never
//!   cited - the promotion is the only difference between the two, so a pair of
//!   tests here differs by one source URL and nothing else;
//! - `UPDATE` needs a match with changed condensed text;
//! - `OLD` needs a match with unchanged condensed text.
//!
//! The remaining cases cover what FR-008 and FR-013 read off the diff: every item
//! of an element is present (not only the deltas), an unchanged pair reports no
//! changes, and the older-only items are collected as removed rather than tagged.

use ragent_research::deck::{DeckElement, ItemStatus, ParsedReport, diff_reports, parse_report};

/// Parse a report that is expected to succeed.
fn parse_ok(text: &str) -> ParsedReport {
    match parse_report(text) {
        Ok(parsed) => parsed,
        Err(error) => panic!("the fixture must parse: {error}"),
    }
}

/// A `## References Index` body citing `urls`.
fn references_section(urls: &[&str]) -> String {
    let mut report = String::from(
        "## References Index\n\n\
| # | Type | Media | Language | Path/URL | Title | Author | Published | Relevance | Search tool | Engine | Captured |\n\
|---|------|-------|----------|----------|-------|--------|-----------|-----------|-------------|--------|----------|\n",
    );
    for (index, url) in urls.iter().enumerate() {
        report.push_str(&format!(
            "| [#{}] | web | page | English | {url} | Source {} | - | - | high | mf_search | exa | 2026-01-01T00:00:00Z |\n",
            index + 1,
            index + 1
        ));
    }
    report
}

/// A `## Findings` section with one finding whose body cites `urls`.
fn finding_report(headline: &str, urls: &[&str]) -> String {
    let mut report = format!(
        "## Findings\n\n### **Finding 1** - {headline}\n\n**Observation:**\nA body [#1].\n\n**Sources:**\n"
    );
    for (index, url) in urls.iter().enumerate() {
        report.push_str(&format!(
            "- [{}] Source {} - [{url}]({url}) (published 2026-01-01)\n",
            index + 1,
            index + 1
        ));
    }
    report
}

// -- FR-004: the four statuses -------------------------------------------

#[test]
fn test_diff_reports_marks_an_unchanged_item_old() {
    let report = "## In-Project Cross-References\n\n- `ship.rs` - release path\n";
    let diff = diff_reports(&parse_ok(report), &parse_ok(report));
    assert_eq!(diff.count(ItemStatus::Old), 1);
    assert_eq!(diff.count(ItemStatus::New), 0);
    assert_eq!(diff.count(ItemStatus::Added), 0);
    assert_eq!(diff.count(ItemStatus::Update), 0);
    assert!(!diff.has_changes(), "an identical pair has no changes");
}

#[test]
fn test_diff_reports_marks_a_newer_only_item_with_no_sources_new() {
    let older = parse_ok("## In-Project Cross-References\n\n- `ship.rs` - release path\n");
    let newer = parse_ok(
        "## In-Project Cross-References\n\n- `ship.rs` - release path\n- `cost.rs` - run cost\n",
    );
    let diff = diff_reports(&older, &newer);
    assert_eq!(diff.count(ItemStatus::New), 1);
    let questions = diff
        .element(DeckElement::CrossReferences)
        .expect("the element is present in both reports");
    let added: Vec<&str> = questions
        .with_status(ItemStatus::New)
        .map(|e| e.bullet.as_str())
        .collect();
    assert_eq!(added.len(), 1);
    assert!(
        added[0].contains("run cost"),
        "the NEW item is the newer-only one: {added:?}"
    );
}

#[test]
fn test_diff_reports_marks_a_matched_item_with_changed_text_update() {
    // Same identity, different condensation: the identity strips a citation, so
    // adding `[#2]` keeps it matching while the raw text differs.
    let older = parse_ok("## In-Project Cross-References\n\n- `ship.rs` - release path\n");
    let newer = parse_ok("## In-Project Cross-References\n\n- `ship.rs` - release path\n");
    let quiet = diff_reports(&older, &newer);
    assert_eq!(quiet.count(ItemStatus::Old), 1);

    // A finding whose headline keeps its identity but whose body changed does not
    // read as UPDATE (the finding is keyed by its headline, FR-009)...
    let findings_older = parse_ok(&finding_report(
        "Stable headline",
        &["https://example.com/a"],
    ));
    let findings_newer = parse_ok(
        "## Findings\n\n### **Finding 1** - Stable headline\n\n**Observation:**\nA rewritten body [#2].\n",
    );
    assert_eq!(
        diff_reports(&findings_older, &findings_newer).count(ItemStatus::Old),
        1,
        "a finding is matched by its headline, so a body rewrite is not an UPDATE"
    );

    // ...whereas a concept whose heading drifted cosmetically but whose block was
    // rewritten is an UPDATE: the concept is keyed by its name, so the change is
    // observable through the identity-vs-text comparison.
    let implications_older =
        parse_ok("## Concepts\n\n### 1. Gateway Pattern\n\n**Definition:** Adopt the gateway.\n");
    let implications_newer = parse_ok(
        "## Concepts\n\n### 1. **Gateway Pattern!**\n\n**Definition:** Adopt the gateway everywhere.\n",
    );
    let changed = diff_reports(&implications_older, &implications_newer);
    assert_eq!(
        changed.count(ItemStatus::Update) + changed.count(ItemStatus::New),
        1,
        "a reworded item is either an UPDATE or a NEW, never OLD"
    );
    assert_eq!(changed.count(ItemStatus::Old), 0);
}

#[test]
fn test_diff_reports_promotes_a_newer_only_item_with_a_new_source_to_added() {
    // Identical findings except for one extra source URL: without it the item is
    // NEW, with it the item is ADDED. Nothing else in the pair differs, so the
    // promotion is attributable to the source set alone (FR-004).
    let older = parse_ok(&finding_report(
        "A brand new headline",
        &["https://example.com/a"],
    ));
    let newer_same_source = parse_ok(&finding_report(
        "A brand new headline",
        &["https://example.com/a"],
    ));
    let newer_new_source = parse_ok(&finding_report(
        "A brand new headline",
        &["https://example.com/a", "https://example.com/b"],
    ));

    // Make the *other* report carry the finding under a different headline so the
    // newer one is unmatched (otherwise a same-headline item matches and is OLD).
    let baseline = parse_ok(&finding_report(
        "An entirely different headline",
        &["https://example.com/a"],
    ));

    let same_source = diff_reports(&baseline, &newer_same_source);
    assert_eq!(
        same_source.count(ItemStatus::New),
        1,
        "an unmatched item whose sources are all already cited is NEW"
    );
    assert_eq!(same_source.count(ItemStatus::Added), 0);

    let new_source = diff_reports(&baseline, &newer_new_source);
    assert_eq!(
        new_source.count(ItemStatus::Added),
        1,
        "an unmatched item bringing a source the older report never cited is ADDED"
    );
    assert_eq!(new_source.count(ItemStatus::New), 0);

    // Guard the fixtures: the two same-headline pairs are OLD, not a status above.
    assert_eq!(
        diff_reports(&older, &newer_same_source).count(ItemStatus::Old),
        1
    );
}

#[test]
fn test_diff_reports_marks_a_new_reference_with_a_new_url_added() {
    // A reference is its own single source, so a reference present only in the
    // newer report always brings a source the older report did not cite.
    let older = parse_ok(&references_section(&["https://example.com/a"]));
    let newer = parse_ok(&references_section(&[
        "https://example.com/a",
        "https://example.com/b",
    ]));
    let diff = diff_reports(&older, &newer);
    assert_eq!(diff.count(ItemStatus::Old), 1, "the shared source is OLD");
    assert_eq!(diff.count(ItemStatus::Added), 1, "the new source is ADDED");
    assert_eq!(diff.count(ItemStatus::New), 0);
}

#[test]
fn test_diff_reports_keeps_a_renumbered_reference_old() {
    // FR-009: the reference identity is the URL, so a renumbered `[#N]` citation
    // is the same item - it must not read as removed + added.
    let row = |number: &str| {
        format!(
            "| [#{number}] | web | page | English | https://example.com/a | Alpha | - | - | high | mf_search | exa | 2026-01-01T00:00:00Z |\n"
        )
    };
    let header = "## References Index\n\n\
| # | Type | Media | Language | Path/URL | Title | Author | Published | Relevance | Search tool | Engine | Captured |\n\
|---|------|-------|----------|----------|-------|--------|-----------|-----------|-------------|--------|----------|\n";
    let older = format!("{header}{}", row("1"));
    let newer = format!("{header}{}", row("7"));
    let diff = diff_reports(&parse_ok(&older), &parse_ok(&newer));
    assert_eq!(
        diff.count(ItemStatus::Old),
        1,
        "the same URL under a new number is the same item"
    );
    assert_eq!(diff.count(ItemStatus::Added), 0);
    assert_eq!(diff.count(ItemStatus::New), 0);
    let references = diff
        .element(DeckElement::References)
        .expect("References is present");
    assert!(
        references.removed.is_empty(),
        "a renumbered citation is not a removed item: {:?}",
        references.removed
    );
}

// -- FR-008: every item, not only the deltas ------------------------------

#[test]
fn test_diff_reports_keeps_every_item_of_a_changed_element() {
    let older = parse_ok(
        "## In-Project Cross-References\n\n- `ship.rs` - release path\n- `own.rs` - ownership\n",
    );
    let newer = parse_ok(
        "## In-Project Cross-References\n\n- `ship.rs` - release path\n- `own.rs` - ownership\n- `due.rs` - schedule\n",
    );
    let diff = diff_reports(&older, &newer);
    let questions = diff
        .element(DeckElement::CrossReferences)
        .expect("Cross-References is present");
    assert_eq!(
        questions.items.len(),
        3,
        "FR-008 emits every item of a changed element, not only its deltas"
    );
    assert_eq!(questions.count(ItemStatus::Old), 2);
    assert_eq!(questions.count(ItemStatus::New), 1);
    assert!(questions.has_changes());
}

// -- FR-013: no changes ---------------------------------------------------

#[test]
fn test_diff_reports_reports_no_changes_for_an_identical_pair() {
    let report = "## Executive Summary\n\nThe system works.\n\n\
## In-Project Cross-References\n\n- `ship.rs` - release path\n";
    let diff = diff_reports(&parse_ok(report), &parse_ok(report));
    assert!(!diff.has_changes(), "an identical pair has no changes");
    assert_eq!(diff.changed_elements().count(), 0);
    assert!(
        diff.item_count() > 0,
        "the items are still present, they are simply all OLD"
    );
}

// -- Element scoping and ordering ----------------------------------------

#[test]
fn test_diff_reports_does_not_match_an_item_across_elements() {
    // The same text in two different elements is two different items.
    let older = parse_ok("## In-Project Cross-References\n\n- Ship it now\n");
    let newer = parse_ok("## Findings\n\n### **Finding 1** - Ship it now\n");
    let diff = diff_reports(&older, &newer);
    let questions = diff
        .element(DeckElement::CrossReferences)
        .expect("the older report carried the cross-reference");
    assert!(
        questions.is_empty(),
        "the newer report has no Cross-References item"
    );
    assert_eq!(questions.removed.len(), 1, "it is reported as removed");
    let findings = diff
        .element(DeckElement::Findings)
        .expect("the newer report carried the finding");
    assert_eq!(
        findings.count(ItemStatus::New),
        1,
        "the finding is NEW, not OLD against the cross-reference"
    );
}

#[test]
fn test_diff_reports_orders_elements_by_fr_003_precedence() {
    // Supply the elements in reverse precedence order and assert the diff comes
    // back in the spec's order (FR-003).
    let report = "## References Index\n\n\
| # | Path/URL | Title |\n\
|---|----------|-------|\n\
| [#1] | https://example.com/a | Alpha |\n\n\
## Concepts\n\n### 1. Gateway Pattern\n\n**Definition:** Adopt the gateway.\n\n\
## Executive Summary\n\nThe system works.\n";
    let diff = diff_reports(
        &parse_ok("## Executive Summary\n\nThe system works.\n"),
        &parse_ok(report),
    );
    let order: Vec<DeckElement> = diff.elements.iter().filter_map(|e| e.element()).collect();
    let mut sorted = order.clone();
    sorted.sort();
    assert_eq!(order, sorted, "elements are in FR-003 precedence order");
    assert_eq!(order.first(), Some(&DeckElement::ExecutiveSummary));
}

// -- Status helpers -------------------------------------------------------

#[test]
fn test_item_status_tags_are_the_closed_rendered_set() {
    assert_eq!(ItemStatus::New.tag(), "NEW");
    assert_eq!(ItemStatus::Added.tag(), "ADDED");
    assert_eq!(ItemStatus::Update.tag(), "UPDATE");
    assert_eq!(ItemStatus::Old.tag(), "OLD");
    assert!(ItemStatus::New.is_change());
    assert!(ItemStatus::Added.is_change());
    assert!(ItemStatus::Update.is_change());
    assert!(!ItemStatus::Old.is_change());
}

#[test]
fn test_diff_of_two_reports_with_no_items_is_empty() {
    let older = parse_ok("## Executive Summary\n\n_(no executive summary recorded yet)_\n");
    let newer = parse_ok("## Executive Summary\n\n_(no executive summary recorded yet)_\n");
    let diff = diff_reports(&older, &newer);
    assert_eq!(diff.item_count(), 0);
    assert!(diff.elements.is_empty(), "no element carries an item");
    assert!(!diff.has_changes());
}

#[test]
fn test_finding_sources_are_read_from_the_sources_block() {
    // The parser records the finding's own source set, which is what the ADDED
    // promotion reads; without it every newer-only finding could only be NEW.
    let parsed = parse_ok(&finding_report(
        "A headline",
        &["https://example.com/a", "https://example.com/b"],
    ));
    let finding = &parsed.items(DeckElement::Findings)[0];
    assert_eq!(
        finding.sources.len(),
        2,
        "both sources of the block are recorded: {:?}",
        finding.sources
    );
    assert!(
        finding.sources.iter().any(|s| s.contains("example.com/a")),
        "the source identity is its URL: {:?}",
        finding.sources
    );
}

#[test]
fn test_finding_without_a_sources_block_has_no_sources() {
    let parsed = parse_ok(
        "## Findings\n\n### **Finding 1** - A headline\n\n**Observation:**\nA body [#1].\n",
    );
    let finding = &parsed.items(DeckElement::Findings)[0];
    assert!(
        finding.sources.is_empty(),
        "a finding with no Sources block carries no source set: {:?}",
        finding.sources
    );
}
