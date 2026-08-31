//! Tests for the Context side panel (spec `contextpanel`).
//!
//! - **T-013**: Non-blocking refresh scheduling (FR-015).

mod support;

use ragent_agent::message::{Message, MessagePart, Role};

#[tokio::test]
async fn test_schedule_refresh_is_idempotent_while_inflight() {
    // FR-015: scheduling a refresh must not stack concurrent blocking tasks;
    // a second call while one is in flight is a no-op.
    let mut app = support::make_app();
    app.show_context_panel = true;
    app.schedule_context_snapshot_refresh();
    let inflight_before = app.context_refresh_inflight;
    app.schedule_context_snapshot_refresh();
    assert_eq!(
        app.context_refresh_inflight, inflight_before,
        "refresh scheduling must not stack while a task is in flight"
    );
    // Give the blocking task a real chance to deposit its snapshot instead
    // of racing a single poll: poll with tiny yields until the latch clears
    // (blocking tasks run on the shared blocking pool).
    for _ in 0..100 {
        app.poll_context_snapshot_refresh();
        if !app.context_refresh_inflight && app.context_snapshot_cache.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(
        app.context_snapshot_cache.is_some(),
        "blocking task must deposit a snapshot"
    );
}

#[test]
fn test_poll_adopts_refresh_result_and_clears_inflight() {
    // FR-015: the background task deposits its snapshot in the result
    // channel; the UI-thread poll adopts it, clears the in-flight latch and
    // flags the panel for redraw.
    let mut app = support::make_app();
    app.context_refresh_inflight = true;

    let snapshot = app.context_partition_snapshot();
    {
        let mut guard = app
            .context_snapshot_result
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = Some(snapshot);
    }

    app.poll_context_snapshot_refresh();

    assert!(
        !app.context_refresh_inflight,
        "in-flight latch must clear after adoption"
    );
    assert_eq!(
        app.context_snapshot_cache,
        Some(snapshot),
        "poll must adopt the deposited snapshot"
    );
    assert!(app.needs_redraw, "adopting a snapshot must flag redraw");
}

#[test]
fn test_cache_refreshes_after_history_change() {
    // FR-015 + FR-013: after the history changes and a refresh completes,
    // the cached snapshot reflects the new history size (not the stale
    // value) without blocking the UI thread.
    let mut app = support::make_app();
    app.show_context_panel = true;

    // Seed the cache with a stale snapshot (empty history).
    let stale = app.context_partition_snapshot();
    app.context_snapshot_cache = Some(stale);

    app.messages.push(Message::new(
        "session-1",
        Role::User,
        vec![MessagePart::Text {
            text: "fresh content that must appear in a refreshed snapshot".into(),
        }],
    ));

    // Deliver a fresh snapshot the way the blocking task would.
    let fresh = app.context_partition_snapshot();
    assert!(fresh.history_tokens > stale.history_tokens);
    {
        let mut guard = app
            .context_snapshot_result
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = Some(fresh);
    }
    app.context_refresh_inflight = true;
    app.poll_context_snapshot_refresh();

    assert_eq!(app.context_snapshot_cache, Some(fresh));
    assert_ne!(app.context_snapshot_cache, Some(stale));
}

#[test]
fn test_effective_snapshot_prefers_cache() {
    // FR-015: renders read the cached snapshot rather than recomputing
    // disk-backed values per frame. The first frame before any refresh
    // completes falls back to the synchronous computation.
    let mut app = support::make_app();
    let fallback = app.context_effective_snapshot();
    let direct = app.context_partition_snapshot();
    assert_eq!(fallback, direct, "first-frame fallback must be accurate");

    let mut cached_value = direct;
    cached_value.system_prompt_tokens += 1;
    app.context_snapshot_cache = Some(cached_value);
    assert_eq!(
        app.context_effective_snapshot(),
        cached_value,
        "effective snapshot must prefer the cache over recompute"
    );
}
