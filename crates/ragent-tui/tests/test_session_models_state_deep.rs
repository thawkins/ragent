//! Deep coverage for `app::session_ops`, `app::models` and `app::state`.
//!
//! These tests drive the `pub`/`pub(crate)` `App` methods of the three
//! modules through the observable surfaces the TUI actually uses: key events
//! ([`App::handle_key_event`]), slash commands
//! ([`App::execute_slash_command`]), mouse events, the render path and the
//! main-loop poll methods. Nothing under `src/` is modified.
//!
//! Tests that read or write the working directory or the config tree run
//! inside an isolated tempdir project (`slash_support::enter_project`), so a
//! test run never touches the developer's real `~/.config/ragent/` and never
//! writes into the repository. Env mutation is `unsafe` in edition 2024 and the
//! workspace denies `unsafe_code`, so this target opts back in explicitly; the
//! isolation harness holds the process-global mutex across the async body, so
//! `clippy::await_holding_lock` is allowed too.
#![allow(unsafe_code)]
#![cfg(test)]
#![allow(clippy::await_holding_lock)]

use std::sync::Arc;
use std::time::Duration;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;

use ragent_agent::event::Event;
use ragent_agent::message::Message;
use ragent_agent::team::{MemberStatus, TeamConfig, TeamMember, TeamStore};
use ragent_codeindex::CodeIndex;
use ragent_codeindex::types::{CodeIndexConfig, ScanConfig};
use ragent_connectors::{CatalogueBrowser, ConnectorSession, store_and_config};
use ragent_plugins::{StoreEntry, StoreKind};
use ragent_tui::App;
use ragent_tui::app::{
    ConfiguredProvider, FileMenuEntry, FileMenuState, HOUSEKEEPING_INTERVAL, LogEntry, LogLevel,
    LogLineGroup, PlanApprovalState, PluginStoreBrowser, ProviderSetupStep, ProviderSource,
    QuestionRequest, ScrollbarDragPane,
};
use ragent_tui::input_field::InputField;
use ragent_tui::layout;

#[path = "slash_support/mod.rs"]
mod slash_support;
#[path = "support/mod.rs"]
mod support;

use slash_support::{app_with_session, enter_project, last_message};

// ---------------------------------------------------------------------------
// harness helpers
// ---------------------------------------------------------------------------

/// A plain key event with no modifiers.
fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// A key event carrying one modifier.
fn key_mod(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

/// An app in a fresh isolated project, optionally seeded with config JSON.
fn isolated(
    config: &str,
) -> (
    std::sync::MutexGuard<'static, ()>,
    slash_support::EnvGuard,
    tempfile::TempDir,
    App,
) {
    let (guard, env, temp) = enter_project(config);
    let app = app_with_session();
    (guard, env, temp, app)
}

/// The default isolated project: one agent, no provider overrides.
fn isolated_app() -> (
    std::sync::MutexGuard<'static, ()>,
    slash_support::EnvGuard,
    tempfile::TempDir,
    App,
) {
    isolated(r#"{ "defaultAgent": "general" }"#)
}

/// Mark the app as fully configured so the send path passes its guards.
fn configure_provider(app: &mut App, provider: &str, model: &str) {
    app.configured_provider = Some(ConfiguredProvider {
        id: provider.to_string(),
        name: provider.to_string(),
        source: ProviderSource::Database,
    });
    app.selected_model = Some(model.to_string());
}

/// Type `text` into the prompt and press Enter.
async fn type_and_enter(app: &mut App, text: &str) {
    app.input = text.to_string();
    app.input_cursor = app.input_len_chars();
    app.handle_key_event(key(KeyCode::Enter)).await;
}

/// Render the app once into a `width` x `height` test terminal.
fn render(app: &mut App, width: u16, height: u16) {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, app))
        .expect("render");
}

/// A log entry with the given sequence number.
fn log_entry(seq: u64) -> LogEntry {
    LogEntry {
        timestamp: chrono::Utc::now(),
        level: LogLevel::Info,
        message: format!("entry {seq}"),
        session_id: None,
        agent_id: None,
        seq,
    }
}

/// An empty (stale) log line cache group.
fn log_group() -> LogLineGroup {
    LogLineGroup {
        lines: Vec::new(),
        wrapped_lines: Vec::new(),
        content_lines: Vec::new(),
        wrapped_count: 0,
        version: 0,
    }
}

/// A plugin-store entry with the given id and install source.
fn store_entry(id: &str, source: &str) -> StoreEntry {
    StoreEntry {
        id: id.to_string(),
        name: id.to_string(),
        version: "1.0.0".to_string(),
        source: source.to_string(),
        description: "test entry".to_string(),
        dialect: None,
        tags: Vec::new(),
        homepage: None,
        marketplace: String::new(),
        category: None,
    }
}

/// Attach an in-memory code index rooted at `root`.
fn attach_index(app: &mut App, root: &std::path::Path) -> Arc<CodeIndex> {
    let config = CodeIndexConfig {
        enabled: true,
        project_root: root.to_path_buf(),
        index_dir: root.join(".ragent/codeindex"),
        scan_config: ScanConfig::default(),
    };
    let idx = Arc::new(CodeIndex::open_in_memory(&config).expect("in-memory code index"));
    app.code_index = Some(idx.clone());
    idx
}

/// Poll `poll` until it reports progress, at most ~2s.
async fn wait_until(mut poll: impl FnMut()) {
    for _ in 0..200 {
        poll();
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// Close the provider-setup `Done` screen with any key, which is the only
/// surface that reaches `check_provider_health`.
async fn close_done_step(app: &mut App) {
    app.provider_setup = Some(ProviderSetupStep::Done {
        provider_name: "test".to_string(),
        model_name: None,
    });
    app.handle_key_event(key(KeyCode::Enter)).await;
}

// ===========================================================================
// session_ops.rs - dispatch paths
// ===========================================================================

#[tokio::test]
async fn bang_command_via_enter_renders_and_dispatches() {
    let (_g, _e, _t, mut app) = isolated_app();

    type_and_enter(&mut app, "!echo hi").await;

    assert_eq!(app.last_prompt, "!echo hi");
    assert_eq!(app.input, "");
    assert_eq!(app.input_cursor, 0);
    let text = last_message(&app);
    assert_eq!(text, "$ echo hi");
    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("bang command: echo hi")),
        "the bang command is logged"
    );
}

#[tokio::test]
async fn bang_command_with_empty_command_warns() {
    let (_g, _e, _t, mut app) = isolated_app();

    type_and_enter(&mut app, "!   ").await;

    assert_eq!(app.status, "[warn] Empty shell command");
    assert_eq!(
        app.messages.len(),
        0,
        "nothing is rendered for an empty bang"
    );
}

#[tokio::test]
async fn bang_command_while_busy_is_refused() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.is_processing = true;

    type_and_enter(&mut app, "!echo hi").await;

    assert_eq!(app.status, "busy - wait for the current turn to finish");
    assert_eq!(app.messages.len(), 0);
}

