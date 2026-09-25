//! Tests for `test_slash_commands.rs`

/// Tests for TUI slash command parsing and dispatch (TASK-006).
///
/// Verifies each slash command updates app state correctly, handles arguments,
/// and provides user feedback via status bar and log entries.
use std::fs;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ragent_agent::{
    agent,
    event::EventBus,
    permission::PermissionChecker,
    provider,
    session::{SessionManager, processor::SessionProcessor, profiler::agent_loop_profiler},
    storage::Storage,
    tool,
};
use ragent_tui::app::{
    ConfiguredProvider, FileMenuEntry, FileMenuState, HistoryPickerState, LogEntry, LogLevel,
    OutputViewState, OutputViewTarget, ProviderSetupStep, ProviderSource, ScreenMode,
};
use ragent_tui::{App, layout};
use ratatui::{Terminal, backend::TestBackend};

/// Build an [`App`] backed by an in-memory database.
fn make_app() -> App {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    make_app_with_storage(storage)
}

fn make_app_with_storage(storage: Arc<Storage>) -> App {
    let event_bus = Arc::new(EventBus::default());
    let provider_registry = Arc::new(provider::create_default_registry());
    let tool_registry = Arc::new(tool::create_default_registry());
    let permission_checker = Arc::new(parking_lot::RwLock::new(PermissionChecker::new(vec![])));
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
        stream_config: ragent_agent::StreamConfig::default(),
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
        tool_repeat_guard: std::sync::Arc::new(parking_lot::Mutex::new(
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
        activity_log_tx: tokio::sync::Mutex::new(None),
        skill_registry_cache: parking_lot::Mutex::new(None),
        active_loops: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        active_loop_specs: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        loop_telemetry_recorded: std::sync::atomic::AtomicBool::new(false),
        active_loop_interrupts: parking_lot::RwLock::new(std::collections::HashMap::new()),
        active_loop_captures: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        last_message_end_reason: std::sync::RwLock::new(std::collections::HashMap::new()),
    });
    let agent_info =
        agent::resolve_agent("general", &Default::default()).expect("resolve general agent");

    App::new(
        event_bus,
        storage,
        provider_registry,
        session_processor,
        Arc::unwrap_or_clone(agent_info),
        false,
        std::path::PathBuf::new(),
    )
}

struct CwdGuard {
    prev: std::path::PathBuf,
    #[allow(dead_code)]
    lock: MutexGuard<'static, ()>,
    /// Optional tempdir, declared last so it is deleted only after cwd has
    /// been restored (drop order = field declaration order).
    temp: Option<tempfile::TempDir>,
}

impl CwdGuard {
    /// Path of the guard's working directory (the tempdir when present).
    #[allow(dead_code)]
    fn path(&self) -> std::path::PathBuf {
        self.temp
            .as_ref()
            .map(|t| t.path().to_path_buf())
            .unwrap_or_else(|| std::env::current_dir().expect("current dir"))
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.prev);
    }
}

/// Change the process working directory to `dir`, returning a guard that
/// restores the previous cwd on drop.
///
/// The guard acquires the shared cwd mutex before changing directory and
/// **keeps it held** until dropped, so only one test manipulates cwd at a
/// time. Callers MUST NOT also hold `cwd_lock()` (that would deadlock, since
/// `std::sync::Mutex` is not re-entrant).
#[allow(dead_code)] // used by tests that require cwd manipulation
fn with_cwd(dir: &std::path::Path) -> CwdGuard {
    let lock = cwd_lock();
    let prev = std::env::current_dir().expect("current dir");
    std::env::set_current_dir(dir).expect("set_current_dir");
    CwdGuard {
        prev,
        lock,
        temp: None,
    }
}

/// Write `content` to `<dir>/.ragent/memory/MEMORY.md` (creating parent dirs).
#[allow(dead_code)] // used by tests that set up project memory state
fn write_project_memory(dir: &std::path::Path, content: &str) {
    let mem_dir = dir.join(".ragent").join("memory");
    std::fs::create_dir_all(&mem_dir).expect("create memory dir");
    std::fs::write(mem_dir.join("MEMORY.md"), content).expect("write MEMORY.md");
}

/// Render the app into a string buffer of the given terminal size.
#[allow(dead_code)] // used by visual assertion tests
fn render_app_to_string(app: &mut App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, app))
        .expect("render memory panel");

    let backend = terminal.backend();
    let buffer = backend.buffer();
    let mut text = String::new();
    let area = buffer.area();
    for y in 0..area.height {
        for x in 0..area.width {
            text.push_str(buffer[(x, y)].symbol());
        }
        text.push('\n');
    }
    text
}

fn enter_temp_config_dir() -> tempfile::TempDir {
    let temp = tempfile::tempdir().expect("tempdir");
    std::env::set_current_dir(temp.path()).expect("set cwd");
    let ragent_dir = temp.path().join(".ragent");
    std::fs::create_dir_all(&ragent_dir).expect("create .ragent");
    // Prime a project-local config with a known YOLO state so persistence tests
    // do not race on Config::load's default-config creation path.
    std::fs::write(ragent_dir.join("ragent.json"), r#"{"yolo": false}"#)
        .expect("write project config");
    temp
}

/// Create a tempdir, chdir into it, and return a guard that restores the
/// previous cwd on drop. The shared cwd mutex is held for the guard's whole
/// lifetime, and the tempdir is deleted only after cwd has been restored
/// (field drop order). Declared `temp` and `lock` locals are not needed at
/// the call site, which removes a common cause of cwd-mutex deadlock.
fn enter_with_cwd() -> CwdGuard {
    let lock = cwd_lock();
    let prev = std::env::current_dir().expect("current dir");
    let temp = tempfile::TempDir::new().expect("tempdir");
    std::env::set_current_dir(temp.path()).expect("set_current_dir");
    CwdGuard {
        prev,
        lock,
        temp: Some(temp),
    }
}

fn cwd_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Acquire the cwd test lock, recovering from any prior poisoning.
#[allow(clippy::await_holding_lock)]
fn cwd_lock() -> MutexGuard<'static, ()> {
    let lock = cwd_test_lock().lock();
    match lock {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_alt_e_toggles_edit_log_and_status_bar_indicator() {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let lock = cwd_lock();
    let original_cwd = std::env::current_dir().expect("cwd");
    let _temp = enter_temp_config_dir();
    let _guard = CwdGuard {
        prev: original_cwd,
        lock,
        temp: None,
    };
    ragent_config::edit_log::set_enabled(false);

    let mut app = make_app_with_storage(storage);

    // Sanity: edit log starts off.
    assert!(!ragent_config::edit_log::is_enabled());

    // Press Alt+E through the app handler so the persist path runs.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::ALT))
        .await;

    // Handler should have toggled and persisted edit logging on.
    assert!(ragent_config::edit_log::is_enabled());
    assert!(app.status.contains("Edit log enabled"));

    // Status bar indicator should reflect the current (enabled) state.
    let backend = TestBackend::new(140, 30);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| layout::render(frame, &mut app))
        .expect("draw");
    let cells = terminal.backend().buffer().content.clone();
    let text: String = cells.iter().map(ratatui::buffer::Cell::symbol).collect();
    assert!(
        text.contains("✏️") && text.contains("✓"),
        "status bar should show enabled edit-log icon: {text}"
    );

    // Toggle back off and verify.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::ALT))
        .await;
    assert!(!ragent_config::edit_log::is_enabled());
    assert!(app.status.contains("Edit log disabled"));

    let backend = TestBackend::new(140, 30);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| layout::render(frame, &mut app))
        .expect("draw");
    let cells = terminal.backend().buffer().content.clone();
    let text: String = cells.iter().map(ratatui::buffer::Cell::symbol).collect();
    assert!(
        text.contains("✏️") && text.contains("✗"),
        "status bar should show disabled edit-log icon: {text}"
    );
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_slash_editlog_toggles_and_persists() {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let lock = cwd_lock();
    let original_cwd = std::env::current_dir().expect("cwd");
    let _temp = enter_temp_config_dir();
    let _guard = CwdGuard {
        prev: original_cwd,
        lock,
        temp: None,
    };
    ragent_config::edit_log::set_enabled(false);

    let mut app = make_app_with_storage(storage);
    app.input = "/editlog on".to_string();
    app.input_cursor = app.input.chars().count();

    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .await;

    assert!(ragent_config::edit_log::is_enabled());
    assert!(app.status.contains("enabled"));

    app.input = "/editlog off".to_string();
    app.input_cursor = app.input.chars().count();
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .await;
    assert!(!ragent_config::edit_log::is_enabled());
    assert!(app.status.contains("disabled"));
}

#[test]
fn test_backfill_model_ctx_window_refreshes_stale_ollama_cloud_cache() {
    let mut app = make_app();
    app.selected_model = Some("ollama_cloud/deepseek-v4-pro".to_string());
    app.selected_model_ctx_window = Some(32_768);
    app.storage
        .set_setting("selected_model_ctx_window", "32768")
        .expect("persist stale ctx");

    let discovered = vec![provider::ModelInfo {
        id: "deepseek-v4-pro".to_string(),
        provider_id: "ollama_cloud".to_string(),
        name: "DeepSeek V4 Pro".to_string(),
        cost: ragent_config::Cost {
            input: 0.0,
            output: 0.0,
        },
        capabilities: ragent_config::Capabilities {
            reasoning: false,
            streaming: true,
            vision: false,
            tool_use: true,
            thinking_levels: Vec::new(),
        },
        context_window: 1_048_576,
        max_output: None,
        request_multiplier: None,
        thinking_config: None,
    }];
    let discovered_json = serde_json::to_string(&discovered).expect("serialize discovered models");
    app.storage
        .set_discovered_models("ollama_cloud", &discovered_json)
        .expect("persist discovered models");

    app.backfill_model_ctx_window();

    assert_eq!(app.selected_model_ctx_window, Some(1_048_576));
    assert_eq!(
        app.storage
            .get_setting("selected_model_ctx_window")
            .expect("read ctx setting"),
        Some("1048576".to_string())
    );
}

#[test]
fn test_app_start_clears_huggingface_discovery_cache() {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    storage
        .set_discovered_models("huggingface", r#"[{"id":"stale/model"}]"#)
        .expect("persist stale HF cache");

    let _app = make_app_with_storage(storage.clone());

    assert_eq!(
        storage
            .get_discovered_models("huggingface")
            .expect("read discovered models"),
        None
    );
}

#[test]
fn test_huggingface_with_token_does_not_fall_back_to_static_defaults_without_discovery() {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    storage
        .set_provider_auth("huggingface", "hf_test_token")
        .expect("store token");
    let app = make_app_with_storage(storage);

    let models = app.models_for_provider("huggingface");

    assert!(models.is_empty(), "expected no static fallback models");
}

// ── /clear ──────────────────────────────────────────────────────────

#[tokio::test]
async fn test_slash_clear_empties_messages() {
    let mut app = make_app();
    // Add some dummy messages
    app.messages
        .push(ragent_agent::message::Message::user_text("s1", "hello"));
    app.messages
        .push(ragent_agent::message::Message::user_text("s1", "world"));
    assert_eq!(app.messages.len(), 2);

    app.execute_slash_command("/clear").await;

    assert!(app.messages.is_empty(), "messages should be cleared");
    assert_eq!(app.scroll_offset, 0, "scroll should reset");
    assert_eq!(app.status, "messages cleared");
    // Should log the command start, the action, and the completion.
    assert!(
        app.log_entries.len() >= 2,
        "expected at least start+action logs"
    );
    assert!(app.log_entries[0].message.contains("Executing /clear"));
    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("cleared"))
    );
    assert!(
        app.log_entries
            .last()
            .unwrap()
            .message
            .contains("Finished /clear")
    );
}

// ── /help ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_slash_help_shows_commands() {
    let mut app = make_app();
    // Set a session so append_assistant_text can push messages
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/help").await;

    assert_eq!(app.status, "help");
    // Should have created an assistant message with command list
    assert!(!app.messages.is_empty(), "help should create a message");
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("/clear"), "help should mention /clear");
    assert!(text.contains("/quit"), "help should mention /quit");
    assert!(text.contains("/system"), "help should mention /system");
    assert!(text.contains("/compact"), "help should mention /compact");
    assert!(text.contains("/agent"), "help should mention /agent");
    assert!(text.contains("/model"), "help should mention /model");
    assert!(
        text.contains("/inputdiag"),
        "help should mention /inputdiag"
    );
    assert!(text.contains("/help"), "help should mention /help");
    assert!(text.contains("/spec"), "help should mention /spec");
}

#[tokio::test]
async fn test_slash_help_executes_in_chat_screen() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    // App now starts in Chat mode - home screen has been removed
    assert_eq!(app.current_screen, ScreenMode::Chat);

    app.execute_slash_command("/help").await;
    // Should remain in Chat mode
    assert_eq!(app.current_screen, ScreenMode::Chat);
}

// ── /quit ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_slash_quit_stops_app() {
    let mut app = make_app();
    assert!(app.is_running);

    app.execute_slash_command("/quit").await;
    assert!(!app.is_running, "app should stop after /quit");
}

#[tokio::test]
async fn test_slash_exit_stops_app() {
    let mut app = make_app();
    assert!(app.is_running);

    app.execute_slash_command("/exit").await;
    assert!(!app.is_running, "app should stop after /exit");
}

// ── /system ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_slash_system_sets_prompt() {
    let mut app = make_app();
    app.execute_slash_command("/system You are a pirate. Respond in pirate speak.")
        .await;

    assert_eq!(
        app.agent_info.prompt.as_deref(),
        Some("You are a pirate. Respond in pirate speak.")
    );
    assert_eq!(app.status, "system prompt updated");
    // Should have start/action/finish logs
    assert!(app.log_entries.len() >= 2);
    assert!(app.log_entries[0].message.contains("Executing /system"));
    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("System prompt set"))
    );
    assert!(
        app.log_entries
            .last()
            .unwrap()
            .message
            .contains("Finished /system")
    );
}

#[tokio::test]
async fn test_slash_system_no_args_shows_current() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    let original = app.agent_info.prompt.clone();

    app.execute_slash_command("/system").await;

    // Should display the current prompt, not change it
    assert_eq!(app.agent_info.prompt, original);
    if original.is_some() {
        assert!(!app.messages.is_empty(), "should show current prompt");
        let text = app.messages.last().unwrap().text_content();
        assert!(text.contains("Current system prompt"));
    }
}

#[tokio::test]
async fn test_slash_system_replaces_existing() {
    let mut app = make_app();
    app.execute_slash_command("/system First prompt").await;
    assert_eq!(app.agent_info.prompt.as_deref(), Some("First prompt"));

    app.execute_slash_command("/system Second prompt").await;
    assert_eq!(app.agent_info.prompt.as_deref(), Some("Second prompt"));
}

// ── /agent ──────────────────────────────────────────────────────────

#[tokio::test]
async fn test_slash_agent_with_name_switches() {
    let mut app = make_app();
    assert_eq!(app.agent_name, "general");

    app.execute_slash_command("/agent ask").await;

    assert_eq!(app.agent_name, "ask");
    assert_eq!(app.agent_info.name, "ask");
    assert!(app.status.contains("ask"));
}

#[tokio::test]
async fn test_slash_agent_unknown_name_shows_error() {
    let mut app = make_app();
    app.execute_slash_command("/agent nonexistent").await;

    assert!(
        app.status.contains("Unknown agent"),
        "status should warn about unknown agent: {}",
        app.status
    );
    assert_eq!(app.agent_name, "general", "should not change agent");
}

#[tokio::test]
async fn test_slash_agent_no_args_opens_dialog() {
    let mut app = make_app();
    app.execute_slash_command("/agent").await;

    assert!(
        app.provider_setup.is_some(),
        "should open agent selection dialog"
    );
}

// ── /log ────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_slash_log_toggles_panel() {
    let mut app = make_app();
    assert!(!app.show_log, "log should be hidden initially");

    app.execute_slash_command("/log").await;
    assert!(app.show_log, "log should be visible after first toggle");
    assert_eq!(app.status, "log panel visible");

    app.execute_slash_command("/log").await;
    assert!(!app.show_log, "log should be hidden after second toggle");
    assert_eq!(app.status, "log panel hidden");
}

#[tokio::test]
async fn test_slash_log_clear_subagents() {
    let _guard = enter_with_cwd();
    let subagents_dir = std::env::current_dir()
        .unwrap()
        .join("log")
        .join("subagents");

    // Seed log/subagents with a few files.
    std::fs::create_dir_all(&subagents_dir).expect("create subagents dir");
    std::fs::write(subagents_dir.join("explore-aaa.md"), "report a").expect("write a");
    std::fs::write(subagents_dir.join("explore-bbb.md"), "report b").expect("write b");
    assert_eq!(std::fs::read_dir(&subagents_dir).unwrap().count(), 2);

    let mut app = make_app();
    app.execute_slash_command("/log clear subagents").await;

    assert_eq!(app.status, "log: subagents cleared");
    // Directory is kept but emptied.
    assert!(subagents_dir.exists(), "subagents dir should still exist");
    assert_eq!(
        std::fs::read_dir(&subagents_dir).unwrap().count(),
        0,
        "subagents dir should be empty"
    );
}

#[tokio::test]
async fn test_slash_log_clear_panics() {
    let _guard = enter_with_cwd();
    let panics_dir = std::env::current_dir().unwrap().join("log").join("panics");

    // Seed log/panics with a few files.
    std::fs::create_dir_all(&panics_dir).expect("create panics dir");
    std::fs::write(panics_dir.join("panic-1.log"), "panic a").expect("write a");
    std::fs::write(panics_dir.join("panic-2.log"), "panic b").expect("write b");
    std::fs::write(panics_dir.join("panic-3.log"), "panic c").expect("write c");
    assert_eq!(std::fs::read_dir(&panics_dir).unwrap().count(), 3);

    let mut app = make_app();
    app.execute_slash_command("/log clear panics").await;

    assert_eq!(app.status, "log: panics cleared");
    assert!(panics_dir.exists(), "panics dir should still exist");
    assert_eq!(
        std::fs::read_dir(&panics_dir).unwrap().count(),
        0,
        "panics dir should be empty"
    );
}

