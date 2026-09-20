//! Tests for the queue-control menu's `Stop`/`Resume` action (spec `inputqueue`
//! T-016).
//!
//! Covers FR-025 (selecting `Stop` halts the primary agent exactly as
//! `InputAction::CancelAgent` does), FR-026 (the row reads `Stop` while a turn is
//! executing and `Resume` once it has stopped), FR-027 (selecting `Resume`
//! resumes the interrupted work), FR-029 (halting never advances the queue) and
//! NFR-008 (the action paints on the next frame).
//!
//! The row is driven through the public [`App::queue_menu_select_halt`] entry
//! point, mirroring how the T-006 drain tests drive [`App::advance_input_queue`].
//! The Up/Down/Enter key handling that routes a keystroke to this method is
//! covered by the menu key-handling tests.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ragent_agent::event::{Event, FinishReason};
use ragent_agent::message::Role;

use ragent_tui::App;
use ragent_tui::app::QueuedInput;

#[path = "support/mod.rs"]
mod support;

/// Seed a queued entry with no attachments.
fn entry(text: &str) -> QueuedInput {
    QueuedInput {
        text: text.to_string(),
        image_paths: Vec::new(),
    }
}

/// Build an app with an active session so the resume path has a session to use.
fn app_with_session() -> App {
    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());
    app
}

/// Count user messages in the conversation.
fn user_message_count(app: &App) -> usize {
    app.messages.iter().filter(|m| m.role == Role::User).count()
}

// ---------------------------------------------------------------------------
// FR-025 / FR-029 — Stop halts exactly like CancelAgent and never advances
// ---------------------------------------------------------------------------

#[test]
fn test_stop_row_halts_the_running_turn() {
    let mut app = app_with_session();
    app.is_processing = true;
    let flag = Arc::new(AtomicBool::new(false));
    app.cancel_flag = Some(flag.clone());
    app.input_queue.push_back(entry("still queued"));

    app.queue_menu_select_halt();

    assert!(
        flag.load(Ordering::Relaxed),
        "FR-025: selecting Stop must set the turn's cancel flag"
    );
    assert_eq!(
        app.status, "halting agent…",
        "FR-025: Stop must report the same halting status as CancelAgent"
    );
}

#[test]
fn test_stop_row_does_not_advance_the_queue() {
    let mut app = app_with_session();
    app.is_processing = true;
    app.cancel_flag = Some(Arc::new(AtomicBool::new(false)));
    app.input_queue.push_back(entry("first"));
    app.input_queue.push_back(entry("second"));
    let before = user_message_count(&app);

    app.queue_menu_select_halt();

    assert_eq!(
        app.input_queue_len(),
        2,
        "FR-029: Stop must leave every queued entry in place"
    );
    assert_eq!(
        user_message_count(&app),
        before,
        "FR-029: Stop must not dispatch any queued entry"
    );
}

#[test]
fn test_stop_row_closes_the_menu_and_resets_selection() {
    let mut app = app_with_session();
    app.is_processing = true;
    app.cancel_flag = Some(Arc::new(AtomicBool::new(false)));
    app.queue_menu_open = true;
    app.queue_menu_selected = 1;

    app.queue_menu_select_halt();

    assert!(
        !app.queue_menu_open,
        "the menu must close after the action (NFR-008)"
    );
    assert_eq!(
        app.queue_menu_selected, 0,
        "the selection must reset for the next open"
    );
}

#[test]
fn test_stop_row_leaves_input_buffer_and_attachments_untouched() {
    let mut app = app_with_session();
    app.is_processing = true;
    app.cancel_flag = Some(Arc::new(AtomicBool::new(false)));
    app.input = "DRAFT".to_string();
    app.input_cursor = app.input_len_chars();
    app.pending_attachments = vec![std::path::PathBuf::from("/tmp/a.png")];

    app.queue_menu_select_halt();

    assert_eq!(
        app.input, "DRAFT",
        "FR-031: the halt row must not mutate the input buffer"
    );
    assert_eq!(
        app.pending_attachments,
        vec![std::path::PathBuf::from("/tmp/a.png")],
        "the halt row must not touch staged attachments"
    );
}

// ---------------------------------------------------------------------------
// FR-026 — the halt row's label switches Stop -> Resume
// ---------------------------------------------------------------------------

