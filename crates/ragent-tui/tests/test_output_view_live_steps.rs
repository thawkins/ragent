//! TUI tests for the live output-view overlay (running sub-agent steps).
//!
//! The output-view overlay for another session's target must rebuild its
//! cached lines as the agent advances, so completed steps appear live instead
//! of only after the run ends. Two mechanisms make that work:
//!
//! * the interim save persists completed tool-call parts into the child
//!   session's SQLite row mid-run (see
//!   `crates/ragent-agent/tests/test_interim_tool_call_persist.rs`), and
//! * the overlay's generation key mixes in the same per-session step /
//!   tool-call counters the Agents panel shows, so the cache invalidates
//!   without them changing.
//!
//! These tests drive the real render path with a `TestBackend`.

mod support;

use std::sync::Arc;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use serde_json::json;

use ragent_tui::app::{OutputViewLineCache, OutputViewState, OutputViewTarget};
use ragent_types::message::{Message, MessagePart, ToolCallState};

/// Create the child session row first: `messages` has a foreign key on
/// `sessions(id)`, so seeding a transcript requires the session to exist.
fn ensure_session(app: &ragent_tui::App, sid: &str) {
    app.storage
        .create_session(sid, std::env::temp_dir().to_string_lossy().as_ref())
        .expect("seed session row");
}

fn render_once(app: &mut ragent_tui::App, width: u16, height: u16) {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("draw frame");
}

/// Open the output overlay on `sid` and render one frame; returns the
/// resulting cache generation so callers can assert invalidation.
fn open_and_render(
    app: &mut ragent_tui::App,
    sid: &str,
    label: &str,
    width: u16,
    height: u16,
) -> u64 {
    app.output_view = Some(OutputViewState {
        target: OutputViewTarget::Session {
            session_id: sid.to_string(),
            label: label.to_string(),
        },
        scroll_offset: 0,
        max_scroll: 0,
        line_cache: OutputViewLineCache::default(),
    });
    render_once(app, width, height);
    app.output_view
        .as_ref()
        .expect("view open")
        .line_cache
        .source_generation
}

fn screen_text(app: &ragent_tui::App) -> String {
    app.output_view
        .as_ref()
        .expect("view open")
        .line_cache
        .content_lines
        .join("\n")
}

/// A completed `think` tool-call part, as the interim save persists it.
fn think_tool_call_part() -> MessagePart {
    MessagePart::ToolCall {
        tool: "think".to_string(),
        call_id: "call_1".to_string(),
        state: Box::new(ToolCallState {
            status: ragent_types::message::ToolCallStatus::Completed,
            input: json!({"thought": "considering the next step"}),
            output: Some(json!({"thought": "considering the next step"})),
            error: None,
            duration_ms: Some(5),
        }),
    }
}

/// The cache must hold one copy of the rendered rows while nothing advances
/// (same-width frame, unchanged counters) and must rebuild when the agent's
/// tool-call counter advances — the same counter the Agents panel shows.
#[tokio::test]
async fn test_output_view_cache_invalidates_on_agent_progress() {
    let mut app = support::make_app();
    app.session_id = Some("lead-sess".to_string());

    // Register a sub-agent task so its child session becomes tracked.
    app.handle_event(ragent_agent::event::Event::SubagentStart {
        session_id: "lead-sess".to_string(),
        task_id: "explore-a1b2c3d4".to_string(),
        child_session_id: "child-sess-1".to_string(),
        agent: "explore".to_string(),
        task: "find callers".to_string(),
        background: true,
    })
    .await;

    let g0 = open_and_render(&mut app, "child-sess-1", "explore [a1b2c3d4]", 120, 40);
    let rows_before: Vec<String> = app
        .output_view
        .as_ref()
        .expect("view open")
        .line_cache
        .content_lines
        .clone();

    // Same-width frame with no progress: cache reused verbatim.
    render_once(&mut app, 120, 40);
    let view = app.output_view.as_ref().expect("view open");
    assert_eq!(
        view.line_cache.source_generation, g0,
        "unchanged state must reuse the cache"
    );
    let rows_after: Vec<String> = view.line_cache.content_lines.clone();
    assert_eq!(rows_before, rows_after);

    // Advance the agent the way the session processor does per tool call:
    // the counter the Agents panel's steps column shows.
    app.event_bus.increment_tool_calls("child-sess-1");
    render_once(&mut app, 120, 40);
    let view = app.output_view.as_ref().expect("view open");
    assert_ne!(
        view.line_cache.source_generation, g0,
        "a tool-call advance must invalidate the cache"
    );

    // A step advance also invalidates (counter mix covers both axes).
    let g1 = view.line_cache.source_generation;
    app.event_bus.set_step("child-sess-1", 2);
    render_once(&mut app, 120, 40);
    let view = app.output_view.as_ref().expect("view open");
    assert_ne!(
        view.line_cache.source_generation, g1,
        "a step advance must invalidate the cache"
    );
}

