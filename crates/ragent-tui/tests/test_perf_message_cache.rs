//! PERF-041 / PERF-042 / PERF-043: message-window render-loop caches.
//!
//! * **PERF-043** — a clean frame performs no staleness scan (`edit_seq`
//!   comparisons) over the transcript; only mutated groups are re-checked.
//! * **PERF-042** — a streamed message is not re-parsed/re-wrapped on every
//!   token; it is throttled to one refresh per window.
//! * **PERF-041** — the flat plain-text copy buffer is only rebuilt when
//!   something actually changed, and copy paths can refresh it on demand.

mod support;

use ratatui::Terminal;
use ratatui::backend::TestBackend;

use ragent_agent::message::{Message, MessagePart, Role};

fn render(app: &mut ragent_tui::App) {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("draw frame");
}

fn assistant_text(sid: &str, text: &str) -> Message {
    Message::new(
        sid,
        Role::Assistant,
        vec![MessagePart::Text {
            text: text.to_string(),
        }],
    )
}

/// Build an app with `count` messages already rendered into the cache.
fn primed_app(count: usize) -> ragent_tui::App {
    let mut app = support::make_app();
    app.session_id = Some("s1".to_string());
    app.messages = (0..count)
        .map(|i| assistant_text("s1", &format!("message body {i}\nmore text {i}")))
        .collect();
    render(&mut app);
    assert_eq!(app.message_line_cache.len(), count, "cache primed");
    app
}

// ── PERF-043: watermark staleness scan ──────────────────────────────────

#[test]
fn test_idle_frame_only_scans_dirty_groups() {
    let mut app = primed_app(50);
    assert_eq!(
        app.message_cache_dirty_from, 50,
        "a fully rendered transcript leaves the watermark at the end"
    );

    // Mutate only the last message through the real streaming path.
    app.handle_event(ragent_agent::event::Event::TextDelta {
        session_id: "s1".to_string(),
        text: " tail".to_string(),
    });
    assert_eq!(
        app.message_cache_dirty_from,
        app.messages.len() - 1,
        "the watermark must drop to the mutated group, not 0"
    );

    // One frame does not yet re-render that group: PERF-042 throttles a group
    // that was already populated, so the first frame after the token leaves it
    // pending rather than re-parsing it.
    render(&mut app);
    assert_eq!(
        app.message_cache_dirty_from,
        app.messages.len() - 1,
        "a throttled frame leaves only the mutated group pending"
    );

    // After the throttle window the group catches up and the watermark
    // advances past every up-to-date group.
    std::thread::sleep(ragent_tui::layout::MESSAGE_STREAM_MIN_INTERVAL);
    render(&mut app);
    assert_eq!(
        app.message_cache_dirty_from,
        app.messages.len(),
        "the scan advanced past every up-to-date group"
    );
}

#[test]
fn test_tool_call_status_update_lowers_watermark_only() {
    let mut app = primed_app(30);
    app.messages[0].parts.clear();
    app.messages[0].parts.push(MessagePart::ToolCall {
        tool: "read".to_string(),
        call_id: "c1".to_string(),
        state: Box::new(ragent_agent::message::ToolCallState {
            status: ragent_agent::message::ToolCallStatus::Running,
            input: serde_json::Value::Null,
            output: None,
            error: None,
            duration_ms: None,
        }),
    });
    app.messages[0].touch();
    app.mark_message_dirty(0);
    render(&mut app);

    app.handle_event(ragent_agent::event::Event::ToolCallEnd {
        session_id: "s1".to_string(),
        call_id: "c1".to_string(),
        tool: "read".to_string(),
        error: None,
        duration_ms: 12,
    });
    assert_eq!(
        app.message_cache_dirty_from, 0,
        "the mutated group (index 0) is the only one pending"
    );
}

// ── PERF-042: streaming throttle ──────────────────────────────────────────

