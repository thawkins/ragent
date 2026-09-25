//! End-to-end integration tests for the ALT-Q queue-control menu and its three
//! actions (spec `inputqueue` T-019).
//!
//! The per-task suites exercise each piece in isolation: T-013 covers the
//! ALT-Q binding, T-014 the three-option render, T-015 the `Next` action,
//! T-016 the `Stop`/`Resume` action, T-017 the `Clear` action, and T-018 the
//! `Esc` dismissal. This suite composes them into the full user journey —
//! open the menu with `ALT-Q`, select a row, and observe the effect — so the
//! actions are proven to work together rather than only in isolation.
//!
//! Covers FR-021 (ALT-Q opens a menu with exactly `Next`/`Stop`/`Clear`),
//! FR-023 (the `Next` row is selectable only while the queue is non-empty),
//! FR-024 (selecting `Next` stops the running turn and dispatches the oldest
//! entry), FR-025 (selecting `Stop` halts without advancing the queue),
//! FR-026 (the halt row reads `Stop` while executing and `Resume` when idle),
//! FR-027 (selecting `Resume` restarts the interrupted work), FR-028 (selecting
//! `Clear` opens the confirmation dialog rather than emptying the queue),
//! FR-029 (halting never advances the queue) and FR-031 (the menu never mutates
//! the editable input buffer).
//!
//! Rows are selected through the public [`App::queue_menu_select_next`],
//! [`App::queue_menu_select_halt`], and [`App::queue_menu_select_clear`] entry
//! points — the same methods the menu key-handling invokes — after the menu has
//! been opened through the real `ALT-Q` keystroke path.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use ragent_agent::event::{Event, FinishReason};
use ragent_agent::message::Role;

use ragent_tui::App;
use ragent_tui::app::{QUEUE_CLEAR_CONFIRM_NO, QueuedInput};
use ragent_tui::layout;

#[path = "support/mod.rs"]
mod support;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn alt(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::ALT)
}

/// Seed a queued entry with no attachments.
fn entry(text: &str) -> QueuedInput {
    QueuedInput {
        text: text.to_string(),
        image_paths: Vec::new(),
    }
}

/// Build an app with an active session so the dispatch paths have a session.
fn app_with_session() -> App {
    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());
    app
}

/// Open the menu the way the user would (`ALT-Q`) and assert it opened.
async fn open_menu_with_alt_q(app: &mut App) {
    app.handle_key_event(alt(KeyCode::Char('q'))).await;
    assert!(
        app.queue_menu_open,
        "precondition: ALT-Q must open the queue-control menu"
    );
}

/// Deliver a `MessageEnd` for the current session with the given reason.
async fn message_end(app: &mut App, reason: FinishReason) {
    app.handle_event(Event::MessageEnd {
        session_id: "test-session".to_string(),
        message_id: "msg-1".to_string(),
        reason,
    })
    .await;
}

/// Count user messages in the conversation.
fn user_message_count(app: &App) -> usize {
    app.messages.iter().filter(|m| m.role == Role::User).count()
}

/// Render one frame and return the terminal so the painted buffer can be read.
fn render(app: &mut App, width: u16, height: u16) -> Terminal<TestBackend> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, app))
        .expect("draw");
    terminal
}