#[tokio::test]
async fn user_message_dispatch_sets_state_and_logs() {
    let (_g, _e, _t, mut app) = isolated_app();
    configure_provider(&mut app, "ollama", "ollama/qwen3:latest");

    type_and_enter(&mut app, "hello world").await;

    assert_eq!(app.last_prompt, "hello world");
    assert_eq!(app.selected_model.as_deref(), Some("ollama/qwen3:latest"));
    let text = last_message(&app);
    assert_eq!(text, "hello world");
    assert!(
        app.log_entries.iter().any(|e| e
            .message
            .contains("prompt sent [ollama/qwen3:latest]: hello world")),
        "the prompt tag records the selected model: {:?}",
        app.log_entries
            .iter()
            .map(|e| e.message.clone())
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn user_message_with_attachment_renders_attach_prefix() {
    let (_g, _e, temp, mut app) = isolated_app();
    configure_provider(&mut app, "ollama", "ollama/qwen3:latest");
    let png = temp.path().join("shot.png");
    std::fs::write(&png, b"png").expect("write image");
    app.pending_attachments.push(png);

    type_and_enter(&mut app, "see attached").await;

    let text = last_message(&app);
    assert_eq!(text, "[attach shot.png] see attached");
    assert_eq!(app.pending_attachments.len(), 0);
}

#[tokio::test]
async fn user_message_resolves_reference_tokens() {
    let (_g, _e, temp, mut app) = isolated_app();
    configure_provider(&mut app, "ollama", "ollama/qwen3:latest");
    std::fs::write(temp.path().join("notes.txt"), "body").expect("write file");

    type_and_enter(&mut app, "read @notes.txt").await;

    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("resolving refs:") && e.message.contains("notes.txt")),
        "reference tokens are logged: {:?}",
        app.log_entries
            .iter()
            .map(|e| e.message.clone())
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn send_without_provider_reports_guard() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.configured_provider = None;

    type_and_enter(&mut app, "hello").await;

    assert_eq!(
        app.status,
        "[!] No provider configured - use /provider to set up"
    );
}

#[tokio::test]
async fn send_without_model_reports_guard() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.configured_provider = Some(ConfiguredProvider {
        id: "ollama".to_string(),
        name: "Ollama".to_string(),
        source: ProviderSource::Database,
    });
    app.selected_model = None;

    type_and_enter(&mut app, "hello").await;

    assert_eq!(app.status, "[!] No model selected - use /model to choose");
}

#[tokio::test]
async fn auto_compact_threshold_defers_the_send() {
    let (_g, _e, _t, mut app) = isolated_app();
    configure_provider(&mut app, "ollama", "ollama/qwen3:latest");
    app.messages
        .push(Message::user_text("slash-support", "earlier turn"));
    app.selected_model_ctx_window = Some(1_000);
    app.last_input_tokens = 10_000;

    type_and_enter(&mut app, "next turn").await;

    assert!(
        app.pending_send_after_compact.is_some(),
        "the send is deferred behind compaction"
    );
    assert_eq!(app.status, "compacting before send...");
}

#[tokio::test]
async fn resume_continuation_via_slash_command() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.agent_halted = true;

    app.execute_slash_command("/resume").await;

    assert!(!app.agent_halted, "the halted flag is cleared");
    let text = last_message(&app);
    assert!(text.contains("previously interrupted"), "msg: {text}");
    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("Resuming halted agent")),
        "resume is logged"
    );
}

#[tokio::test]
async fn resume_without_halt_reports_nothing_to_resume() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.agent_halted = false;

    app.execute_slash_command("/resume").await;

    assert_eq!(app.status, "Nothing to resume - agent was not halted");
}

#[tokio::test]
async fn ensure_session_creates_one_for_a_slash_command() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.session_id = None;

    app.execute_slash_command("/cost").await;

    assert!(app.session_id.is_some(), "a session is created on demand");
    assert_ne!(app.sid_to_display_name.len(), 0);
}

#[tokio::test]
async fn load_session_restores_messages_and_cwd() {
    let (_g, _e, temp, mut app) = isolated_app();
    let dir = temp.path().join("work");
    std::fs::create_dir_all(&dir).expect("work dir");
    let session = app
        .session_processor
        .session_manager
        .create_session(dir.clone())
        .expect("create session");
    app.storage
        .create_message(&Message::user_text(&session.id, "restored"))
        .expect("seed message");

    app.load_session(&session.id).expect("load session");

    assert_eq!(app.session_id.as_deref(), Some(session.id.as_str()));
    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].text_content(), "restored");
    assert_eq!(app.status, "resumed (1 messages)");
    assert_eq!(app.cwd_path, dir, "the session working directory wins");
}

#[tokio::test]
async fn load_session_with_unknown_id_returns_error() {
    let (_g, _e, _t, mut app) = isolated_app();

    let err = app.load_session("does-not-exist").expect_err("must fail");

    assert!(err.to_string().contains("Session not found"), "err: {err}");
    assert!(
        app.session_id.is_some(),
        "the previous session is untouched"
    );
}

#[tokio::test]
async fn load_session_rebuilds_tool_step_map_from_tool_calls() {
    let (_g, _e, temp, mut app) = isolated_app();
    let session = app
        .session_processor
        .session_manager
        .create_session(temp.path().to_path_buf())
        .expect("create session");
    let mut state = ragent_agent::message::ToolCallState {
        status: ragent_agent::message::ToolCallStatus::Completed,
        input: serde_json::json!({}),
        output: None,
        error: None,
        duration_ms: None,
    };
    state.status = ragent_agent::message::ToolCallStatus::Error;
    let msg = Message::new(
        &session.id,
        ragent_agent::message::Role::Assistant,
        vec![ragent_agent::message::MessagePart::ToolCall {
            tool: "read".to_string(),
            call_id: "call-1".to_string(),
            state: Box::new(state),
        }],
    );
    app.storage.create_message(&msg).expect("seed tool call");

    app.load_session(&session.id).expect("load session");

    assert_eq!(app.tool_step_map.len(), 1);
    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("[1.1] read [x] (restored)")),
        "the restored tool step is logged with its error icon"
    );
}

// ===========================================================================
// session_ops.rs - code index background tasks
// ===========================================================================

#[tokio::test]
async fn codeindex_graph_build_spawns_and_polls() {
    let (_g, _e, temp, mut app) = isolated_app();
    attach_index(&mut app, temp.path());

    app.execute_slash_command("/codeindex graph build").await;
    assert_eq!(app.status, "[wait] codeindex: building graph...");
    assert!(app.code_index_graph_spawned);

    wait_until(|| app.poll_codeindex_bg_result()).await;
    assert!(!app.code_index_graph_spawned, "the latch clears on poll");
}

#[tokio::test]
async fn codeindex_graph_build_for_language_spawns() {
    let (_g, _e, temp, mut app) = isolated_app();
    attach_index(&mut app, temp.path());

    app.execute_slash_command("/codeindex graph lang rust")
        .await;

    assert_eq!(app.status, "[wait] codeindex: building graph (rust)...");
    assert!(app.code_index_graph_spawned);
    wait_until(|| app.poll_codeindex_bg_result()).await;
    assert!(!app.code_index_graph_spawned);
}