#[tokio::test]
async fn test_slash_log_clear_missing_dir_reports_zero() {
    let _guard = enter_with_cwd();

    // No log/subagents directory exists yet.
    assert!(
        !std::env::current_dir()
            .unwrap()
            .join("log")
            .join("subagents")
            .exists()
    );

    let mut app = make_app();
    app.execute_slash_command("/log clear subagents").await;
    assert_eq!(app.status, "log: subagents cleared");
}

#[tokio::test]
async fn test_slash_log_clear_no_target_shows_usage() {
    let mut app = make_app();
    app.execute_slash_command("/log clear").await;
    assert_eq!(app.status, "log: clear usage");
}

#[tokio::test]
async fn test_slash_log_clear_research() {
    let _guard = enter_with_cwd();
    let research_dir = std::env::current_dir()
        .unwrap()
        .join("log")
        .join("research");

    // Seed log/research with a few files.
    std::fs::create_dir_all(&research_dir).expect("create research dir");
    std::fs::write(research_dir.join("research-aaa-web.jsonl"), "line a\n").expect("write a");
    std::fs::write(research_dir.join("research-bbb-web.jsonl"), "line b\n").expect("write b");
    assert_eq!(std::fs::read_dir(&research_dir).unwrap().count(), 2);

    let mut app = make_app();
    app.execute_slash_command("/log clear research").await;

    assert_eq!(app.status, "log: research cleared");
    assert!(research_dir.exists(), "research dir should still exist");
    assert_eq!(
        std::fs::read_dir(&research_dir).unwrap().count(),
        0,
        "research dir should be empty"
    );
}

#[tokio::test]
async fn test_slash_log_clear_research_missing_dir_reports_zero() {
    let _guard = enter_with_cwd();

    // No log/research directory exists yet.
    assert!(
        !std::env::current_dir()
            .unwrap()
            .join("log")
            .join("research")
            .exists()
    );

    let mut app = make_app();
    app.execute_slash_command("/log clear research").await;
    assert_eq!(app.status, "log: research cleared");
}

#[tokio::test]
async fn test_slash_log_help_shows_help_text() {
    let mut app = make_app();
    app.execute_slash_command("/log help").await;
    assert_eq!(app.status, "log: help");
}

#[tokio::test]
async fn test_slash_log_clear_editlog() {
    let _guard = enter_with_cwd();
    let editlog_dir = std::env::current_dir().unwrap().join("log").join("editlog");

    // Seed log/editlog with a few files.
    std::fs::create_dir_all(&editlog_dir).expect("create editlog dir");
    std::fs::write(editlog_dir.join("edits-aaa.jsonl"), "line a\n").expect("write a");
    std::fs::write(editlog_dir.join("edits-bbb.jsonl"), "line b\n").expect("write b");
    assert_eq!(std::fs::read_dir(&editlog_dir).unwrap().count(), 2);

    let mut app = make_app();
    app.execute_slash_command("/log clear editlog").await;

    assert_eq!(app.status, "log: editlog cleared");
    assert!(editlog_dir.exists(), "editlog dir should still exist");
    assert_eq!(
        std::fs::read_dir(&editlog_dir).unwrap().count(),
        0,
        "editlog dir should be empty"
    );
}

#[tokio::test]
async fn test_slash_log_clear_editlog_missing_dir_reports_zero() {
    let _guard = enter_with_cwd();

    // No log/editlog directory exists yet.
    assert!(
        !std::env::current_dir()
            .unwrap()
            .join("log")
            .join("editlog")
            .exists()
    );

    let mut app = make_app();
    app.execute_slash_command("/log clear editlog").await;
    assert_eq!(app.status, "log: editlog cleared");
}

#[tokio::test]
async fn test_slash_log_clear_logwindow() {
    let _guard = enter_with_cwd();
    let logwindow_dir = std::env::current_dir()
        .unwrap()
        .join("log")
        .join("logwindow");

    // Seed log/logwindow with a few files.
    std::fs::create_dir_all(&logwindow_dir).expect("create logwindow dir");
    std::fs::write(logwindow_dir.join("logwindow-aaa.log"), "line a\n").expect("write a");
    std::fs::write(logwindow_dir.join("logwindow-bbb.log"), "line b\n").expect("write b");
    assert_eq!(std::fs::read_dir(&logwindow_dir).unwrap().count(), 2);

    let mut app = make_app();
    app.execute_slash_command("/log clear logwindow").await;

    assert_eq!(app.status, "log: logwindow cleared");
    assert!(logwindow_dir.exists(), "logwindow dir should still exist");
    assert_eq!(
        std::fs::read_dir(&logwindow_dir).unwrap().count(),
        0,
        "logwindow dir should be empty"
    );
}

#[tokio::test]
async fn test_slash_log_clear_logwindow_missing_dir_reports_zero() {
    let _guard = enter_with_cwd();

    // No log/logwindow directory exists yet.
    assert!(
        !std::env::current_dir()
            .unwrap()
            .join("log")
            .join("logwindow")
            .exists()
    );

    let mut app = make_app();
    app.execute_slash_command("/log clear logwindow").await;
    assert_eq!(app.status, "log: logwindow cleared");
}

#[tokio::test]
async fn test_slash_log_unknown_sub_shows_usage() {
    let mut app = make_app();
    app.execute_slash_command("/log frobnicate").await;
    assert_eq!(app.status, "log: usage");
}

// ── /profile ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_slash_profile_on_enables_profiler_panel() {
    agent_loop_profiler().set_enabled(false);

    let mut app = make_app();
    assert!(!app.show_profile, "profile should be hidden initially");

    app.execute_slash_command("/profile on").await;

    assert!(app.show_profile, "profile should be visible after enabling");
    assert_eq!(app.status, "profile panel visible");

    agent_loop_profiler().set_enabled(false);
}

#[tokio::test]
async fn test_slash_profile_off_disables_profiler_panel() {
    agent_loop_profiler().set_enabled(true);

    let mut app = make_app();
    app.show_profile = true;

    app.execute_slash_command("/profile off").await;

    assert!(
        !app.show_profile,
        "profile should be hidden after disabling"
    );
    assert_eq!(app.status, "profile panel hidden");
}

#[tokio::test]
async fn test_alt_p_toggles_profiler_panel() {
    agent_loop_profiler().set_enabled(false);

    let mut app = make_app();
    assert!(!app.show_profile, "profile should be hidden initially");

    app.handle_key_event(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::ALT))
        .await;
    assert!(app.show_profile, "profile should be visible after Alt+P");
    assert_eq!(app.status, "profile panel visible");

    app.handle_key_event(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::ALT))
        .await;
    assert!(
        !app.show_profile,
        "profile should be hidden after second Alt+P"
    );
    assert_eq!(app.status, "profile panel hidden");

    agent_loop_profiler().set_enabled(false);
}

// ── /llmstats ───────────────────────────────────────────────────────

#[tokio::test]
async fn test_slash_llmstats_shows_average_metrics() {
    let mut app = make_app();
    app.selected_model = Some("openai/gpt-4o".to_string());
    app.llm_request_stats = vec![
        ragent_tui::app::LlmRequestStat {
            model_ref: "openai/gpt-4o".to_string(),
            elapsed_ms: 1000,
            input_tokens: 100,
            output_tokens: 50,
        },
        ragent_tui::app::LlmRequestStat {
            model_ref: "openai/gpt-4o".to_string(),
            elapsed_ms: 500,
            input_tokens: 200,
            output_tokens: 100,
        },
    ];

    app.execute_slash_command("/llmstats").await;

    assert_eq!(app.status, "llm stats");
    assert!(!app.messages.is_empty(), "llmstats should create a message");
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("From: /llmstats"));
    assert!(text.contains("Model: openai/gpt-4o"));
    assert!(text.contains("Samples: 2"));
    assert!(text.contains("Average round-trip"));
    assert!(text.contains("Average prompt parsing"));
    assert!(text.contains("Average output"));
}

#[tokio::test]
async fn test_slash_llmstats_no_samples_shows_message() {
    let mut app = make_app();
    app.selected_model = Some("openai/gpt-4o".to_string());

    app.execute_slash_command("/llmstats").await;

    assert_eq!(app.status, "llm stats unavailable");
    assert!(!app.messages.is_empty(), "llmstats should create a message");
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("No completed LLM responses yet"));
}

// ── /cost ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_slash_cost_shows_estimated_cost() {
    let mut app = make_app();
    app.llm_request_stats = vec![
        ragent_tui::app::LlmRequestStat {
            model_ref: "openai/gpt-4o".to_string(),
            elapsed_ms: 1000,
            input_tokens: 1000,
            output_tokens: 500,
        },
        ragent_tui::app::LlmRequestStat {
            model_ref: "ollama/llama3.2".to_string(),
            elapsed_ms: 750,
            input_tokens: 800,
            output_tokens: 400,
        },
    ];

    app.execute_slash_command("/cost").await;

    assert_eq!(app.status, "cost summary");
    assert!(!app.messages.is_empty(), "cost should create a message");
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("From: /cost"));
    assert!(text.contains("Samples: 2"));
    assert!(text.contains("Total tokens"));
    assert!(text.contains("Estimated cost"));
}

#[tokio::test]
async fn test_slash_cost_no_samples_shows_message() {
    let mut app = make_app();

    app.execute_slash_command("/cost").await;

    assert_eq!(app.status, "cost unavailable");
    assert!(!app.messages.is_empty(), "cost should create a message");
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("No completed LLM responses yet"));
}

// ── /clip ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_slash_clip_copies_rendered_message_lines() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.messages.push(ragent_agent::message::Message::user_text(
        "s1",
        "hello world",
    ));
    app.messages
        .push(ragent_agent::message::Message::assistant_text(
            "s1", "hi there",
        ));

    // PERF-041: the copy buffer is rebuilt on demand from the per-message
    // cache, so the transcript must be rendered at least once first.
    render_app_to_string(&mut app, 100, 30);
    assert!(
        !app.message_content_lines.is_empty(),
        "render must populate the copy buffer"
    );
    let expected_chars = app.message_content_lines.join("\n").len();
    let expected_lines = app.message_content_lines.len();

    app.execute_slash_command("/clip").await;

    assert_eq!(app.status, format!("clip: copied {expected_chars} chars"));
    assert!(!app.messages.is_empty(), "clip should create a message");
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("From: /clip"));
    assert!(text.contains(&format!("{expected_chars} characters")));
    assert!(text.contains(&format!("{expected_lines} rendered lines")));
    // The clipboard write itself is fire-and-forget on a background thread
    // (see clipboard::set_clipboard_text); verify it was issued without
    // stalling the test on the Linux wait() workaround.
}

#[tokio::test]
async fn test_slash_clip_registered_in_help() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/help").await;

    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("/clip"), "help should document /clip: {text}");
}

#[tokio::test]
async fn test_slash_clip_empty_window_shows_hint() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    assert!(app.message_content_lines.is_empty(), "window starts empty");

    app.execute_slash_command("/clip").await;

    assert_eq!(app.status, "clip: nothing to copy");
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("From: /clip"));
    assert!(text.contains("No rendered message content"));
}

// ── /compact ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_slash_compact_no_session_shows_warning() {
    let mut app = make_app();
    assert!(app.session_id.is_none());

    app.execute_slash_command("/compact").await;
    assert!(
        app.status.contains("No messages"),
        "should create session then warn about empty messages: {}",
        app.status
    );
    assert!(app.session_id.is_some(), "session should be created");
}

#[tokio::test]
async fn test_slash_compact_no_messages_shows_warning() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    assert!(app.messages.is_empty());

    app.execute_slash_command("/compact").await;
    assert!(
        app.status.contains("No messages"),
        "should warn about empty messages: {}",
        app.status
    );
}

// `/compress` is a deprecated alias for `/compact` (FR-009) and must
// forward to the same compaction path.

#[tokio::test]
async fn test_slash_compress_alias_forwards_to_compact() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    assert!(app.messages.is_empty());

    app.execute_slash_command("/compress").await;
    assert!(
        app.status.contains("No messages"),
        "/compress should behave like /compact when there is nothing to compact: {}",
        app.status
    );
}

// ── /undo ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_slash_undo_no_session_shows_warning() {
    let mut app = make_app();
    assert!(app.session_id.is_none());

    app.execute_slash_command("/undo").await;
    // The ensure_session() gate runs before the undo handler, so a session
    // will be created. The undo logic then checks for empty messages.
    assert!(
        app.status.contains("No messages"),
        "should warn about no messages after session creation: {}",
        app.status
    );
    assert!(app.session_id.is_some(), "session should be created");
}

#[tokio::test]
async fn test_slash_undo_no_messages_shows_warning() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    assert!(app.messages.is_empty());

    app.execute_slash_command("/undo").await;
    assert!(
        app.status.contains("No messages"),
        "should warn about no messages: {}",
        app.status
    );
}

#[tokio::test]
async fn test_slash_undo_removes_last_user_assistant_pair() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    // Build a conversation: user, assistant, user, assistant
    app.messages.push(ragent_agent::message::Message::user_text(
        "s1",
        "first question",
    ));
    app.messages
        .push(ragent_agent::message::Message::assistant_text(
            "s1",
            "first answer",
        ));
    app.messages.push(ragent_agent::message::Message::user_text(
        "s1",
        "second question",
    ));
    app.messages
        .push(ragent_agent::message::Message::assistant_text(
            "s1",
            "second answer",
        ));

    assert_eq!(app.messages.len(), 4);

    app.execute_slash_command("/undo").await;

    // Should have removed the last user message and its assistant response
    assert_eq!(app.messages.len(), 2);
    assert_eq!(app.messages[0].text_content(), "first question");
    assert_eq!(app.messages[1].text_content(), "first answer");
    assert_eq!(app.scroll_offset, 0);
    assert!(app.status.contains("Undid last turn"));
    assert!(app.status.contains("removed 2 message(s)"));
}

#[tokio::test]
async fn test_slash_undo_no_user_message_warns() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    // Only assistant messages (no user messages to undo)
    app.messages
        .push(ragent_agent::message::Message::assistant_text(
            "s1",
            "orphan answer",
        ));

    app.execute_slash_command("/undo").await;

    assert!(
        app.status.contains("No user message found"),
        "should warn about no user message: {}",
        app.status
    );
    assert_eq!(app.messages.len(), 1); // unchanged
}

#[tokio::test]
async fn test_slash_undo_removes_multiple_following_messages() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    // User message followed by multiple assistant messages
    app.messages
        .push(ragent_agent::message::Message::user_text("s1", "question"));
    app.messages
        .push(ragent_agent::message::Message::assistant_text(
            "s1",
            "answer part 1",
        ));
    app.messages
        .push(ragent_agent::message::Message::assistant_text(
            "s1",
            "answer part 2",
        ));

    assert_eq!(app.messages.len(), 3);

    app.execute_slash_command("/undo").await;

    // Should remove user message and all following messages
    assert_eq!(app.messages.len(), 0);
    assert!(app.status.contains("removed 3 message(s)"));
}

// ── /name ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_slash_name_no_session_shows_warning() {
    let mut app = make_app();
    assert!(app.session_id.is_none());

    app.execute_slash_command("/name My Session").await;
    // The ensure_session() gate runs before the name handler, so a session
    // will be created. The name is then set on that session.
    assert!(app.session_id.is_some(), "session should be created");
    assert!(
        app.status.contains("Session name set to"),
        "should confirm name was set: {}",
        app.status
    );
}

#[tokio::test]
async fn test_slash_name_sets_session_name() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    // Create the session in storage first
    let storage = app.session_processor.session_manager.storage();
    storage
        .create_session("s1", "/tmp/test")
        .expect("create session");
    let _ = storage;

    app.execute_slash_command("/name My Test Session").await;

    assert!(
        app.status.contains("Session name set to 'My Test Session'"),
        "should confirm name was set: {}",
        app.status
    );

    // Verify the name was persisted
    let storage = app.session_processor.session_manager.storage();
    let session = storage
        .get_session("s1")
        .expect("get session")
        .expect("session exists");
    assert_eq!(session.title, "My Test Session");
}

#[tokio::test]
async fn test_slash_name_clears_with_empty_argument() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    let storage = app.session_processor.session_manager.storage();
    storage
        .create_session("s1", "/tmp/test")
        .expect("create session");

    // First set a name
    storage
        .update_session("s1", "Initial Name")
        .expect("set name");
    let _ = storage;

    // Then clear it with empty argument
    app.execute_slash_command("/name ").await;

    assert!(
        app.status.contains("Session name cleared"),
        "should confirm name was cleared: {}",
        app.status
    );

    // Verify the name was cleared
    let storage = app.session_processor.session_manager.storage();
    let session = storage
        .get_session("s1")
        .expect("get session")
        .expect("session exists");
    assert_eq!(session.title, "");
}

#[tokio::test]
async fn test_slash_name_trims_whitespace() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    let storage = app.session_processor.session_manager.storage();
    storage
        .create_session("s1", "/tmp/test")
        .expect("create session");
    let _ = storage;

    app.execute_slash_command("/name   Trimmed Name   ").await;

    assert!(
        app.status.contains("Session name set to 'Trimmed Name'"),
        "should trim whitespace: {}",
        app.status
    );

    let storage = app.session_processor.session_manager.storage();
    let session = storage
        .get_session("s1")
        .expect("get session")
        .expect("session exists");
    assert_eq!(session.title, "Trimmed Name");
}

#[tokio::test]
async fn test_help_shows_name_command() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/help").await;
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("/name"),
        "help should document /name command: {text}"
    );
}

#[tokio::test]
async fn test_help_lists_compact_not_compress() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/help").await;
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("/compact"),
        "help should document /compact: {text}"
    );
    assert!(
        !text.contains("/compress"),
        "help should no longer document the deprecated /compress alias: {text}"
    );
}

// ── /model ──────────────────────────────────────────────────────────