#[test]
fn test_halt_label_is_stop_while_processing() {
    let mut app = support::make_app();
    app.is_processing = true;

    assert_eq!(
        app.queue_menu_halt_label(),
        "Stop",
        "FR-026: while a turn executes the halt row reads `Stop`"
    );
}

#[tokio::test]
async fn test_halt_label_is_resume_after_a_cancelled_turn() {
    let mut app = app_with_session();
    app.is_processing = true;

    app.handle_event(Event::MessageEnd {
        session_id: "test-session".to_string(),
        message_id: "msg-1".to_string(),
        reason: FinishReason::Cancelled,
    });

    assert!(
        !app.is_processing,
        "precondition: the cancelled turn has stopped the agent"
    );
    assert!(
        app.agent_halted,
        "precondition: a cancelled turn marks the agent halted"
    );
    assert_eq!(
        app.queue_menu_halt_label(),
        "Resume",
        "FR-026: once stopped the halt row reads `Resume`"
    );
}

#[test]
fn test_halt_label_is_stop_while_compaction_runs() {
    let mut app = support::make_app();
    app.is_processing = false;
    app.compact_in_progress = true;

    assert_eq!(
        app.queue_menu_halt_label(),
        "Stop",
        "FR-026: a compaction run still counts as executing, so the row reads `Stop`"
    );
}

// ---------------------------------------------------------------------------
// FR-027 — selecting Resume resumes the interrupted work
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_resume_row_dispatches_the_continuation_message() {
    let mut app = app_with_session();
    app.is_processing = false;
    app.agent_halted = true;
    app.queue_menu_open = true;

    app.queue_menu_select_halt();

    assert!(
        !app.agent_halted,
        "FR-027: resuming must clear the halted flag"
    );
    assert!(
        app.status.contains("processing"),
        "FR-027: resuming must put the agent back to work, got {:?}",
        app.status
    );
    let resumed = app.messages.iter().any(|m| {
        m.role == Role::User
            && m.text_content()
                .contains("previously interrupted by the user")
    });
    assert!(
        resumed,
        "FR-027: resuming must dispatch the continuation prompt as a user turn"
    );
    assert!(
        app.cancel_flag.is_some(),
        "FR-027: the resumed turn must arm a fresh cancel flag"
    );
    assert!(!app.queue_menu_open, "the menu must close after the action");
}

#[tokio::test]
async fn test_resume_row_does_not_advance_the_queue() {
    let mut app = app_with_session();
    app.agent_halted = true;
    app.input_queue.push_back(entry("still queued"));

    app.queue_menu_select_halt();

    assert_eq!(
        app.input_queue_len(),
        1,
        "FR-029: the halt/resume row must never drain the queue"
    );
}

#[test]
fn test_resume_row_when_not_halted_reports_nothing_to_resume() {
    let mut app = app_with_session();
    app.agent_halted = false;
    let before = user_message_count(&app);

    app.queue_menu_select_halt();

    assert_eq!(
        app.status, "Nothing to resume — agent was not halted",
        "FR-027: resuming a non-halted agent must say so"
    );
    assert_eq!(
        user_message_count(&app),
        before,
        "FR-027: a no-op resume must dispatch nothing"
    );
}

#[test]
fn test_resume_row_without_a_session_reports_no_active_session() {
    let mut app = support::make_app();
    app.session_id = None;
    app.agent_halted = true;

    app.queue_menu_select_halt();

    assert_eq!(
        app.status, "No active session",
        "FR-027: resuming without a session must refuse"
    );
    assert!(
        app.agent_halted,
        "FR-027: a refused resume must leave the halted flag set"
    );
}

// ---------------------------------------------------------------------------
// NFR-008 — the action repaints on the next frame
// ---------------------------------------------------------------------------

#[test]
fn test_stop_row_sets_the_redraw_flag() {
    let mut app = app_with_session();
    app.is_processing = true;
    app.cancel_flag = Some(Arc::new(AtomicBool::new(false)));
    app.needs_redraw = false;

    app.queue_menu_select_halt();

    assert!(
        app.needs_redraw,
        "NFR-008: the Stop/Resume action must repaint on the next frame"
    );
}

#[test]
fn test_resume_row_sets_the_redraw_flag() {
    let mut app = app_with_session();
    app.needs_redraw = false;

    app.queue_menu_select_halt();

    assert!(
        app.needs_redraw,
        "NFR-008: the Stop/Resume action must repaint on the next frame"
    );
}