#[tokio::test]
async fn codeindex_graph_build_refuses_a_second_build() {
    let (_g, _e, temp, mut app) = isolated_app();
    attach_index(&mut app, temp.path());
    app.code_index_graph_spawned = true;

    app.execute_slash_command("/codeindex graph build").await;

    assert_eq!(app.status, "codeindex: graph build failed");
    let text = last_message(&app);
    assert!(text.contains("already running"), "msg: {text}");
}

#[tokio::test]
async fn codeindex_graph_build_without_index_is_refused() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/codeindex graph build").await;

    assert_eq!(app.status, "codeindex: not active");
}

#[tokio::test]
async fn codeindex_reindex_spawns_and_polls() {
    let (_g, _e, temp, mut app) = isolated_app();
    attach_index(&mut app, temp.path());

    app.execute_slash_command("/codeindex reindex").await;
    assert_eq!(app.status, "[wait] codeindex: reindexing...");
    assert!(app.code_index_reindex_spawned);

    wait_until(|| app.poll_codeindex_bg_result()).await;
    assert!(!app.code_index_reindex_spawned);
}

#[tokio::test]
async fn codeindex_reindex_refuses_a_second_run() {
    let (_g, _e, temp, mut app) = isolated_app();
    attach_index(&mut app, temp.path());
    app.code_index_reindex_spawned = true;

    app.execute_slash_command("/codeindex reindex").await;

    assert_eq!(app.status, "codeindex: reindex failed");
    assert!(last_message(&app).contains("already running"));
}

// ===========================================================================
// session_ops.rs - plugin store / connector catalogue install paths
// ===========================================================================

#[tokio::test]
async fn plugin_store_install_runs_off_loop_and_reports_failure() {
    let (_g, _e, _t, mut app) = isolated_app();
    let mut browser = PluginStoreBrowser::new(StoreKind::Codex, "", false);
    browser.set_entries(vec![store_entry("ghost", "no-such-plugin-source")]);
    app.plugin_store = Some(browser);

    app.plugin_store_install_selected();
    assert_eq!(
        app.plugin_store
            .as_ref()
            .and_then(|b| b.last_install.clone()),
        Some("installing ghost...".to_string()),
        "the panel records the in-flight install before the worker starts"
    );

    wait_until(|| app.poll_plugin_store_install_result()).await;

    let notice = app
        .plugin_store
        .as_ref()
        .and_then(|b| b.last_install.clone())
        .unwrap_or_default();
    assert!(notice.starts_with("install failed:"), "notice: {notice:?}");
    assert!(last_message(&app).contains("[err]"));
}

#[tokio::test]
async fn connector_catalogue_install_selected_records_in_flight_notice() {
    let (_g, _e, _t, mut app) = isolated_app();
    let mut browser = CatalogueBrowser::new("", false);
    browser.set_entries(
        vec![ragent_connectors::ConnectorDescriptor {
            id: ragent_connectors::ConnectorId::new("ghost").expect("valid id"),
            name: "ghost".to_string(),
            description: "ghost connector".to_string(),
            category: "developer".to_string(),
            tags: Vec::new(),
            source: "https://example.org/ghost.json".to_string(),
            marketplace: String::new(),
            provenance: Default::default(),
            auth: ragent_connectors::ConnectorAuthShape::None,
            auth_scope: Vec::new(),
            credential: None,
            servers: vec![ragent_connectors::ConnectorServer {
                id: "main".to_string(),
                transport: "stdio".to_string(),
                command: Some("echo".to_string()),
                args: Vec::new(),
                env: Default::default(),
                url: None,
                headers: Default::default(),
            }],
            unsupported: Vec::new(),
        }],
        0,
    );
    app.connector_store = Some(browser);

    app.connector_catalogue_install_selected();

    // The install itself contacts the live catalogue; only the state transition
    // the panel records before spawning the worker is asserted here.
    assert_eq!(
        app.connector_store
            .as_ref()
            .and_then(|b| b.last_install.clone()),
        Some("installing ghost...".to_string())
    );
}

#[tokio::test]
async fn connector_catalogue_install_refuses_already_installed() {
    let (_g, _e, _t, mut app) = isolated_app();
    let mut browser = CatalogueBrowser::new("", false);
    browser.set_installed(std::iter::once("ghost".to_string()).collect());
    browser.set_entries(Vec::new(), 0);
    app.connector_store = Some(browser);

    app.connector_catalogue_install_selected();

    assert_eq!(
        app.connector_store
            .as_ref()
            .and_then(|b| b.last_install.clone()),
        Some("no result highlighted".to_string())
    );
}

#[tokio::test]
async fn refresh_connector_catalogue_installed_without_panel_is_noop() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.refresh_connector_catalogue_installed();

    assert!(app.connector_store.is_none());
}

// ===========================================================================
// session_ops.rs - editor / cursor / clipboard helpers
// ===========================================================================

#[tokio::test]
async fn cursor_moves_by_word_and_logical_line() {
    let mut app = support::make_app();
    app.input = "alpha beta\ngamma delta".to_string();
    app.input_cursor = 0;

    app.handle_key_event(key_mod(KeyCode::Right, KeyModifiers::CONTROL))
        .await;
    assert_eq!(app.input_cursor, 6, "Ctrl+Right lands after the first word");

    app.handle_key_event(key_mod(KeyCode::Right, KeyModifiers::CONTROL))
        .await;
    assert_eq!(app.input_cursor, 11, "the second hop crosses the newline");

    app.handle_key_event(key(KeyCode::Up)).await;
    assert_eq!(
        app.input_cursor, 0,
        "Up keeps column zero on the first line"
    );

    app.handle_key_event(key(KeyCode::Down)).await;
    assert_eq!(
        app.input_cursor, 11,
        "Down returns to line two, column zero"
    );
}

#[tokio::test]
async fn cursor_up_and_down_clamp_to_shorter_lines() {
    let mut app = support::make_app();
    app.input = "short\na much longer line".to_string();
    app.input_cursor = app.input_len_chars();

    // Up from the end of the long line lands at the end of the short one.
    app.handle_key_event(key(KeyCode::Up)).await;
    assert_eq!(app.input_cursor, 5);

    // Down from the end of the short line keeps the same column.
    app.handle_key_event(key(KeyCode::Down)).await;
    assert_eq!(app.input_cursor, 11);
}

#[tokio::test]
async fn cut_kb_selection_removes_the_selected_span() {
    let mut app = support::make_app();
    app.input = "hello world".to_string();
    app.input_cursor = 0;

    // Shift+Right selects one char at a time; select "hello".
    for _ in 0..5 {
        app.handle_key_event(key_mod(KeyCode::Right, KeyModifiers::SHIFT))
            .await;
    }
    app.handle_key_event(key_mod(KeyCode::Char('x'), KeyModifiers::CONTROL))
        .await;

    assert_eq!(app.input, " world");
    assert_eq!(app.kb_select_anchor, None);
}