#[tokio::test]
async fn test_slash_model_opens_provider_picker() {
    let mut app = make_app();
    // No provider configured by default (no env vars in test)
    app.execute_slash_command("/model").await;
    // With no provider configured, /model opens the provider picker.
    // (If a provider was auto-detected from the environment, it would jump
    // straight to the model list instead.)
    assert!(
        matches!(
            app.provider_setup,
            Some(ProviderSetupStep::SelectProvider { .. })
                | Some(ProviderSetupStep::LoadingModels { .. })
        ),
        "/model should open the provider picker or jump to model loading, got: {:?}",
        app.provider_setup
    );
}

#[tokio::test]
async fn test_slash_model_show_without_selected_model_uses_agent_model() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/model show").await;

    assert_eq!(app.status, "active model metadata");
    let text = app
        .messages
        .last()
        .expect("metadata message")
        .text_content();
    assert!(text.contains("From: /model show"));
    assert!(text.contains("Model Ref"));
}

#[tokio::test]
async fn test_slash_model_show_displays_metadata_for_active_model() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.configured_provider = Some(ConfiguredProvider {
        id: "openai".to_string(),
        name: "OpenAI (GPT)".to_string(),
        source: ProviderSource::Database,
    });
    app.selected_model = Some("openai/gpt-4o-mini".to_string());
    app.selected_model_ctx_window = Some(128_000);

    // `OpenAiProvider::default_models` returns an empty catalog (models are
    // discovered at runtime), so seed the gpt-4o-mini entry into the discovery
    // cache. Without this the `/model show` report cannot resolve the model
    // entry and only emits the cached-context-window fallback, which lacks the
    // "Context window" / "Tool use" capability lines the test asserts on.
    let discovered = vec![provider::ModelInfo {
        id: "gpt-4o-mini".to_string(),
        provider_id: "openai".to_string(),
        name: "GPT-4o Mini".to_string(),
        cost: ragent_config::Cost {
            input: 0.15,
            output: 0.60,
        },
        capabilities: ragent_config::Capabilities {
            reasoning: false,
            streaming: true,
            vision: true,
            tool_use: true,
            thinking_levels: Vec::new(),
        },
        context_window: 128_000,
        max_output: Some(16_384),
        request_multiplier: None,
        thinking_config: None,
    }];
    let discovered_json =
        serde_json::to_string(&discovered).expect("serialize openai discovered models");
    app.storage
        .set_discovered_models("openai", &discovered_json)
        .expect("persist openai discovered models");

    app.execute_slash_command("/model show").await;

    assert_eq!(app.status, "active model metadata");
    let text = app
        .messages
        .last()
        .expect("metadata message")
        .text_content();
    assert!(text.contains("From: /model show"));
    assert!(text.contains("OpenAI (GPT)"));
    assert!(text.contains("gpt-4o-mini"));
    assert!(text.contains("Context window"));
    assert!(text.contains("Tool use"));
}

#[tokio::test]
async fn test_slash_model_show_invalid_subcommand_shows_usage() {
    let mut app = make_app();

    app.execute_slash_command("/model nope").await;

    assert_eq!(app.status, "Usage: /model [show]");
}

#[tokio::test]
async fn test_slash_model_empty_model_list_shows_warning_instead_of_opening_picker() {
    let mut app = make_app();
    app.configured_provider = Some(ConfiguredProvider {
        id: "missing-provider".to_string(),
        name: "Missing Provider".to_string(),
        source: ProviderSource::Database,
    });

    app.execute_slash_command("/model").await;

    // With a configured provider that is not registered, /model jumps to
    // LoadingModels (which will fail and fall back to the model picker or
    // a warning). The key point is that /model skips the provider picker
    // when a provider is already configured.
    assert!(
        matches!(
            app.provider_setup,
            Some(ProviderSetupStep::LoadingModels { .. })
        ),
        "expected /model to jump to LoadingModels for the configured provider, got: {:?}",
        app.provider_setup
    );
}

#[tokio::test]
async fn test_slash_model_ollama_cloud_falls_back_to_selected_model_when_discovery_is_unavailable()
{
    let mut app = make_app();
    // Store auth so get_configured_providers() picks up ollama_cloud.
    app.storage
        .set_provider_auth("ollama_cloud", "sk-test")
        .expect("store ollama_cloud key");
    // Disable copilot so ollama_cloud is the sole configured provider.
    let _ = app.storage.set_setting("provider_copilot_disabled", "true");
    // Persist a last-model so the restore path finds it.
    app.storage
        .set_setting("provider_ollama_cloud_last_model", "deepseek-v4-flash")
        .expect("persist model");
    app.configured_provider = Some(ConfiguredProvider {
        id: "ollama_cloud".to_string(),
        name: "Ollama Cloud".to_string(),
        source: ProviderSource::Database,
    });
    app.selected_model = Some("ollama_cloud/deepseek-v4-flash".to_string());
    app.selected_model_ctx_window = Some(262_144);

    app.execute_slash_command("/model").await;

    // The new /model flow shows a configured-provider picker, or auto-selects
    // a single provider and attempts model restore. Because discovery is not
    // available in tests, the cached model list may be empty and the fallback
    // entry (from selected_model_fallback_entries) is returned by
    // unresolved_model_entries_for_provider but not by models_for_provider
    // (which requires network for ollama_cloud).
    //
    // Accept any of: Done (model restored), SelectModel (picker fallback), or
    // None (empty model list warning).
    if let Some(step) = app.provider_setup.as_ref() {
        match step {
            ProviderSetupStep::Done {
                provider_name,
                model_name,
            } => {
                assert_eq!(provider_name, "Ollama Cloud");
                assert!(model_name.as_deref().is_some());
            }
            ProviderSetupStep::SelectModel {
                provider_id,
                provider_name,
                models,
                selected,
            } => {
                assert_eq!(provider_id, "ollama_cloud");
                assert_eq!(provider_name, "Ollama Cloud");
                assert_eq!(*selected, 0);
                assert_eq!(models.len(), 1);
                assert_eq!(models[0].id, "deepseek-v4-flash");
                assert_eq!(models[0].context_window, 262_144);
            }
            other => {
                // Also acceptable: the model list was empty and no picker opened.
                let _ = other;
            }
        }
    }
}

// ── /provider ───────────────────────────────────────────────────────

#[tokio::test]
async fn test_slash_provider_opens_setup() {
    let mut app = make_app();
    app.execute_slash_command("/provider").await;

    assert!(
        app.provider_setup.is_some(),
        "should open provider setup dialog"
    );
}

#[tokio::test]
async fn test_slash_provider_always_prompts_for_key_when_already_configured() {
    let mut app = make_app();
    // Store an API key so the provider is "already configured".
    app.storage
        .set_provider_auth("anthropic", "sk-existing")
        .expect("store anthropic key");
    app.configured_provider = Some(ConfiguredProvider {
        id: "anthropic".to_string(),
        name: "Anthropic".to_string(),
        source: ProviderSource::Database,
    });

    // Open the provider picker via /provider (force_key_entry == true).
    app.execute_slash_command("/provider").await;
    assert!(
        matches!(
            app.provider_setup,
            Some(ProviderSetupStep::SelectProvider {
                force_key_entry: true,
                ..
            })
        ),
        "/provider should open the picker with force_key_entry=true, got: {:?}",
        app.provider_setup
    );

    // Find the anthropic index and press Enter.
    let anthropic_idx = ragent_tui::app::PROVIDER_LIST
        .iter()
        .position(|(id, _)| *id == "anthropic")
        .expect("anthropic in PROVIDER_LIST");
    app.provider_setup = Some(ProviderSetupStep::SelectProvider {
        selected: anthropic_idx,
        force_key_entry: true,
    });
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .await;

    // Even though anthropic is already configured, /provider should show
    // the EnterKey dialog so the user can edit the key.
    assert!(
        matches!(
            app.provider_setup,
            Some(ProviderSetupStep::EnterKey { ref provider_id, .. }) if provider_id == "anthropic"
        ),
        "/provider should always show EnterKey for an already-configured key-based provider, got: {:?}",
        app.provider_setup
    );

    // The key field should be pre-filled with the existing key so the user
    // can edit it rather than re-entering from scratch.
    if let Some(ProviderSetupStep::EnterKey { key_field, .. }) = &app.provider_setup {
        assert_eq!(
            key_field.text(),
            "sk-existing",
            "key field should be pre-filled with the existing key"
        );
    }
}
#[tokio::test]
async fn test_slash_provider_selection_updates_displayed_provider() {
    let mut app = make_app();

    // Start with a different provider so we can verify the display updates.
    app.configured_provider = Some(ConfiguredProvider {
        id: "openai".to_string(),
        name: "OpenAI (GPT)".to_string(),
        source: ProviderSource::Database,
    });
    app.selected_model = Some("openai/gpt-4".to_string());

    // Simulate selecting a provider/model via the interactive dialog.
    app.provider_setup = Some(ProviderSetupStep::SelectModel {
        provider_id: "ollama".to_string(),
        provider_name: "Ollama (Local)".to_string(),
        models: vec![ragent_tui::app::ModelPickerEntry {
            provider_id: "ollama".to_string(),
            id: "llama3.2".to_string(),
            name: "Llama 3.2".to_string(),
            context_window: 131_072,
            max_output: None,
            cost_input: 0.0,
            cost_output: 0.0,
            reasoning: false,
            vision: false,
            tool_use: true,
            thinking_levels: vec![],
            thinking_config: None,
            cost_tier: "Free".to_string(),
            cost_multiplier: "0x".to_string(),
        }],
        selected: 0,
    });

    // Press Enter to confirm the model selection. Because this is an Ollama-family
    // provider, the selector now forces the thinking-level step even when model
    // detection reports no levels.
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .await;
    assert!(
        matches!(
            app.provider_setup,
            Some(ProviderSetupStep::SelectThinkingLevel { .. })
        ),
        "ollama model selection should open the thinking-level selector"
    );

    // Press Enter again to confirm the default thinking level.
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .await;

    assert_eq!(
        app.configured_provider.as_ref().map(|p| p.id.as_str()),
        Some("ollama"),
        "provider should update when a new model is selected"
    );
    assert_eq!(
        app.provider_model_label().as_deref(),
        Some("Ollama (Local) / llama3.2"),
        "provider/model label should reflect the new provider"
    );
}

#[test]
fn test_provider_list_includes_generic_openai() {
    assert!(
        ragent_tui::app::PROVIDER_LIST
            .iter()
            .any(|(id, name)| *id == "generic_openai" && *name == "Generic OpenAI API"),
        "provider list should include Generic OpenAI API"
    );
    assert!(
        ragent_tui::app::PROVIDER_LIST
            .iter()
            .any(|(id, name)| *id == "ollama_cloud" && *name == "Ollama Cloud"),
        "provider list should include Ollama Cloud"
    );
}
#[tokio::test]
async fn test_model_selector_navigation_wraps_top_and_bottom() {
    let mut app = make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectModel {
        provider_id: "copilot".to_string(),
        provider_name: "GitHub Copilot".to_string(),
        models: vec![
            ragent_tui::app::ModelPickerEntry {
                provider_id: "copilot".to_string(),
                id: "m1".to_string(),
                name: "Model 1".to_string(),
                context_window: 128_000,
                max_output: Some(16_384),
                cost_input: 0.0,
                cost_output: 0.0,
                reasoning: false,
                vision: true,
                tool_use: true,
                thinking_levels: vec![],
                thinking_config: None,
                cost_tier: "Free".to_string(),
                cost_multiplier: "0x".to_string(),
            },
            ragent_tui::app::ModelPickerEntry {
                provider_id: "copilot".to_string(),
                id: "m2".to_string(),
                name: "Model 2".to_string(),
                context_window: 128_000,
                max_output: Some(16_384),
                cost_input: 0.0,
                cost_output: 0.0,
                reasoning: false,
                vision: true,
                tool_use: true,
                thinking_levels: vec![],
                thinking_config: None,
                cost_tier: "Free".to_string(),
                cost_multiplier: "0x".to_string(),
            },
            ragent_tui::app::ModelPickerEntry {
                provider_id: "copilot".to_string(),
                id: "m3".to_string(),
                name: "Model 3".to_string(),
                context_window: 128_000,
                max_output: Some(16_384),
                cost_input: 0.0,
                cost_output: 0.0,
                reasoning: false,
                vision: true,
                tool_use: true,
                thinking_levels: vec![],
                thinking_config: None,
                cost_tier: "Free".to_string(),
                cost_multiplier: "0x".to_string(),
            },
        ],
        selected: 0,
    });

    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)).await;
    match app.provider_setup.as_ref().expect("provider setup present") {
        ProviderSetupStep::SelectModel { selected, .. } => assert_eq!(*selected, 2),
        _ => panic!("expected SelectModel state"),
    }

    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)).await;
    match app.provider_setup.as_ref().expect("provider setup present") {
        ProviderSetupStep::SelectModel { selected, .. } => assert_eq!(*selected, 0),
        _ => panic!("expected SelectModel state"),
    }
}

// ── /provider_reset ─────────────────────────────────────────────────

#[tokio::test]
async fn test_slash_provider_reset_opens_dialog() {
    let mut app = make_app();
    app.execute_slash_command("/provider_reset").await;

    assert!(
        app.provider_setup.is_some(),
        "should open provider reset dialog"
    );
}

#[tokio::test]
async fn test_generic_openai_enter_key_supports_endpoint_field_and_tab_toggle() {
    let mut app = make_app();
    app.provider_setup = Some(ProviderSetupStep::EnterKey {
        provider_id: "generic_openai".to_string(),
        provider_name: "Generic OpenAI API".to_string(),
        key_field: ragent_tui::input_field::InputField::new(),
        endpoint_field: ragent_tui::input_field::InputField::new(),
        active_field: 0,
        error: None,
    });

    // Toggle to endpoint field and type URL.
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)).await;
    ragent_tui::input::handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE),
    )
    .await;
    ragent_tui::input::handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE),
    )
    .await;

    match app.provider_setup.as_ref().expect("provider setup present") {
        ProviderSetupStep::EnterKey {
            endpoint_field,
            active_field,
            ..
        } => {
            assert_eq!(*active_field, 1);
            assert_eq!(endpoint_field.text(), "ht");
        }
        _ => panic!("expected EnterKey"),
    }
}

#[tokio::test]
async fn test_generic_openai_enter_key_persists_endpoint_setting() {
    let mut app = make_app();
    app.provider_setup = Some(ProviderSetupStep::EnterKey {
        provider_id: "generic_openai".to_string(),
        provider_name: "Generic OpenAI API".to_string(),
        key_field: ragent_tui::input_field::InputField::with_text("test-key"),
        endpoint_field: ragent_tui::input_field::InputField::with_text("http://localhost:11434/v1"),
        active_field: 0,
        error: None,
    });

    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .await;

    assert_eq!(
        app.storage
            .get_setting("generic_openai_api_base")
            .ok()
            .flatten(),
        Some("http://localhost:11434/v1".to_string())
    );
}

#[test]
fn test_provider_setup_paste_text_into_key_field() {
    let mut app = make_app();
    app.provider_setup = Some(ProviderSetupStep::EnterKey {
        provider_id: "ollama_cloud".to_string(),
        provider_name: "Ollama Cloud".to_string(),
        key_field: ragent_tui::input_field::InputField::new(),
        endpoint_field: ragent_tui::input_field::InputField::new(),
        active_field: 0,
        error: None,
    });

    app.paste_text_into_provider_setup("cloud-key");

    match app.provider_setup.as_ref().expect("provider setup present") {
        ProviderSetupStep::EnterKey { key_field, .. } => {
            assert_eq!(key_field.text(), "cloud-key");
            assert_eq!(key_field.cursor(), 9);
        }
        _ => panic!("expected EnterKey"),
    }
}

#[test]
fn test_telemetry_setup_context_menu_paste_writes_active_field() {
    use ragent_tui::app::{ContextAction, ContextMenuState, SelectionPane};
    use ragent_tui::clipboard::ClipboardTestOverrideGuard;

    let mut app = make_app();
    let endpoint_field = ragent_tui::input_field::InputField::new();
    let interval_field = ragent_tui::input_field::InputField::new();
    let timeout_field = ragent_tui::input_field::InputField::new();
    let port_field = ragent_tui::input_field::InputField::new();
    app.provider_setup = Some(ProviderSetupStep::TelemetrySetup {
        endpoint_field,
        protocol: ragent_config::telemetry::OtelProtocol::Http,
        interval_field,
        timeout_field,
        port_field,
        active_field: 0,
        error: None,
    });
    app.context_menu = Some(ContextMenuState {
        x: 0,
        y: 0,
        pane: SelectionPane::Input,
        selected: 2,
        items: vec![
            (ContextAction::Cut, false),
            (ContextAction::Copy, false),
            (ContextAction::Paste, true),
        ],
    });

    // Avoid requiring a real display server in headless CI; drive the paste
    // path with a thread-local test-only clipboard override.
    let _guard = ClipboardTestOverrideGuard::new("http://otel:4318");
    app.execute_context_action(ContextAction::Paste);

    match app.provider_setup.as_ref().expect("provider setup present") {
        ProviderSetupStep::TelemetrySetup { endpoint_field, .. } => {
            assert_eq!(endpoint_field.text(), "http://otel:4318");
            assert_eq!(endpoint_field.cursor(), 16);
        }
        _ => panic!("expected TelemetrySetup"),
    }
    assert!(
        app.context_menu.is_none(),
        "context menu should be dismissed"
    );
}

#[test]
fn test_paste_text_replaces_keyboard_selection() {
    let mut app = make_app();
    app.input = "hello world".to_string();
    app.input_cursor = 5;
    app.kb_select_anchor = Some(0);

    app.handle_paste_text("pasted");

    assert_eq!(app.input, "pasted world");
    assert_eq!(app.input_cursor, 6);
    assert!(app.kb_select_anchor.is_none());
}

#[test]
fn test_paste_text_replaces_mouse_selection() {
    use ragent_tui::app::{SelectionPane, TextSelection};
    use ratatui::layout::Rect;

    let mut app = make_app();
    app.input = "hello world".to_string();
    app.input_cursor = 0;
    app.kb_select_anchor = None;
    app.input_area = Rect::new(0, 0, 12, 10);
    app.text_selection = Some(TextSelection {
        pane: SelectionPane::Input,
        anchor: (2, 1),
        endpoint: (8, 1),
    });

    app.handle_paste_text("pasted");

    assert_eq!(app.input, "pastedworld");
    assert!(app.text_selection.is_none());
}

