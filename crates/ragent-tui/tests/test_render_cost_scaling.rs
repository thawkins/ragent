//! Regression tests for idle-CPU hotspots (v1.0.64 CPU investigation).
//!
//! These tests pin the render-cost contracts the idle-CPU fix relies on:
//!
//! 1. `code_index_busy` is cleared once a reindex completes (done == total),
//!    so the TUI loop drops back to its 2 s idle deadline instead of waking
//!    4x/sec forever.
//! 2. `render_messages` / `render_log_panel` only wrap the visible scroll
//!    window, so per-frame cost is bounded by viewport height rather than
//!    transcript length (long transcripts no longer pin a core at idle).

use std::sync::Arc;

use ragent_agent::{
    StreamConfig, agent,
    event::EventBus,
    provider,
    session::{SessionManager, processor::SessionProcessor},
    storage::Storage,
    tool,
};
use ragent_tui::{App, layout};
use ragent_types::message::{Message, MessagePart, Role};
use ratatui::{Terminal, backend::TestBackend};

/// Build an App mirroring the make_app() pattern used by the other TUI tests.
fn make_app() -> App {
    let event_bus = Arc::new(EventBus::default());
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let provider_registry = Arc::new(provider::create_default_registry());
    let tool_registry = Arc::new(tool::create_default_registry());
    let permission_checker = Arc::new(parking_lot::RwLock::new(
        ragent_agent::permission::PermissionChecker::new(vec![]),
    ));
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));
    let session_processor = Arc::new(SessionProcessor {
        session_manager,
        provider_registry: provider_registry.clone(),
        tool_registry,
        permission_checker,
        event_bus: event_bus.clone(),
        agent_manager: std::sync::OnceLock::new(),
        bg_service: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        code_index: std::sync::OnceLock::new(),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: StreamConfig::default(),
        active_spec: tokio::sync::RwLock::new(None),
        spec_manager: std::sync::OnceLock::new(),
        cached_tool_definitions: parking_lot::RwLock::new(None),
        cached_tool_names: parking_lot::RwLock::new(None),
        cached_tool_definition_bytes: parking_lot::RwLock::new(None),
        llm_client_cache: parking_lot::RwLock::new(std::collections::HashMap::new()),
        cached_config: parking_lot::Mutex::new(None),
        team_context_cache: std::sync::Arc::new(parking_lot::RwLock::new(
            std::collections::HashMap::new(),
        )),
        auto_approve: false,
        system_prompt_cache: parking_lot::RwLock::new(None),
        skill_body_cache: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        telemetry: std::sync::Arc::new(ragent_agent::telemetry::TelemetrySubsystem::disabled()),
        activity_log: std::sync::OnceLock::new(),
        skill_registry_cache: parking_lot::Mutex::new(None),
    });
    let agent_info =
        agent::resolve_agent("general", &Default::default()).expect("resolve general agent");
    App::new(
        event_bus.clone(),
        storage,
        provider_registry,
        session_processor,
        Arc::unwrap_or_clone(agent_info),
        true,
        std::path::PathBuf::new(),
    )
}

/// Public test hook (#[doc(hidden)]) on App for refresh-code-index behavior.

fn long_message(id: &str, session: &str, text: &str) -> Message {
    let mut msg = Message::new(
        session,
        Role::Assistant,
        vec![MessagePart::Text {
            text: text.to_string(),
        }],
    );
    msg.id = id.to_string();
    msg
}

/// Render the app at 120x40 and return the visible frame text.
fn render_app_to_string(app: &mut App) -> String {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, app))
        .expect("render app");
    let rows: Vec<String> = (0..40)
        .map(|y| {
            (0..120)
                .map(|x| {
                    terminal
                        .backend()
                        .buffer()
                        .cell((x, y))
                        .map(|c| c.symbol().to_string())
                        .unwrap_or_default()
                })
                .collect::<String>()
        })
        .collect();
    rows.join("\n")
}

const TAIL_MARKER: &str = "TAILMARKER-7f3a";

