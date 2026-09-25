//! Tests for queuing slash commands while the primary agent is executing
//! (spec `inputqueue` FR-017 amendment).
//!
//! FR-017 was amended so a slash command submitted while a turn is running is
//! enqueued like a plain message and runs at the next turn boundary, instead of
//! being refused with the busy message. Bang commands and teammate-targeted
//! messages keep their existing busy refusal.
//!
//! These exercise the full key-event path through [`App::handle_key_event`] so
//! the `InputAction::SlashCommand` arm is tested as the user drives it, then
//! drain the queue through the public [`App::advance_input_queue`] boundary.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use ragent_tui::App;

#[path = "support/mod.rs"]
mod support;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// Build an app with an active session so dispatch can run.
fn app_with_session() -> App {
    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());
    app
}

/// Simulate the user typing `text` and pressing Enter.
async fn type_and_submit(app: &mut App, text: &str) {
    app.input = text.to_string();
    app.input_cursor = app.input_len_chars();
    app.handle_key_event(key(KeyCode::Enter)).await;
}

// ---------------------------------------------------------------------------
// FR-017 amendment — a slash command is enqueued while busy
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_slash_command_is_enqueued_while_processing() {
    let mut app = app_with_session();
    app.is_processing = true;

    type_and_submit(&mut app, "/status").await;

    assert_eq!(
        app.input_queue_len(),
        1,
        "FR-017 amendment: Enter must enqueue a slash command while busy"
    );
    assert_eq!(app.input_queue[0].text, "/status");
    assert!(
        app.input.is_empty(),
        "the input field is cleared after enqueueing"
    );
}

#[tokio::test]
async fn test_slash_command_is_not_refused_while_processing() {
    let mut app = app_with_session();
    app.is_processing = true;

    type_and_submit(&mut app, "/status").await;

    assert_ne!(
        app.status, "busy - wait for the current turn to finish",
        "a slash command must not keep the busy refusal"
    );
}

#[tokio::test]
async fn test_slash_command_dispatches_immediately_when_idle() {
    let mut app = app_with_session();

    type_and_submit(&mut app, "/status").await;

    assert_eq!(
        app.input_queue_len(),
        0,
        "an idle turn dispatches a slash command immediately, not queued"
    );
}

#[tokio::test]
async fn test_bang_command_still_refused_while_processing() {
    let mut app = app_with_session();
    app.is_processing = true;

    type_and_submit(&mut app, "! ls").await;

    assert_eq!(
        app.input_queue_len(),
        0,
        "FR-017: bang commands keep their busy refusal and are never queued"
    );
    assert_eq!(app.status, "busy - wait for the current turn to finish");
}

#[tokio::test]
async fn test_slash_command_enqueue_respects_capacity() {
    let mut app = app_with_session();
    app.is_processing = true;
    app.input_queue_capacity = 1;

    type_and_submit(&mut app, "/status").await;
    assert_eq!(app.input_queue_len(), 1);

    type_and_submit(&mut app, "/about").await;

    assert_eq!(app.input_queue_len(), 1, "FR-004: the cap is enforced");
    assert_eq!(
        app.input, "/about",
        "FR-004/FR-018: the rejected command is restored, not lost"
    );
    assert!(
        app.status.contains("queue full"),
        "a status message explains the rejection, got: {}",
        app.status
    );
}

#[tokio::test]
async fn test_slash_command_enqueue_clears_slash_menu() {
    let mut app = app_with_session();
    app.is_processing = true;
    // A visible completion menu must not linger over a cleared field.
    app.update_slash_menu();
    app.input = "/st".to_string();
    app.update_slash_menu();
    app.input = "/status".to_string();
    app.input_cursor = app.input_len_chars();

    app.handle_key_event(key(KeyCode::Enter)).await;

    assert_eq!(app.input_queue_len(), 1);
    assert!(
        app.slash_menu.is_none(),
        "the completion menu is dismissed when the command is queued"
    );
}

// ---------------------------------------------------------------------------
// FR-017 amendment — a queued slash command drains at the turn boundary
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_queued_slash_command_drains_at_the_boundary() {
    let mut app = app_with_session();
    app.is_processing = true;
    app.input_queue.push_back(ragent_tui::app::QueuedInput {
        text: "/status".to_string(),
        image_paths: Vec::new(),
    });

    // The turn ends: the queued slash command runs and leaves the queue empty.
    app.is_processing = false;
    app.advance_input_queue().await;

    assert_eq!(
        app.input_queue_len(),
        0,
        "FR-017 amendment: the queued slash command is drained at the boundary"
    );
    assert_eq!(
        app.last_prompt, "/status",
        "the slash command is recorded as the last prompt"
    );
}

#[tokio::test]
async fn test_consecutive_queued_slash_commands_all_drain() {
    let mut app = app_with_session();
    for text in ["/status", "/about", "/queue list"] {
        app.input_queue.push_back(ragent_tui::app::QueuedInput {
            text: text.to_string(),
            image_paths: Vec::new(),
        });
    }

    // Synchronous slash commands leave the boundary free, so a single drain
    // must run the whole batch rather than strand the tail behind the first.
    app.advance_input_queue().await;

    assert_eq!(
        app.input_queue_len(),
        0,
        "consecutive synchronous slash commands all drain from one boundary"
    );
}

#[tokio::test]
async fn test_queued_slash_command_defers_while_processing() {
    let mut app = app_with_session();
    app.is_processing = true;
    app.input_queue.push_back(ragent_tui::app::QueuedInput {
        text: "/status".to_string(),
        image_paths: Vec::new(),
    });

    app.advance_input_queue().await;

    assert_eq!(
        app.input_queue_len(),
        1,
        "FR-016: nothing dispatches over a running turn"
    );
}