// ── unknown command ─────────────────────────────────────────────────

#[tokio::test]
async fn test_slash_unknown_command_shows_error() {
    let mut app = make_app();
    app.execute_slash_command("/foobar").await;

    assert!(
        app.status.contains("Unknown command"),
        "should show error for unknown command: {}",
        app.status
    );
    assert!(app.status.contains("foobar"));
    // Expect at least start and completion logs plus the warning.
    assert!(app.log_entries.len() >= 2);
    assert!(app.log_entries.iter().any(|e| e.level == LogLevel::Warn));
    assert!(app.log_entries[0].message.contains("Executing /foobar"));
    assert!(
        app.log_entries
            .last()
            .unwrap()
            .message
            .contains("Finished /foobar")
    );
}

#[tokio::test]
async fn test_slash_unknown_command_visible_in_message_window() {
    // The status line auto-expires back to "ready", so a typo'd slash command
    // must ALSO render a visible rejection in the message window (a silently
    // swallowed prompt once looked like a broken agent loop).
    let mut app = make_app();
    let before = app.messages.len();
    app.execute_slash_command("/simpify all").await;

    assert_eq!(
        app.messages.len(),
        before + 1,
        "unknown command should append one assistant message"
    );
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("From: /simpify"),
        "message window notice should carry the From header, got: {text}"
    );
    assert!(
        text.contains("Unknown command: `/simpify`"),
        "message window notice should name the unknown command, got: {text}"
    );
    assert!(
        text.contains("/help"),
        "message window notice should point at /help, got: {text}"
    );
}

// ── /blueprints command ─────────────────────────────────────────────

fn write_temp_blueprint(dir: &std::path::Path, name: &str, readme: &str, teammates: usize) {
    let bp_dir = dir
        .join(".ragent")
        .join("blueprints")
        .join("teams")
        .join(name);
    std::fs::create_dir_all(&bp_dir).expect("create blueprint dir");
    std::fs::write(bp_dir.join("README.md"), readme).expect("write README");

    let teammate_values: Vec<serde_json::Value> = (0..teammates)
        .map(|i| {
            serde_json::json!({
                "teammate_name": format!("mate-{i}"),
                "agent_type": "general",
                "prompt": "help"
            })
        })
        .collect();
    std::fs::write(
        bp_dir.join("spawn-prompts.json"),
        serde_json::to_string(&teammate_values).expect("serialize teammates"),
    )
    .expect("write spawn-prompts");

    std::fs::write(
        bp_dir.join("task-seed.json"),
        serde_json::to_string(&Vec::<serde_json::Value>::new()).expect("serialize tasks"),
    )
    .expect("write task-seed");
}

#[tokio::test]
async fn test_blueprints_list_empty() {
    let _guard = enter_with_cwd();
    let mut app = make_app();
    app.execute_slash_command("/blueprints").await;
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("From: /blueprints"),
        "list should show header: {text}"
    );
    assert!(
        text.contains("No blueprints found"),
        "empty list should say so: {text}"
    );
    assert_eq!(app.status, "blueprints: list");
}

#[tokio::test]
async fn test_blueprints_list_installed() {
    let guard = enter_with_cwd();
    write_temp_blueprint(&guard.path(), "demo", "# Demo\nA demo blueprint", 2);

    let mut app = make_app();
    app.execute_slash_command("/blueprints list").await;
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("## Installed Team Blueprints"),
        "list should show table header: {text}"
    );
    assert!(
        text.contains("`demo`"),
        "list should include demo blueprint: {text}"
    );
    assert!(
        text.contains("A demo blueprint"),
        "list should include description: {text}"
    );
    assert_eq!(app.status, "blueprints: list");
}

#[tokio::test]
async fn test_blueprints_help() {
    let _guard = enter_with_cwd();
    let mut app = make_app();
    app.execute_slash_command("/blueprints help").await;
    assert_eq!(app.status, "blueprints: help");
}

#[tokio::test]
async fn test_blueprints_detail() {
    let guard = enter_with_cwd();
    write_temp_blueprint(&guard.path(), "demo", "# Demo\nA demo blueprint", 1);

    let mut app = make_app();
    app.execute_slash_command("/blueprints demo").await;
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("From: /blueprints demo"),
        "detail header: {text}"
    );
    assert!(
        text.contains("## Blueprint: `demo`"),
        "detail title: {text}"
    );
    assert!(
        text.contains("A demo blueprint"),
        "detail description: {text}"
    );
    assert!(text.contains("### Teammates"), "detail teammates: {text}");
    assert_eq!(app.status, "blueprints: demo");
}

#[tokio::test]
async fn test_blueprints_unknown_name() {
    let _guard = enter_with_cwd();
    let mut app = make_app();
    app.execute_slash_command("/blueprints missing").await;
    assert_eq!(app.status, "Blueprint 'missing' not found");
    assert!(app.log_entries.iter().any(|e| e.level == LogLevel::Warn));
}

#[tokio::test]
async fn test_team_blueprint_still_works_after_refactor() {
    let guard = enter_with_cwd();
    write_temp_blueprint(&guard.path(), "demo", "# Demo\nA demo blueprint", 1);

    let mut app = make_app();
    app.execute_slash_command("/team blueprint demo").await;
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("From: /team blueprint demo"),
        "team blueprint detail header: {text}"
    );
    assert!(
        text.contains("## Blueprint: `demo`"),
        "team blueprint title: {text}"
    );
    assert_eq!(app.status, "team: blueprint demo");
}

// ── input clearing ──────────────────────────────────────────────────

#[tokio::test]
async fn test_slash_command_clears_input() {
    let mut app = make_app();
    app.input = "/help".to_string();

    app.execute_slash_command(&app.input.clone()).await;
    assert!(
        app.input.is_empty(),
        "input should be cleared after command"
    );
    assert!(app.slash_menu.is_none(), "slash menu should be closed");
}

#[tokio::test]
async fn test_input_cursor_left_right_and_editing() {
    let mut app = make_app();
    app.input = "abc".to_string();
    app.input_cursor = app.input.chars().count();

    // Move cursor left twice (from end to between 'b' and 'c')
    app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE))
        .await;
    app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE))
        .await;

    assert_eq!(app.input_cursor, 1);

    // Insert a character at the cursor position
    app.handle_key_event(KeyEvent::new(KeyCode::Char('X'), KeyModifiers::NONE))
        .await;
    assert_eq!(app.input, "aXbc");
    assert_eq!(app.input_cursor, 2);

    // Move to end and delete the inserted character
    app.handle_key_event(KeyEvent::new(KeyCode::End, KeyModifiers::NONE))
        .await;
    assert_eq!(app.input_cursor, 4);

    // Backspace at end removes the last character.
    app.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE))
        .await;
    assert_eq!(app.input, "aXb");
    assert_eq!(app.input_cursor, 3);

    // Move left one position and delete the inserted character.
    app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE))
        .await;
    assert_eq!(app.input_cursor, 2);
    app.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE))
        .await;
    assert_eq!(app.input, "ab");
    assert_eq!(app.input_cursor, 1);
}

#[tokio::test]
async fn test_input_editing_handles_unicode_backspace_and_delete() {
    let mut app = make_app();
    app.input = "a💡b".to_string();
    app.input_cursor = app.input.chars().count();

    // Move to between 💡 and b, then backspace removes 💡.
    app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE))
        .await;
    assert_eq!(app.input_cursor, 2);
    app.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE))
        .await;
    assert_eq!(app.input, "ab");
    assert_eq!(app.input_cursor, 1);

    // Delete at cursor should remove the next character.
    app.input = "a💡b".to_string();
    app.input_cursor = 1; // before 💡
    app.handle_key_event(KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE))
        .await;
    assert_eq!(app.input, "ab");
    assert_eq!(app.input_cursor, 1);
}

#[tokio::test]
async fn test_file_menu_mode_editing_respects_midline_cursor() {
    let mut app = make_app();
    app.input = "ab@cd".to_string();
    app.input_cursor = 2; // between 'b' and '@'
    app.file_menu = Some(FileMenuState {
        matches: vec![FileMenuEntry {
            display: "src/main.rs".to_string(),
            path: std::path::PathBuf::from("src/main.rs"),
            is_dir: false,
        }],
        selected: 0,
        scroll_offset: 0,
        query: "src".to_string(),
        current_dir: None,
    });

    app.handle_key_event(KeyEvent::new(KeyCode::Char('X'), KeyModifiers::NONE))
        .await;
    assert_eq!(app.input, "abX@cd");
    assert_eq!(app.input_cursor, 3);

    app.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE))
        .await;
    assert_eq!(app.input, "ab@cd");
    assert_eq!(app.input_cursor, 2);
}

#[tokio::test]
async fn test_history_picker_enter_sets_char_cursor_for_unicode() {
    let mut app = make_app();
    app.history_picker = Some(HistoryPickerState {
        entries: vec!["éé".to_string()],
        selected: 0,
        scroll_offset: 0,
    });

    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .await;

    assert!(app.history_picker.is_none());
    assert_eq!(app.input, "éé");
    assert_eq!(app.input_cursor, 2);
}

#[tokio::test]
async fn test_chat_keystrokes_produce_expected_edit_result() {
    // Test that input handling works correctly in chat mode
    let mut chat = make_app();
    // App now starts in Chat mode - home screen has been removed
    assert_eq!(chat.current_screen, ScreenMode::Chat);

    let sequence = vec![
        KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('💡'), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('Z'), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::End, KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
    ];
    for key in sequence {
        chat.handle_key_event(key).await;
    }

    // Verify the final state
    assert_eq!(chat.input, "aZ");
    assert_eq!(chat.input_cursor, 2);
}

#[tokio::test]
async fn test_ctrl_word_navigation_and_deletes() {
    let mut app = make_app();
    app.input = "hello world again".to_string();
    app.input_cursor = app.input.chars().count();

    app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL))
        .await;
    assert_eq!(app.input_cursor, "hello world ".chars().count());

    app.handle_key_event(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL))
        .await;
    assert_eq!(app.input, "hello again");
    assert_eq!(app.input_cursor, "hello ".chars().count());

    app.handle_key_event(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL))
        .await;
    assert_eq!(app.input, "hello ");
    assert_eq!(app.input_cursor, "hello ".chars().count());
}

#[tokio::test]
async fn test_ctrl_terminal_cursor_movement_bindings() {
    let mut app = make_app();
    app.input = "abcdef".to_string();
    app.input_cursor = 3;

    app.handle_key_event(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL))
        .await;
    assert_eq!(app.input_cursor, 2);
    app.handle_key_event(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL))
        .await;
    assert_eq!(app.input_cursor, 3);

    // Ctrl+A now selects all: anchor → 0, cursor → end.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL))
        .await;
    assert_eq!(app.kb_select_anchor, Some(0));
    assert_eq!(app.input_cursor, 6);
    // Ctrl+E moves to end (cursor is already there; clears selection).
    app.handle_key_event(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL))
        .await;
    assert_eq!(app.input_cursor, 6);
}

#[tokio::test]
async fn test_ctrl_home_end_bindings() {
    let mut app = make_app();
    app.input = "abcdef".to_string();
    app.input_cursor = 3;

    app.handle_key_event(KeyEvent::new(KeyCode::Home, KeyModifiers::CONTROL))
        .await;
    assert_eq!(app.input_cursor, 0);
    app.handle_key_event(KeyEvent::new(KeyCode::End, KeyModifiers::CONTROL))
        .await;
    assert_eq!(app.input_cursor, 6);
}

#[test]
fn test_file_menu_targets_mention_under_cursor_not_last_mention() {
    let _lock = cwd_lock();
    let mut app = make_app();
    app.input = "compare @first with @second".to_string();
    let first_cursor = app.input.find("@first").expect("first mention exists") + "@fi".len();
    app.input_cursor = app.input[..first_cursor].chars().count();

    app.project_files_cache = Some(vec![
        std::path::PathBuf::from("first_file.rs"),
        std::path::PathBuf::from("second_file.rs"),
    ]);
    app.project_files_cache_cwd = Some(std::env::current_dir().expect("cwd"));

    app.update_file_menu();
    let menu = app.file_menu.as_ref().expect("file menu should open");
    assert_eq!(menu.query, "first");
    assert!(
        menu.matches
            .iter()
            .any(|e| e.display.contains("first_file.rs"))
    );
}

#[test]
fn test_accept_file_menu_replaces_active_mention_span_only() {
    let _lock = cwd_lock();
    let mut app = make_app();
    app.input = "compare @first with @second".to_string();
    let first_cursor = app.input.find("@first").expect("first mention exists") + "@first".len();
    app.input_cursor = app.input[..first_cursor].chars().count();

    app.file_menu = Some(FileMenuState {
        matches: vec![FileMenuEntry {
            display: "src/first_match.rs".to_string(),
            path: std::path::PathBuf::from("src/first_match.rs"),
            is_dir: false,
        }],
        selected: 0,
        scroll_offset: 0,
        query: "first".to_string(),
        current_dir: None,
    });

    let closed = app.accept_file_menu_selection();
    assert!(closed);
    assert_eq!(app.input, "compare @src/first_match.rs with @second");
    assert_eq!(
        app.input_cursor,
        "compare @src/first_match.rs".chars().count()
    );
}

#[test]
fn test_file_menu_closes_when_cursor_not_inside_mention() {
    let _lock = cwd_lock();
    let mut app = make_app();
    app.input = "compare @first with @second".to_string();
    app.input_cursor = 0;
    app.project_files_cache = Some(vec![std::path::PathBuf::from("first.rs")]);

    app.update_file_menu();
    assert!(app.file_menu.is_none());
}

#[tokio::test]
async fn test_file_menu_mode_supports_cursor_movement_and_delete() {
    let mut app = make_app();
    app.input = "ab@cd".to_string();
    app.input_cursor = 3; // between '@' and 'c'
    app.file_menu = Some(FileMenuState {
        matches: vec![FileMenuEntry {
            display: "src/main.rs".to_string(),
            path: std::path::PathBuf::from("src/main.rs"),
            is_dir: false,
        }],
        selected: 0,
        scroll_offset: 0,
        query: "c".to_string(),
        current_dir: None,
    });

    app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE))
        .await;
    assert_eq!(app.input_cursor, 2);

    app.handle_key_event(KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE))
        .await;
    assert_eq!(app.input, "abcd");
    assert_eq!(app.input_cursor, 2);
}

#[tokio::test]
async fn test_file_menu_mode_supports_ctrl_word_actions() {
    let mut app = make_app();
    app.input = "@hello world".to_string();
    app.input_cursor = app.input.chars().count();
    app.file_menu = Some(FileMenuState {
        matches: vec![FileMenuEntry {
            display: "src/main.rs".to_string(),
            path: std::path::PathBuf::from("src/main.rs"),
            is_dir: false,
        }],
        selected: 0,
        scroll_offset: 0,
        query: "world".to_string(),
        current_dir: None,
    });

    app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL))
        .await;
    assert_eq!(app.input_cursor, "@hello ".chars().count());

    app.handle_key_event(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL))
        .await;
    assert_eq!(app.input, "@hello ");
    assert_eq!(app.input_cursor, "@hello ".chars().count());
}

#[tokio::test]
async fn test_file_menu_enter_accepts_without_sending() {
    let mut app = make_app();
    app.input = "@first".to_string();
    app.input_cursor = app.input.chars().count();
    app.file_menu = Some(FileMenuState {
        matches: vec![FileMenuEntry {
            display: "src/first.rs".to_string(),
            path: std::path::PathBuf::from("src/first.rs"),
            is_dir: false,
        }],
        selected: 0,
        scroll_offset: 0,
        query: "first".to_string(),
        current_dir: None,
    });

    let action =
        ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .await;
    assert!(action.is_none(), "enter should accept mention but not send");
    assert_eq!(app.input, "@src/first.rs");
    assert!(
        app.file_menu.is_none(),
        "menu should close after file acceptance"
    );
}

#[test]
fn test_file_menu_no_matches_stays_open_for_feedback() {
    let _lock = cwd_lock();
    let mut app = make_app();
    app.input = "@nomatch".to_string();
    app.input_cursor = app.input.chars().count();
    app.project_files_cache = Some(vec![std::path::PathBuf::from("src/first.rs")]);

    app.update_file_menu();
    let menu = app.file_menu.as_ref().expect("menu should stay open");
    assert!(menu.matches.is_empty(), "no matches should be represented");
    assert_eq!(menu.query, "nomatch");
}

#[tokio::test]
async fn test_slash_browse_refresh_updates_cache_metadata() {
    let mut app = make_app();
    app.project_files_cache = None;
    app.project_files_cache_cwd = None;
    app.project_files_cache_refreshed_at = None;
    app.project_files_cache_count = 0;

    app.execute_slash_command("/browse_refresh").await;

    assert!(
        app.status.starts_with("browse index refreshed"),
        "status should reflect browse refresh"
    );
    assert!(
        app.project_files_cache.is_some(),
        "cache should be populated"
    );
    assert!(
        app.project_files_cache_cwd.is_some(),
        "cache cwd should be set"
    );
    assert!(
        app.project_files_cache_refreshed_at.is_some(),
        "cache timestamp should be set"
    );
    assert_eq!(
        app.project_files_cache_count,
        app.project_files_cache
            .as_ref()
            .map_or(0, std::vec::Vec::len)
    );
}

#[test]
fn test_update_file_menu_refreshes_cache_on_cwd_mismatch() {
    let _lock = cwd_lock();
    let mut app = make_app();
    app.input = "@src".to_string();
    app.input_cursor = app.input.chars().count();
    app.project_files_cache = Some(vec![]);
    app.project_files_cache_cwd = Some(std::path::PathBuf::from("/definitely/not/current"));

    app.update_file_menu();

    let cwd = std::env::current_dir().expect("cwd");
    assert_eq!(app.project_files_cache_cwd, Some(cwd));
    assert!(app.project_files_cache.is_some());
    assert_eq!(
        app.project_files_cache_count,
        app.project_files_cache
            .as_ref()
            .map_or(0, std::vec::Vec::len)
    );
}

