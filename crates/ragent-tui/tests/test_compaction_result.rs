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

// =========================================================================
// poll_context_snapshot_refresh — stale-snapshot drop after /compact
// =========================================================================
//
// Regression: when `/compact` finished while a Context panel snapshot refresh
// was already in flight, the background task would later deposit its
// PRE-compaction snapshot and `poll_context_snapshot_refresh` would adopt it,
// overwriting the cache with stale history counts. The stale-snapshot guard
// detects the message-count mismatch, drops the stale snapshot, and
// immediately schedules a fresh refresh so the Context panel reflects the
// compacted history (FR-013).

#[tokio::test]
async fn test_poll_context_snapshot_drops_stale_after_compaction() {
    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());
    app.show_context_panel = true;

    // Pre-compaction state: 50 messages. The background refresh was scheduled
    // when history_message_count was 50, so it captures 50.
    let mut pre_compact_messages = Vec::<ragent_agent::message::Message>::with_capacity(50);
    for i in 0..50 {
        pre_compact_messages.push(Message::user_text(
            "test-session",
            format!("pre-compaction message #{i}"),
        ));
    }
    app.messages = pre_compact_messages;
    let stale_snapshot = app.context_partition_snapshot();
    assert_eq!(
        stale_snapshot.history_message_count, 50,
        "pre-compaction snapshot must reflect 50 messages"
    );

    // Simulate the in-flight refresh having deposited its (pre-compaction)
    // result while the user ran /compact. The UI has not polled yet.
    {
        let mut guard = app
            .context_snapshot_result
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = Some(stale_snapshot);
    }
    app.context_refresh_inflight = true;
    // Seed the cache with the same stale snapshot to prove the post-compaction
    // path overwrites it (without the fix it would already be there and stay).
    app.context_snapshot_cache = Some(stale_snapshot);

    // Run /compact: replace `messages` with the compacted form.
    app.compact_in_progress = true;
    {
        let mut guard = app.compact_result.lock().unwrap();
        *guard = Some(Ok(vec![compaction_message()]));
    }
    app.poll_compaction_result();
    assert_eq!(
        app.messages.len(),
        1,
        "compaction result must replace the message list"
    );
    assert!(
        !app.compact_in_progress,
        "compact_in_progress must clear after poll"
    );

    // The first poll drains the stale in-flight snapshot. With the fix it
    // detects the count mismatch (50 vs 1), drops it, and triggers a fresh
    // refresh. The cache retains its previous value until the new blocking
    // task lands (so the panel may render briefly stale; the next frame
    // shows the fresh snapshot).
    app.poll_context_snapshot_refresh();
    assert!(
        app.context_refresh_inflight,
        "stale-snapshot drop must immediately schedule a fresh refresh"
    );

    // Wait for the fresh refresh to land and adopt it.
    let mut fresh = None;
    for _ in 0..100 {
        app.poll_context_snapshot_refresh();
        if let Some(snap) = app.context_snapshot_cache
            && snap.history_message_count == 1
        {
            fresh = Some(snap);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let fresh = fresh.expect("fresh snapshot must be adopted within the polling window");
    assert_eq!(
        fresh.history_message_count, 1,
        "Context panel must reflect the compacted message count"
    );
}