/// Once the interim save has landed a tool-call part in the child session's
/// row, a re-rendered overlay shows the step (tool name + summary) instead of
/// the "No output yet" placeholder.
#[tokio::test]
async fn test_output_view_shows_mid_run_tool_call_rows() {
    let mut app = support::make_app();
    app.session_id = Some("lead-sess".to_string());

    let child = "child-sess-2";
    ensure_session(&app, child);
    // Seed the child session the way a run does: a user prompt plus the
    // assistant placeholder holding a completed tool-call part (the interim
    // save writes exactly this shape mid-run).
    app.storage
        .create_message(&Message::user_text(child, "find callers"))
        .expect("seed user message");
    let mut assistant = Message::new(child, ragent_types::message::Role::Assistant, vec![]);
    assistant.parts.push(think_tool_call_part());
    assistant.edit_seq = 1;
    app.storage
        .create_message(&assistant)
        .expect("seed assistant message");

    let g0 = open_and_render(&mut app, child, "explore [b2c3d4e5]", 120, 40);
    let text = screen_text(&app);
    assert!(
        !text.contains("No output yet"),
        "seeded tool-call row must render: {text}"
    );
    assert!(
        text.contains("Think"),
        "the tool name row must be visible: {text}"
    );

    // Advancing the counter re-renders; the seeded row stays.
    app.event_bus.increment_tool_calls(child);
    render_once(&mut app, 120, 40);
    let view = app.output_view.as_ref().expect("view open");
    assert_ne!(view.line_cache.source_generation, g0);
    assert!(screen_text(&app).contains("Think"));
}

/// A storage-backed target (a session other than the live one — exactly the
/// path a running sub-agent's overlay takes) renders its persisted transcript,
/// and the counter mix must not corrupt the content on rebuild.
#[tokio::test]
async fn test_output_view_storage_backed_target_renders_transcript() {
    let mut app = support::make_app();
    app.session_id = Some("lead-sess".to_string());
    let sid = "peer-sess-1".to_string();

    ensure_session(&app, &sid);
    app.storage
        .create_message(&Message::user_text(&sid, "hello"))
        .expect("seed message");

    let g0 = open_and_render(&mut app, &sid, "peer", 120, 40);
    assert!(
        screen_text(&app).contains("hello"),
        "storage-backed transcript must render"
    );

    // Counters for the peer session also advance during a run; the mix must
    // not break the storage-backed path (content identical, cache rebuilds).
    app.event_bus.increment_tool_calls(&sid);
    render_once(&mut app, 120, 40);
    let view = app.output_view.as_ref().expect("view open");
    assert_ne!(view.line_cache.source_generation, g0);
    assert!(screen_text(&app).contains("hello"));
}

/// Guard against a regression where the mix made every frame rebuild: with
/// zero progress and an empty child session the second frame must reuse the
/// cached "No output yet" row set.
#[tokio::test]
async fn test_output_view_empty_child_session_cache_is_stable() {
    let mut app = support::make_app();
    app.session_id = Some("lead-sess".to_string());

    let g0 = open_and_render(&mut app, "child-sess-3", "explore [c3d4e5f6]", 120, 40);
    let text = screen_text(&app);
    assert!(
        text.contains("No output yet"),
        "empty child session shows placeholder"
    );

    render_once(&mut app, 120, 40);
    let view = app.output_view.as_ref().expect("view open");
    assert_eq!(view.line_cache.source_generation, g0);
    assert_eq!(screen_text(&app), text);
    // Sanity: the placeholder occupied at least one cached row.
    assert!(!view.line_cache.wrapped_lines.is_empty());
}

/// The `Arc<Message>` round-trip through storage keeps the tool-call part
/// serialisable — a cheap sanity check that the seeded shape matches what the
/// interim save writes (`MessagePart::ToolCall` with a completed state).
#[test]
fn test_seeded_tool_call_part_serialises_like_interim_save() {
    let mut assistant = Message::new("s", ragent_types::message::Role::Assistant, vec![]);
    assistant.parts.push(think_tool_call_part());
    let stored = Arc::new(assistant);
    let json = serde_json::to_string(&stored).expect("serialise");
    let round: Message = serde_json::from_str(&json).expect("deserialise");
    assert!(matches!(round.parts[0], MessagePart::ToolCall { .. }));
}
