//! Inline tests for [`crate::deck_progress`] (spec `diffdeck` FR-031).
//!
//! They cover the three things the deck progress path must get right: a report
//! survives the trip out to the event bus and back, the tracker replaces a stage
//! row in place rather than appending one per transition, and the rendered
//! message starts with the header the message window matches on.

use super::*;
use ragent_research::deck_progress::{DeckEvent, DeckStage, DeckStepStatus};

#[test]
fn test_decode_round_trips_a_stage_report() {
    let event = DeckEvent::progress(DeckStage::EnrichFindings, 3, 7, "third finding");
    let encoded = encode_deck_progress_event("my-item", &event);
    let decoded = decode_deck_progress_event(&encoded).expect("decodes");
    assert_eq!(decoded.name, "my-item");
    assert_eq!(decoded.stage, DeckStage::EnrichFindings);
    assert_eq!(decoded.status, DeckStepStatus::Started);
    assert_eq!(decoded.detail, "third finding");
    assert_eq!(decoded.total, Some(7));
    assert_eq!(decoded.completed, 3);
    assert_eq!(decoded.event(), event);
}

#[test]
fn test_decode_ignores_foreign_messages() {
    assert!(decode_deck_progress_event("agent notice: hello").is_none());
    assert!(decode_deck_progress_event("__research_progress__{}").is_none());
    assert!(decode_deck_progress_event(DECK_PROGRESS_SENTINEL).is_none());
    assert!(decode_deck_progress_event(&format!("{DECK_PROGRESS_SENTINEL}not json")).is_none());
}

#[test]
fn test_decode_rejects_unknown_stage_and_status() {
    let bad_stage = format!(
        "{DECK_PROGRESS_SENTINEL}{{\"name\":\"x\",\"stage\":\"nope\",\"status\":\"started\",\"detail\":\"\",\"completed\":0,\"total\":null}}"
    );
    assert!(decode_deck_progress_event(&bad_stage).is_none());
    let bad_status = format!(
        "{DECK_PROGRESS_SENTINEL}{{\"name\":\"x\",\"stage\":\"select\",\"status\":\"exploded\",\"detail\":\"\",\"completed\":0,\"total\":null}}"
    );
    assert!(decode_deck_progress_event(&bad_status).is_none());
}

#[test]
fn test_tracker_replaces_a_stage_row_in_place() {
    let mut tracker = DeckTracker::new();
    tracker.apply(&DeckEvent::started(DeckStage::Select, "looking up"));
    tracker.apply(&DeckEvent::done(DeckStage::Select, "comparing A to B"));
    assert_eq!(
        tracker.rows.len(),
        1,
        "one row per stage, not per transition"
    );
    assert_eq!(tracker.rows[0].status, DeckStepStatus::Done);
    assert_eq!(tracker.rows[0].detail, "comparing A to B");
}

#[test]
fn test_tracker_keeps_stage_order() {
    let mut tracker = DeckTracker::new();
    for stage in [DeckStage::Select, DeckStage::Read, DeckStage::Diff] {
        tracker.apply(&DeckEvent::started(stage, "working"));
    }
    let order: Vec<DeckStage> = tracker.rows.iter().map(|row| row.stage).collect();
    assert_eq!(
        order,
        vec![DeckStage::Select, DeckStage::Read, DeckStage::Diff]
    );
}

#[test]
fn test_tracker_current_stage_is_the_live_one() {
    let mut tracker = DeckTracker::new();
    tracker.apply(&DeckEvent::done(DeckStage::Select, "done"));
    tracker.apply(&DeckEvent::started(DeckStage::Read, "reading"));
    let current = tracker.current().expect("a live stage");
    assert_eq!(current.stage, DeckStage::Read);
    assert_eq!(current.status, DeckStepStatus::Started);
}

#[test]
fn test_tracker_has_no_current_stage_when_all_are_terminal() {
    let mut tracker = DeckTracker::new();
    tracker.apply(&DeckEvent::done(DeckStage::Select, "done"));
    tracker.apply(&DeckEvent::skipped(DeckStage::Export, "no sink"));
    assert!(tracker.current().is_none());
}

#[test]
fn test_render_header_matches_the_progress_message_header() {
    // The message window replaces an existing progress message by matching its
    // first line, so a deck run must use the same header shape the session
    // tracker does or every stage would append a new bubble.
    let mut tracker = DeckTracker::new();
    tracker.apply(&DeckEvent::started(DeckStage::Select, "looking up"));
    let rendered = tracker.render("my-item");
    assert_eq!(
        rendered.lines().next(),
        Some("[research] Research Progress - `my-item`")
    );
    assert!(rendered.contains("Command: /research deck"));
}

#[test]
fn test_render_shows_the_unit_counter() {
    let mut tracker = DeckTracker::new();
    tracker.apply(&DeckEvent::progress(
        DeckStage::EnrichFindings,
        2,
        5,
        "a finding",
    ));
    let rendered = tracker.render("my-item");
    assert!(rendered.contains("[2/5]"), "rendered: {rendered}");
    assert!(rendered.contains("a finding"));
}

#[test]
fn test_render_marks_a_finished_run() {
    let mut tracker = DeckTracker::new();
    tracker.apply(&DeckEvent::done(DeckStage::Export, "complete"));
    assert!(!tracker.render("x").contains("Complete"));
    tracker.finish();
    assert!(tracker.render("x").contains("[ok] Complete"));
}

#[test]
fn test_row_counter_clamps_to_total() {
    let row = DeckStageRow {
        stage: DeckStage::Export,
        status: DeckStepStatus::Started,
        detail: String::new(),
        completed: 9,
        total: Some(4),
    };
    assert_eq!(row.counter(), "[4/4] ");
}

#[test]
fn test_row_counter_is_empty_without_a_total() {
    let row = DeckStageRow {
        stage: DeckStage::Select,
        status: DeckStepStatus::Started,
        detail: String::new(),
        completed: 0,
        total: None,
    };
    assert_eq!(row.counter(), "");
}