#[tokio::test]
async fn paste_image_from_clipboard_without_image_logs_notice() {
    let _guard = ragent_tui::clipboard::ClipboardTestOverrideGuard::new("just plain text");
    let mut app = support::make_app();

    app.handle_key_event(key_mod(KeyCode::Char('v'), KeyModifiers::ALT))
        .await;

    assert_eq!(app.pending_attachments.len(), 0);
    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("No image data found in clipboard")),
        "the notice is logged: {:?}",
        app.log_entries
            .iter()
            .map(|e| e.message.clone())
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn paste_outside_scope_image_attaches_with_a_warning() {
    let (_g, _e, _temp, mut app) = isolated_app();
    // A path under the tempdir project is inside cwd, so use a second tempdir
    // (outside both the cwd and $HOME) to trigger the scope warning.
    let outside = tempfile::tempdir().expect("outside dir");
    let png = outside.path().join("shot.png");
    std::fs::write(&png, b"png").expect("write image");
    let _guard =
        ragent_tui::clipboard::ClipboardTestOverrideGuard::new(png.to_string_lossy().to_string());

    app.handle_key_event(key_mod(KeyCode::Char('v'), KeyModifiers::ALT))
        .await;

    assert_eq!(app.pending_attachments, vec![png.clone()]);
    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("outside the working directory and home")),
        "the scope warning is logged: {:?}",
        app.log_entries
            .iter()
            .map(|e| e.message.clone())
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn scrollbar_drag_maps_the_gutter_row_to_an_offset() {
    let mut app = support::make_app();
    app.message_area = Rect::new(0, 0, 20, 11);
    app.message_max_scroll = 100;

    app.handle_mouse_event(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 19,
        row: 0,
        modifiers: KeyModifiers::NONE,
    });

    assert_eq!(app.scrollbar_drag, Some(ScrollbarDragPane::Messages));
    assert_eq!(
        app.scroll_offset, 100,
        "the top of the track is the oldest line"
    );
}

#[tokio::test]
async fn scrollbar_drag_on_a_memory_pane_uses_top_down_offsets() {
    let mut app = support::make_app();
    app.show_memory = true;
    app.memory_area = Rect::new(0, 0, 20, 11);
    app.memory_max_scroll = 40;

    app.handle_mouse_event(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 19,
        row: 0,
        modifiers: KeyModifiers::NONE,
    });

    assert_eq!(app.scrollbar_drag, Some(ScrollbarDragPane::Memory));
    assert_eq!(app.memory_scroll_offset, 0);
}

#[tokio::test]
async fn spool_log_window_history_writes_the_visible_log() {
    let (_g, _e, temp, mut app) = isolated_app();
    let spool = temp.path().join("logwindow.log");
    app.log_window_path = Some(spool.clone());
    app.log_entries.push(log_entry(1));

    // Alt+L toggles the log panel on, which spools the full history.
    app.handle_key_event(key_mod(KeyCode::Char('l'), KeyModifiers::ALT))
        .await;

    assert!(app.show_log);
    let body = std::fs::read_to_string(&spool).expect("spool written");
    assert!(body.contains("entry 1"), "body: {body}");
}

#[tokio::test]
async fn log_entries_are_trimmed_to_the_cap() {
    let (_g, _e, _t, mut app) = isolated_app();
    for seq in 0..1_200 {
        app.log_entries.push(log_entry(seq));
        app.log_line_cache.push(log_group());
    }

    // Any command pushes a log entry, which runs the FIFO trim.
    app.execute_slash_command("/cost").await;

    assert_eq!(app.log_entries.len(), 1_000);
    assert_eq!(app.log_line_cache.len(), 1_000);
}

#[test]
fn accept_file_menu_selection_handles_every_shape() {
    let mut app = support::make_app();

    // No menu at all.
    assert!(!app.accept_file_menu_selection());

    // An empty match list is refused.
    app.file_menu = Some(FileMenuState {
        matches: Vec::new(),
        selected: 0,
        scroll_offset: 0,
        query: String::new(),
        current_dir: None,
    });
    assert!(!app.accept_file_menu_selection());

    // The `<back to fuzzy>` row re-derives the fuzzy list and returns false.
    app.file_menu = Some(FileMenuState {
        matches: vec![FileMenuEntry {
            display: "<back to fuzzy>".to_string(),
            path: std::path::PathBuf::new(),
            is_dir: true,
        }],
        selected: 0,
        scroll_offset: 0,
        query: String::new(),
        current_dir: Some(std::path::PathBuf::from(".")),
    });
    assert!(!app.accept_file_menu_selection());

    // A file row with no active mention span just closes the menu.
    app.file_menu = Some(FileMenuState {
        matches: vec![FileMenuEntry {
            display: "notes.txt".to_string(),
            path: std::path::PathBuf::from("notes.txt"),
            is_dir: false,
        }],
        selected: 0,
        scroll_offset: 0,
        query: String::new(),
        current_dir: None,
    });
    app.input.clear();
    assert!(!app.accept_file_menu_selection());
    assert!(app.file_menu.is_none());
}

#[test]
fn accept_file_menu_selection_inserts_the_mention() {
    let mut app = support::make_app();
    app.input = "@not".to_string();
    app.input_cursor = app.input_len_chars();
    app.file_menu = Some(FileMenuState {
        matches: vec![FileMenuEntry {
            display: "notes.txt".to_string(),
            path: std::path::PathBuf::from("notes.txt"),
            is_dir: false,
        }],
        selected: 0,
        scroll_offset: 0,
        query: "not".to_string(),
        current_dir: None,
    });

    assert!(app.accept_file_menu_selection());
    assert_eq!(app.input, "@notes.txt");
    assert_eq!(app.input_cursor, app.input_len_chars());
    assert!(app.file_menu.is_none());
}

