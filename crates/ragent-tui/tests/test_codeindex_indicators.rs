//! Tests for the codeindex busy indicators on the status bar (line 2,
//! top-right) and the background codeindex task result poller.
//!
//! Covers:
//! - `⟳idx` renders when `code_index_busy` is latched (FTS indexing in
//!   progress).
//! - `⟳graph` renders when `code_index_graph_busy` is latched (semantic graph
//!   being built) and does NOT render when idle.
//! - Both indicators appear on the second status-bar line.
//! - `poll_codeindex_bg_result` drains the off-thread completion payload,
//!   clears the spawned latches, and sets the carried status text.
//! - The graph-busy latch self-clears when no index is attached.

use std::sync::Arc;

use ragent_agent::{
    StreamConfig, agent,
    event::EventBus,
    provider,
    session::{SessionManager, processor::SessionProcessor},
    storage::Storage,
    tool,
};
use ragent_codeindex::CodeIndex;
use ragent_codeindex::types::CodeIndexConfig;
use ragent_tui::{App, layout};
use ratatui::{Terminal, backend::TestBackend};

/// Open an in-memory CodeIndex and attach it to the app, mirroring how the
/// real app attaches the index after `/codeindex on`.
fn attach_in_memory_index(app: &mut App) -> Arc<CodeIndex> {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let config = CodeIndexConfig {
        enabled: true,
        project_root: dir.path().to_path_buf(),
        index_dir: dir.path().join(".ragent/codeindex"),
        scan_config: ragent_codeindex::types::ScanConfig::default(),
    };
    let idx = Arc::new(CodeIndex::open_in_memory(&config).expect("in-memory code index"));
    app.code_index = Some(idx.clone());
    idx
}

/// Age out the refresh throttle so the refresh body actually runs.
fn age_refresh_throttle(app: &mut App) {
    app.code_index_stats_last_refresh = std::time::Instant::now()
        .checked_sub(std::time::Duration::from_secs(10))
        .unwrap_or(std::time::Instant::now());
}

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

#[test]
fn test_statusbar_renders_graph_busy_indicator() {
    let mut app = make_app();
    app.code_index_graph_busy = true;
    let frame = render_app_to_string(&mut app);
    let line2 = frame.lines().nth(1).unwrap_or("");
    assert!(
        line2.contains("\u{27f3}graph"),
        "second status-bar line must contain the graph-busy indicator when the \
         graph is building; line was: {line2}"
    );
}

#[test]
fn test_statusbar_renders_index_busy_indicator() {
    let mut app = make_app();
    app.code_index_busy = true;
    let frame = render_app_to_string(&mut app);
    let line2 = frame.lines().nth(1).unwrap_or("");
    assert!(
        line2.contains("\u{27f3}idx"),
        "second status-bar line must contain the index-busy indicator while \
         indexing; line was: {line2}"
    );
}

#[test]
fn test_statusbar_renders_both_indicators_together() {
    let mut app = make_app();
    app.code_index_busy = true;
    app.code_index_graph_busy = true;
    let frame = render_app_to_string(&mut app);
    let line2 = frame.lines().nth(1).unwrap_or("");
    assert!(
        line2.contains("\u{27f3}idx") && line2.contains("\u{27f3}graph"),
        "both busy indicators must render when indexing and graph building \
         overlap; line was: {line2}"
    );
}

#[test]
fn test_statusbar_hides_indicators_when_idle() {
    let mut app = make_app();
    let frame = render_app_to_string(&mut app);
    let line2 = frame.lines().nth(1).unwrap_or("");
    assert!(
        !line2.contains("\u{27f3}idx") && !line2.contains("\u{27f3}graph"),
        "no busy indicators may render when the index is idle; line was: {line2}"
    );
}

#[test]
fn test_poll_codeindex_bg_result_drains_completion() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.code_index_graph_spawned = true;
    app.status = "[wait] codeindex: building graph…".to_string();

    let payload = Ok(
        "\u{2705} Graph built: 42 edges (30 EXTRACTED, 12 INFERRED) in 100ms.\n\n\
         STATUS:codeindex: graph built (42 edges)"
            .to_string(),
    );
    {
        let mut guard = app.code_index_bg_result.lock().unwrap();
        *guard = Some(payload);
    }

    app.poll_codeindex_bg_result();

    assert_eq!(app.status, "codeindex: graph built (42 edges)");
    assert!(
        !app.code_index_graph_spawned,
        "spawned latch must clear once the result is drained"
    );
    // The rendered message window is repopulated on each render pass.
    let _frame = render_app_to_string(&mut app);
    let window = app.message_content_lines.join("\n");
    assert!(
        window.contains("Graph built: 42 edges"),
        "message window must show the completion message; got: {window}"
    );
}