/// Test 1: after `refresh_code_index_stats` runs with no code index
/// attached, the busy latch must clear so the loop deadline returns to the
/// idle cadence. Regression for the `total > 0 -> reindex_active` latch
/// that kept `compute_next_deadline` at 250 ms forever after any reindex.
#[test]
fn test_code_index_busy_clears_when_no_index_attached() {
    let mut app = make_app();

    // Simulate the "stats locks were momentarily busy" entry point that sets
    // the busy flag, and age out the refresh throttle so the refresh body
    // actually runs.
    app.code_index_busy = true;
    app.needs_redraw = false;
    app.code_index_stats_last_refresh = std::time::Instant::now()
        .checked_sub(std::time::Duration::from_secs(10))
        .unwrap_or(std::time::Instant::now());

    // No real CodeIndex handle is attached, so refresh takes the `else`
    // branch, which must unconditionally clear the busy flag.
    app.refresh_code_index_stats_for_test();
    assert!(
        !app.code_index_busy,
        "busy latch must clear when no code index is attached"
    );
}

/// Test 2: render cost must NOT scale with transcript length. Render the
/// same viewport twice — short transcript vs 500-message transcript — after
/// warming caches, and assert steady-state per-frame cost stays bounded.
/// Regression for the whole-transcript Paragraph wrap that dominated perf
/// profiles after long agent turns.
#[test]
fn test_render_cost_bounded_by_viewport_not_transcript() {
    let chunk = format!(
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod \
         tempor incididunt ut labore et dolore magna aliqua. {TAIL_MARKER}"
    );

    let make_messages = |count: usize| -> Vec<Message> {
        (0..count)
            .map(|i| long_message(&format!("m{i}"), "s1", &chunk))
            .collect()
    };

    let mut app_short = make_app();
    app_short.messages = make_messages(5);
    let mut app_long = make_app();
    app_long.messages = make_messages(500);

    // Warm both caches so we measure steady-state per-frame cost.
    let _ = render_app_to_string(&mut app_short);
    let _ = render_app_to_string(&mut app_long);

    let frames = 10;
    let start = std::time::Instant::now();
    for _ in 0..frames {
        let _ = render_app_to_string(&mut app_short);
    }
    let short_elapsed = start.elapsed();

    let start = std::time::Instant::now();
    for _ in 0..frames {
        let _ = render_app_to_string(&mut app_long);
    }
    let long_elapsed = start.elapsed();

    // A 100x larger transcript must not cost dramatically more per frame.
    // Allow generous headroom for allocator noise; the un-sliced
    // implementation costs ~20-100x on 500-message transcripts.
    assert!(
        long_elapsed.as_secs_f64() < 3.0 * short_elapsed.as_secs_f64(),
        "render cost must be bounded by viewport, not transcript length \
         (short 5 msgs: {short_elapsed:?}, long 500 msgs: {long_elapsed:?})"
    );
}

/// Test 3: auto-scroll must still show the newest message — the scroll
/// window slice must expose the tail of the transcript, not blank content.
#[test]
fn test_auto_scroll_shows_tail_after_slice_fix() {
    let mut app = make_app();
    for i in 0..500 {
        app.messages.push(long_message(
            &format!("m{i}"),
            "s1",
            &format!("filler message {i}"),
        ));
    }
    // Newest message carries a unique marker.
    app.messages.push(long_message("m-tail", "s1", TAIL_MARKER));

    let frame = render_app_to_string(&mut app);
    assert!(
        frame.contains(TAIL_MARKER),
        "newest message must be visible after auto-scroll with sliced rendering"
    );
}

/// Test 4: scrolling back up must still render older content through the
/// slice path (scroll_offset > 0 shows earlier lines, not the tail).
#[test]
fn test_scroll_up_shows_older_content_after_slice_fix() {
    let mut app = make_app();
    for i in 0..500 {
        app.messages.push(long_message(
            &format!("m{i}"),
            "s1",
            &format!("FILLER-{i:04} unique content"),
        ));
    }

    // Render once to populate caches and max_scroll.
    let _ = render_app_to_string(&mut app);
    // Jump to the very top.
    app.scroll_offset = app.message_max_scroll;
    let frame = render_app_to_string(&mut app);
    assert!(
        frame.contains("FILLER-0000"),
        "scrolled-to-top view must show the first message"
    );
    assert!(
        !frame.contains("FILLER-0499"),
        "scrolled-to-top view must not show the last message"
    );
}