#[test]
fn accept_file_menu_selection_navigates_into_a_directory() {
    let (_g, _e, temp, mut app) = isolated_app();
    std::fs::create_dir_all(temp.path().join("src")).expect("src dir");
    std::fs::write(temp.path().join("src").join("main.rs"), "fn main() {}").expect("file");
    app.file_menu = Some(FileMenuState {
        matches: vec![FileMenuEntry {
            display: "src/".to_string(),
            path: std::path::PathBuf::from("src"),
            is_dir: true,
        }],
        selected: 0,
        scroll_offset: 0,
        query: String::new(),
        current_dir: None,
    });

    assert!(!app.accept_file_menu_selection());

    let menu = app.file_menu.as_ref().expect("directory menu");
    assert_eq!(
        menu.current_dir.as_deref(),
        Some(std::path::Path::new("src"))
    );
    assert_eq!(menu.matches[0].display, "<back to fuzzy>");
    assert!(
        menu.matches.iter().any(|m| m.display == "src/main.rs"),
        "the directory contents are listed: {:?}",
        menu.matches
            .iter()
            .map(|m| m.display.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn usage_display_covers_quota_context_and_unknown() {
    let mut app = support::make_app();

    // No provider and no context window.
    app.configured_provider = None;
    assert_eq!(app.usage_display(), ("unknown".to_string(), true));

    // A provider quota percentage wins over the context display.
    app.configured_provider = Some(ConfiguredProvider {
        id: "openai".to_string(),
        name: "OpenAI".to_string(),
        source: ProviderSource::Database,
    });
    app.quota_percent = Some(41.5);
    assert_eq!(app.usage_display(), ("quota: 41.5%".to_string(), false));

    // Without a quota the context-window usage is shown.
    app.quota_percent = None;
    app.selected_model = Some("gemini/gemini-2.5-pro-preview-05-06".to_string());
    app.last_input_tokens = 1_000;
    let (label, is_rate) = app.usage_display();
    assert!(label.starts_with("ctx: "), "label: {label}");
    assert!(!is_rate);

    // An ollama-family provider with no context window reports "local".
    app.configured_provider = Some(ConfiguredProvider {
        id: "ollama".to_string(),
        name: "Ollama".to_string(),
        source: ProviderSource::Database,
    });
    app.selected_model = None;
    assert_eq!(app.usage_display(), ("local".to_string(), false));

    // A non-ollama provider with no context window reports unknown + rate.
    app.configured_provider = Some(ConfiguredProvider {
        id: "openai".to_string(),
        name: "OpenAI".to_string(),
        source: ProviderSource::Database,
    });
    assert_eq!(app.usage_display(), ("unknown".to_string(), true));
}

#[test]
fn team_panel_render_syncs_member_session_ids() {
    let (_g, _e, temp, mut app) = isolated_app();
    let store = TeamStore::create("alpha", "slash-support", temp.path(), true).expect("team store");
    let mut member = TeamMember::new("writer", "tm-001", "general");
    member.session_id = Some("child-session".to_string());
    member.status = MemberStatus::Working;
    member.current_task_id = Some("task-1".to_string());
    let mut config = store.config.clone();
    config.members.push(member);
    let updated = TeamStore {
        dir: store.dir.clone(),
        config,
    };
    updated.save().expect("save team");
    app.active_team = Some(updated.config.clone());
    app.team_members
        .push(TeamMember::new("writer", "tm-001", "general"));
    app.show_teams_window = true;

    render(&mut app, 120, 40);

    let member = app.team_members.first().expect("member");
    assert_eq!(member.session_id.as_deref(), Some("child-session"));
    assert_eq!(member.status, MemberStatus::Working);
    assert_eq!(member.current_task_id.as_deref(), Some("task-1"));
    assert!(
        !app.sid_to_display_name.is_empty(),
        "the session id is registered for log display"
    );
}

#[test]
fn team_panel_render_tolerates_a_missing_team_store() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.active_team = Some(TeamConfig::new("missing-team", "slash-support"));
    app.show_teams_window = true;

    render(&mut app, 120, 40);

    assert!(app.sid_to_display_name.is_empty());
}

#[tokio::test]
async fn plan_approval_restores_the_previous_agent() {
    let (_g, _e, _t, mut app) = isolated_app();
    let previous = app.agent_info.clone();
    app.agent_stack.push(previous);
    app.plan_approval_pending = Some(PlanApprovalState {
        plan_text: "1. do the thing".to_string(),
        cursor_approve: true,
    });

    app.handle_key_event(key(KeyCode::Enter)).await;

    assert!(app.plan_approval_pending.is_none());
    let all = slash_support::all_messages(&app);
    assert!(all.contains("Plan summary"), "messages: {all}");
    assert!(all.contains("1. do the thing"), "messages: {all}");
    assert!(app.force_new_message);
}

#[tokio::test]
async fn plan_approval_with_empty_stack_logs_an_error() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.plan_approval_pending = Some(PlanApprovalState {
        plan_text: "1. orphan plan".to_string(),
        cursor_approve: true,
    });

    app.handle_key_event(key(KeyCode::Enter)).await;

    // Only the prose half ("agent stack is empty") is asserted here. The tool
    // name half of the same log line was migrated to `ExitPlanMode` by the
    // FR-023 rename, so this case is unaffected by that spelling change.
    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("agent stack is empty")),
        "the empty stack is logged"
    );
}

// ===========================================================================
// models.rs - reports and model resolution
// ===========================================================================