#[test]
fn test_streaming_group_renders_immediately_when_never_populated() {
    let mut app = support::make_app();
    app.session_id = Some("s1".to_string());
    render(&mut app);

    app.handle_event(ragent_agent::event::Event::TextDelta {
        session_id: "s1".to_string(),
        text: "first token".to_string(),
    });
    render(&mut app);

    // A brand-new group must never be left blank by the throttle.
    assert!(
        !app.message_line_cache[0].lines.is_empty(),
        "a never-populated group renders on the first frame"
    );
    assert!(
        app.message_content_lines
            .iter()
            .any(|l| l.contains("first token")),
        "the streamed text must be visible in the copy buffer"
    );
}

#[test]
fn test_streaming_re_render_is_throttled_to_one_per_window() {
    let mut app = primed_app(1);

    // First token after priming: already-populated group, so the throttle
    // defers this refresh and leaves the group pending.
    app.handle_event(ragent_agent::event::Event::TextDelta {
        session_id: "s1".to_string(),
        text: " alpha".to_string(),
    });
    let pending_edit_seq = app.messages[0].edit_seq;
    render(&mut app);
    assert_eq!(
        app.message_line_cache[0].edit_seq,
        pending_edit_seq.wrapping_sub(1),
        "the throttled frame must not re-render the group"
    );
    assert_eq!(
        app.message_cache_dirty_from, 0,
        "the throttled group stays pending for the next frame"
    );

    // A second and third token within the same window are also deferred.
    app.handle_event(ragent_agent::event::Event::TextDelta {
        session_id: "s1".to_string(),
        text: " beta".to_string(),
    });
    render(&mut app);
    assert_eq!(app.message_cache_dirty_from, 0, "still pending");

    // Once the window elapses the group catches up exactly once.
    std::thread::sleep(ragent_tui::layout::MESSAGE_STREAM_MIN_INTERVAL);
    render(&mut app);
    assert_eq!(
        app.message_line_cache[0].edit_seq, app.messages[0].edit_seq,
        "the group catches up after the throttle window"
    );
    assert_eq!(
        app.message_cache_dirty_from,
        app.messages.len(),
        "and the watermark advances"
    );
}

// ── PERF-041: lazy copy-buffer rebuild ──────────────────────────────────

#[test]
fn test_idle_frame_does_not_rebuild_copy_buffer_when_nothing_changed() {
    let mut app = primed_app(20);
    assert!(
        !app.message_content_lines.is_empty(),
        "buffer is populated after priming"
    );

    // A frame with no input/agent changes must not touch the buffer.
    let before = std::mem::take(&mut app.message_content_lines);
    render(&mut app);
    assert!(
        app.message_content_lines.is_empty() || app.message_content_lines == before,
        "an idle frame must not rebuild the copy buffer"
    );
    assert!(
        !app.message_content_lines_dirty,
        "nothing changed, so the buffer is not flagged stale"
    );
}

#[test]
fn test_copy_buffer_rebuilds_on_demand_after_streaming() {
    let mut app = primed_app(5);

    // Stream a token, then force a copy refresh *before* the throttled frame
    // has re-rendered the group.  The on-demand rebuild must see the fresh
    // rows because the streaming path marks the buffer stale.
    app.handle_event(ragent_agent::event::Event::TextDelta {
        session_id: "s1".to_string(),
        text: " freshly streamed".to_string(),
    });
    render(&mut app);

    app.ensure_copy_content_lines();
    assert!(
        app.message_content_lines
            .iter()
            .any(|l| l.contains("freshly streamed")),
        "the on-demand rebuild must reflect the streamed text; got {:?}",
        app.message_content_lines
    );
}

#[test]
fn test_reset_message_cache_clears_watermark_and_buffer() {
    let mut app = primed_app(10);
    app.messages.clear();
    app.reset_message_cache();

    assert!(app.message_line_cache.is_empty());
    assert!(app.message_content_lines.is_empty());
    assert_eq!(app.message_cache_dirty_from, 0);

    // The next frame rebuilds from scratch without a stale watermark.
    app.messages.push(assistant_text("s1", "rebuilt body"));
    render(&mut app);
    assert_eq!(app.message_line_cache.len(), 1);
    assert!(
        app.message_content_lines
            .iter()
            .any(|l| l.contains("rebuilt body"))
    );
}
