//! UI-side tests for compaction result handling (`poll_compaction_result`)
//! and Mutex poisoning recovery, mirroring `test_opt_result.rs`.

use std::sync::Arc;

use ragent_agent::message::{Message, Role};

#[path = "support/mod.rs"]
mod support;

/// Build a synthetic compaction message like the one the runner produces.
fn compaction_message() -> Message {
    Message::new(
        "test-session",
        Role::Compaction,
        vec![ragent_agent::message::MessagePart::Text {
            text: "[Conversation compacted]\n\nSummary of the prior conversation.".to_string(),
        }],
    )
}

// =========================================================================
// poll_compaction_result — no result pending
// =========================================================================

#[test]
fn test_poll_compaction_result_noop_when_empty() {
    let mut app = support::make_app();
    let status_before = app.status.clone();
    app.poll_compaction_result();
    // Status shouldn't change when there's no pending result.
    assert_eq!(app.status, status_before);
    assert!(!app.compact_in_progress);
}

// =========================================================================
// poll_compaction_result — Ok result replaces history and clears state
// =========================================================================

#[test]
fn test_poll_compaction_result_ok_replaces_messages() {
    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());

    // Seed pre-compaction in-memory state.
    app.messages = vec![Message::user_text("test-session", "hello")];
    app.message_line_cache
        .push(ragent_tui::app::MessageLineGroup {
            lines: Vec::new(),
            wrapped_lines: Vec::new(),
            content_lines: Vec::new(),
            wrapped_count: 0,
            edit_seq: 0,
        });
    app.compact_in_progress = true;

    {
        let mut guard = app.compact_result.lock().unwrap();
        *guard = Some(Ok(vec![compaction_message()]));
    }

    app.poll_compaction_result();

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::Compaction);
    // The render cache must be cleared by the structural history change.
    assert!(app.message_line_cache.is_empty());
    assert!(!app.compact_in_progress);
    assert_eq!(app.status, "ready");
    // The mutex should now be empty.
    assert!(app.compact_result.lock().unwrap().is_none());
}

// =========================================================================
// poll_compaction_result — Err result clears state and blocks queued send
// =========================================================================

#[tokio::test]
async fn test_poll_compaction_result_err_blocks_queued_send() {
    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());

    app.auto_compact_in_progress = true;
    app.auto_compact_failed = false;
    app.compact_in_progress = true;
    app.pending_send_after_compact = Some(("queued prompt".to_string(), Vec::new()));

    {
        let mut guard = app.compact_result.lock().unwrap();
        *guard = Some(Err("LLM summarisation call failed".to_string()));
    }

    app.poll_compaction_result();

    assert!(app.auto_compact_failed, "failure must set the retry latch");
    assert!(!app.auto_compact_in_progress);
    assert!(!app.compact_in_progress);
    assert!(
        app.pending_send_after_compact.is_none(),
        "queued send must be dropped on failure"
    );
    assert!(app.status.contains("compact failed"));
}

// =========================================================================
// poll_compaction_result — Ok result dispatches the queued send
// =========================================================================

#[tokio::test]
async fn test_poll_compaction_result_ok_dispatches_queued_send() {
    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());

    app.auto_compact_in_progress = true;
    app.pending_send_after_compact = Some(("queued prompt".to_string(), Vec::new()));

    {
        let mut guard = app.compact_result.lock().unwrap();
        *guard = Some(Ok(vec![compaction_message()]));
    }

    app.poll_compaction_result();

    assert!(
        app.pending_send_after_compact.is_none(),
        "queued send must be dispatched after successful compaction"
    );
    assert!(!app.auto_compact_in_progress);
    assert!(!app.compact_in_progress);
}

// =========================================================================
// poll_compaction_result — Mutex poisoned
// =========================================================================

#[test]
fn test_poll_compaction_result_recovers_from_poisoned_mutex() {
    let mut app = support::make_app();

    // Poison the mutex by panicking inside a lock.
    let compact_result_clone = Arc::clone(&app.compact_result);
    let _ = std::thread::spawn(move || {
        let _guard = compact_result_clone.lock().unwrap();
        panic!("intentional poison");
    })
    .join();

    // The mutex is now poisoned. poll_compaction_result should recover
    // gracefully instead of propagating the panic.
    app.poll_compaction_result();
    assert!(
        !app.status.contains("panic"),
        "should recover without propagating panic"
    );
}