#[tokio::test]
async fn provider_config_report_renders_config_and_models() {
    let config = r#"
    {
      "defaultAgent": "general",
      "provider": {
        "gemini": {
          "env": ["GEMINI_API_KEY", "GOOGLE_API_KEY"],
          "api": { "base_url": "https://example.invalid", "headers": { "X-Test": "1" } },
          "models": { "gemini-2.5-pro-preview-05-06": { "name": "Custom Pro" } },
          "options": { "region": "eu" }
        }
      }
    }
    "#;
    let (_g, _e, temp, mut app) = isolated(r#"{ "defaultAgent": "general" }"#);
    // A project config has its privileged provider `api` block stripped, so the
    // endpoint/headers must come from the user-global file the loader reads.
    let global = temp.path().join(".config").join("ragent");
    std::fs::create_dir_all(&global).expect("global config dir");
    std::fs::write(global.join("ragent.json"), config).expect("write global config");
    ragent_config::Config::invalidate_load_cache();
    app.storage
        .set_provider_auth("gemini", "test-key")
        .expect("store auth");

    app.execute_slash_command("/provider show").await;
    assert!(matches!(
        app.provider_setup,
        Some(ProviderSetupStep::ShowProviderConfig { .. })
    ));

    let enter = app.handle_key_event(key(KeyCode::Enter));
    futures::executor::block_on(enter);

    let text = last_message(&app);
    assert!(text.contains("From: /provider show"), "text: {text}");
    assert!(
        text.contains("Database (stored credential)"),
        "text: {text}"
    );
    assert!(text.contains("GEMINI_API_KEY"), "text: {text}");
    assert!(text.contains("API base URL"), "text: {text}");
    assert!(text.contains("X-Test"), "text: {text}");
    assert!(text.contains("Custom Pro"), "text: {text}");
    assert!(text.contains("region"), "text: {text}");
    assert!(text.contains("Available Models"), "text: {text}");
}

#[tokio::test]
async fn provider_config_report_without_config_section_notes_it() {
    let (_g, _e, _t, mut app) = isolated_app();
    let _ = ProviderSource::EnvVar;
    app.storage
        .set_provider_auth("openai", "test-key")
        .expect("store auth");
    // `/provider show` -> Enter renders the report for the first provider.
    app.execute_slash_command("/provider show").await;
    app.handle_key_event(key(KeyCode::Enter)).await;

    let text = last_message(&app);
    assert!(
        text.contains("Database (stored credential)"),
        "text: {text}"
    );
    assert!(
        text.contains("No custom configuration in ragent.json"),
        "text: {text}"
    );
    assert!(
        text.contains("No models available for this provider"),
        "text: {text}"
    );
}

#[tokio::test]
async fn provider_show_lists_the_router_when_a_cluster_is_saved() {
    let config = r#"
    {
      "defaultAgent": "general",
      "provider": {
        "gemini": { "env": ["GEMINI_API_KEY"] },
        "router": {
          "enabled": true,
          "tiers": {
            "SIMPLE": { "models": [ { "provider": "gemini", "model": "gemini-2.5-flash-preview-05-20" } ] }
          }
        }
      }
    }
    "#;
    let (_g, _e, temp, mut app) = isolated(r#"{ "defaultAgent": "general" }"#);
    // `load_raw_router_config` walks the App's `config_paths` (the project
    // config), reading the raw JSON directly, so the router block must live in
    // the project file rather than the user-global one.
    std::fs::create_dir_all(temp.path().join(".ragent")).expect("create .ragent");
    std::fs::write(temp.path().join(".ragent/ragent.json"), config).expect("write project config");
    ragent_config::Config::invalidate_load_cache();
    app.storage
        .set_provider_auth("gemini", "test-key")
        .expect("store auth");

    app.execute_slash_command("/provider show").await;

    let providers = match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::ShowProviderConfig { providers, .. } => providers.clone(),
        other => panic!("expected ShowProviderConfig, got {other:?}"),
    };
    assert!(
        providers.iter().any(|p| p.id == "router"),
        "the saved cluster is listed: {:?}",
        providers.iter().map(|p| p.id.clone()).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn provider_show_skips_the_router_for_invalid_json() {
    let (_g, _e, temp, mut app) = isolated_app();
    app.storage
        .set_provider_auth("gemini", "test-key")
        .expect("store auth");
    let path = temp.path().join("broken.json");
    std::fs::write(&path, "{ not json").expect("write broken config");
    app.config_paths = vec![path];

    app.execute_slash_command("/provider show").await;

    let providers = match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::ShowProviderConfig { providers, .. } => providers.clone(),
        other => panic!("expected ShowProviderConfig, got {other:?}"),
    };
    assert!(providers.iter().all(|p| p.id != "router"));
}

#[tokio::test]
async fn cost_summary_groups_by_provider_and_uses_known_costs() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.llm_request_stats = vec![
        ragent_tui::app::LlmRequestStat {
            model_ref: "gemini/gemini-2.5-flash-preview-05-20".to_string(),
            elapsed_ms: 1_000,
            input_tokens: 1_000,
            output_tokens: 500,
        },
        ragent_tui::app::LlmRequestStat {
            model_ref: "unknown-model".to_string(),
            elapsed_ms: 500,
            input_tokens: 100,
            output_tokens: 100,
        },
    ];

    app.execute_slash_command("/cost").await;

    assert_eq!(app.status, "cost summary");
    let text = last_message(&app);
    assert!(text.contains("Samples: 2"), "text: {text}");
    assert!(text.contains("By provider:"), "text: {text}");
    assert!(text.contains("gemini:"), "text: {text}");
}

#[tokio::test]
async fn cost_summary_without_a_session_reports_unknown_duration() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.llm_request_stats = vec![ragent_tui::app::LlmRequestStat {
        model_ref: "gemini/gemini-2.5-pro-preview-05-06".to_string(),
        elapsed_ms: 10,
        input_tokens: 10,
        output_tokens: 10,
    }];

    // `/cost` ensures a session first, so point the app at a session id the
    // store does not hold: the duration lookup then reports "unknown".
    app.execute_slash_command("/cost").await;
    // `/cost` re-arms only the status timer; pointing the app at a session the
    // store does not hold makes the duration lookup report "unknown".
    app.session_id = Some("no-such-session".to_string());
    app.execute_slash_command("/cost").await;
    let text = last_message(&app);
    assert!(text.contains("Session duration: unknown"), "text: {text}");
}

#[tokio::test]
async fn model_show_renders_resolved_metadata_and_thinking() {
    let (_g, _e, _t, mut app) = isolated_app();
    configure_provider(&mut app, "gemini", "gemini/gemini-2.5-pro-preview-05-06");
    app.selected_model_ctx_window = Some(1_048_576);

    app.execute_slash_command("/model show").await;

    assert_eq!(app.status, "active model metadata");
    let text = last_message(&app);
    assert!(text.contains("## Capabilities"), "text: {text}");
    assert!(text.contains("Context window"), "text: {text}");
    assert!(text.contains("## Cost"), "text: {text}");
    assert!(text.contains("## Thinking"), "text: {text}");
}

#[tokio::test]
async fn model_show_falls_back_when_metadata_is_unresolvable() {
    let (_g, _e, _t, mut app) = isolated_app();
    configure_provider(&mut app, "gemini", "gemini/not-a-real-model");
    app.selected_model_ctx_window = Some(8_192);

    app.execute_slash_command("/model show").await;

    let text = last_message(&app);
    assert!(
        text.contains("Metadata could not be resolved"),
        "text: {text}"
    );
    assert!(text.contains("Cached context window"), "text: {text}");
    assert!(text.contains("8192"), "text: {text}");
}

#[tokio::test]
async fn tools_list_renders_every_tool_and_its_source() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/tools list").await;

    let text = last_message(&app);
    assert!(text.contains("Tools ("), "text: {text}");
    assert!(
        !text.contains("Tool Family Visibility"),
        "the visibility table is gone: {text}"
    );
    assert!(text.contains("internal"), "text: {text}");
    assert!(
        text.contains("codeindex_search"),
        "a family tool is listed like any other: {text}"
    );
}

#[test]
fn backfill_model_ctx_window_fills_from_the_registry() {
    let mut app = support::make_app();
    app.selected_model = Some("gemini/gemini-2.5-pro-preview-05-06".to_string());
    app.selected_model_ctx_window = None;

    app.backfill_model_ctx_window();

    assert_eq!(app.selected_model_ctx_window, Some(1_048_576));
}

#[test]
fn backfill_model_ctx_window_ignores_unparsable_and_unknown_models() {
    let mut app = support::make_app();

    // No model at all.
    app.selected_model = None;
    app.backfill_model_ctx_window();
    assert_eq!(app.selected_model_ctx_window, None);

    // No provider separator.
    app.selected_model = Some("bare-model".to_string());
    app.backfill_model_ctx_window();
    assert_eq!(app.selected_model_ctx_window, None);

    // A model the registry does not advertise.
    app.selected_model = Some("gemini/not-a-model".to_string());
    app.backfill_model_ctx_window();
    assert_eq!(app.selected_model_ctx_window, None);
}

#[test]
fn try_restore_provider_model_restores_and_prunes() {
    let mut app = support::make_app();
    app.storage
        .set_setting("provider_gemini_last_model", "gemini-2.5-pro-preview-05-06")
        .expect("seed setting");

    let entry = app
        .try_restore_provider_model("gemini", "Google Gemini")
        .expect("model restored");
    assert_eq!(entry.id, "gemini-2.5-pro-preview-05-06");
    assert_eq!(
        app.selected_model.as_deref(),
        Some("gemini/gemini-2.5-pro-preview-05-06")
    );
    assert_eq!(
        app.configured_provider.as_ref().map(|p| p.id.as_str()),
        Some("gemini")
    );

    // A stale persisted model is pruned and reported.
    app.storage
        .set_setting("provider_gemini_last_model", "retired-model")
        .expect("seed stale setting");
    assert!(
        app.try_restore_provider_model("gemini", "Google Gemini")
            .is_none()
    );
    assert!(
        app.status.contains("no longer available"),
        "status: {}",
        app.status
    );
    assert_eq!(
        app.storage
            .get_setting("provider_gemini_last_model")
            .expect("read setting"),
        None
    );

    // No persisted model at all.
    assert!(
        app.try_restore_provider_model("gemini", "Google Gemini")
            .is_none()
    );
}