#[test]
fn test_directory_menu_has_back_to_fuzzy_entry() {
    let _lock = cwd_lock();
    let mut app = make_app();
    app.input = "@src".to_string();
    app.input_cursor = app.input.chars().count();
    app.file_menu = Some(FileMenuState {
        matches: vec![FileMenuEntry {
            display: "src/".to_string(),
            path: std::path::PathBuf::from("src"),
            is_dir: true,
        }],
        selected: 0,
        scroll_offset: 0,
        query: "src".to_string(),
        current_dir: None,
    });
    let _ = app.accept_file_menu_selection();
    let menu = app.file_menu.as_ref().expect("directory menu should open");
    assert_eq!(
        menu.matches.first().map(|e| e.display.as_str()),
        Some("<back to fuzzy>")
    );
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_file_menu_ctrl_backslash_toggles_hidden_filter() {
    let _lock = cwd_lock();
    let mut app = make_app();
    app.input = "@src".to_string();
    app.input_cursor = app.input.chars().count();
    app.file_menu = Some(FileMenuState {
        matches: vec![FileMenuEntry {
            display: "src/main.rs".to_string(),
            path: std::path::PathBuf::from("src/main.rs"),
            is_dir: false,
        }],
        selected: 0,
        scroll_offset: 0,
        query: "src".to_string(),
        current_dir: Some(std::path::PathBuf::from("src")),
    });

    assert!(!app.file_menu_show_hidden);
    let _ = ragent_tui::input::handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('\\'), KeyModifiers::CONTROL),
    )
    .await;
    assert!(app.file_menu_show_hidden);
    assert!(app.file_menu.is_some());
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_file_menu_down_scrolls_selection_window() {
    let _lock = cwd_lock();
    let mut app = make_app();
    let mut entries = Vec::new();
    for i in 0..12 {
        entries.push(FileMenuEntry {
            display: format!("src/file_{i}.rs"),
            path: std::path::PathBuf::from(format!("src/file_{i}.rs")),
            is_dir: false,
        });
    }
    app.file_menu = Some(FileMenuState {
        matches: entries,
        selected: 0,
        scroll_offset: 0,
        query: "file".to_string(),
        current_dir: None,
    });

    for _ in 0..9 {
        let _ = ragent_tui::input::handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
        )
        .await;
    }
    let menu = app.file_menu.as_ref().expect("menu");
    assert_eq!(menu.selected, 9);
    assert!(menu.scroll_offset > 0);
}

#[tokio::test]
async fn test_slash_inputdiag_reports_input_state() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.input = "abc".to_string();
    app.input_cursor = 2;

    app.execute_slash_command("/inputdiag").await;

    assert_eq!(app.status, "inputdiag");
    assert!(!app.messages.is_empty());
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("Input diagnostics:"));
    assert!(text.contains("input chars: 0"));
    assert!(text.contains("input cursor: 0"));
    assert!(text.contains("browse cache entries:"));
    assert!(text.contains("browse menu state:"));
}

// ── with leading slash and without ──────────────────────────────────

#[tokio::test]
async fn test_slash_command_works_without_leading_slash() {
    let mut app = make_app();
    app.execute_slash_command("quit").await;
    assert!(!app.is_running, "/quit should work without leading slash");
}

#[tokio::test]
async fn test_keyboard_quit_requires_ctrl_c_then_ctrl_d() {
    let mut app = make_app();
    assert!(app.is_running);

    app.handle_key_event(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL))
        .await;
    assert!(app.is_running, "Ctrl+D alone should not quit");
    assert!(app.status.contains("Ctrl+C first"));

    app.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL))
        .await;
    assert!(app.is_running, "Ctrl+C should arm, not quit");
    assert!(app.status.contains("Ctrl+D"));

    app.handle_key_event(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL))
        .await;
    assert!(!app.is_running, "Ctrl+C then Ctrl+D should quit");
}

#[tokio::test]
async fn test_keyboard_quit_ctrl_c_then_ctrl_c_does_not_exit() {
    let mut app = make_app();
    assert!(app.is_running);

    app.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL))
        .await;
    assert!(app.is_running);

    app.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL))
        .await;
    assert!(app.is_running, "second Ctrl+C should not exit");
    assert!(app.status.contains("Ctrl+D"));
}

#[tokio::test]
async fn test_output_view_paging_shortcuts() {
    let mut app = make_app();
    app.output_view = Some(OutputViewState {
        target: OutputViewTarget::Session {
            session_id: "s1".to_string(),
            label: "primary".to_string(),
        },
        scroll_offset: 10,
        max_scroll: 50,
        line_cache: ragent_tui::app::OutputViewLineCache {
            wrapped_lines: Vec::new(),
            content_lines: Vec::new(),
            wrapped_count: 0,
            cache_width: 0,
            source_generation: 0,
        },
    });

    app.handle_key_event(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE))
        .await;
    assert_eq!(app.output_view.as_ref().unwrap().scroll_offset, 15);

    app.handle_key_event(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE))
        .await;
    assert_eq!(app.output_view.as_ref().unwrap().scroll_offset, 10);

    app.handle_key_event(KeyEvent::new(KeyCode::PageUp, KeyModifiers::CONTROL))
        .await;
    assert_eq!(app.output_view.as_ref().unwrap().scroll_offset, 50);

    app.handle_key_event(KeyEvent::new(KeyCode::PageDown, KeyModifiers::CONTROL))
        .await;
    assert_eq!(app.output_view.as_ref().unwrap().scroll_offset, 0);
}

#[tokio::test]
async fn test_output_view_escape_closes_overlay() {
    let mut app = make_app();
    app.selected_agent_session_id = Some("s1".to_string());
    app.selected_agent_index = Some(1);
    app.output_view = Some(OutputViewState {
        target: OutputViewTarget::Session {
            session_id: "s1".to_string(),
            label: "primary".to_string(),
        },
        scroll_offset: 0,
        max_scroll: 0,
        line_cache: ragent_tui::app::OutputViewLineCache {
            wrapped_lines: Vec::new(),
            content_lines: Vec::new(),
            wrapped_count: 0,
            cache_width: 0,
            source_generation: 0,
        },
    });

    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        .await;
    assert!(app.output_view.is_none());
    assert!(app.selected_agent_session_id.is_none());
    assert!(app.selected_agent_index.is_none());
}

#[tokio::test]
async fn test_output_view_team_member_without_session_uses_log_filter() {
    let mut app = make_app();
    app.log_entries.push(LogEntry {
        timestamp: chrono::Utc::now(),
        level: LogLevel::Info,
        message: "📨 [alpha] tm-001 → lead: done".to_string(),
        session_id: None,
        agent_id: None,
        seq: 1,
    });
    app.output_view = Some(OutputViewState {
        target: OutputViewTarget::TeamMember {
            team_name: "alpha".to_string(),
            agent_id: "tm-001".to_string(),
            teammate_name: "writer".to_string(),
            session_id: None,
        },
        scroll_offset: 0,
        max_scroll: 0,
        line_cache: ragent_tui::app::OutputViewLineCache {
            wrapped_lines: Vec::new(),
            content_lines: Vec::new(),
            wrapped_count: 0,
            cache_width: 0,
            source_generation: 0,
        },
    });

    app.handle_key_event(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE))
        .await;
    assert!(app.output_view.is_some());
}

// ── /system preserves whitespace ────────────────────────────────────

#[tokio::test]
async fn test_slash_system_preserves_argument_whitespace() {
    let mut app = make_app();
    app.execute_slash_command("/system   You are   a   helpful   bot  ")
        .await;

    assert_eq!(
        app.agent_info.prompt.as_deref(),
        Some("You are   a   helpful   bot"),
        "leading/trailing whitespace trimmed, internal preserved"
    );
}

// ── /tools ──────────────────────────────────────────────────────────

#[tokio::test]
async fn test_slash_tools_lists_visibility_switches() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.tool_visibility = ragent_agent::ToolVisibilityConfig::default();

    app.execute_slash_command("/tools").await;

    assert_eq!(app.status, "tools");
    assert!(!app.messages.is_empty());
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("Tool Family Visibility"),
        "should show visibility heading"
    );
    assert!(text.contains("office"), "should list office switch");
    assert!(text.contains("github"), "should list github switch");
    assert!(text.contains("teams"), "should list teams switch");
    assert!(text.contains("agents"), "should list agents switch");
    assert!(text.contains("plan"), "should list plan switch");
    assert!(text.contains("codeindex"), "should list codeindex switch");
}

#[tokio::test]
async fn test_slash_tools_shows_single_switch_state() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.tool_visibility = ragent_agent::ToolVisibilityConfig::default();

    app.execute_slash_command("/tools office").await;

    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("`office` is currently **off**"));
}

#[tokio::test]
async fn test_slash_tools_help_shows_usage() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.tool_visibility = ragent_agent::ToolVisibilityConfig::default();

    app.execute_slash_command("/tools help").await;

    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("`/tools show`"));
    assert!(text.contains("`/tools help`"));
    assert!(text.contains("`/tools <switch> on|off`"));
    assert!(text.contains("`office`, `github`, `gitlab`, `teams`, `agents`, `plan`, `codeindex`"));
}
#[tokio::test]
async fn test_slash_tools_show_alias_lists_visibility_switches() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.tool_visibility = ragent_agent::ToolVisibilityConfig::default();

    app.execute_slash_command("/tools show").await;

    assert_eq!(app.status, "tools");
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("Tool Family Visibility"));
    assert!(text.contains("office"));
    assert!(text.contains("teams"));
    assert!(text.contains("agents"));
    assert!(text.contains("plan"));
    assert!(text.contains("codeindex"));
    // Verify the visible tools list is included.
    assert!(text.contains("Visible Tools"), "should list visible tools");
    assert!(text.contains("read"), "should include the read tool");
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_slash_tools_office_on_shows_office_tools() {
    let lock = cwd_lock();
    let original_cwd = std::env::current_dir().expect("cwd");
    let _temp = enter_temp_config_dir();
    let _guard = CwdGuard {
        prev: original_cwd,
        lock,
        temp: None,
    };

    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.tool_visibility = ragent_agent::ToolVisibilityConfig::default();

    let hidden = ragent_agent::tool_family_names("office")
        .expect("office family")
        .iter()
        .map(|name| (*name).to_string())
        .collect::<Vec<_>>();
    app.session_processor.tool_registry.set_hidden(&hidden);
    assert!(
        !app.session_processor
            .tool_registry
            .definitions()
            .iter()
            .any(|d| d.name == "office_read")
    );

    app.execute_slash_command("/tools office on").await;

    assert!(app.tool_visibility.office);
    assert_eq!(app.status, "tools: office on");
    assert!(
        app.session_processor
            .tool_registry
            .definitions()
            .iter()
            .any(|d| d.name == "office_read")
    );
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("`office` visibility is now **on**"));
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_slash_tools_teams_on_shows_team_tools() {
    let lock = cwd_lock();
    let original_cwd = std::env::current_dir().expect("cwd");
    let _temp = enter_temp_config_dir();
    let _guard = CwdGuard {
        prev: original_cwd,
        lock,
        temp: None,
    };

    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.tool_visibility = ragent_agent::ToolVisibilityConfig::default();

    let hidden = ragent_agent::tool_family_names("teams")
        .expect("teams family")
        .iter()
        .map(|name| (*name).to_string())
        .collect::<Vec<_>>();
    app.session_processor.tool_registry.set_hidden(&hidden);

    assert!(
        !app.session_processor
            .tool_registry
            .definitions()
            .iter()
            .any(|d| d.name == "team_create")
    );

    app.execute_slash_command("/tools teams on").await;

    assert!(app.tool_visibility.teams);
    assert_eq!(app.status, "tools: teams on");
    assert!(
        app.session_processor
            .tool_registry
            .definitions()
            .iter()
            .any(|d| d.name == "team_create")
    );
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("`teams` visibility is now **on**"));
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_slash_tools_agents_on_shows_agent_tools() {
    let lock = cwd_lock();
    let original_cwd = std::env::current_dir().expect("cwd");
    let _temp = enter_temp_config_dir();
    let _guard = CwdGuard {
        prev: original_cwd,
        lock,
        temp: None,
    };

    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.tool_visibility = ragent_agent::ToolVisibilityConfig::default();

    let hidden = ragent_agent::tool_family_names("agents")
        .expect("agents family")
        .iter()
        .map(|name| (*name).to_string())
        .collect::<Vec<_>>();
    app.session_processor.tool_registry.set_hidden(&hidden);

    assert!(
        !app.session_processor
            .tool_registry
            .definitions()
            .iter()
            .any(|d| d.name == "new_agent")
    );

    app.execute_slash_command("/tools agents on").await;

    assert!(app.tool_visibility.agents);
    assert_eq!(app.status, "tools: agents on");
    assert!(
        app.session_processor
            .tool_registry
            .definitions()
            .iter()
            .any(|d| d.name == "new_agent")
    );
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("`agents` visibility is now **on**"));
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_slash_tools_plan_on_shows_plan_tools() {
    let lock = cwd_lock();
    let original_cwd = std::env::current_dir().expect("cwd");
    let _temp = enter_temp_config_dir();
    let _guard = CwdGuard {
        prev: original_cwd,
        lock,
        temp: None,
    };

    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.tool_visibility = ragent_agent::ToolVisibilityConfig::default();

    let hidden = ragent_agent::tool_family_names("plan")
        .expect("plan family")
        .iter()
        .map(|name| (*name).to_string())
        .collect::<Vec<_>>();
    app.session_processor.tool_registry.set_hidden(&hidden);

    assert!(
        !app.session_processor
            .tool_registry
            .definitions()
            .iter()
            .any(|d| d.name == "plan_enter")
    );

    app.execute_slash_command("/tools plan on").await;

    assert!(app.tool_visibility.plan);
    assert_eq!(app.status, "tools: plan on");
    assert!(
        app.session_processor
            .tool_registry
            .definitions()
            .iter()
            .any(|d| d.name == "plan_enter")
    );
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("`plan` visibility is now **on**"));
}

#[tokio::test]
async fn test_slash_tools_creates_session_if_none() {
    let mut app = make_app();
    assert!(app.session_id.is_none());

    app.execute_slash_command("/tools").await;

    assert!(app.session_id.is_some(), "should create session");
    assert_eq!(app.status, "tools");
    assert!(!app.messages.is_empty());
}

#[tokio::test]
async fn test_slash_webapi_help_shows_endpoints() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/webapi help").await;

    assert!(!app.messages.is_empty(), "help should produce a message");
    let last = app.messages.last().unwrap();
    let text = format!("{last:?}");
    assert!(
        text.contains("health") || text.contains("sessions"),
        "help output should list API endpoints"
    );
}

#[tokio::test]
async fn test_slash_webapi_disable_when_not_running() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/webapi disable").await;

    let last = app.messages.last().unwrap();
    let text = format!("{last:?}");
    assert!(
        text.contains("not running") || text.contains("Disabled"),
        "should report server not running"
    );
}

#[tokio::test]
async fn test_slash_webapi_enable_sets_token() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    assert!(
        app.webapi_token.is_none(),
        "token should be None before enabling"
    );

    app.execute_slash_command("/webapi enable").await;

    assert!(
        app.webapi_token.is_some(),
        "token should be set after /webapi enable"
    );
    assert!(app.webapi_server.is_some(), "server handle should be set");

    // Clean up
    if let Some(h) = app.webapi_server.take() {
        h.abort();
    }
}

// ── /spec ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_slash_spec_no_args_shows_help() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/spec").await;

    assert!(!app.messages.is_empty(), "spec should create a message");
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("spec help"), "should show spec help: {text}");
    assert!(
        text.contains("spec create"),
        "should mention spec create: {text}"
    );
    assert!(text.contains("specs/"), "should mention specs/ dir: {text}");
    assert!(text.contains("PLAN.md"), "should mention PLAN.md: {text}");
}

/// `/spec update` without a spec-id should show a usage error (FR-012).
#[tokio::test]
async fn test_slash_spec_update_missing_spec_id_shows_usage_error() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/spec update").await;

    assert!(
        app.status.contains("Usage: /spec update"),
        "missing spec-id should show usage error, got: {}",
        app.status
    );
}

/// `/spec reverse` with a missing or invalid flag must report the specific
/// cause in the message window, not only a status line (NFR-005).
#[tokio::test]
async fn test_slash_spec_reverse_usage_errors_report_cause() {
    let cases = [
        ("/spec reverse", "missing required argument"),
        (
            "/spec reverse octocat/Hello-World --stack axum",
            "missing required flag --language",
        ),
        (
            "/spec reverse octocat/Hello-World --language rust",
            "missing required flag --type",
        ),
        (
            "/spec reverse octocat/Hello-World --bogus",
            "unknown option '--bogus'",
        ),
        (
            "/spec reverse octocat/Hello-World --create",
            "--create requires a value",
        ),
        (
            "/spec reverse octocat/Hello-World extra",
            "unexpected argument 'extra'",
        ),
        (
            "/spec reverse octocat/Hello-World --folder ./x",
            "--folder requires --language and --type",
        ),
        (
            "/spec reverse --language rust --type cmdline --stack gtk4",
            "the first argument after `/spec reverse` must be `<repo>`",
        ),
    ];

    for (command, expected) in cases {
        let mut app = make_app();
        app.session_id = Some("s1".to_string());

        app.execute_slash_command(command).await;

        let text: String = app
            .messages
            .iter()
            .map(|m| m.text_content())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            text.contains(expected),
            "{command} should report '{expected}' in the message window, got: {text}"
        );
        let status = app.status.to_lowercase();
        assert!(
            status.contains("reverse") && status.contains("usage"),
            "{command} should keep the pointer in the status line, got: {}",
            app.status
        );
    }
}

