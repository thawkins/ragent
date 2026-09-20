//! Tests for the optional `/queue` slash command (spec `inputqueue` T-009,
//! FR-013).
//!
//! The command inspects the in-memory input queue through the public
//! `App::execute_slash_command` entry point:
//!
//! - `/queue list` names the queued entries in submission order (FR-001);
//! - `/queue clear` empties the queue and repaints the bare prompt (FR-028);
//! - `/queue next` dispatches the oldest entry only when the turn boundary is
//!   free, deferring while a turn or compaction owns it (FR-016/FR-030);
//! - `/queue help` prints the sub-command usage;
//! - an unknown sub-command is rejected without touching the queue.

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

/// Build an app with an active session.
fn app_with_session() -> App {
    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());
    app
}

/// Count user messages in the conversation.
fn user_message_count(app: &App) -> usize {
    app.messages.iter().filter(|m| m.role == Role::User).count()
}

/// The text of the last assistant message appended by a command.
fn last_assistant_text(app: &App) -> String {
    app.messages
        .iter()
        .rev()
        .find(|m| m.role == Role::Assistant)
        .map(|m| m.text_content())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// FR-013 — /queue is registered as a slash command
// ---------------------------------------------------------------------------

#[test]
fn test_queue_command_is_registered() {
    let found = ragent_tui::app::SLASH_COMMANDS
        .iter()
        .find(|cmd| cmd.trigger == "queue");
    let def = found.expect("/queue must be registered in SLASH_COMMANDS (FR-013)");
    assert!(
        def.description.contains("queue"),
        "registration description should mention the queue: {}",
        def.description
    );
}

// ---------------------------------------------------------------------------
// /queue list — names the entries in order
// ---------------------------------------------------------------------------

#[test]
fn test_queue_list_names_entries_in_order() {
    let mut app = app_with_session();
    app.input_queue.push_back(entry("QUEUE-CMD-1"));
    app.input_queue.push_back(entry("QUEUE-CMD-2"));

    app.execute_slash_command("/queue list");

    let text = last_assistant_text(&app);
    assert!(
        text.contains("From: /queue list"),
        "should show the /queue list header: {text}"
    );
    let first = text
        .find("QUEUE-CMD-1")
        .expect("first entry must be listed");
    let second = text
        .find("QUEUE-CMD-2")
        .expect("second entry must be listed");
    assert!(
        first < second,
        "entries must be listed oldest-first (FR-001): {text}"
    );
    assert_eq!(app.status, "queue: 2 entries");
}

#[test]
fn test_bare_queue_defaults_to_list() {
    let mut app = app_with_session();
    app.input_queue.push_back(entry("BARE-QUEUE"));

    app.execute_slash_command("/queue");

    let text = last_assistant_text(&app);
    assert!(
        text.contains("From: /queue list") && text.contains("BARE-QUEUE"),
        "a bare /queue must behave as /queue list: {text}"
    );
}

#[test]
fn test_queue_list_empty_reports_empty() {
    let mut app = app_with_session();

    app.execute_slash_command("/queue list");

    let text = last_assistant_text(&app);
    assert!(
        text.contains("input queue is empty"),
        "an empty queue must be reported: {text}"
    );
    assert_eq!(app.status, "queue: list empty");
}

// ---------------------------------------------------------------------------
// /queue clear — empties the queue without dispatching
// ---------------------------------------------------------------------------

#[test]
fn test_queue_clear_empties_the_queue() {
    let mut app = app_with_session();
    app.input_queue.push_back(entry("QUEUE-CMD-1"));
    app.input_queue.push_back(entry("QUEUE-CMD-2"));
    let before = user_message_count(&app);

    app.execute_slash_command("/queue clear");

    assert_eq!(
        app.input_queue_len(),
        0,
        "FR-028: /queue clear must empty the queue"
    );
    assert_eq!(
        user_message_count(&app),
        before,
        "/queue clear must not dispatch any queued entry"
    );
    let text = last_assistant_text(&app);
    assert!(
        text.contains("Cleared 2 entries"),
        "the clear result must be reported: {text}"
    );
    assert_eq!(app.status, "queue: cleared");
}

#[test]
fn test_queue_clear_leaves_the_running_turn_untouched() {
    let mut app = app_with_session();
    app.is_processing = true;
    app.input_queue.push_back(entry("QUEUE-CMD-1"));

    app.execute_slash_command("/queue clear");

    assert!(
        app.is_processing,
        "clearing the queue must not disturb the executing turn"
    );
    assert!(app.cancel_flag.is_none(), "no cancel must be armed");
    assert_eq!(app.input_queue_len(), 0);
}

#[test]
fn test_queue_clear_on_empty_queue_is_a_noop() {
    let mut app = app_with_session();

    app.execute_slash_command("/queue clear");

    assert_eq!(app.input_queue_len(), 0);
    let text = last_assistant_text(&app);
    assert!(
        text.contains("Cleared 0 entries"),
        "clearing an empty queue reports zero: {text}"
    );
}

// ---------------------------------------------------------------------------
// /queue next — dispatches the oldest entry when free
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_queue_next_dispatches_oldest_entry_when_free() {
    let mut app = app_with_session();
    app.input_queue.push_back(entry("NEXT-QUEUE-1"));
    app.input_queue.push_back(entry("NEXT-QUEUE-2"));

    app.execute_slash_command("/queue next");

    assert_eq!(
        app.input_queue_len(),
        1,
        "/queue next pops exactly one entry"
    );
    assert_eq!(
        app.last_prompt, "NEXT-QUEUE-1",
        "FIFO: the oldest entry must be dispatched first"
    );
    assert!(
        app.messages
            .iter()
            .any(|m| m.role == Role::User && m.text_content() == "NEXT-QUEUE-1"),
        "the dispatched entry is added to the conversation"
    );
    assert_eq!(app.status, "queue: next dispatched");
}

#[test]
fn test_queue_next_deferrs_while_the_agent_is_executing() {
    let mut app = app_with_session();
    app.is_processing = true;
    app.input_queue.push_back(entry("DEFER-QUEUE-1"));
    let before = user_message_count(&app);

    app.execute_slash_command("/queue next");

    assert_eq!(
        app.input_queue_len(),
        1,
        "FR-016/FR-030: the queue is left untouched while the turn runs"
    );
    assert_eq!(
        user_message_count(&app),
        before,
        "no entry may be dispatched while executing"
    );
    assert_eq!(app.status, "queue: next deferred");
    let text = last_assistant_text(&app);
    assert!(
        text.contains("still executing"),
        "the deferral must be reported: {text}"
    );
}

#[test]
fn test_queue_next_on_empty_queue_reports_empty() {
    let mut app = app_with_session();

    app.execute_slash_command("/queue next");

    assert_eq!(app.input_queue_len(), 0);
    assert_eq!(app.status, "queue: next empty");
    let text = last_assistant_text(&app);
    assert!(
        text.contains("nothing to run"),
        "an empty queue must be reported: {text}"
    );
}

// ---------------------------------------------------------------------------
// /queue help + unknown sub-commands
// ---------------------------------------------------------------------------

#[test]
fn test_queue_help_lists_subcommands() {
    let mut app = app_with_session();

    app.execute_slash_command("/queue help");

    let text = last_assistant_text(&app);
    for needle in ["From: /queue help", "list", "clear", "next", "help"] {
        assert!(text.contains(needle), "help must mention {needle}: {text}");
    }
    assert_eq!(app.status, "queue: help");
}

#[test]
fn test_queue_help_accepts_flag_aliases() {
    for args in ["--help", "-h"] {
        let mut app = app_with_session();
        app.execute_slash_command(&format!("/queue {args}"));
        assert_eq!(app.status, "queue: help", "/queue {args} must show help");
    }
}

#[test]
fn test_queue_unknown_subcommand_is_rejected() {
    let mut app = app_with_session();
    app.input_queue.push_back(entry("UNTOUCHED-QUEUE"));

    app.execute_slash_command("/queue bogus");

    assert_eq!(
        app.input_queue_len(),
        1,
        "an unknown sub-command must not touch the queue"
    );
    assert_eq!(app.status, "queue: unknown");
    let text = last_assistant_text(&app);
    assert!(
        text.contains("Unknown sub-command"),
        "the unknown sub-command must be reported: {text}"
    );
}