#[tokio::test]
async fn model_discovery_finished_event_caches_models() {
    let (_g, _e, _t, mut app) = isolated_app();
    let discovery = ragent_llm::providers::gemini::gemini_default_models("gemini");
    let models: Vec<serde_json::Value> = discovery
        .iter()
        .map(|m| serde_json::to_value(m).expect("serialize model"))
        .collect();

    app.handle_event(Event::ProviderLoadingFinished {
        provider_id: "gemini".to_string(),
        provider_name: "Google Gemini".to_string(),
        models,
        error: None,
    })
    .await;

    assert!(app.model_loading_state.is_none());
    let cached = app
        .storage
        .get_discovered_models("gemini")
        .expect("read cache")
        .unwrap_or_default();
    assert!(
        cached.contains("gemini-2.5-pro-preview-05-06"),
        "cached: {cached}"
    );
}

#[test]
fn paste_text_into_provider_setup_covers_every_field_shape() {
    let mut app = support::make_app();

    // No dialog open: nothing happens.
    app.paste_text_into_provider_setup("ignored");
    assert!(app.provider_setup.is_none());

    // EnterKey: active field 0 is the key, field 1 the endpoint.
    app.provider_setup = Some(ProviderSetupStep::EnterKey {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
        key_field: InputField::new(),
        endpoint_field: InputField::new(),
        active_field: 0,
        error: None,
    });
    app.paste_text_into_provider_setup("sk-key\r\n");
    match app.provider_setup.as_ref().expect("dialog") {
        ProviderSetupStep::EnterKey { key_field, .. } => assert_eq!(key_field.text(), "sk-key\n"),
        other => panic!("expected EnterKey, got {other:?}"),
    }
    app.paste_text_into_provider_setup("");

    // GitLabSetup: URL field then token field.
    app.provider_setup = Some(ProviderSetupStep::GitLabSetup {
        url_input: String::new(),
        url_cursor: 0,
        token_input: String::new(),
        token_cursor: 0,
        active_field: 0,
        error: None,
    });
    app.paste_text_into_provider_setup("https://gitlab.example");
    app.paste_text_into_provider_setup("glpat-token");
    match app.provider_setup.as_ref().expect("dialog") {
        ProviderSetupStep::GitLabSetup {
            url_input,
            url_cursor,
            token_input,
            token_cursor,
            ..
        } => {
            assert_eq!(url_input, "https://gitlab.exampleglpat-token");
            assert_eq!(*url_cursor, url_input.chars().count());
            assert_eq!(token_input, "");
            assert_eq!(*token_cursor, 0);
        }
        other => panic!("expected GitLabSetup, got {other:?}"),
    }
    if let Some(ProviderSetupStep::GitLabSetup { active_field, .. }) = app.provider_setup.as_mut() {
        *active_field = 1;
    }
    app.paste_text_into_provider_setup("glpat-token");
    match app.provider_setup.as_ref().expect("dialog") {
        ProviderSetupStep::GitLabSetup { token_input, .. } => {
            assert_eq!(token_input, "glpat-token")
        }
        other => panic!("expected GitLabSetup, got {other:?}"),
    }

    // TelemetrySetup: endpoint (0), interval (2), timeout (3), port (4), and
    // the unmapped field 1 which is ignored.
    app.provider_setup = Some(ProviderSetupStep::TelemetrySetup {
        endpoint_field: InputField::new(),
        protocol: ragent_config::OtelProtocol::Http,
        interval_field: InputField::new(),
        timeout_field: InputField::new(),
        port_field: InputField::new(),
        active_field: 0,
        error: None,
    });
    app.paste_text_into_provider_setup("http://collector");
    for (field, text) in [(1u8, "ignored"), (2, "30"), (3, "5"), (4, "9090")] {
        if let Some(ProviderSetupStep::TelemetrySetup { active_field, .. }) =
            app.provider_setup.as_mut()
        {
            *active_field = field;
        }
        app.paste_text_into_provider_setup(text);
    }
    match app.provider_setup.as_ref().expect("dialog") {
        ProviderSetupStep::TelemetrySetup {
            endpoint_field,
            interval_field,
            timeout_field,
            port_field,
            ..
        } => {
            assert_eq!(endpoint_field.text(), "http://collector");
            assert_eq!(interval_field.text(), "30");
            assert_eq!(timeout_field.text(), "5");
            assert_eq!(port_field.text(), "9090");
        }
        other => panic!("expected TelemetrySetup, got {other:?}"),
    }
}

#[tokio::test]
async fn provider_health_check_marks_unknown_then_available() {
    let (_g, _e, _t, mut app) = isolated_app();

    // No configured provider: the latch stays "unknown" (0).
    app.configured_provider = None;
    close_done_step(&mut app).await;
    assert_eq!(
        app.provider_health
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );

    // An API-key provider is trusted present, so the check reports "up" (1).
    configure_provider(&mut app, "openai", "openai/gpt-4o");
    close_done_step(&mut app).await;
    for _ in 0..50 {
        if app
            .provider_health
            .load(std::sync::atomic::Ordering::Relaxed)
            != 0
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        app.provider_health
            .load(std::sync::atomic::Ordering::Relaxed),
        1
    );
}

/// Render the router setup panel so `router_model_picker_entry` and
/// `estimate_entry_cost`, whose branches depend on registry metadata, run.
#[test]
fn router_setup_panel_renders_model_metadata_and_cost() {
    let mut app = support::make_app();
    let mut draft = ragent_llm::providers::router_config::RouterConfig::default();
    draft.tiers.insert(
        "SIMPLE".to_string(),
        ragent_llm::providers::router_config::TierConfig {
            models: vec![ragent_llm::providers::router_config::TierEntry {
                provider: "gemini".to_string(),
                model: "gemini-2.5-pro-preview-05-06".to_string(),
            }],
            timeout_ms: None,
        },
    );
    app.provider_setup = Some(ProviderSetupStep::SetupRouter {
        providers: Vec::new(),
        selected_provider_ids: Vec::new(),
        selected_provider_index: 0,
        draft_config: draft,
        active_bucket: ragent_llm::providers::router_config::Tier::Simple,
        active_bucket_index: 0,
        left_pane_focused: true,
        error: None,
    });

    render(&mut app, 120, 40);
}

// ===========================================================================
// state.rs - MCP enable, connector status, polls and housekeeping
// ===========================================================================