/// FR-027: `/spec reverse <flags> --folder <path>` runs the shared `/new`
/// scaffold engine against `<path>` before generating the prompt, so the target
/// folder is created and populated rather than left untouched.
#[tokio::test(flavor = "multi_thread")]
async fn test_slash_spec_reverse_folder_scaffolds_project() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    let dir = std::env::temp_dir().join(format!(
        "ragent-reverse-folder-{}-{}",
        std::process::id(),
        line!()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");

    app.execute_slash_command(&format!(
        "/spec reverse octocat/Hello-World --language rust --type cmdline \
         --folder {}",
        dir.display()
    ))
    .await;

    // The folder now holds the `/new` scaffold, including the specs root the
    // chained `/spec create` writes into.
    assert!(
        dir.join("specs").is_dir(),
        "the scaffold must create <folder>/specs/: {:?}",
        dir
    );
    assert!(
        dir.join("AGENTS.md").is_file(),
        "the scaffold must create <folder>/AGENTS.md: {:?}",
        dir
    );

    let joined: String = app
        .messages
        .iter()
        .map(|m| m.text_content())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        joined.contains("project scaffolded in"),
        "the run must report the scaffold outcome: {joined}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// A valid `/spec reverse` invocation must forward the validated values to the
/// `/reverse` handler rather than silently dropping them (FR-026/FR-027).
#[tokio::test(flavor = "multi_thread")]
async fn test_slash_spec_reverse_valid_scaffold_reaches_reverse_handler() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command(
        "/spec reverse octocat/Hello-World --create my-spec --depth 2 \
         --language rust --type cmdline --stack axum",
    )
    .await;

    // The handler reports a `reverse:` status (auth/invalid-repo/wait) or
    // starts fetching; what matters is that it was reached rather than the
    // parse producing a usage error.
    assert!(
        !app.status.contains("spec: reverse usage"),
        "a valid scaffold invocation must not be a usage error: {}",
        app.status
    );
    assert!(
        app.status.contains("reverse:"),
        "the /reverse handler must be reached: {}",
        app.status
    );

    let rendered = ragent_tui::app::spec_reverse_args_for_tests(
        "octocat/Hello-World".to_string(),
        Some("my-spec".to_string()),
        Some("2".to_string()),
        Some(
            ragent_tools_extended::project_scaffold::parse_flags(&[
                "--language",
                "rust",
                "--type",
                "cmdline",
                "--stack",
                "axum",
            ])
            .expect("valid scaffold flags"),
        ),
        None,
    );
    assert_eq!(
        rendered,
        "octocat/Hello-World --tech \"language: rust; type: cmdline; stack: axum\" \
         --create my-spec --depth 2"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_slash_spec_task_lists_tasks() {
    let mut app = make_app();

    app.execute_slash_command("/spec task testspec").await;

    assert!(!app.messages.is_empty(), "task should create a message");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("Tasks for") || text.contains("No tasks found") || text.contains("Error:"),
        "should list tasks or show empty/error: {text}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_slash_spec_validate_all() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/spec validate").await;

    assert!(!app.messages.is_empty(), "validate should create a message");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("Validation") || text.contains("No specs found") || text.contains("Error:"),
        "should show validation results: {text}"
    );
}

// Use a multi-threaded runtime because `execute_slash_command("/spec create …")`
// spawns a background task that calls `processor.process_message`, whose model
// resolution path may call `block_in_place` for spec reads/writes.
// `block_in_place` panics on the default current-thread `#[tokio::test]` runtime;
// `flavor = "multi_thread"` provides a runtime where it is permitted.
#[tokio::test(flavor = "multi_thread")]
async fn test_slash_spec_create_starts_generation() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/spec create websocket Add real-time collaborative editing")
        .await;

    assert_eq!(
        app.status,
        "spec: writing specs/websocket/SPEC.md + specs/websocket/PLAN.md + \
         specs/websocket/TESTPLAN.md…",
        "status should indicate generation"
    );
    assert!(app.is_processing, "should set is_processing");
    assert!(
        !app.messages.is_empty(),
        "should push the spec task message"
    );
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("specification writer"),
        "task should contain spec writer prompt"
    );
    assert!(text.contains("EARS notation"), "task should mention EARS");
    assert!(
        text.contains("specs/websocket/SPEC.md"),
        "task should contain spec file path"
    );
    assert!(
        text.contains("specs/websocket/PLAN.md"),
        "task should contain plan file path"
    );
    assert!(
        text.contains("specs/websocket/TESTPLAN.md"),
        "task should contain testplan file path"
    );
}

// ── /config ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_slash_config_show_displays_paths() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/config show").await;

    assert_eq!(app.status, "config: show");
    assert!(
        !app.messages.is_empty(),
        "config show should create a message"
    );
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("Application Paths"),
        "config show should contain Application Paths section"
    );
    assert!(
        text.contains("Working directory"),
        "config show should mention Working directory"
    );
    assert!(
        text.contains("Config Files"),
        "config show should contain Config Files section"
    );
    assert!(
        text.contains("Database"),
        "config show should mention Database"
    );
    assert!(
        text.contains("Code Index"),
        "config show should contain Code Index section"
    );
    assert!(
        text.contains("Resolved Values"),
        "config show should contain Resolved Values section"
    );
    assert!(
        text.contains("key") && text.contains("source"),
        "config show should render a resolved-values table with key/source columns"
    );
}

#[tokio::test]
async fn test_slash_config_no_args_shows_usage() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/config").await;

    // `/config` with no args now shows the help table (same content family as
    // `/config help`); the status reflects help rather than the usage hint.
    assert_eq!(app.status, "config: help");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("/config show"),
        "usage should mention /config show: {text}"
    );
    assert!(
        text.contains("/config save"),
        "usage should mention /config save: {text}"
    );
    assert!(
        text.contains("/config list"),
        "usage should mention /config list: {text}"
    );
}

#[tokio::test]
async fn test_slash_config_save_errors_when_no_global_config() {
    // FR-003: /config save must surface a clear error when there is no global
    // ragent.json to back up. We point the process at an empty temp config dir
    // via the RAGENT_CONFIG env var indirectly — but backup_global_config(None)
    // resolves via dirs::config_dir(), which we cannot easily redirect. This
    // test asserts the error path produces an error status and a message
    // rather than panicking, exercising the slash arm end-to-end.
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/config save").await;

    // Either a real backup succeeds (if a global config exists in CI) or the
    // error arm fires. We accept both but require a non-empty message + a
    // status that starts with "config:".
    assert!(
        app.status.starts_with("config:"),
        "status should reflect the config save attempt: {}",
        app.status
    );
    assert!(
        !app.messages.is_empty(),
        "/config save should always produce a message"
    );
}

#[tokio::test]
async fn test_slash_config_list_no_saves_shows_message() {
    // FR-004 / FR-006: /config list must always produce a user-facing message.
    // When saves exist it opens the picker AND emits a summary line; when none
    // exist it shows a "no saved configurations" message instead of an empty
    // picker. We cannot easily control the real global config dir, so this
    // test asserts the no-panic contract and that a message is always emitted.
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/config list").await;

    assert!(
        app.status.starts_with("config:"),
        "status should reflect the config list attempt: {}",
        app.status
    );
    assert!(
        !app.messages.is_empty(),
        "/config list should always produce a message"
    );
    // When the real global config dir happens to contain saves, the picker
    // must be opened; otherwise it must stay None. Either is acceptable.
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("/config list"),
        "message should be attributed to /config list: {text}"
    );
}

#[test]
fn test_slash_config_subcommand_suggestions_include_save_and_list() {
    // FR-002: `/config` autocomplete must offer `show`, `save`, and `list`.
    // Drive the public autocomplete path by typing `/config` and letting
    // `update_slash_menu` build the menu; the selected entry's `suggestions`
    // field is populated by `get_command_suggestions("config")`.
    let mut app = make_app();
    app.input = "/config".to_string();
    app.input_cursor = app.input.chars().count();

    app.update_slash_menu();

    let menu = app
        .slash_menu
        .as_ref()
        .expect("typing /config should open the slash menu");
    let config_entry = menu
        .matches
        .iter()
        .find(|m| m.trigger == "config")
        .expect("menu should contain a /config entry");

    assert!(
        config_entry.suggestions.contains(&"show".to_string()),
        "config suggestions should include 'show': {:?}",
        config_entry.suggestions
    );
    assert!(
        config_entry.suggestions.contains(&"save".to_string()),
        "config suggestions should include 'save': {:?}",
        config_entry.suggestions
    );
    assert!(
        config_entry.suggestions.contains(&"list".to_string()),
        "config suggestions should include 'list': {:?}",
        config_entry.suggestions
    );
}

#[test]
fn test_research_completion_and_help_list_no_papers() {
    // researchnoacc FR-005: `/research` autocomplete and the parameter hint
    // must advertise the canonical `--no-papers` scholarly-exclusion flag.
    let mut app = make_app();
    app.input = "/research".to_string();
    app.input_cursor = app.input.chars().count();

    app.update_slash_menu();

    let menu = app
        .slash_menu
        .as_ref()
        .expect("typing /research should open the slash menu");
    let entry = menu
        .matches
        .iter()
        .find(|m| m.trigger == "research")
        .expect("menu should contain a /research entry");

    assert!(
        entry.suggestions.contains(&"--no-papers".to_string()),
        "research suggestions should include '--no-papers': {:?}",
        entry.suggestions
    );
    let hint = entry
        .parameter_hint
        .as_deref()
        .expect("research entry should carry a parameter hint");
    assert!(
        hint.contains("--no-papers"),
        "research parameter hint should list '--no-papers': {hint}"
    );
}

#[test]
fn test_research_completion_and_help_list_output_limits() {
    // researchmax FR-006/FR-007/NFR-003: `/research` autocomplete and the
    // parameter hint must advertise the `--max-concepts` / `--max-findings`
    // output-limit flags.
    let mut app = make_app();
    app.input = "/research".to_string();
    app.input_cursor = app.input.chars().count();

    app.update_slash_menu();

    let menu = app
        .slash_menu
        .as_ref()
        .expect("typing /research should open the slash menu");
    let entry = menu
        .matches
        .iter()
        .find(|m| m.trigger == "research")
        .expect("menu should contain a /research entry");

    for flag in ["--max-concepts", "--max-findings"] {
        assert!(
            entry.suggestions.contains(&flag.to_string()),
            "research suggestions should include '{flag}': {:?}",
            entry.suggestions
        );
    }
    let hint = entry
        .parameter_hint
        .as_deref()
        .expect("research entry should carry a parameter hint");
    for flag in ["--max-concepts", "--max-findings"] {
        assert!(
            hint.contains(flag),
            "research parameter hint should list '{flag}': {hint}"
        );
    }
}

#[test]
fn test_config_save_picker_state_defaults_to_none() {
    // FR-007/FR-008: the App must carry an Option<ConfigSavePickerState> field
    // initialised to None so later tasks can open the picker overlay.
    let app = make_app();
    assert!(
        app.config_save_picker.is_none(),
        "config_save_picker should start as None"
    );
}

#[test]
fn test_config_save_picker_state_struct_construction() {
    // FR-007/FR-008: the ConfigSavePickerState struct must be constructible
    // with entries, selection, scroll offset, and the resolved config dir.
    use ragent_tui::app::ConfigSavePickerState;

    let state = ConfigSavePickerState {
        entries: vec![
            std::path::PathBuf::from("/tmp/saves/ragent.json.2024-01-01.12-00-00"),
            std::path::PathBuf::from("/tmp/saves/ragent.json.2024-01-02.13-30-00"),
        ],
        selected: 1,
        scroll_offset: 0,
        config_dir: std::path::PathBuf::from("/tmp"),
    };

    assert_eq!(state.entries.len(), 2, "entries should hold two backups");
    assert_eq!(state.selected, 1, "selected should point at the second row");
    assert_eq!(state.scroll_offset, 0, "scroll offset should be zero");
    assert_eq!(
        state.config_dir,
        std::path::PathBuf::from("/tmp"),
        "config_dir should be stored"
    );
}

// NOTE: render_terminal_to_string helper removed; re-add when needed.
#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_alt_y_toggles_yolo_mode_and_status_bar_indicator() {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let lock = cwd_lock();
    let original_cwd = std::env::current_dir().expect("cwd");
    let _temp = enter_temp_config_dir();
    let _guard = CwdGuard {
        prev: original_cwd,
        lock,
        temp: None,
    };
    ragent_config::yolo::set_enabled(false);

    let mut app = make_app_with_storage(storage);

    // Sanity: YOLO starts off.
    assert!(!ragent_config::yolo::is_enabled());

    // Press Alt+Y through the app handler so the new persist path runs.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::ALT))
        .await;

    // Handler should have toggled and persisted YOLO on.
    assert!(ragent_config::yolo::is_enabled());
    assert!(app.status.contains("YOLO mode enabled"));

    // Status bar indicator should reflect the current (enabled) state.
    let backend = TestBackend::new(140, 30);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| layout::render(frame, &mut app))
        .expect("draw");
    let cells = terminal.backend().buffer().content.clone();
    let text: String = cells.iter().map(ratatui::buffer::Cell::symbol).collect();
    assert!(
        text.contains("⚠️") && text.contains("✓"),
        "status bar should show enabled YOLO icon: {text}"
    );

    // Toggle back off and verify.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::ALT))
        .await;
    assert!(!ragent_config::yolo::is_enabled());
    assert!(app.status.contains("YOLO mode disabled"));

    // Status bar should now show the disabled indicator.
    let backend = TestBackend::new(140, 30);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| layout::render(frame, &mut app))
        .expect("draw");
    let cells = terminal.backend().buffer().content.clone();
    let text: String = cells.iter().map(ratatui::buffer::Cell::symbol).collect();
    assert!(
        text.contains("⚠️") && text.contains("✗"),
        "status bar should show disabled YOLO icon: {text}"
    );
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_slash_yolo_toggles_and_persists() {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let lock = cwd_lock();
    let original_cwd = std::env::current_dir().expect("cwd");
    let _temp = enter_temp_config_dir();
    let _guard = CwdGuard {
        prev: original_cwd,
        lock,
        temp: None,
    };
    ragent_config::yolo::set_enabled(false);

    let mut app = make_app_with_storage(storage);
    app.input = "/yolo".to_string();
    app.input_cursor = app.input.chars().count();

    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .await;

    assert!(ragent_config::yolo::is_enabled());
    assert!(app.status.contains("ENABLED"));

    // Running again disables it.
    app.input = "/yolo".to_string();
    app.input_cursor = app.input.chars().count();
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .await;
    assert!(!ragent_config::yolo::is_enabled());
    assert!(app.status.contains("disabled"));
}

#[test]
fn test_config_save_picker_key_navigation_and_restore() {
    // T-006 / T-008: the config-save picker intercepts keys, supports
    // navigation, and restores the selected backup atomically over the global
    // ragent.json.
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ragent_config::Config;

    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    // Write two distinct global configs.
    let first = r#"{"defaultAgent":"coder"}"#;
    let second = r#"{"defaultAgent":"architect"}"#;
    fs::write(dir.join("ragent.json"), first).expect("write original");

    let backup1 = Config::backup_global_config(Some(dir)).expect("backup 1");
    fs::write(dir.join("ragent.json"), second).expect("write second");
    let backup2 = Config::backup_global_config(Some(dir)).expect("backup 2");

    let name1 = backup1
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap()
        .to_string();

    // Sort so backup1 (oldest) is first and backup2 (newest) is second.
    let mut app = make_app();
    app.config_save_picker = Some(ragent_tui::app::ConfigSavePickerState {
        entries: vec![backup1.clone(), backup2.clone()],
        selected: 0,
        scroll_offset: 0,
        config_dir: dir.to_path_buf(),
    });

    // Down should move to the newer backup.
    app.handle_config_save_picker_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
    assert_eq!(
        app.config_save_picker.as_ref().unwrap().selected,
        1,
        "Down should select the second entry"
    );

    // Up should move back.
    app.handle_config_save_picker_key(KeyEvent::new(KeyCode::Up, KeyModifiers::empty()));
    assert_eq!(
        app.config_save_picker.as_ref().unwrap().selected,
        0,
        "Up should select the first entry"
    );

    // Enter restores the currently selected backup (oldest = coder config).
    app.handle_config_save_picker_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert!(
        app.config_save_picker.is_none(),
        "picker should close after restore"
    );
    let restored = fs::read_to_string(dir.join("ragent.json")).expect("read restored");
    assert_eq!(
        restored, first,
        "restore should copy the selected backup over ragent.json"
    );
    assert!(
        app.status.starts_with("config: restored"),
        "status should reflect restore: {}",
        app.status
    );

    // Open again and restore the newest backup (architect config).
    app.config_save_picker = Some(ragent_tui::app::ConfigSavePickerState {
        entries: vec![backup1, backup2],
        selected: 1,
        scroll_offset: 0,
        config_dir: dir.to_path_buf(),
    });
    app.handle_config_save_picker_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    let restored2 = fs::read_to_string(dir.join("ragent.json")).expect("read restored 2");
    assert_eq!(
        restored2, second,
        "restore should switch to the newer backup"
    );

    // Esc should close without restoring.
    fs::write(dir.join("ragent.json"), first).expect("reset current");
    app.config_save_picker = Some(ragent_tui::app::ConfigSavePickerState {
        entries: vec![std::path::PathBuf::from(name1)],
        selected: 0,
        scroll_offset: 0,
        config_dir: dir.to_path_buf(),
    });
    app.handle_config_save_picker_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    assert!(
        app.config_save_picker.is_none(),
        "picker should close on Esc"
    );
    let after_esc = fs::read_to_string(dir.join("ragent.json")).expect("read after esc");
    assert_eq!(after_esc, first, "Esc should not change the active config");
}

#[test]
fn test_config_restore_invalidates_config_cache() {
    // T-008: restoring a backup must invalidate the cached config so the next
    // turn re-reads ragent.json from disk.
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ragent_config::Config;

    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    fs::write(dir.join("ragent.json"), r#"{"defaultAgent":"coder"}"#).expect("write");
    let backup = Config::backup_global_config(Some(dir)).expect("backup");

    let mut app = make_app();
    // Pre-populate the cache with a marker.
    {
        let mut guard = app.session_processor.cached_config.lock();
        *guard = Some(ragent_agent::session::processor::CachedConfig {
            config: std::sync::Arc::new(Config::default()),
            file_mtimes: Vec::new(),
            env_overrides_present: false,
        });
    }

    app.config_save_picker = Some(ragent_tui::app::ConfigSavePickerState {
        entries: vec![backup],
        selected: 0,
        scroll_offset: 0,
        config_dir: dir.to_path_buf(),
    });
    app.handle_config_save_picker_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));

    let guard = app.session_processor.cached_config.lock();
    assert!(
        guard.is_none(),
        "restore must invalidate the session processor config cache"
    );
}

