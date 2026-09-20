//! Tests for the queue drain at the `AgentError` turn boundary
//! (spec `inputqueue` T-007).
//!
//! Covers FR-007 (an errored turn pops the oldest queued entry and dispatches
//! it as the next user turn), FR-018 (the queue is never discarded because a
//! turn failed; a foreign-session error cannot drain another session's queue)
//! and the shared-boundary guarantees the `MessageEnd` drain also relies on:
//! FIFO order (FR-019), the counter decrement repaint (FR-008) and the
//! asynchronous dispatch path (NFR-004).
//!
//! The drain is exercised through the public `handle_event` path so the
//! `AgentError` handler is tested as the event bus drives it.

use ragent_agent::event::Event;
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

/// Build an app with an active session so `advance_input_queue` is willing to
/// dispatch.
fn app_with_session() -> App {
    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());
    app
}

/// Deliver an `AgentError` for the current session and capture the dispatched
/// prompt, if any.
fn agent_error(app: &mut App) {
    app.handle_event(Event::AgentError {
        session_id: "test-session".to_string(),
        error: "simulated failure".to_string(),
    });
}

/// Count user messages in the conversation.
fn user_message_count(app: &App) -> usize {
    app.messages.iter().filter(|m| m.role == Role::User).count()
}

#[tokio::test]
async fn test_agent_error_drains_oldest_entry() {
    let mut app = app_with_session();
    app.is_processing = true;
    app.input_queue.push_back(entry("first queued"));

    agent_error(&mut app);

    assert_eq!(
        app.input_queue_len(),
        0,
        "FR-007: the oldest entry is popped at the AgentError boundary"
    );
    assert_eq!(
        app.last_prompt, "first queued",
        "FR-007: the popped entry is dispatched as the next user turn"
    );
    assert!(
        app.messages
            .iter()
            .any(|m| m.role == Role::User && m.text_content() == "first queued"),
        "FR-007: the dispatched message is added to the conversation"
    );
}

#[tokio::test]
async fn test_agent_error_drains_strict_fifo_order() {
    let mut app = app_with_session();
    app.is_processing = true;
    app.input_queue.push_back(entry("A"));
    app.input_queue.push_back(entry("B"));
    app.input_queue.push_back(entry("C"));

    agent_error(&mut app);
    assert_eq!(app.last_prompt, "A", "FR-019: oldest entry goes first");
    assert_eq!(app.input_queue_len(), 2);

    // A second failing turn opens the next boundary.
    agent_error(&mut app);
    assert_eq!(app.last_prompt, "B");
    assert_eq!(app.input_queue_len(), 1);

    agent_error(&mut app);
    assert_eq!(app.last_prompt, "C");
    assert_eq!(app.input_queue_len(), 0);
}

#[tokio::test]
async fn test_agent_error_retains_the_remaining_entries() {
    let mut app = app_with_session();
    app.is_processing = true;
    app.input_queue.push_back(entry("ERROR-QUEUE-1"));
    app.input_queue.push_back(entry("ERROR-QUEUE-2"));

    agent_error(&mut app);

    assert_eq!(
        app.input_queue_len(),
        1,
        "FR-018: a failed turn must not discard the remaining queued entries"
    );
    assert_eq!(
        app.last_prompt, "ERROR-QUEUE-1",
        "FR-007: FIFO order is kept"
    );
}

#[tokio::test]
async fn test_agent_error_with_empty_queue_is_a_noop() {
    let mut app = app_with_session();
    app.is_processing = true;

    agent_error(&mut app);

    assert_eq!(app.input_queue_len(), 0);
    assert_eq!(user_message_count(&app), 0);
}

#[tokio::test]
async fn test_agent_error_clears_processing_before_draining() {
    let mut app = app_with_session();
    app.is_processing = true;
    app.cancel_flag = Some(std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
        false,
    )));
    app.input_queue.push_back(entry("queued"));

    agent_error(&mut app);

    assert!(
        !app.is_processing,
        "the error handler clears the processing gate before the drain"
    );
    assert!(
        app.cancel_flag.is_some(),
        "the drain re-armed the cancel flag for the dispatched turn"
    );
    assert_eq!(app.last_prompt, "queued");
}

#[tokio::test]
async fn test_agent_error_for_another_session_does_not_drain() {
    let mut app = app_with_session();
    app.is_processing = true;
    app.input_queue.push_back(entry("foreign"));

    app.handle_event(Event::AgentError {
        session_id: "other-session".to_string(),
        error: "simulated failure".to_string(),
    });

    assert_eq!(
        app.input_queue_len(),
        1,
        "FR-018: a foreign-session error cannot drain this session's queue"
    );
    assert!(
        app.is_processing,
        "a foreign-session error does not touch this session's processing state"
    );
}

#[tokio::test]
async fn test_agent_error_drain_is_skipped_without_an_active_session() {
    let mut app = support::make_app();
    app.is_processing = true;
    app.input_queue.push_back(entry("no session"));

    // Deliver the error for the (still unbound) session id; the handler's
    // current-session guard rejects it, so the entry must be retained.
    app.handle_event(Event::AgentError {
        session_id: "test-session".to_string(),
        error: "simulated failure".to_string(),
    });

    assert_eq!(
        app.input_queue_len(),
        1,
        "FR-018: with no active session the entry is retained, never dropped"
    );
}

#[tokio::test]
async fn test_agent_error_drain_requests_redraw_for_the_counter() {
    let mut app = app_with_session();
    app.is_processing = true;
    app.input_queue.push_back(entry("one"));
    app.input_queue.push_back(entry("two"));
    app.needs_redraw = false;

    agent_error(&mut app);

    assert!(
        app.needs_redraw,
        "FR-008: the decremented counter must be painted on the next frame"
    );
}

#[tokio::test]
async fn test_agent_error_drain_uses_the_async_dispatch_path() {
    let mut app = app_with_session();
    app.is_processing = true;
    app.input_queue.push_back(entry("async"));

    agent_error(&mut app);

    // NFR-004: dispatch_user_message spawns the turn on a tokio task and arms a
    // cancel flag; it never runs the turn inline, so the UI thread stays free.
    assert!(
        app.cancel_flag.is_some(),
        "NFR-004: the async dispatch armed a cancel flag for the new turn"
    );
}

#[tokio::test]
async fn test_error_and_message_end_boundaries_both_drain() {
    // The two turn boundaries share `advance_input_queue`; an error followed by
    // a normal completion drains one entry each, in FIFO order.
    let mut app = app_with_session();
    app.is_processing = true;
    app.input_queue.push_back(entry("ERROR-QUEUE-1"));
    app.input_queue.push_back(entry("ERROR-QUEUE-2"));

    agent_error(&mut app);
    assert_eq!(app.last_prompt, "ERROR-QUEUE-1");
    assert_eq!(app.input_queue_len(), 1);

    app.is_processing = true;
    app.handle_event(Event::MessageEnd {
        session_id: "test-session".to_string(),
        message_id: "msg-1".to_string(),
        reason: ragent_agent::event::FinishReason::Stop,
    });
    assert_eq!(app.last_prompt, "ERROR-QUEUE-2");
    assert_eq!(app.input_queue_len(), 0);
}