#[tokio::test]
async fn set_mcp_server_enabled_rejects_an_unknown_server() {
    let (_g, _e, _t, mut app) = isolated_app();

    let report = app.set_mcp_server_enabled("nope", true).await;

    assert!(report.contains("[err]"), "report: {report}");
    assert!(
        report.contains("not a known MCP server"),
        "report: {report}"
    );
}

#[tokio::test]
async fn set_mcp_server_enabled_without_live_client_persists_the_choice() {
    let (_g, _e, temp, mut app) = isolated(
        r#"{ "defaultAgent": "general", "mcp": { "echo": { "type": "stdio", "command": "echo" } } }"#,
    );
    // The project config strips the privileged `mcp` block, so seed the global
    // config file the loader actually reads.
    let global = temp.path().join(".config").join("ragent");
    std::fs::create_dir_all(&global).expect("global config dir");
    std::fs::write(
        global.join("ragent.json"),
        r#"{ "mcp": { "echo": { "type": "stdio", "command": "echo" } } }"#,
    )
    .expect("write global config");
    ragent_config::Config::invalidate_load_cache();

    let report = app.set_mcp_server_enabled("echo", false).await;

    assert!(report.contains("[ok]"), "report: {report}");
    assert!(report.contains("is now disabled"), "report: {report}");
    let ledger = ragent_agent::mcp::McpEnableLedger::load();
    assert!(!ledger.is_enabled("echo"));
    assert_eq!(app.mcp_enabled_map.get("echo").copied(), Some(false));
}

#[tokio::test]
async fn connector_status_refresh_without_a_live_session_is_a_noop() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/connectors disable ghost").await;

    assert!(
        app.session_processor
            .connector_statuses::<Vec<ragent_connectors::ConnectorStatus>>()
            .await
            .is_none(),
        "no live session means nothing is published"
    );
}

#[tokio::test]
async fn connector_status_refresh_publishes_from_the_live_session() {
    let (_g, _e, _t, mut app) = isolated_app();
    let (dirs, config) = store_and_config(&app.cwd_path);
    app.session_processor
        .set_connector_session(Arc::new(tokio::sync::Mutex::new(ConnectorSession::new(
            dirs,
            config,
            Vec::new(),
        ))))
        .await;

    app.execute_slash_command("/connectors disable ghost").await;

    let published = app
        .session_processor
        .connector_statuses::<Vec<ragent_connectors::ConnectorStatus>>()
        .await
        .expect("statuses published");
    assert_eq!(published.len(), 0, "an empty store publishes an empty set");
}

#[test]
fn needs_periodic_redraw_tracks_every_live_elapsed_source() {
    use ragent_agent::task::{ReportStatus, TaskEntry, TaskStatus};

    let mut app = support::make_app();
    assert!(!app.needs_periodic_redraw());

    // A running agent task in a visible Agents panel.
    app.show_agents_window = true;
    app.active_tasks.push(TaskEntry {
        id: "t1".to_string(),
        parent_session_id: "s1".to_string(),
        child_session_id: "c1".to_string(),
        agent_name: "explore".to_string(),
        task_prompt: "x".to_string(),
        background: true,
        detached: false,
        status: TaskStatus::Running,
        result: None,
        error: None,
        created_at: chrono::Utc::now(),
        completed_at: None,
        reported: false,
        waiter_count: 0,
        output_file: None,
        report_status: ReportStatus::default(),
    });
    assert!(app.needs_periodic_redraw());
    app.active_tasks.clear();

    // A finished task does not keep the clock running.
    assert!(!app.needs_periodic_redraw());

    // A running background shell task does.
    app.bg_tasks.push(ragent_tui::app::BgTaskView {
        id: "b1".to_string(),
        session_id: "s1".to_string(),
        command: "sleep 1".to_string(),
        status: "running".to_string(),
        created_at: chrono::Utc::now(),
        completed_at: None,
    });
    assert!(app.needs_periodic_redraw());
    app.bg_tasks.clear();
    app.show_agents_window = false;

    // A working teammate in a visible Teams panel does.
    app.show_teams_window = true;
    app.active_team = Some(TeamConfig::new("alpha", "s1"));
    let mut member = TeamMember::new("writer", "tm-001", "general");
    member.status = MemberStatus::Working;
    app.team_members.push(member);
    assert!(app.needs_periodic_redraw());

    // A stopped teammate does not.
    app.team_members[0].status = MemberStatus::Stopped;
    assert!(!app.needs_periodic_redraw());

    // No active team means the panel is not live even when shown.
    app.active_team = None;
    assert!(!app.needs_periodic_redraw());
}

#[test]
fn housekeeping_due_at_tracks_the_poll_interval() {
    let mut app = support::make_app();
    app.jobs_last_poll = std::time::Instant::now();

    let due = app.housekeeping_due_at();

    assert_eq!(due, app.jobs_last_poll + HOUSEKEEPING_INTERVAL);
    assert!(due > app.jobs_last_poll);
}

#[test]
fn poll_active_tasks_reconcile_uses_the_lag_counter() {
    let mut app = support::make_app();
    app.seen_event_lag = 0;
    app.tui_event_lag
        .store(7, std::sync::atomic::Ordering::Relaxed);

    app.poll_active_tasks_reconcile();

    assert_eq!(app.seen_event_lag, 7, "the lag counter is adopted");
    assert!(!app.active_tasks_reconcile_inflight, "no manager to query");

    // A landed snapshot clears the in-flight latch.
    {
        let mut guard = app
            .active_tasks_reconcile_result
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = Some(Vec::new());
    }
    app.active_tasks_reconcile_inflight = true;
    app.poll_active_tasks_reconcile();
    assert!(!app.active_tasks_reconcile_inflight);
}

#[test]
fn poll_active_tasks_reconcile_skips_while_in_flight() {
    let mut app = support::make_app();
    app.active_tasks_reconcile_inflight = true;
    app.seen_event_lag = 3;
    app.tui_event_lag
        .store(9, std::sync::atomic::Ordering::Relaxed);

    app.poll_active_tasks_reconcile();

    assert_eq!(
        app.seen_event_lag, 3,
        "an in-flight fetch suppresses a new one"
    );
}

#[test]
fn poll_question_expiry_falls_back_to_the_first_option() {
    let mut app = support::make_app();
    app.question_queue.push_back(QuestionRequest {
        id: "q1".to_string(),
        session_id: "s1".to_string(),
        question: "choose".to_string(),
        options: vec!["first".to_string(), "second".to_string()],
        default_index: None,
        expire_at_ms: Some(1),
        secret: false,
    });
    app.pending_question_input = "half typed".to_string();

    app.poll_question_expiry();

    assert_eq!(app.question_queue.len(), 0);
    assert_eq!(app.pending_question_input, "");
    assert_eq!(app.question_selected_index, 0);
    assert!(app.needs_redraw);
    assert!(
        app.log_entries.iter().any(|e| e
            .message
            .contains("question timed out; defaulting to: first")),
        "the fallback answer is logged"
    );
}

#[test]
fn refresh_plugin_store_installed_is_a_noop_without_a_panel() {
    let mut app = support::make_app();

    app.refresh_plugin_store_installed();

    assert!(app.plugin_store.is_none());
}