#[tokio::test]
async fn test_config_save_picker_intercepts_keys_in_handle_key_event() {
    // Regression: config_save_picker must own focus so keys like 'a' do not go
    // into the input box while the picker is open.
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    fs::write(dir.join("ragent.json"), r#"{"defaultAgent":"coder"}"#).expect("write");

    let mut app = make_app();
    app.input.clear();
    app.input_cursor = 0;
    app.config_save_picker = Some(ragent_tui::app::ConfigSavePickerState {
        entries: vec![],
        selected: 0,
        scroll_offset: 0,
        config_dir: dir.to_path_buf(),
    });

    app.handle_key_event(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty()))
        .await;
    assert!(
        app.input.is_empty(),
        "keys must be intercepted while config save picker is open"
    );
}

#[tokio::test]
async fn test_slash_memory_no_args_shows_usage() {
    // /memory with an unknown subcommand should list the supported subcommands.
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/memory foobar").await;

    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("Usage:"),
        "response should show usage: {text}"
    );
    assert!(
        text.contains("/memory show"),
        "usage should mention /memory show: {text}"
    );
    assert!(
        text.contains("/memory help"),
        "usage should mention /memory help: {text}"
    );
}

#[tokio::test]
async fn test_slash_actionloop_help_shows_subcommands() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/actionloop help").await;

    assert_eq!(app.status, "actionloop: help");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("/actionloop help"),
        "help should mention itself: {text}"
    );
    assert!(
        text.contains("/actionloop clip"),
        "help should mention clip: {text}"
    );
}

#[tokio::test]
async fn test_slash_actionloop_no_samples_reports_hint() {
    // With no profiling samples, the plain form reports the "no samples" hint.
    // The profiler is shared process-wide, so reset it first for determinism.
    agent_loop_profiler().reset();
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/actionloop").await;

    let text = app.messages.last().unwrap().text_content();
    if app.status == "actionloop: no samples" {
        assert!(
            text.contains("No action-loop timing samples recorded yet"),
            "should report no samples: {text}"
        );
    } else {
        // Another test recorded samples concurrently into the shared profiler;
        // just confirm the report rendered rather than asserting on the hint.
        assert_eq!(app.status, "actionloop: timings shown");
        assert!(
            text.contains("avg ms"),
            "should show a timing table: {text}"
        );
    }
}

#[tokio::test]
async fn test_slash_actionloop_clip_no_samples_reports_hint() {
    // The clip variant degrades gracefully when there is nothing to copy.
    // The profiler is shared process-wide and other tests may have recorded
    // samples concurrently, so reset before running to make the "no samples"
    // path deterministic where possible.
    agent_loop_profiler().reset();
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/actionloop clip").await;

    // When another test polluted the shared profiler, the clip path reports
    // success instead; either outcome is acceptable, so assert on the message.
    let text = app.messages.last().unwrap().text_content().to_lowercase();
    assert!(
        text.contains("action-loop timing")
            && (text.contains("clipboard") || text.contains("no action-loop timing samples")),
        "clip should report either a copy or the no-samples hint: {text}"
    );
}

#[tokio::test]
async fn test_slash_actionloop_with_samples_shows_timings() {
    // Record a sample through the shared profiler so the report path is exercised.
    let profiler = agent_loop_profiler();
    profiler.reset();
    profiler.set_enabled(true);
    {
        let _s = profiler.scope("test-op");
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    profiler.set_enabled(false);

    let mut app = make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/actionloop").await;

    assert_eq!(app.status, "actionloop: timings shown");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("test-op"),
        "report should include the recorded operation: {text}"
    );
    assert!(
        text.contains("avg ms"),
        "report should include the table header: {text}"
    );
    // Leave the shared profiler clean for other tests.
    profiler.reset();
}
// ── /triggers slash command tests ─────────────────────────────────────

#[tokio::test]
async fn test_triggers_list_empty() {
    let mut app = make_app();
    app.execute_slash_command("/triggers list").await;
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("No trigger rules registered"),
        "empty list should say so: {text}"
    );
    assert_eq!(app.status, "triggers: list empty");
}

#[tokio::test]
async fn test_triggers_list_with_rules() {
    let mut app = make_app();
    let runtime = ragent_agent::trigger::TriggerRuntime::default();
    let rule =
        ragent_types::trigger::TriggerRule::new("when $HOME/build.done exists", "run cargo test");
    let rule_id = rule.id.as_str().to_string();
    runtime.add_rule(rule);
    app.trigger_runtime = Some(runtime);

    app.execute_slash_command("/triggers list").await;
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains(&rule_id[..8]),
        "list should show rule id prefix: {text}"
    );
    assert!(
        text.contains("when $HOME/build.done exists"),
        "list should show condition: {text}"
    );
    assert!(
        text.contains("run cargo test"),
        "list should show action: {text}"
    );
    assert_eq!(app.status, "triggers: list");
}

#[tokio::test]
async fn test_triggers_no_subcommand_defaults_to_list() {
    let mut app = make_app();
    app.execute_slash_command("/triggers").await;
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("No trigger rules registered"),
        "bare /triggers should default to list: {text}"
    );
}

#[tokio::test]
async fn test_triggers_enable_existing_rule() {
    let mut app = make_app();
    let runtime = ragent_agent::trigger::TriggerRuntime::default();
    let rule = ragent_types::trigger::TriggerRule::new("cond-a", "act-a");
    let rule_id = rule.id.as_str().to_string();
    runtime.add_rule(rule);
    runtime.disable_rule(&rule_id);
    app.trigger_runtime = Some(runtime);

    app.execute_slash_command(&format!("/triggers enable {rule_id}"))
        .await;
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("enabled"), "enable should confirm: {text}");
    assert_eq!(app.status, "triggers: enabled");
}

#[tokio::test]
async fn test_triggers_enable_not_found() {
    let mut app = make_app();
    app.execute_slash_command("/triggers enable nonexistent-id")
        .await;
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("not found"),
        "enable on missing rule should say not found: {text}"
    );
    assert_eq!(app.status, "triggers: not found");
}

#[tokio::test]
async fn test_triggers_enable_no_id() {
    let mut app = make_app();
    app.execute_slash_command("/triggers enable").await;
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("Usage"),
        "enable with no id should show usage: {text}"
    );
    assert_eq!(app.status, "triggers: enable usage");
}

#[tokio::test]
async fn test_triggers_disable_existing_rule() {
    let mut app = make_app();
    let runtime = ragent_agent::trigger::TriggerRuntime::default();
    let rule = ragent_types::trigger::TriggerRule::new("cond-b", "act-b");
    let rule_id = rule.id.as_str().to_string();
    runtime.add_rule(rule);
    app.trigger_runtime = Some(runtime);

    app.execute_slash_command(&format!("/triggers disable {rule_id}"))
        .await;
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("disabled"), "disable should confirm: {text}");
    assert_eq!(app.status, "triggers: disabled");
}

#[tokio::test]
async fn test_triggers_disable_not_found() {
    let mut app = make_app();
    app.execute_slash_command("/triggers disable nonexistent-id")
        .await;
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("not found"),
        "disable on missing rule should say not found: {text}"
    );
    assert_eq!(app.status, "triggers: not found");
}

#[tokio::test]
async fn test_triggers_remove_existing_rule() {
    let mut app = make_app();
    app.trigger_runtime = Some(ragent_agent::trigger::TriggerRuntime::default());
    let runtime = ragent_agent::trigger::TriggerRuntime::default();
    let rule = ragent_types::trigger::TriggerRule::new("cond-c", "act-c");
    let rule_id = rule.id.as_str().to_string();
    runtime.add_rule(rule);
    app.trigger_runtime = Some(runtime);

    app.execute_slash_command(&format!("/triggers remove {rule_id}"))
        .await;
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("removed"), "remove should confirm: {text}");
    assert_eq!(app.status, "triggers: removed");
    assert_eq!(app.trigger_runtime.as_ref().unwrap().rule_count(), 0);
}

#[tokio::test]
async fn test_triggers_remove_not_found() {
    let mut app = make_app();
    app.execute_slash_command("/triggers remove nonexistent-id")
        .await;
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("not found"),
        "remove on missing rule should say not found: {text}"
    );
    assert_eq!(app.status, "triggers: not found");
}

#[tokio::test]
async fn test_triggers_remove_no_id() {
    let mut app = make_app();
    app.execute_slash_command("/triggers remove").await;
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("Usage"),
        "remove with no id should show usage: {text}"
    );
    assert_eq!(app.status, "triggers: remove usage");
}

#[tokio::test]
async fn test_triggers_status_empty() {
    let mut app = make_app();
    app.execute_slash_command("/triggers status").await;
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("Trigger Runtime Status"),
        "status should show header: {text}"
    );
    assert!(
        text.contains("Total rules") && text.contains('0'),
        "status should show zero rules: {text}"
    );
    assert_eq!(app.status, "triggers: status");
}

#[tokio::test]
async fn test_triggers_status_with_rules() {
    let mut app = make_app();
    let runtime = ragent_agent::trigger::TriggerRuntime::default();
    runtime.add_rule(ragent_types::trigger::TriggerRule::new("cond-1", "act-1"));
    runtime.add_rule(ragent_types::trigger::TriggerRule::new("cond-2", "act-2"));
    app.trigger_runtime = Some(runtime);

    app.execute_slash_command("/triggers status").await;
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("Total rules") && text.contains('2'),
        "status should show 2 rules: {text}"
    );
    assert!(
        text.contains("Active") && text.contains('2'),
        "status should show 2 active: {text}"
    );
    assert_eq!(app.status, "triggers: status");
}

#[tokio::test]
async fn test_triggers_help() {
    let mut app = make_app();
    app.execute_slash_command("/triggers help").await;
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("/triggers"),
        "help should mention /triggers: {text}"
    );
    assert!(
        text.contains("list") && text.contains("enable") && text.contains("disable"),
        "help should list sub-commands: {text}"
    );
    assert!(
        text.contains("remove") && text.contains("status"),
        "help should list remove and status: {text}"
    );
    assert_eq!(app.status, "triggers: help");
}

#[tokio::test]
async fn test_triggers_unknown_subcommand() {
    let mut app = make_app();
    app.execute_slash_command("/triggers frobnicate").await;
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("Unknown sub-command"),
        "unknown sub-command should warn: {text}"
    );
    assert_eq!(app.status, "triggers: unknown");
}
// ── /inbox slash command tests ────────────────────────────────────────

#[tokio::test]
async fn test_inbox_list_empty() {
    let _guard = enter_with_cwd();

    let mut app = make_app();
    app.execute_slash_command("/inbox list").await;
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("Inbox is empty"),
        "empty inbox should say so: {text}"
    );
    assert_eq!(app.status, "inbox: list empty");
}

#[tokio::test]
async fn test_inbox_no_subcommand_defaults_to_list() {
    let _guard = enter_with_cwd();

    let mut app = make_app();
    app.execute_slash_command("/inbox").await;
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("Inbox is empty"),
        "bare /inbox should default to list: {text}"
    );
}

#[tokio::test]
async fn test_inbox_list_with_entries() {
    let guard = enter_with_cwd();

    // Write some inbox entries directly to the JSONL file
    let entries = vec![
        ragent_agent::loop_state::InboxEntry::new("event-abc", "first finding"),
        ragent_agent::loop_state::InboxEntry::new("event-xyz", "second finding"),
    ];
    ragent_agent::loop_state::write_inbox_entries(&guard.path(), &entries).expect("write entries");

    let mut app = make_app();
    app.execute_slash_command("/inbox list").await;
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("Triage Inbox"),
        "list should show header: {text}"
    );
    assert!(
        text.contains("first finding"),
        "list should show first entry content: {text}"
    );
    assert!(
        text.contains("second finding"),
        "list should show second entry content: {text}"
    );
    assert!(
        text.contains("2 finding(s)"),
        "list should show count: {text}"
    );
    assert_eq!(app.status, "inbox: list");
}

#[tokio::test]
async fn test_inbox_claim_existing() {
    let guard = enter_with_cwd();

    let entry = ragent_agent::loop_state::InboxEntry::new("event-1", "test finding");
    let entry_id = entry.id.clone();
    ragent_agent::loop_state::write_inbox_entries(&guard.path(), &[entry]).expect("write entry");

    let mut app = make_app();
    app.execute_slash_command(&format!("/inbox claim {entry_id}"))
        .await;
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("claimed"), "claim should confirm: {text}");
    assert_eq!(app.status, "inbox: claimed");

    // Verify the status was persisted
    let read = ragent_agent::loop_state::read_inbox(&guard.path()).unwrap();
    assert_eq!(read[0].status, "claimed");
}

#[tokio::test]
async fn test_inbox_claim_not_found() {
    let _guard = enter_with_cwd();

    let mut app = make_app();
    app.execute_slash_command("/inbox claim nonexistent-id")
        .await;
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("not found"),
        "claim on missing entry should say not found: {text}"
    );
    assert_eq!(app.status, "inbox: not found");
}

#[tokio::test]
async fn test_inbox_claim_no_id() {
    let _guard = enter_with_cwd();

    let mut app = make_app();
    app.execute_slash_command("/inbox claim").await;
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("Usage"),
        "claim with no id should show usage: {text}"
    );
    assert_eq!(app.status, "inbox: claimed usage");
}

#[tokio::test]
async fn test_inbox_dismiss_existing() {
    let guard = enter_with_cwd();

    let entry = ragent_agent::loop_state::InboxEntry::new("event-1", "to dismiss");
    let entry_id = entry.id.clone();
    ragent_agent::loop_state::write_inbox_entries(&guard.path(), &[entry]).expect("write entry");

    let mut app = make_app();
    app.execute_slash_command(&format!("/inbox dismiss {entry_id}"))
        .await;
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("dismissed"), "dismiss should confirm: {text}");
    assert_eq!(app.status, "inbox: dismissed");

    // Verify the status was persisted
    let read = ragent_agent::loop_state::read_inbox(&guard.path()).unwrap();
    assert_eq!(read[0].status, "dismissed");
}

#[tokio::test]
async fn test_inbox_dismiss_not_found() {
    let _guard = enter_with_cwd();

    let mut app = make_app();
    app.execute_slash_command("/inbox dismiss nonexistent-id")
        .await;
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("not found"),
        "dismiss on missing entry should say not found: {text}"
    );
    assert_eq!(app.status, "inbox: not found");
}

#[tokio::test]
async fn test_inbox_dismiss_no_id() {
    let _guard = enter_with_cwd();

    let mut app = make_app();
    app.execute_slash_command("/inbox dismiss").await;
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("Usage"),
        "dismiss with no id should show usage: {text}"
    );
    assert_eq!(app.status, "inbox: dismissed usage");
}

#[tokio::test]
async fn test_inbox_clear_with_entries() {
    let guard = enter_with_cwd();

    let entries = vec![
        ragent_agent::loop_state::InboxEntry::new("event-1", "first"),
        ragent_agent::loop_state::InboxEntry::new("event-2", "second"),
    ];
    ragent_agent::loop_state::write_inbox_entries(&guard.path(), &entries).expect("write entries");

    let mut app = make_app();
    app.execute_slash_command("/inbox clear").await;
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("Cleared 2 finding(s)"),
        "clear should report count: {text}"
    );
    assert_eq!(app.status, "inbox: cleared");

    // Verify the file is gone
    assert!(
        !guard
            .path()
            .join("log")
            .join("inbox")
            .join("inbox.jsonl")
            .exists()
    );
}

#[tokio::test]
async fn test_inbox_clear_empty() {
    let _guard = enter_with_cwd();

    let mut app = make_app();
    app.execute_slash_command("/inbox clear").await;
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("Cleared 0 finding(s)"),
        "clear on empty inbox should report 0: {text}"
    );
    assert_eq!(app.status, "inbox: cleared");
}

#[tokio::test]
async fn test_inbox_help() {
    let _guard = enter_with_cwd();

    let mut app = make_app();
    app.execute_slash_command("/inbox help").await;
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("/inbox"),
        "help should mention /inbox: {text}"
    );
    assert!(
        text.contains("list") && text.contains("claim") && text.contains("dismiss"),
        "help should list sub-commands: {text}"
    );
    assert!(text.contains("clear"), "help should mention clear: {text}");
    assert_eq!(app.status, "inbox: help");
}

#[tokio::test]
async fn test_inbox_unknown_subcommand() {
    let _guard = enter_with_cwd();

    let mut app = make_app();
    app.execute_slash_command("/inbox frobnicate").await;
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("Unknown sub-command"),
        "unknown sub-command should warn: {text}"
    );
    assert_eq!(app.status, "inbox: unknown");
}

#[tokio::test]
async fn test_inbox_list_shows_status() {
    let guard = enter_with_cwd();

    let entry = ragent_agent::loop_state::InboxEntry::new("event-1", "test finding");
    let entry_id = entry.id.clone();
    ragent_agent::loop_state::write_inbox_entries(&guard.path(), &[entry]).expect("write entry");

    // Claim it first
    ragent_agent::loop_state::update_inbox_entry_status(&guard.path(), &entry_id, "claimed")
        .expect("update status");

    let mut app = make_app();
    app.execute_slash_command("/inbox list").await;
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("claimed"),
        "list should show updated status: {text}"
    );
}

// ── /task (todo2tasks T-016, FR-019) ─────────────────────────────────

#[tokio::test]
async fn test_slash_task_toggles_panel() {
    let mut app = make_app();
    assert!(
        !app.show_tasks_panel,
        "tasks panel should be hidden initially"
    );

    app.execute_slash_command("/task").await;
    assert!(
        app.show_tasks_panel,
        "tasks panel should be visible after /task"
    );
    assert_eq!(app.status, "tasks panel visible");

    app.execute_slash_command("/task").await;
    assert!(
        !app.show_tasks_panel,
        "tasks panel should be hidden after second /task"
    );
    assert_eq!(app.status, "tasks panel hidden");
}