#[test]
fn test_poll_codeindex_bg_result_reports_error() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.code_index_reindex_spawned = true;
    {
        let mut guard = app.code_index_bg_result.lock().unwrap();
        *guard = Some(Err("Re-index failed: disk on fire".to_string()));
    }

    app.poll_codeindex_bg_result();

    assert!(!app.code_index_reindex_spawned);
    assert!(
        app.status.contains("failed"),
        "status must surface the failure; got: {}",
        app.status
    );
    // The rendered message window is repopulated on each render pass.
    let _frame = render_app_to_string(&mut app);
    let window = app.message_content_lines.join("\n");
    assert!(
        window.contains("Re-index failed"),
        "message window must show the error; got: {window}"
    );
}

#[test]
fn test_poll_codeindex_bg_result_noop_when_empty() {
    let mut app = make_app();
    app.status = "ready".to_string();
    app.poll_codeindex_bg_result();
    assert_eq!(app.status, "ready", "idle poll must not touch the status");
}

#[test]
fn test_graph_busy_latch_clears_when_no_index_attached() {
    let mut app = make_app();
    app.code_index_graph_busy = true;
    app.needs_redraw = false;
    // Age out the refresh throttle so the refresh body actually runs.
    app.code_index_stats_last_refresh = std::time::Instant::now()
        .checked_sub(std::time::Duration::from_secs(10))
        .unwrap_or(std::time::Instant::now());

    // No real CodeIndex handle is attached, so refresh takes the `else`
    // branch, which must clear the graph-busy latch too.
    app.refresh_code_index_stats_for_test();
    assert!(
        !app.code_index_graph_busy,
        "graph-busy latch must clear when no code index is attached"
    );
}

#[test]
fn test_graph_build_holding_store_lock_does_not_latch_idx_indicator() {
    let mut app = make_app();
    let idx = attach_in_memory_index(&mut app);
    age_refresh_throttle(&mut app);

    // Simulate the graph phase of a reindex (or a standalone graph build):
    // the graph build holds the store mutex for its entire duration, so
    // try_status() returns None while graph_busy is set.
    let store_guard = idx.try_lock_store_for_test().expect("store lock");
    idx.set_graph_busy_for_test(true);

    app.refresh_code_index_stats_for_test();

    drop(store_guard);
    assert!(
        app.code_index_graph_busy,
        "graph-busy latch must be set while the graph build runs"
    );
    assert!(
        !app.code_index_busy,
        "idx indicator must NOT latch while only the graph build holds the \
         store lock (false-positive made both indicators vanish together)"
    );
}

#[test]
fn test_idx_indicator_clears_when_graph_phase_starts() {
    let mut app = make_app();
    let idx = attach_in_memory_index(&mut app);
    age_refresh_throttle(&mut app);

    // End of the indexing phase: the idx latch is set (from a previous
    // poll), then the reindex counters complete and the graph phase begins.
    app.code_index_busy = true;
    idx.set_graph_progress_for_test(5, 5); // done == total -> reindex not active
    let store_guard = idx.try_lock_store_for_test().expect("store lock");
    idx.set_graph_busy_for_test(true);

    app.refresh_code_index_stats_for_test();

    drop(store_guard);
    assert!(
        !app.code_index_busy,
        "idx indicator must clear as soon as the graph phase starts, not when \
         the whole pipeline finishes"
    );
    assert!(
        app.code_index_graph_busy,
        "graph indicator must be latched during the graph phase"
    );
}

#[test]
fn test_graph_phase_end_clears_only_graph_indicator() {
    let mut app = make_app();
    let idx = attach_in_memory_index(&mut app);
    age_refresh_throttle(&mut app);

    // Graph phase running with a stale idx latch from before the fix.
    app.code_index_busy = true;
    app.code_index_graph_busy = true;
    let store_guard = idx.try_lock_store_for_test().expect("store lock");
    idx.set_graph_busy_for_test(true);
    app.refresh_code_index_stats_for_test();
    drop(store_guard);
    assert!(!app.code_index_busy);
    assert!(app.code_index_graph_busy);

    // Graph build completes: the lock frees and graph_busy clears. Only the
    // graph indicator must clear; the idx latch must stay off.
    idx.set_graph_busy_for_test(false);
    age_refresh_throttle(&mut app);
    app.refresh_code_index_stats_for_test();
    assert!(
        !app.code_index_graph_busy,
        "graph-busy latch must clear when the graph build finishes"
    );
    assert!(
        !app.code_index_busy,
        "idx indicator must stay clear after the graph phase ends"
    );
}