/// The painted row containing `needle` within the menu's inner area.
fn menu_row(terminal: &Terminal<TestBackend>, app: &App, needle: &str) -> String {
    let area = app.queue_menu_area;
    let x0 = area.x + 1;
    let x1 = area.x + area.width.saturating_sub(1);
    let buffer = terminal.backend().buffer();
    (area.y + 1..area.y + area.height.saturating_sub(1))
        .map(|y| {
            (x0..x1)
                .map(|x| buffer[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .find(|row| row.contains(needle))
        .unwrap_or_else(|| panic!("expected a menu row containing {needle:?}"))
}

// ---------------------------------------------------------------------------
// FR-021 / FR-022 / FR-031 — opening the menu through ALT-Q
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_alt_q_opens_a_menu_with_exactly_the_four_options() {
    let mut app = app_with_session();
    open_menu_with_alt_q(&mut app).await;

    let labels = app.queue_menu_labels();
    assert_eq!(labels.len(), 4, "FR-021: exactly four options");
    assert_eq!(labels[0], "Next", "FR-021: first option is `Next`");
    assert_eq!(labels[2], "Clear", "FR-021: third option is `Clear`");
    assert_eq!(labels[3], "Show", "FR-021: fourth option is `Show`");
}

#[tokio::test]
async fn test_alt_q_opens_without_disturbing_the_draft_or_running_turn() {
    let mut app = app_with_session();
    app.is_processing = true;
    let flag = Arc::new(AtomicBool::new(false));
    app.cancel_flag = Some(flag.clone());
    app.input = "INTEG-DRAFT".to_string();
    app.input_cursor = app.input_len_chars();
    app.pending_attachments
        .push(PathBuf::from("/tmp/probe.png"));

    open_menu_with_alt_q(&mut app).await;

    assert_eq!(
        app.input, "INTEG-DRAFT",
        "FR-022/FR-031: opening the menu must not touch the input buffer"
    );
    assert_eq!(
        app.input_cursor,
        app.input_len_chars(),
        "the cursor is unmoved"
    );
    assert_eq!(
        app.pending_attachments.len(),
        1,
        "FR-022: staged attachments are preserved"
    );
    assert!(app.is_processing, "FR-022: the running turn is untouched");
    assert!(
        !flag.load(Ordering::Relaxed),
        "FR-022: opening the menu must not cancel the running turn"
    );
    assert_eq!(
        app.input_queue_len(),
        0,
        "FR-022: opening the menu must not enqueue anything"
    );
}

// ---------------------------------------------------------------------------
// FR-023 — the `Next` row is selectable only while the queue is non-empty
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_next_row_selectability_tracks_the_queue_end_to_end() {
    let mut app = app_with_session();
    open_menu_with_alt_q(&mut app).await;

    // Empty queue: the `Next` row is painted dimmed and carries no marker.
    let empty = render(&mut app, 120, 40);
    let empty_next = menu_row(&empty, &app, "Next");
    assert!(
        !empty_next.starts_with("> "),
        "FR-023: with an empty queue `Next` must not be selectable, got {empty_next:?}"
    );

    // Enqueue one entry while the menu is open; `Next` becomes selectable.
    app.input_queue.push_back(entry("VISIBLE-1"));
    let filled = render(&mut app, 120, 40);
    let filled_next = menu_row(&filled, &app, "Next");
    assert!(
        filled_next.starts_with("> "),
        "FR-023: with a queued entry `Next` becomes selectable, got {filled_next:?}"
    );
}

// ---------------------------------------------------------------------------
// FR-024 / FR-029 — `Next` dispatches the oldest entry in FIFO order
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_alt_q_next_dispatches_oldest_entry_and_preserves_fifo() {
    let mut app = app_with_session();
    app.input_queue.push_back(entry("INTEG-QUEUE-1"));
    app.input_queue.push_back(entry("INTEG-QUEUE-2"));

    open_menu_with_alt_q(&mut app).await;
    app.queue_menu_select_next().await;

    assert!(
        !app.queue_menu_open,
        "the menu closes after the `Next` action"
    );
    assert_eq!(
        app.last_prompt, "INTEG-QUEUE-1",
        "FR-024: the oldest queued entry is dispatched first"
    );
    assert_eq!(
        app.input_queue_len(),
        1,
        "FR-024: only the oldest entry is consumed"
    );

    open_menu_with_alt_q(&mut app).await;
    app.queue_menu_select_next().await;

    assert_eq!(
        app.last_prompt, "INTEG-QUEUE-2",
        "FR-019: FIFO order is preserved across selections"
    );
    assert_eq!(
        app.input_queue_len(),
        0,
        "the queue drains one entry at a time"
    );
}

#[tokio::test]
async fn test_alt_q_next_stops_running_turn_then_dispatches_at_the_boundary() {
    let mut app = app_with_session();
    app.is_processing = true;
    let flag = Arc::new(AtomicBool::new(false));
    app.cancel_flag = Some(flag.clone());
    app.input_queue.push_back(entry("INTEG-STOP-THEN-RUN"));

    open_menu_with_alt_q(&mut app).await;
    app.queue_menu_select_next().await;

    assert!(
        flag.load(Ordering::Relaxed),
        "FR-024: selecting `Next` halts the running turn"
    );
    assert!(
        app.queue_next_pending,
        "FR-029/FR-030: the dispatch is deferred to the turn boundary the cancel opens"
    );
    assert_eq!(
        app.input_queue_len(),
        1,
        "FR-029: a running turn is never dispatched over"
    );
    assert_eq!(
        user_message_count(&app),
        0,
        "FR-029: nothing is dispatched while the turn is still executing"
    );

    // The cancel settles; the deferred entry runs at the boundary.
    message_end(&mut app, FinishReason::Cancelled).await;

    assert_eq!(
        app.last_prompt, "INTEG-STOP-THEN-RUN",
        "FR-024: the deferred entry runs at the boundary the cancel opened"
    );
    assert_eq!(app.input_queue_len(), 0);
    assert!(
        !app.queue_next_pending,
        "FR-030: the pending flag is consumed exactly once"
    );
}

// ---------------------------------------------------------------------------
// FR-025 / FR-029 — `Stop` halts without advancing the queue
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_alt_q_stop_halts_without_advancing_the_queue() {
    let mut app = app_with_session();
    app.is_processing = true;
    let flag = Arc::new(AtomicBool::new(false));
    app.cancel_flag = Some(flag.clone());
    app.input_queue.push_back(entry("STOP-A"));
    app.input_queue.push_back(entry("STOP-B"));
    let before = user_message_count(&app);

    open_menu_with_alt_q(&mut app).await;
    assert_eq!(
        app.queue_menu_halt_label(),
        "Stop",
        "FR-026: an executing turn labels the halt row `Stop`"
    );
    app.queue_menu_select_halt();

    assert!(
        flag.load(Ordering::Relaxed),
        "FR-025: `Stop` cancels the turn exactly like `CancelAgent`"
    );
    assert!(
        !app.queue_menu_open,
        "the menu closes after the `Stop` action"
    );
    assert_eq!(
        app.input_queue_len(),
        2,
        "FR-029: `Stop` must not advance the queue"
    );
    assert_eq!(
        user_message_count(&app),
        before,
        "FR-029: `Stop` must not dispatch any queued entry"
    );
    assert!(
        !app.queue_next_pending,
        "FR-029: `Stop` must not arm a deferred dispatch"
    );
}

// ---------------------------------------------------------------------------
// FR-026 / FR-027 — halt row label switches to `Resume` and resumes
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_alt_q_halt_row_reads_resume_and_resumes_after_a_stop() {
    let mut app = app_with_session();
    app.is_processing = true;
    app.cancel_flag = Some(Arc::new(AtomicBool::new(false)));
    app.input_queue.push_back(entry("RESUME-A"));

    // Stop the turn through the menu, then let the cancel settle.
    open_menu_with_alt_q(&mut app).await;
    assert_eq!(
        app.queue_menu_halt_label(),
        "Stop",
        "FR-026: while executing the row reads `Stop`"
    );
    app.queue_menu_select_halt();
    message_end(&mut app, FinishReason::Cancelled).await;

    assert!(
        !app.is_processing,
        "precondition: the cancelled turn stopped"
    );
    assert!(
        app.agent_halted,
        "precondition: the cancelled turn marked the agent halted"
    );

    // Reopen: the same row now reads `Resume` and restarts the work.
    open_menu_with_alt_q(&mut app).await;
    assert_eq!(
        app.queue_menu_halt_label(),
        "Resume",
        "FR-026: once stopped the row reads `Resume`"
    );
    let before = user_message_count(&app);
    app.queue_menu_select_halt();

    assert!(
        !app.agent_halted,
        "FR-027: selecting `Resume` clears the halted flag"
    );
    assert!(
        user_message_count(&app) > before,
        "FR-027: `Resume` dispatches the continuation as a user turn"
    );
    assert_eq!(
        app.input_queue_len(),
        1,
        "FR-027/FR-029: `Resume` does not consume the queue"
    );
    assert!(
        !app.queue_menu_open,
        "the menu closes after the `Resume` action"
    );
}

// ---------------------------------------------------------------------------
// FR-028 — `Clear` opens the confirmation dialog without emptying the queue
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_alt_q_clear_opens_the_confirmation_dialog_without_emptying() {
    let mut app = app_with_session();
    app.input_queue.push_back(entry("CLEAR-A"));
    app.input_queue.push_back(entry("CLEAR-B"));

    open_menu_with_alt_q(&mut app).await;
    assert_eq!(
        app.queue_menu_labels()[2],
        "Clear",
        "FR-021: the third row is `Clear`"
    );
    app.queue_menu_select_clear();

    assert!(
        app.queue_clear_confirm_open,
        "FR-028/FR-033: selecting `Clear` opens the confirmation dialog"
    );
    assert!(
        !app.queue_menu_open,
        "the queue-control menu closes when the dialog opens"
    );
    assert_eq!(
        app.input_queue_len(),
        2,
        "FR-037: the queue is untouched until the user confirms with `Yes`"
    );
    assert_eq!(
        app.queue_clear_confirm_selected, QUEUE_CLEAR_CONFIRM_NO,
        "FR-034: the dialog defaults to `No`"
    );
}

// ---------------------------------------------------------------------------
// FR-031 / FR-032 — Esc dismisses the menu without mutating state
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_alt_q_esc_dismisses_without_mutating_input_or_queue() {
    let mut app = app_with_session();
    app.is_processing = true;
    app.cancel_flag = Some(Arc::new(AtomicBool::new(false)));
    app.input = "INTEG-ESC-DRAFT".to_string();
    app.input_cursor = app.input_len_chars();
    app.pending_attachments
        .push(PathBuf::from("/tmp/probe.png"));
    app.input_queue.push_back(entry("KEEP-A"));
    app.input_queue.push_back(entry("KEEP-B"));

    open_menu_with_alt_q(&mut app).await;
    app.handle_key_event(key(KeyCode::Esc)).await;

    assert!(!app.queue_menu_open, "FR-032: Esc dismisses the menu");
    assert_eq!(
        app.input, "INTEG-ESC-DRAFT",
        "FR-031: dismissing the menu must not mutate the input buffer"
    );
    assert_eq!(
        app.input_cursor,
        app.input_len_chars(),
        "the cursor is unmoved"
    );
    assert_eq!(
        app.pending_attachments.len(),
        1,
        "FR-031: staged attachments are preserved"
    );
    assert_eq!(
        app.input_queue_len(),
        2,
        "FR-032: dismissing the menu must leave the queue unchanged"
    );
    assert!(app.is_processing, "FR-032: the running turn continues");
}

// ---------------------------------------------------------------------------
// FR-031 — no row action inserts characters into the editable buffer
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_next_row_never_mutates_the_live_draft() {
    let mut app = app_with_session();
    app.input = "unfinished draft".to_string();
    app.input_cursor = 5;
    app.pending_attachments
        .push(PathBuf::from("/tmp/diagram.png"));
    app.input_queue.push_back(entry("RUN-ME"));

    open_menu_with_alt_q(&mut app).await;
    app.queue_menu_select_next().await;

    assert_eq!(
        app.input, "unfinished draft",
        "FR-031: the `Next` row must not mutate the editable input buffer"
    );
    assert_eq!(app.input_cursor, 5, "the cursor position is unchanged");
    assert_eq!(
        app.pending_attachments.len(),
        1,
        "FR-031: staging is untouched by the menu action"
    );
    assert_eq!(
        app.last_prompt, "RUN-ME",
        "precondition: the queued entry was still dispatched"
    );
}