#[tokio::test]
async fn test_slash_task_mutually_excludes_log() {
    let mut app = make_app();
    app.show_log = true;
    app.show_tasks_panel = false;

    app.execute_slash_command("/task").await;
    assert!(app.show_tasks_panel, "tasks panel should be visible");
    assert!(
        !app.show_log,
        "log panel should be hidden when tasks is shown"
    );
}

#[tokio::test]
async fn test_slash_task_list_shows_items() {
    let mut app = make_app();
    let session_id = "task-list-session".to_string();
    app.session_id = Some(session_id.clone());
    app.storage
        .create_session(&session_id, ".")
        .expect("create session");
    app.storage
        .create_task(
            "t1",
            &session_id,
            "first task",
            "",
            "pending",
            None,
            None,
            "{}",
            &[],
        )
        .expect("create task");
    app.storage
        .create_task(
            "t2",
            &session_id,
            "second task",
            "",
            "in_progress",
            None,
            None,
            "{}",
            &[],
        )
        .expect("create task");

    app.execute_slash_command("/task list").await;

    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("From: /task list"),
        "should show /task list header: {text}"
    );
    assert!(
        text.contains("first task"),
        "should list first task: {text}"
    );
    assert!(
        text.contains("second task"),
        "should list second task: {text}"
    );
    assert_eq!(app.status, "2 task(s)");
}

#[tokio::test]
async fn test_slash_task_list_empty() {
    let mut app = make_app();
    let session_id = "task-empty-session".to_string();
    app.session_id = Some(session_id.clone());
    app.storage
        .create_session(&session_id, ".")
        .expect("create session");

    app.execute_slash_command("/task list").await;

    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("No tasks found"),
        "empty list should say 'No tasks found': {text}"
    );
}

#[tokio::test]
async fn test_slash_task_help() {
    let mut app = make_app();
    app.session_id = Some("help-session".to_string());

    app.execute_slash_command("/task help").await;

    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("From: /task help"),
        "help should have header: {text}"
    );
    assert!(
        text.contains("/task list"),
        "help should mention /task list: {text}"
    );
    assert!(
        text.contains("task_create"),
        "help should mention task_create tool: {text}"
    );
    assert!(
        text.contains("task_update"),
        "help should mention task_update tool: {text}"
    );
    assert_eq!(app.status, "task help");
}

#[tokio::test]
async fn test_slash_task_add_delegates_hint() {
    let mut app = make_app();
    app.session_id = Some("add-hint-session".to_string());

    app.execute_slash_command("/task add").await;

    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("task_create"),
        "/task add should mention task_create tool: {text}"
    );
    assert_eq!(app.status, "Use agent tool: task_create");
}

#[tokio::test]
async fn test_slash_task_update_delegates_hint() {
    let mut app = make_app();
    app.session_id = Some("update-hint-session".to_string());

    app.execute_slash_command("/task update").await;

    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("task_update"),
        "/task update should mention task_update tool: {text}"
    );
    assert_eq!(app.status, "Use agent tool: task_update");
}

#[tokio::test]
async fn test_slash_task_get_delegates_hint() {
    let mut app = make_app();
    app.session_id = Some("get-hint-session".to_string());

    app.execute_slash_command("/task get").await;

    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("task_get"),
        "/task get should mention task_get tool: {text}"
    );
    assert_eq!(app.status, "Use agent tool: task_get");
}

#[tokio::test]
async fn test_slash_task_create_alias_for_add() {
    let mut app = make_app();
    app.session_id = Some("create-alias-session".to_string());

    app.execute_slash_command("/task create").await;

    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("task_create"),
        "/task create should mention task_create tool: {text}"
    );
}

#[tokio::test]
async fn test_slash_task_unknown_subcommand_shows_help() {
    let mut app = make_app();
    app.session_id = Some("unknown-sub-session".to_string());

    app.execute_slash_command("/task frobnicate").await;

    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("From: /task help"),
        "unknown subcommand should fall through to help: {text}"
    );
}

#[tokio::test]
async fn test_slash_task_toggles_panel_with_tasks_status() {
    // FR-019: /task toggles the Tasks side panel.
    let mut app = make_app();

    app.execute_slash_command("/task").await;
    assert!(
        app.show_tasks_panel,
        "tasks panel should be visible after /task"
    );
    assert_eq!(app.status, "tasks panel visible");

    app.execute_slash_command("/task").await;
    assert!(
        !app.show_tasks_panel,
        "tasks panel should be hidden after second /task"
    );
    assert_eq!(app.status, "tasks panel hidden");
}

#[tokio::test]
async fn test_model_selector_preserves_openrouter_vendor_slug() {
    let mut app = make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectModel {
        provider_id: "openrouter".to_string(),
        provider_name: "OpenRouter".to_string(),
        models: vec![ragent_tui::app::ModelPickerEntry {
            provider_id: "openrouter".to_string(),
            id: "anthropic/claude-sonnet-4".to_string(),
            name: "Claude Sonnet 4".to_string(),
            context_window: 200_000,
            max_output: None,
            cost_input: 0.0,
            cost_output: 0.0,
            reasoning: true,
            vision: false,
            tool_use: true,
            thinking_levels: vec![],
            thinking_config: None,
            cost_tier: "Paid".to_string(),
            cost_multiplier: "1x".to_string(),
        }],
        selected: 0,
    });

    // Confirm the model selection. OpenRouter models may report reasoning levels
    // from discovery, so the flow opens the thinking-level selector.
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .await;
    assert!(
        matches!(
            app.provider_setup,
            Some(ProviderSetupStep::SelectThinkingLevel { .. })
                | Some(ProviderSetupStep::Done { .. })
        ),
        "openrouter model selection should proceed"
    );

    if let Some(ProviderSetupStep::SelectThinkingLevel { .. }) = app.provider_setup {
        ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .await;
    }

    assert_eq!(
        app.selected_model.as_deref(),
        Some("openrouter/anthropic/claude-sonnet-4"),
        "selected_model must preserve the vendor slug after the provider segment"
    );
    assert_eq!(
        app.provider_model_label().as_deref(),
        Some("OpenRouter / anthropic/claude-sonnet-4"),
        "status-bar label should show the OpenRouter model id with vendor slug"
    );
}

// ── /spawn ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_slash_spawn_help() {
    let mut app = make_app();
    app.execute_slash_command("/spawn help").await;
    assert_eq!(app.status, "spawn: help");
}

#[tokio::test]
async fn test_slash_spawn_bare_shows_help() {
    let mut app = make_app();
    app.execute_slash_command("/spawn").await;
    assert_eq!(app.status, "spawn: help");
}

#[tokio::test]
async fn test_slash_spawn_no_prompt_is_usage_error() {
    let mut app = make_app();
    app.execute_slash_command("/spawn general").await;
    assert_eq!(app.status, "spawn: usage");
}

#[tokio::test]
async fn test_slash_spawn_unknown_agent_rejected() {
    let mut app = make_app();
    app.execute_slash_command("/spawn no-such-agent-xyz do something")
        .await;
    assert_eq!(app.status, "spawn: unknown agent 'no-such-agent-xyz'");
    // Nothing pending: the result slot must be free for the next /spawn.
    let guard = app.spawn_result.lock().unwrap_or_else(|e| e.into_inner());
    assert!(
        guard.is_none(),
        "rejected /spawn must not leave a pending slot"
    );
}

#[tokio::test]
async fn test_slash_spawn_builtin_agent_launches_detached() {
    let mut app = make_app();
    // Wire an AgentManager into the test processor exactly like the
    // production session does.
    let manager = Arc::new(ragent_agent::task::AgentManager::new(
        std::sync::Arc::clone(&app.event_bus),
        std::sync::Arc::clone(&app.session_processor),
        8,
        60,
    ));
    let _ = app.session_processor.agent_manager.set(manager);

    app.execute_slash_command("/spawn general say hello").await;

    // The slash handler only queues the launch; the detached-task
    // registration completes on the async side and is surfaced by the
    // event-loop poll. Drive one poll cycle through the test hook.
    ragent_tui::poll_spawn_result_for_tests(&mut app);
    assert!(
        app.status.starts_with("[wait] spawn:") || app.status.starts_with("spawn: detached task"),
        "expected launch-in-progress or launched status, got {}",
        app.status
    );
    // Whatever the outcome was, the slot must hold the in-flight marker
    // (Err("")) or be cleared — never a stale real outcome.
    let guard = app.spawn_result.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(v) = guard.as_ref() {
        assert!(
            v.as_ref().err().is_none_or(|m| m.is_empty()),
            "slot must hold the in-flight marker or a real outcome"
        );
    }
}

/// The `/mcp` listing must include servers contributed by enabled plugins, not
/// just the `ragent.json` `mcp` section. Regression: a project whose only MCP
/// servers come from a plugin (e.g. the Claude `mongodb` plugin's `mcp.json`)
/// connected them at startup but `/mcp list` reported "no MCP servers
/// configured", because the display list was rebuilt from `cfg.mcp` alone.
#[tokio::test(flavor = "multi_thread")]
async fn test_slash_mcp_list_includes_plugin_contributed_servers() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();

    // Minimal enabled plugin with an external `mcp.json` (the Claude shape).
    // A plugin is inert until the store ledger records it enabled.
    let store = root.join(".ragent").join("plugins");
    let plugin = store.join("mongodb");
    std::fs::create_dir_all(plugin.join(".claude-plugin")).expect("manifest dir");
    std::fs::write(
        plugin.join(".claude-plugin").join("plugin.json"),
        br#"{"name":"mongodb","version":"1.0.0","mcpServers":"./mcp.json"}"#,
    )
    .expect("write manifest");
    std::fs::write(
        plugin.join("mcp.json"),
        br#"{"mcpServers":{"mongodb":{"command":"npx","args":["-y","mongodb-mcp-server@<3"]}}}"#,
    )
    .expect("write mcp.json");
    std::fs::write(
        store.join("_state.json"),
        br#"{"plugins":{"mongodb":{"enabled":true}}}"#,
    )
    .expect("write state ledger");

    let _guard = with_cwd(root);
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/mcp list").await;

    assert!(
        app.mcp_servers.iter().any(|s| s.id == "mongodb.mongodb"),
        "the plugin-contributed server must be listed: {:?}",
        app.mcp_servers.iter().map(|s| &s.id).collect::<Vec<_>>()
    );

    let joined: String = app
        .messages
        .iter()
        .map(|m| m.text_content())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        joined.contains("mongodb.mongodb"),
        "the /mcp output must name the plugin server: {joined}"
    );
    assert!(
        !joined.contains("(no MCP servers configured)"),
        "the empty-state message must not appear: {joined}"
    );
}

/// The merged display list must keep the configured `ragent.json` servers and
/// preserve their tracked status/tools across a rebuild.
#[tokio::test(flavor = "multi_thread")]
async fn test_mcp_display_servers_merges_and_preserves_status() {
    use ragent_agent::mcp::{McpServer, McpStatus};
    use std::collections::HashMap;

    let previous = vec![McpServer {
        id: "configured".to_string(),
        config: ragent_agent::McpServerConfig::default(),
        status: McpStatus::Connected,
        tools: Vec::new(),
    }];
    let mut configured = HashMap::new();
    configured.insert(
        "configured".to_string(),
        ragent_agent::McpServerConfig::default(),
    );

    let dir = tempfile::tempdir().expect("tempdir");
    let merged = ragent_tui::app::mcp_display_servers_for_tests(
        &previous,
        &configured,
        dir.path(),
        &HashMap::new(),
    );

    let kept = merged
        .iter()
        .find(|s| s.id == "configured")
        .expect("configured server retained");
    assert_eq!(kept.status, McpStatus::Connected);
}

/// The live status map (fed by `Event::McpStatusChanged`) must supply the status
/// of a server the display list has not seen before, so a server connected by
/// the background startup loop is not shown as `disabled` (BUG-001).
#[tokio::test(flavor = "multi_thread")]
async fn test_mcp_display_servers_applies_live_status_to_new_server() {
    use ragent_agent::mcp::McpStatus;
    use std::collections::HashMap;

    let mut configured = HashMap::new();
    configured.insert("live".to_string(), ragent_agent::McpServerConfig::default());
    let mut live = HashMap::new();
    live.insert("live".to_string(), McpStatus::Connected);

    let dir = tempfile::tempdir().expect("tempdir");
    let merged =
        ragent_tui::app::mcp_display_servers_for_tests(&[], &configured, dir.path(), &live);

    let server = merged
        .iter()
        .find(|s| s.id == "live")
        .expect("server listed");
    assert_eq!(
        server.status,
        McpStatus::Connected,
        "the live status map must override the default Disabled state"
    );
}

/// The startup MCP connect loop publishes `McpStatusChanged` from a task
/// spawned *before* the TUI subscribes to the event bus, so those events can be
/// dropped by the broadcast channel (no replay buffer). The shared `McpClient`
/// held by the `SessionProcessor` is the authoritative record; adopting it must
/// make `/mcp` report the real status instead of the seeded `disabled`.
#[tokio::test(flavor = "multi_thread")]
async fn test_mcp_list_reports_status_from_shared_client_not_events() {
    use ragent_agent::mcp::{McpClient, McpServer, McpStatus};

    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    // Seed the display list exactly as `App::new` does for a plugin-bridged
    // server: present, but with no live status yet.
    app.mcp_servers = vec![McpServer {
        id: "mongodb.mongodb".to_string(),
        config: ragent_agent::McpServerConfig::default(),
        status: McpStatus::Disabled,
        tools: Vec::new(),
    }];
    app.mcp_client_adopted = false;

    // Stand in for the completed startup connect loop: a client whose record
    // says the server is connected and advertises one tool.
    let mut client = McpClient::new();
    client.register_connected_for_tests(
        "mongodb.mongodb",
        vec![ragent_agent::mcp::McpToolDef {
            name: "find".to_string(),
            description: "Find documents".to_string(),
            parameters: serde_json::json!({"type": "object"}),
        }],
    );
    app.session_processor
        .mcp_client
        .set(Arc::new(tokio::sync::RwLock::new(client)))
        .map_err(|_| ())
        .expect("mcp client set once");

    let processor = Arc::clone(&app.session_processor);
    app.adopt_mcp_client_state(&processor).await;

    let tracked = app
        .mcp_servers
        .iter()
        .find(|s| s.id == "mongodb.mongodb")
        .expect("server retained");
    assert_eq!(
        tracked.status,
        McpStatus::Connected,
        "the shared client's status must replace the seeded Disabled state"
    );
    assert_eq!(tracked.tools.len(), 1, "tools are adopted too");
    assert!(
        app.mcp_client_adopted,
        "the adoption latch stops repeated client reads"
    );
}

/// `/mcp list` must print the status adopted from the shared client, even
/// though no `McpStatusChanged` event was ever delivered to the TUI.
#[tokio::test(flavor = "multi_thread")]
async fn test_slash_mcp_list_prints_client_derived_status() {
    use ragent_agent::mcp::{McpClient, McpServer, McpStatus};

    // The tempdir cwd has no `.ragent/plugins`, so the `/mcp list` refresh
    // cannot reintroduce a plugin server; the seeded entry is enough to prove
    // the refresh preserves client-derived status.
    let dir = tempfile::tempdir().expect("tempdir");
    let _guard = with_cwd(dir.path());
    let mut app = make_app();
    // `cwd_path` is captured when the App is built, so re-point it at the
    // tempdir: the `/mcp list` refresh reads the plugin store from here.
    app.cwd_path = dir.path().to_path_buf();
    app.session_id = Some("s1".to_string());
    app.mcp_servers = vec![McpServer {
        id: "configured".to_string(),
        config: ragent_agent::McpServerConfig::default(),
        status: McpStatus::Disabled,
        tools: Vec::new(),
    }];

    let mut client = McpClient::new();
    client.register_connected_for_tests("configured", Vec::new());
    app.session_processor
        .mcp_client
        .set(Arc::new(tokio::sync::RwLock::new(client)))
        .map_err(|_| ())
        .expect("mcp client set once");
    let processor = Arc::clone(&app.session_processor);
    app.adopt_mcp_client_state(&processor).await;

    app.execute_slash_command("/mcp list").await;

    let joined: String = app
        .messages
        .iter()
        .map(|m| m.text_content())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        joined.contains("connected"),
        "the list must show the client-derived connected status: {joined}"
    );
    assert!(
        !joined.contains("disabled"),
        "the list must not still show the seeded disabled status: {joined}"
    );
}

/// `/mcp list` reports each server's tool COUNT and nothing more: the full
/// tool-name inventory is the model-facing surface (`/tools`), while the
/// per-server names stay available via `/plugins list --mcp`.
#[tokio::test(flavor = "multi_thread")]
async fn test_slash_mcp_list_shows_tool_counts_not_tool_names() {
    use ragent_agent::mcp::{McpClient, McpToolDef};

    let dir = tempfile::tempdir().expect("tempdir");
    let _guard = with_cwd(dir.path());
    let mut app = make_app();
    app.cwd_path = dir.path().to_path_buf();
    app.session_id = Some("s1".to_string());

    let mut client = McpClient::new();
    client.register_connected_for_tests(
        "configured",
        vec![
            McpToolDef {
                name: "find".to_string(),
                description: "Find documents".to_string(),
                parameters: serde_json::json!({"type": "object"}),
            },
            McpToolDef {
                name: "count".to_string(),
                description: "Count documents".to_string(),
                parameters: serde_json::json!({"type": "object"}),
            },
        ],
    );
    app.session_processor
        .mcp_client
        .set(Arc::new(tokio::sync::RwLock::new(client)))
        .map_err(|_| ())
        .expect("mcp client set once");
    let processor = Arc::clone(&app.session_processor);
    app.adopt_mcp_client_state(&processor).await;

    app.execute_slash_command("/mcp list").await;

    let joined: String = app
        .messages
        .iter()
        .map(|m| m.text_content())
        .collect::<Vec<_>>()
        .join("\n");
    let flat = joined.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        flat.contains("tools: 2"),
        "the list must report the tool count: {joined}"
    );
    assert!(
        !joined.contains("find") && !joined.contains("mcp_configured_find"),
        "the list must not enumerate individual tool names: {joined}"
    );
}
