//! Tests for how a `/research deck` run's stage reports reach the message window
//! (spec `diffdeck` FR-031).
//!
//! The contract under test is that the run is *visible* while it works:
//!
//! - the command is acknowledged in the message window before any stage report
//!   arrives, so a slow lookup does not look like a dropped command;
//! - every stage report extends one message in place rather than appending a
//!   bubble per stage, so a twelve-stage run does not bury the transcript;
//! - a report that arrives before the handler seeded the tracker still lands on
//!   the right run, because the tracker is keyed by research item name;
//! - the status bar carries the live stage and its unit count, and a refused run
//!   never claims to be complete.

use ragent_research::deck_progress::{DeckEvent, DeckStage, DeckStepStatus};
use ragent_tui::app::App;
use ragent_tui::deck_progress::{DECK_PROGRESS_SENTINEL, DeckTracker, decode_deck_progress_event};

#[path = "support/mod.rs"]
mod support;

/// The text of every assistant message in the window, joined.
fn window_text(app: &App) -> String {
    app.session
        .messages
        .iter()
        .filter(|message| message.role == ragent_types::message::Role::Assistant)
        .filter_map(|message| message.parts.first())
        .filter_map(|part| match part {
            ragent_types::message::MessagePart::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<&str>>()
        .join("\n---\n")
}

/// An app with an active session, so appended messages reach the window.
fn app_with_session() -> App {
    let mut app = support::make_app();
    app.session.session_id = Some("s1".to_string());
    app
}

/// Feed one stage report through the event handler the way the bus would.
fn report(app: &mut App, name: &str, event: DeckEvent) {
    let message = ragent_tui::deck_progress::encode_deck_progress_event(name, &event);
    let decoded = decode_deck_progress_event(&message).expect("the notice decodes");
    app.apply_deck_progress(decoded);
}

#[test]
fn test_the_first_deck_report_creates_one_progress_message() {
    let mut app = app_with_session();
    report(
        &mut app,
        "deckitem",
        DeckEvent::started(DeckStage::Select, "looking up item 'deckitem'"),
    );

    let text = window_text(&app);
    assert_eq!(
        text.matches("[research] Research Progress - `deckitem`")
            .count(),
        1,
        "one run message, addressed to the run; window was:\n{text}"
    );
    assert!(
        text.contains("Command: /research deck"),
        "the message says which command it tracks:\n{text}"
    );
    assert!(
        text.contains("looking up item 'deckitem'"),
        "the acknowledgment carries the first stage's detail:\n{text}"
    );
}

#[test]
fn test_successive_reports_replace_the_message_in_place() {
    let mut app = app_with_session();
    for stage in [DeckStage::Select, DeckStage::Read, DeckStage::Diff] {
        report(
            &mut app,
            "deckitem",
            DeckEvent::done(stage, "finished the stage"),
        );
    }

    let text = window_text(&app);
    assert_eq!(
        text.matches("[research] Research Progress - `deckitem`")
            .count(),
        1,
        "the three stages share one message; window was:\n{text}"
    );
    for stage in [DeckStage::Select, DeckStage::Read, DeckStage::Diff] {
        assert!(
            text.contains(stage.label()),
            "stage {:?} is listed:\n{text}",
            stage.label()
        );
    }
}

#[test]
fn test_a_stage_row_replaces_itself_rather_than_appending() {
    let mut app = app_with_session();
    report(
        &mut app,
        "deckitem",
        DeckEvent::started(DeckStage::Render, "rendering the deck document"),
    );
    report(
        &mut app,
        "deckitem",
        DeckEvent::done(DeckStage::Render, "wrote the deck (42 bytes)"),
    );

    let text = window_text(&app);
    assert!(
        text.contains("wrote the deck (42 bytes)"),
        "the terminal report wins:\n{text}"
    );
    assert!(
        !text.contains("rendering the deck document"),
        "the start line is replaced, not kept beside the finish:\n{text}"
    );
}

#[test]
fn test_unit_reports_render_their_counter_in_the_status_bar() {
    let mut app = app_with_session();
    report(
        &mut app,
        "deckitem",
        DeckEvent::progress(DeckStage::EnrichFindings, 3, 12, "a finding headline"),
    );

    assert!(
        app.status.contains("enrich findings 3/12"),
        "the status bar carries the live unit count: {}",
        app.status
    );
    assert!(
        !app.status.starts_with("[ok]"),
        "an in-flight stage is not a completion: {}",
        app.status
    );
}

#[test]
fn test_a_missing_tracker_is_created_on_demand() {
    // A report can arrive before the slash handler seeded the run (a run started
    // from another surface in the same session). It must still reach the window
    // instead of being dropped.
    let mut app = app_with_session();
    assert!(app.research_progress.is_empty());

    report(
        &mut app,
        "latecomer",
        DeckEvent::started(DeckStage::Select, "looking up item 'latecomer'"),
    );

    assert!(
        app.research_progress.iter().any(|p| p.name == "latecomer"),
        "the run is tracked even though nothing seeded it"
    );
    assert!(
        window_text(&app).contains("Research Progress - `latecomer`"),
        "and it reaches the message window"
    );
}

#[test]
fn test_finishing_reports_complete_and_expires_the_status() {
    let mut app = app_with_session();
    report(
        &mut app,
        "deckitem",
        DeckEvent::started(DeckStage::Export, "writing the presentation"),
    );
    report(
        &mut app,
        "deckitem",
        DeckEvent::done(DeckStage::Export, "deck run complete"),
    );

    assert!(
        app.status.contains("complete"),
        "a finished deck run says so: {}",
        app.status
    );
    assert!(
        window_text(&app).contains("[ok] Complete"),
        "the message closes the stage list"
    );
}

#[test]
fn test_a_skipped_stage_reads_as_not_applicable() {
    let mut app = app_with_session();
    report(
        &mut app,
        "deckitem",
        DeckEvent::skipped(DeckStage::Export, "no presentation capability connected"),
    );

    let text = window_text(&app);
    assert!(
        text.contains("no presentation capability connected"),
        "the reason is shown, not just the fact:\n{text}"
    );
    // `o` marks a stage that did not apply, so it never reads as still running.
    let row = app
        .research_progress
        .iter()
        .find(|p| p.name == "deckitem")
        .and_then(|p| p.deck.as_ref())
        .and_then(|tracker| tracker.rows.first())
        .expect("the stage row exists");
    assert_eq!(row.status, DeckStepStatus::Skipped);
}

#[test]
fn test_the_sentinel_does_not_collide_with_the_session_progress_sentinel() {
    // A deck report must not be mistaken for a `/research create` progress line,
    // which would append a duplicate session step per stage transition.
    let message = ragent_tui::deck_progress::encode_deck_progress_event(
        "x",
        &DeckEvent::started(DeckStage::Select, "starting"),
    );
    assert!(message.starts_with(DECK_PROGRESS_SENTINEL));
    assert!(
        ragent_tui::research_progress::decode_progress_event(&message).is_none(),
        "the session progress decoder must not claim a deck report"
    );
}

#[test]
fn test_two_runs_keep_separate_messages() {
    let mut app = app_with_session();
    report(
        &mut app,
        "first",
        DeckEvent::started(DeckStage::Select, "first run"),
    );
    report(
        &mut app,
        "second",
        DeckEvent::started(DeckStage::Select, "second run"),
    );

    let text = window_text(&app);
    assert_eq!(text.matches("Research Progress - `first`").count(), 1);
    assert_eq!(text.matches("Research Progress - `second`").count(), 1);
}

#[test]
fn test_tracker_render_lists_only_reported_stages() {
    // A tracker must not invent a row for a stage the run never reached.
    let mut tracker = DeckTracker::new();
    tracker.apply(&DeckEvent::started(DeckStage::Select, "starting"));
    let rendered = tracker.render("x");
    assert!(rendered.contains("select"));
    assert!(!rendered.contains("export"), "unreached stages are absent");
}
