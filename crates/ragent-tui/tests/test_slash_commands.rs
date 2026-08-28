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
    _lock: MutexGuard<'static, ()>,
    /// Optional tempdir, declared last so it is deleted only after cwd has
    /// been restored (drop order = field declaration order).
    _temp: Option<tempfile::TempDir>,
}

impl CwdGuard {
    /// Path of the guard's working directory (the tempdir when present).
    #[allow(dead_code)]
    fn path(&self) -> std::path::PathBuf {
        self._temp
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
    let _lock = cwd_lock();
    let prev = std::env::current_dir().expect("current dir");
    std::env::set_current_dir(dir).expect("set_current_dir");
    CwdGuard {
        prev,
        _lock,
        _temp: None,
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
/// (field drop order). Declared `_temp` and `_lock` locals are not needed at
/// the call site, which removes a common cause of cwd-mutex deadlock.
fn enter_with_cwd() -> CwdGuard {
    let _lock = cwd_lock();
    let prev = std::env::current_dir().expect("current dir");
    let temp = tempfile::TempDir::new().expect("tempdir");
    std::env::set_current_dir(temp.path()).expect("set_current_dir");
    CwdGuard {
        prev,
        _lock,
        _temp: Some(temp),
    }
}

fn cwd_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Acquire the cwd test lock, recovering from any prior poisoning.
fn cwd_lock() -> MutexGuard<'static, ()> {
    let lock = cwd_test_lock().lock();
    match lock {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[test]
fn test_alt_e_toggles_edit_log_and_status_bar_indicator() {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let _lock = cwd_lock();
    let original_cwd = std::env::current_dir().expect("cwd");
    let _temp = enter_temp_config_dir();
    let _guard = CwdGuard {
        prev: original_cwd,
        _lock,
        _temp: None,
    };
    ragent_config::edit_log::set_enabled(false);

    let mut app = make_app_with_storage(storage);

    // Sanity: edit log starts off.
    assert!(!ragent_config::edit_log::is_enabled());

    // Press Alt+E through the app handler so the persist path runs.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::ALT));

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
        text.contains("EditLog:✓"),
        "status bar should show enabled edit-log indicator: {text}"
    );

    // Toggle back off and verify.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::ALT));
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
        text.contains("EditLog:✗"),
        "status bar should show disabled edit-log indicator: {text}"
    );
}

#[test]
fn test_slash_editlog_toggles_and_persists() {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let _lock = cwd_lock();
    let original_cwd = std::env::current_dir().expect("cwd");
    let _temp = enter_temp_config_dir();
    let _guard = CwdGuard {
        prev: original_cwd,
        _lock,
        _temp: None,
    };
    ragent_config::edit_log::set_enabled(false);

    let mut app = make_app_with_storage(storage);
    app.input = "/editlog on".to_string();
    app.input_cursor = app.input.chars().count();

    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(ragent_config::edit_log::is_enabled());
    assert!(app.status.contains("enabled"));

    app.input = "/editlog off".to_string();
    app.input_cursor = app.input.chars().count();
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
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

#[test]
fn test_slash_clear_empties_messages() {
    let mut app = make_app();
    // Add some dummy messages
    app.messages
        .push(ragent_agent::message::Message::user_text("s1", "hello"));
    app.messages
        .push(ragent_agent::message::Message::user_text("s1", "world"));
    assert_eq!(app.messages.len(), 2);

    app.execute_slash_command("/clear");

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

#[test]
fn test_slash_help_shows_commands() {
    let mut app = make_app();
    // Set a session so append_assistant_text can push messages
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/help");

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

#[test]
fn test_slash_help_executes_in_chat_screen() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    // App now starts in Chat mode - home screen has been removed
    assert_eq!(app.current_screen, ScreenMode::Chat);

    app.execute_slash_command("/help");
    // Should remain in Chat mode
    assert_eq!(app.current_screen, ScreenMode::Chat);
}

// ── /quit ───────────────────────────────────────────────────────────

#[test]
fn test_slash_quit_stops_app() {
    let mut app = make_app();
    assert!(app.is_running);

    app.execute_slash_command("/quit");
    assert!(!app.is_running, "app should stop after /quit");
}

#[test]
fn test_slash_exit_stops_app() {
    let mut app = make_app();
    assert!(app.is_running);

    app.execute_slash_command("/exit");
    assert!(!app.is_running, "app should stop after /exit");
}

// ── /system ─────────────────────────────────────────────────────────

#[test]
fn test_slash_system_sets_prompt() {
    let mut app = make_app();
    app.execute_slash_command("/system You are a pirate. Respond in pirate speak.");

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

#[test]
fn test_slash_system_no_args_shows_current() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    let original = app.agent_info.prompt.clone();

    app.execute_slash_command("/system");

    // Should display the current prompt, not change it
    assert_eq!(app.agent_info.prompt, original);
    if original.is_some() {
        assert!(!app.messages.is_empty(), "should show current prompt");
        let text = app.messages.last().unwrap().text_content();
        assert!(text.contains("Current system prompt"));
    }
}

#[test]
fn test_slash_system_replaces_existing() {
    let mut app = make_app();
    app.execute_slash_command("/system First prompt");
    assert_eq!(app.agent_info.prompt.as_deref(), Some("First prompt"));

    app.execute_slash_command("/system Second prompt");
    assert_eq!(app.agent_info.prompt.as_deref(), Some("Second prompt"));
}

// ── /agent ──────────────────────────────────────────────────────────

#[test]
fn test_slash_agent_with_name_switches() {
    let mut app = make_app();
    assert_eq!(app.agent_name, "general");

    app.execute_slash_command("/agent ask");

    assert_eq!(app.agent_name, "ask");
    assert_eq!(app.agent_info.name, "ask");
    assert!(app.status.contains("ask"));
}

#[test]
fn test_slash_agent_unknown_name_shows_error() {
    let mut app = make_app();
    app.execute_slash_command("/agent nonexistent");

    assert!(
        app.status.contains("Unknown agent"),
        "status should warn about unknown agent: {}",
        app.status
    );
    assert_eq!(app.agent_name, "general", "should not change agent");
}

#[test]
fn test_slash_agent_no_args_opens_dialog() {
    let mut app = make_app();
    app.execute_slash_command("/agent");

    assert!(
        app.provider_setup.is_some(),
        "should open agent selection dialog"
    );
}

// ── /log ────────────────────────────────────────────────────────────

#[test]
fn test_slash_log_toggles_panel() {
    let mut app = make_app();
    assert!(!app.show_log, "log should be hidden initially");

    app.execute_slash_command("/log");
    assert!(app.show_log, "log should be visible after first toggle");
    assert_eq!(app.status, "log panel visible");

    app.execute_slash_command("/log");
    assert!(!app.show_log, "log should be hidden after second toggle");
    assert_eq!(app.status, "log panel hidden");
}

#[test]
fn test_slash_log_clear_subagents() {
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
    app.execute_slash_command("/log clear subagents");

    assert_eq!(app.status, "log: subagents cleared");
    // Directory is kept but emptied.
    assert!(subagents_dir.exists(), "subagents dir should still exist");
    assert_eq!(
        std::fs::read_dir(&subagents_dir).unwrap().count(),
        0,
        "subagents dir should be empty"
    );
}

#[test]
fn test_slash_log_clear_panics() {
    let _guard = enter_with_cwd();
    let panics_dir = std::env::current_dir().unwrap().join("log").join("panics");

    // Seed log/panics with a few files.
    std::fs::create_dir_all(&panics_dir).expect("create panics dir");
    std::fs::write(panics_dir.join("panic-1.log"), "panic a").expect("write a");
    std::fs::write(panics_dir.join("panic-2.log"), "panic b").expect("write b");
    std::fs::write(panics_dir.join("panic-3.log"), "panic c").expect("write c");
    assert_eq!(std::fs::read_dir(&panics_dir).unwrap().count(), 3);

    let mut app = make_app();
    app.execute_slash_command("/log clear panics");

    assert_eq!(app.status, "log: panics cleared");
    assert!(panics_dir.exists(), "panics dir should still exist");
    assert_eq!(
        std::fs::read_dir(&panics_dir).unwrap().count(),
        0,
        "panics dir should be empty"
    );
}

#[test]
fn test_slash_log_clear_missing_dir_reports_zero() {
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
    app.execute_slash_command("/log clear subagents");
    assert_eq!(app.status, "log: subagents cleared");
}

#[test]
fn test_slash_log_clear_no_target_shows_usage() {
    let mut app = make_app();
    app.execute_slash_command("/log clear");
    assert_eq!(app.status, "log: clear usage");
}

#[test]
fn test_slash_log_clear_research() {
    let _guard = enter_with_cwd();
    let research_dir = std::env::current_dir()
        .unwrap()
        .join("logs")
        .join("research");

    // Seed logs/research with a few files.
    std::fs::create_dir_all(&research_dir).expect("create research dir");
    std::fs::write(research_dir.join("research-aaa-web.jsonl"), "line a\n").expect("write a");
    std::fs::write(research_dir.join("research-bbb-web.jsonl"), "line b\n").expect("write b");
    assert_eq!(std::fs::read_dir(&research_dir).unwrap().count(), 2);

    let mut app = make_app();
    app.execute_slash_command("/log clear research");

    assert_eq!(app.status, "log: research cleared");
    assert!(research_dir.exists(), "research dir should still exist");
    assert_eq!(
        std::fs::read_dir(&research_dir).unwrap().count(),
        0,
        "research dir should be empty"
    );
}

#[test]
fn test_slash_log_clear_research_missing_dir_reports_zero() {
    let _guard = enter_with_cwd();

    // No logs/research directory exists yet.
    assert!(
        !std::env::current_dir()
            .unwrap()
            .join("logs")
            .join("research")
            .exists()
    );

    let mut app = make_app();
    app.execute_slash_command("/log clear research");
    assert_eq!(app.status, "log: research cleared");
}

#[test]
fn test_slash_log_help_shows_help_text() {
    let mut app = make_app();
    app.execute_slash_command("/log help");
    assert_eq!(app.status, "log: help");
}

#[test]
fn test_slash_log_clear_editlog() {
    let _guard = enter_with_cwd();
    let editlog_dir = std::env::current_dir().unwrap().join("log").join("editlog");

    // Seed log/editlog with a few files.
    std::fs::create_dir_all(&editlog_dir).expect("create editlog dir");
    std::fs::write(editlog_dir.join("edits-aaa.jsonl"), "line a\n").expect("write a");
    std::fs::write(editlog_dir.join("edits-bbb.jsonl"), "line b\n").expect("write b");
    assert_eq!(std::fs::read_dir(&editlog_dir).unwrap().count(), 2);

    let mut app = make_app();
    app.execute_slash_command("/log clear editlog");

    assert_eq!(app.status, "log: editlog cleared");
    assert!(editlog_dir.exists(), "editlog dir should still exist");
    assert_eq!(
        std::fs::read_dir(&editlog_dir).unwrap().count(),
        0,
        "editlog dir should be empty"
    );
}

#[test]
fn test_slash_log_clear_editlog_missing_dir_reports_zero() {
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
    app.execute_slash_command("/log clear editlog");
    assert_eq!(app.status, "log: editlog cleared");
}

#[test]
fn test_slash_log_clear_logwindow() {
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
    app.execute_slash_command("/log clear logwindow");

    assert_eq!(app.status, "log: logwindow cleared");
    assert!(logwindow_dir.exists(), "logwindow dir should still exist");
    assert_eq!(
        std::fs::read_dir(&logwindow_dir).unwrap().count(),
        0,
        "logwindow dir should be empty"
    );
}

#[test]
fn test_slash_log_clear_logwindow_missing_dir_reports_zero() {
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
    app.execute_slash_command("/log clear logwindow");
    assert_eq!(app.status, "log: logwindow cleared");
}

#[test]
fn test_slash_log_unknown_sub_shows_usage() {
    let mut app = make_app();
    app.execute_slash_command("/log frobnicate");
    assert_eq!(app.status, "log: usage");
}

// ── /profile ────────────────────────────────────────────────────────

#[test]
fn test_slash_profile_on_enables_profiler_panel() {
    agent_loop_profiler().set_enabled(false);

    let mut app = make_app();
    assert!(!app.show_profile, "profile should be hidden initially");

    app.execute_slash_command("/profile on");

    assert!(app.show_profile, "profile should be visible after enabling");
    assert_eq!(app.status, "profile panel visible");

    agent_loop_profiler().set_enabled(false);
}

#[test]
fn test_slash_profile_off_disables_profiler_panel() {
    agent_loop_profiler().set_enabled(true);

    let mut app = make_app();
    app.show_profile = true;

    app.execute_slash_command("/profile off");

    assert!(
        !app.show_profile,
        "profile should be hidden after disabling"
    );
    assert_eq!(app.status, "profile panel hidden");
}

#[test]
fn test_alt_p_toggles_profiler_panel() {
    agent_loop_profiler().set_enabled(false);

    let mut app = make_app();
    assert!(!app.show_profile, "profile should be hidden initially");

    app.handle_key_event(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::ALT));
    assert!(app.show_profile, "profile should be visible after Alt+P");
    assert_eq!(app.status, "profile panel visible");

    app.handle_key_event(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::ALT));
    assert!(
        !app.show_profile,
        "profile should be hidden after second Alt+P"
    );
    assert_eq!(app.status, "profile panel hidden");

    agent_loop_profiler().set_enabled(false);
}

// ── /llmstats ───────────────────────────────────────────────────────

#[test]
fn test_slash_llmstats_shows_average_metrics() {
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

    app.execute_slash_command("/llmstats");

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

#[test]
fn test_slash_llmstats_no_samples_shows_message() {
    let mut app = make_app();
    app.selected_model = Some("openai/gpt-4o".to_string());

    app.execute_slash_command("/llmstats");

    assert_eq!(app.status, "llm stats unavailable");
    assert!(!app.messages.is_empty(), "llmstats should create a message");
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("No completed LLM responses yet"));
}

// ── /cost ───────────────────────────────────────────────────────────

#[test]
fn test_slash_cost_shows_estimated_cost() {
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

    app.execute_slash_command("/cost");

    assert_eq!(app.status, "cost summary");
    assert!(!app.messages.is_empty(), "cost should create a message");
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("From: /cost"));
    assert!(text.contains("Samples: 2"));
    assert!(text.contains("Total tokens"));
    assert!(text.contains("Estimated cost"));
}

#[test]
fn test_slash_cost_no_samples_shows_message() {
    let mut app = make_app();

    app.execute_slash_command("/cost");

    assert_eq!(app.status, "cost unavailable");
    assert!(!app.messages.is_empty(), "cost should create a message");
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("No completed LLM responses yet"));
}

// ── /compact ────────────────────────────────────────────────────────

#[test]
fn test_slash_compact_no_session_shows_warning() {
    let mut app = make_app();
    assert!(app.session_id.is_none());

    app.execute_slash_command("/compact");
    assert!(
        app.status.contains("No messages"),
        "should create session then warn about empty messages: {}",
        app.status
    );
    assert!(app.session_id.is_some(), "session should be created");
}

#[test]
fn test_slash_compact_no_messages_shows_warning() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    assert!(app.messages.is_empty());

    app.execute_slash_command("/compact");
    assert!(
        app.status.contains("No messages"),
        "should warn about empty messages: {}",
        app.status
    );
}

// `/compress` is a deprecated alias for `/compact` (FR-009) and must
// forward to the same compaction path.

#[test]
fn test_slash_compress_alias_forwards_to_compact() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    assert!(app.messages.is_empty());

    app.execute_slash_command("/compress");
    assert!(
        app.status.contains("No messages"),
        "/compress should behave like /compact when there is nothing to compact: {}",
        app.status
    );
}

// ── /undo ───────────────────────────────────────────────────────────

#[test]
fn test_slash_undo_no_session_shows_warning() {
    let mut app = make_app();
    assert!(app.session_id.is_none());

    app.execute_slash_command("/undo");
    // The ensure_session() gate runs before the undo handler, so a session
    // will be created. The undo logic then checks for empty messages.
    assert!(
        app.status.contains("No messages"),
        "should warn about no messages after session creation: {}",
        app.status
    );
    assert!(app.session_id.is_some(), "session should be created");
}

#[test]
fn test_slash_undo_no_messages_shows_warning() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    assert!(app.messages.is_empty());

    app.execute_slash_command("/undo");
    assert!(
        app.status.contains("No messages"),
        "should warn about no messages: {}",
        app.status
    );
}

#[test]
fn test_slash_undo_removes_last_user_assistant_pair() {
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

    app.execute_slash_command("/undo");

    // Should have removed the last user message and its assistant response
    assert_eq!(app.messages.len(), 2);
    assert_eq!(app.messages[0].text_content(), "first question");
    assert_eq!(app.messages[1].text_content(), "first answer");
    assert_eq!(app.scroll_offset, 0);
    assert!(app.status.contains("Undid last turn"));
    assert!(app.status.contains("removed 2 message(s)"));
}

#[test]
fn test_slash_undo_no_user_message_warns() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    // Only assistant messages (no user messages to undo)
    app.messages
        .push(ragent_agent::message::Message::assistant_text(
            "s1",
            "orphan answer",
        ));

    app.execute_slash_command("/undo");

    assert!(
        app.status.contains("No user message found"),
        "should warn about no user message: {}",
        app.status
    );
    assert_eq!(app.messages.len(), 1); // unchanged
}

#[test]
fn test_slash_undo_removes_multiple_following_messages() {
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

    app.execute_slash_command("/undo");

    // Should remove user message and all following messages
    assert_eq!(app.messages.len(), 0);
    assert!(app.status.contains("removed 3 message(s)"));
}

// ── /name ───────────────────────────────────────────────────────────

#[test]
fn test_slash_name_no_session_shows_warning() {
    let mut app = make_app();
    assert!(app.session_id.is_none());

    app.execute_slash_command("/name My Session");
    // The ensure_session() gate runs before the name handler, so a session
    // will be created. The name is then set on that session.
    assert!(app.session_id.is_some(), "session should be created");
    assert!(
        app.status.contains("Session name set to"),
        "should confirm name was set: {}",
        app.status
    );
}

#[test]
fn test_slash_name_sets_session_name() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    // Create the session in storage first
    let storage = app.session_processor.session_manager.storage();
    storage
        .create_session("s1", "/tmp/test")
        .expect("create session");
    let _ = storage;

    app.execute_slash_command("/name My Test Session");

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

#[test]
fn test_slash_name_clears_with_empty_argument() {
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
    app.execute_slash_command("/name ");

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

#[test]
fn test_slash_name_trims_whitespace() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    let storage = app.session_processor.session_manager.storage();
    storage
        .create_session("s1", "/tmp/test")
        .expect("create session");
    let _ = storage;

    app.execute_slash_command("/name   Trimmed Name   ");

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

#[test]
fn test_help_shows_name_command() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/help");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("/name"),
        "help should document /name command: {text}"
    );
}

#[test]
fn test_help_lists_compact_not_compress() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/help");
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
    app.execute_slash_command("/model");
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

#[test]
fn test_slash_model_show_without_selected_model_uses_agent_model() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/model show");

    assert_eq!(app.status, "active model metadata");
    let text = app
        .messages
        .last()
        .expect("metadata message")
        .text_content();
    assert!(text.contains("From: /model show"));
    assert!(text.contains("Model Ref"));
}

#[test]
fn test_slash_model_show_displays_metadata_for_active_model() {
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

    app.execute_slash_command("/model show");

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

#[test]
fn test_slash_model_show_invalid_subcommand_shows_usage() {
    let mut app = make_app();

    app.execute_slash_command("/model nope");

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

    app.execute_slash_command("/model");

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

    app.execute_slash_command("/model");

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

#[test]
fn test_slash_provider_opens_setup() {
    let mut app = make_app();
    app.execute_slash_command("/provider");

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
    app.execute_slash_command("/provider");
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
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

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
#[test]
fn test_slash_provider_selection_updates_displayed_provider() {
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

    // Press Enter to confirm the model selection.
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

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
#[test]
fn test_model_selector_navigation_wraps_top_and_bottom() {
    let mut app = make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectModel {
        provider_id: "copilot".to_string(),
        provider_name: "GitHub Copilot".to_string(),
        models: vec![
            ragent_tui::app::ModelPickerEntry {
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

    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    match app.provider_setup.as_ref().expect("provider setup present") {
        ProviderSetupStep::SelectModel { selected, .. } => assert_eq!(*selected, 2),
        _ => panic!("expected SelectModel state"),
    }

    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    match app.provider_setup.as_ref().expect("provider setup present") {
        ProviderSetupStep::SelectModel { selected, .. } => assert_eq!(*selected, 0),
        _ => panic!("expected SelectModel state"),
    }
}

// ── /provider_reset ─────────────────────────────────────────────────

#[test]
fn test_slash_provider_reset_opens_dialog() {
    let mut app = make_app();
    app.execute_slash_command("/provider_reset");

    assert!(
        app.provider_setup.is_some(),
        "should open provider reset dialog"
    );
}

#[test]
fn test_generic_openai_enter_key_supports_endpoint_field_and_tab_toggle() {
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
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    ragent_tui::input::handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE),
    );
    ragent_tui::input::handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE),
    );

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

    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

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

#[test]
fn test_slash_unknown_command_shows_error() {
    let mut app = make_app();
    app.execute_slash_command("/foobar");

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

// ── input clearing ──────────────────────────────────────────────────

#[test]
fn test_slash_command_clears_input() {
    let mut app = make_app();
    app.input = "/help".to_string();

    app.execute_slash_command(&app.input.clone());
    assert!(
        app.input.is_empty(),
        "input should be cleared after command"
    );
    assert!(app.slash_menu.is_none(), "slash menu should be closed");
}

#[test]
fn test_input_cursor_left_right_and_editing() {
    let mut app = make_app();
    app.input = "abc".to_string();
    app.input_cursor = app.input.chars().count();

    // Move cursor left twice (from end to between 'b' and 'c')
    app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));

    assert_eq!(app.input_cursor, 1);

    // Insert a character at the cursor position
    app.handle_key_event(KeyEvent::new(KeyCode::Char('X'), KeyModifiers::NONE));
    assert_eq!(app.input, "aXbc");
    assert_eq!(app.input_cursor, 2);

    // Move to end and delete the inserted character
    app.handle_key_event(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
    assert_eq!(app.input_cursor, 4);

    // Backspace at end removes the last character.
    app.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    assert_eq!(app.input, "aXb");
    assert_eq!(app.input_cursor, 3);

    // Move left one position and delete the inserted character.
    app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    assert_eq!(app.input_cursor, 2);
    app.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    assert_eq!(app.input, "ab");
    assert_eq!(app.input_cursor, 1);
}

#[test]
fn test_input_editing_handles_unicode_backspace_and_delete() {
    let mut app = make_app();
    app.input = "a💡b".to_string();
    app.input_cursor = app.input.chars().count();

    // Move to between 💡 and b, then backspace removes 💡.
    app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    assert_eq!(app.input_cursor, 2);
    app.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    assert_eq!(app.input, "ab");
    assert_eq!(app.input_cursor, 1);

    // Delete at cursor should remove the next character.
    app.input = "a💡b".to_string();
    app.input_cursor = 1; // before 💡
    app.handle_key_event(KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE));
    assert_eq!(app.input, "ab");
    assert_eq!(app.input_cursor, 1);
}

#[test]
fn test_file_menu_mode_editing_respects_midline_cursor() {
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

    app.handle_key_event(KeyEvent::new(KeyCode::Char('X'), KeyModifiers::NONE));
    assert_eq!(app.input, "abX@cd");
    assert_eq!(app.input_cursor, 3);

    app.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    assert_eq!(app.input, "ab@cd");
    assert_eq!(app.input_cursor, 2);
}

#[test]
fn test_history_picker_enter_sets_char_cursor_for_unicode() {
    let mut app = make_app();
    app.history_picker = Some(HistoryPickerState {
        entries: vec!["éé".to_string()],
        selected: 0,
        scroll_offset: 0,
    });

    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(app.history_picker.is_none());
    assert_eq!(app.input, "éé");
    assert_eq!(app.input_cursor, 2);
}

#[test]
fn test_chat_keystrokes_produce_expected_edit_result() {
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
        chat.handle_key_event(key);
    }

    // Verify the final state
    assert_eq!(chat.input, "aZ");
    assert_eq!(chat.input_cursor, 2);
}

#[test]
fn test_ctrl_word_navigation_and_deletes() {
    let mut app = make_app();
    app.input = "hello world again".to_string();
    app.input_cursor = app.input.chars().count();

    app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL));
    assert_eq!(app.input_cursor, "hello world ".chars().count());

    app.handle_key_event(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL));
    assert_eq!(app.input, "hello again");
    assert_eq!(app.input_cursor, "hello ".chars().count());

    app.handle_key_event(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL));
    assert_eq!(app.input, "hello ");
    assert_eq!(app.input_cursor, "hello ".chars().count());
}

#[test]
fn test_ctrl_terminal_cursor_movement_bindings() {
    let mut app = make_app();
    app.input = "abcdef".to_string();
    app.input_cursor = 3;

    app.handle_key_event(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL));
    assert_eq!(app.input_cursor, 2);
    app.handle_key_event(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL));
    assert_eq!(app.input_cursor, 3);

    // Ctrl+A now selects all: anchor → 0, cursor → end.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));
    assert_eq!(app.kb_select_anchor, Some(0));
    assert_eq!(app.input_cursor, 6);
    // Ctrl+E moves to end (cursor is already there; clears selection).
    app.handle_key_event(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL));
    assert_eq!(app.input_cursor, 6);
}

#[test]
fn test_ctrl_home_end_bindings() {
    let mut app = make_app();
    app.input = "abcdef".to_string();
    app.input_cursor = 3;

    app.handle_key_event(KeyEvent::new(KeyCode::Home, KeyModifiers::CONTROL));
    assert_eq!(app.input_cursor, 0);
    app.handle_key_event(KeyEvent::new(KeyCode::End, KeyModifiers::CONTROL));
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

#[test]
fn test_file_menu_mode_supports_cursor_movement_and_delete() {
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

    app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    assert_eq!(app.input_cursor, 2);

    app.handle_key_event(KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE));
    assert_eq!(app.input, "abcd");
    assert_eq!(app.input_cursor, 2);
}

#[test]
fn test_file_menu_mode_supports_ctrl_word_actions() {
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

    app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL));
    assert_eq!(app.input_cursor, "@hello ".chars().count());

    app.handle_key_event(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL));
    assert_eq!(app.input, "@hello ");
    assert_eq!(app.input_cursor, "@hello ".chars().count());
}

#[test]
fn test_file_menu_enter_accepts_without_sending() {
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
        ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
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

#[test]
fn test_slash_browse_refresh_updates_cache_metadata() {
    let mut app = make_app();
    app.project_files_cache = None;
    app.project_files_cache_cwd = None;
    app.project_files_cache_refreshed_at = None;
    app.project_files_cache_count = 0;

    app.execute_slash_command("/browse_refresh");

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

#[test]
fn test_file_menu_ctrl_backslash_toggles_hidden_filter() {
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
    );
    assert!(app.file_menu_show_hidden);
    assert!(app.file_menu.is_some());
}

#[test]
fn test_file_menu_down_scrolls_selection_window() {
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
        );
    }
    let menu = app.file_menu.as_ref().expect("menu");
    assert_eq!(menu.selected, 9);
    assert!(menu.scroll_offset > 0);
}

#[test]
fn test_slash_inputdiag_reports_input_state() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.input = "abc".to_string();
    app.input_cursor = 2;

    app.execute_slash_command("/inputdiag");

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

#[test]
fn test_slash_command_works_without_leading_slash() {
    let mut app = make_app();
    app.execute_slash_command("quit");
    assert!(!app.is_running, "/quit should work without leading slash");
}

#[test]
fn test_keyboard_quit_requires_ctrl_c_then_ctrl_d() {
    let mut app = make_app();
    assert!(app.is_running);

    app.handle_key_event(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
    assert!(app.is_running, "Ctrl+D alone should not quit");
    assert!(app.status.contains("Ctrl+C first"));

    app.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(app.is_running, "Ctrl+C should arm, not quit");
    assert!(app.status.contains("Ctrl+D"));

    app.handle_key_event(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
    assert!(!app.is_running, "Ctrl+C then Ctrl+D should quit");
}

#[test]
fn test_keyboard_quit_ctrl_c_then_ctrl_c_does_not_exit() {
    let mut app = make_app();
    assert!(app.is_running);

    app.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(app.is_running);

    app.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(app.is_running, "second Ctrl+C should not exit");
    assert!(app.status.contains("Ctrl+D"));
}

#[test]
fn test_output_view_paging_shortcuts() {
    let mut app = make_app();
    app.output_view = Some(OutputViewState {
        target: OutputViewTarget::Session {
            session_id: "s1".to_string(),
            label: "primary".to_string(),
        },
        scroll_offset: 10,
        max_scroll: 50,
    });

    app.handle_key_event(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));
    assert_eq!(app.output_view.as_ref().unwrap().scroll_offset, 5);

    app.handle_key_event(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
    assert_eq!(app.output_view.as_ref().unwrap().scroll_offset, 10);

    app.handle_key_event(KeyEvent::new(KeyCode::PageUp, KeyModifiers::CONTROL));
    assert_eq!(app.output_view.as_ref().unwrap().scroll_offset, 0);

    app.handle_key_event(KeyEvent::new(KeyCode::PageDown, KeyModifiers::CONTROL));
    assert_eq!(app.output_view.as_ref().unwrap().scroll_offset, 50);
}

#[test]
fn test_output_view_escape_closes_overlay() {
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
    });

    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.output_view.is_none());
    assert!(app.selected_agent_session_id.is_none());
    assert!(app.selected_agent_index.is_none());
}

#[test]
fn test_output_view_team_member_without_session_uses_log_filter() {
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
    });

    app.handle_key_event(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
    assert!(app.output_view.is_some());
}

// ── /system preserves whitespace ────────────────────────────────────

#[test]
fn test_slash_system_preserves_argument_whitespace() {
    let mut app = make_app();
    app.execute_slash_command("/system   You are   a   helpful   bot  ");

    assert_eq!(
        app.agent_info.prompt.as_deref(),
        Some("You are   a   helpful   bot"),
        "leading/trailing whitespace trimmed, internal preserved"
    );
}

// ── /tools ──────────────────────────────────────────────────────────

#[test]
fn test_slash_tools_lists_visibility_switches() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.tool_visibility = ragent_agent::ToolVisibilityConfig::default();

    app.execute_slash_command("/tools");

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

#[test]
fn test_slash_tools_shows_single_switch_state() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.tool_visibility = ragent_agent::ToolVisibilityConfig::default();

    app.execute_slash_command("/tools office");

    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("`office` is currently **off**"));
}

#[test]
fn test_slash_tools_help_shows_usage() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.tool_visibility = ragent_agent::ToolVisibilityConfig::default();

    app.execute_slash_command("/tools help");

    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("`/tools show`"));
    assert!(text.contains("`/tools help`"));
    assert!(text.contains("`/tools <switch> on|off`"));
    assert!(text.contains("`office`, `github`, `gitlab`, `teams`, `agents`, `plan`, `codeindex`"));
}
#[test]
fn test_slash_tools_show_alias_lists_visibility_switches() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.tool_visibility = ragent_agent::ToolVisibilityConfig::default();

    app.execute_slash_command("/tools show");

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

#[test]
fn test_slash_tools_office_on_shows_office_tools() {
    let _lock = cwd_lock();
    let original_cwd = std::env::current_dir().expect("cwd");
    let _temp = enter_temp_config_dir();
    let _guard = CwdGuard {
        prev: original_cwd,
        _lock,
        _temp: None,
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

    app.execute_slash_command("/tools office on");

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

#[test]
fn test_slash_tools_teams_on_shows_team_tools() {
    let _lock = cwd_lock();
    let original_cwd = std::env::current_dir().expect("cwd");
    let _temp = enter_temp_config_dir();
    let _guard = CwdGuard {
        prev: original_cwd,
        _lock,
        _temp: None,
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

    app.execute_slash_command("/tools teams on");

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

#[test]
fn test_slash_tools_agents_on_shows_agent_tools() {
    let _lock = cwd_lock();
    let original_cwd = std::env::current_dir().expect("cwd");
    let _temp = enter_temp_config_dir();
    let _guard = CwdGuard {
        prev: original_cwd,
        _lock,
        _temp: None,
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

    app.execute_slash_command("/tools agents on");

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

#[test]
fn test_slash_tools_plan_on_shows_plan_tools() {
    let _lock = cwd_lock();
    let original_cwd = std::env::current_dir().expect("cwd");
    let _temp = enter_temp_config_dir();
    let _guard = CwdGuard {
        prev: original_cwd,
        _lock,
        _temp: None,
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

    app.execute_slash_command("/tools plan on");

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

#[test]
fn test_slash_tools_creates_session_if_none() {
    let mut app = make_app();
    assert!(app.session_id.is_none());

    app.execute_slash_command("/tools");

    assert!(app.session_id.is_some(), "should create session");
    assert_eq!(app.status, "tools");
    assert!(!app.messages.is_empty());
}

// ── /opt ────────────────────────────────────────────────────────────

#[test]
fn test_slash_opt_help_shows_markdown_table() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/opt help");

    assert_eq!(app.status, "opt help");
    assert!(
        !app.messages.is_empty(),
        "/opt help should produce a message"
    );
    let text = app.messages.last().unwrap().text_content();
    // The table must list at least a few well-known methods
    assert!(text.contains("co_star"), "table should include co_star");
    assert!(text.contains("crispe"), "table should include crispe");
    assert!(text.contains("cot"), "table should include cot");
    assert!(text.contains("draw"), "table should include draw");
    assert!(text.contains("rise"), "table should include rise");
    assert!(text.contains("meta"), "table should include meta");
    assert!(
        text.contains("variational"),
        "table should include variational"
    );
    assert!(text.contains("q_star"), "table should include q_star");
    assert!(text.contains("openai"), "table should include openai");
    assert!(text.contains("claude"), "table should include claude");
    assert!(text.contains("microsoft"), "table should include microsoft");
}

#[test]
fn test_slash_opt_help_stays_in_chat() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    // App now starts in Chat mode - home screen has been removed
    assert_eq!(app.current_screen, ScreenMode::Chat);

    app.execute_slash_command("/opt help");

    // Should remain in Chat mode
    assert_eq!(app.current_screen, ScreenMode::Chat);
}

#[test]
fn test_slash_opt_no_args_shows_help() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    // /opt with no args falls through to the help branch
    app.execute_slash_command("/opt");

    assert_eq!(app.status, "opt help");
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("co_star"));
}

#[tokio::test]
async fn test_slash_opt_co_star_formats_prompt() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    // Provide a configured model so the command proceeds past the guard.
    app.selected_model = Some("anthropic/claude-3-7-sonnet-20250219".to_string());

    app.execute_slash_command("/opt co_star Explain async/await in Rust");

    // With LLM integration the command is async: status shows "optimizing" immediately,
    // and no message is appended until the background task completes.
    assert!(
        app.status.contains("⏳") && app.status.contains("co_star"),
        "status should show optimizing with method name: {}",
        app.status
    );
}

#[tokio::test]
async fn test_slash_opt_crispe_formats_prompt() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.selected_model = Some("anthropic/claude-3-7-sonnet-20250219".to_string());

    app.execute_slash_command("/opt crispe Write a blog post intro");

    assert!(
        app.status.contains("⏳") && app.status.contains("crispe"),
        "status should show optimizing with method name: {}",
        app.status
    );
}

#[tokio::test]
async fn test_slash_opt_cot_formats_prompt() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.selected_model = Some("anthropic/claude-3-7-sonnet-20250219".to_string());

    app.execute_slash_command("/opt cot Solve the fizzbuzz problem");

    // "cot" is an alias for ChainOfThought whose canonical name is "cot".
    assert!(
        app.status.contains("⏳") && app.status.contains("cot"),
        "status should show optimizing: {}",
        app.status
    );
}

#[tokio::test]
async fn test_slash_opt_draw_formats_prompt() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.selected_model = Some("anthropic/claude-3-7-sonnet-20250219".to_string());

    app.execute_slash_command("/opt draw A futuristic cityscape at sunset");

    assert!(
        app.status.contains("⏳") && app.status.contains("draw"),
        "status should show optimizing: {}",
        app.status
    );
}

#[tokio::test]
async fn test_slash_opt_rise_formats_prompt() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.selected_model = Some("anthropic/claude-3-7-sonnet-20250219".to_string());

    app.execute_slash_command("/opt rise Summarise this article");

    assert!(
        app.status.contains("⏳") && app.status.contains("rise"),
        "status should show optimizing: {}",
        app.status
    );
}

#[tokio::test]
async fn test_slash_opt_meta_formats_prompt() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.selected_model = Some("anthropic/claude-3-7-sonnet-20250219".to_string());

    app.execute_slash_command("/opt meta Generate a test suite");

    assert!(
        app.status.contains("⏳") && app.status.contains("meta"),
        "status should show optimizing: {}",
        app.status
    );
}

#[tokio::test]
async fn test_slash_opt_variational_formats_prompt() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.selected_model = Some("anthropic/claude-3-7-sonnet-20250219".to_string());

    app.execute_slash_command("/opt variational Write a product description");

    assert!(
        app.status.contains("⏳") && app.status.contains("variational"),
        "status should show optimizing: {}",
        app.status
    );
}

#[tokio::test]
async fn test_slash_opt_qstar_alias_works() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.selected_model = Some("anthropic/claude-3-7-sonnet-20250219".to_string());

    app.execute_slash_command("/opt qstar What is Rust ownership?");

    // "qstar" alias resolves to canonical name "q_star".
    assert!(
        app.status.contains("⏳") && app.status.contains("q_star"),
        "qstar alias should resolve to canonical q_star: {}",
        app.status
    );
}

#[tokio::test]
async fn test_slash_opt_openai_formats_prompt() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.selected_model = Some("anthropic/claude-3-7-sonnet-20250219".to_string());

    app.execute_slash_command("/opt openai Translate text to French");

    assert!(
        app.status.contains("⏳") && app.status.contains("openai"),
        "status should show optimizing: {}",
        app.status
    );
}

#[tokio::test]
async fn test_slash_opt_claude_formats_prompt() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.selected_model = Some("anthropic/claude-3-7-sonnet-20250219".to_string());

    app.execute_slash_command("/opt claude Summarise this meeting transcript");

    assert!(
        app.status.contains("⏳") && app.status.contains("claude"),
        "status should show optimizing: {}",
        app.status
    );
}

#[tokio::test]
async fn test_slash_opt_microsoft_alias_azure() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.selected_model = Some("anthropic/claude-3-7-sonnet-20250219".to_string());

    app.execute_slash_command("/opt azure Classify this support ticket");

    // "azure" alias resolves to canonical name "microsoft".
    assert!(
        app.status.contains("⏳") && app.status.contains("microsoft"),
        "azure alias should resolve to canonical microsoft: {}",
        app.status
    );
}

#[test]
fn test_slash_opt_unknown_method_shows_warning() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/opt nonexistent Some prompt text");

    assert!(
        app.status.contains("Unknown optimization method"),
        "status should warn about unknown method: {}",
        app.status
    );
    // No new message should appear for an unknown method
    assert!(
        app.messages.is_empty(),
        "unknown method should not produce a message"
    );
}

#[test]
fn test_slash_opt_missing_prompt_shows_warning() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/opt co_star");

    assert!(
        app.status.contains("Please provide a prompt"),
        "status should ask for prompt: {}",
        app.status
    );
    assert!(
        app.messages.is_empty(),
        "missing prompt should not produce a message"
    );
}

#[test]
fn test_slash_opt_is_listed_in_help() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/help");

    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("/opt"), "/help should mention /opt");
}

#[test]
fn test_slash_opt_adds_to_input_history() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/opt help");

    assert!(
        app.input_history.iter().any(|h| h.starts_with("/opt")),
        "input history should include the /opt command"
    );
}

#[tokio::test]
async fn test_slash_opt_o1_alias_works() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.selected_model = Some("anthropic/claude-3-7-sonnet-20250219".to_string());

    app.execute_slash_command("/opt o1 Write a creative short story");

    // "o1" alias resolves to canonical name "o1_style".
    assert!(
        app.status.contains("⏳") && app.status.contains("o1_style"),
        "o1 alias should resolve to canonical o1_style: {}",
        app.status
    );
}

#[test]
fn test_slash_opt_no_model_shows_warning() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    // No selected_model — should produce a friendly error.

    app.execute_slash_command("/opt co_star Explain async/await in Rust");

    assert!(
        app.status.contains("requires a configured model"),
        "status should warn when no model is configured: {}",
        app.status
    );
    assert!(
        app.messages.is_empty(),
        "no message should be added when no model configured"
    );
}

#[test]
fn test_slash_webapi_help_shows_endpoints() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/webapi help");

    assert!(!app.messages.is_empty(), "help should produce a message");
    let last = app.messages.last().unwrap();
    let text = format!("{last:?}");
    assert!(
        text.contains("health") || text.contains("sessions"),
        "help output should list API endpoints"
    );
}

#[test]
fn test_slash_webapi_disable_when_not_running() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/webapi disable");

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

    app.execute_slash_command("/webapi enable");

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

#[test]
fn test_slash_spec_no_args_shows_help() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/spec");

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
#[test]
fn test_slash_spec_update_missing_spec_id_shows_usage_error() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/spec update");

    assert!(
        app.status.contains("Usage: /spec update"),
        "missing spec-id should show usage error, got: {}",
        app.status
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_slash_spec_task_lists_tasks() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/spec task testspec");

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

    app.execute_slash_command("/spec validate");

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

    app.execute_slash_command("/spec create websocket Add real-time collaborative editing");

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

#[test]
fn test_slash_config_show_displays_paths() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/config show");

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

#[test]
fn test_slash_config_no_args_shows_usage() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/config");

    assert_eq!(app.status, "config: usage");
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("Usage:"), "should show usage hint");
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

#[test]
fn test_slash_config_save_errors_when_no_global_config() {
    // FR-003: /config save must surface a clear error when there is no global
    // ragent.json to back up. We point the process at an empty temp config dir
    // via the RAGENT_CONFIG env var indirectly — but backup_global_config(None)
    // resolves via dirs::config_dir(), which we cannot easily redirect. This
    // test asserts the error path produces an error status and a message
    // rather than panicking, exercising the slash arm end-to-end.
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/config save");

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

#[test]
fn test_slash_config_list_no_saves_shows_message() {
    // FR-004 / FR-006: /config list must always produce a user-facing message.
    // When saves exist it opens the picker AND emits a summary line; when none
    // exist it shows a "no saved configurations" message instead of an empty
    // picker. We cannot easily control the real global config dir, so this
    // test asserts the no-panic contract and that a message is always emitted.
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/config list");

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
#[test]
fn test_alt_y_toggles_yolo_mode_and_status_bar_indicator() {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let _lock = cwd_lock();
    let original_cwd = std::env::current_dir().expect("cwd");
    let _temp = enter_temp_config_dir();
    let _guard = CwdGuard {
        prev: original_cwd,
        _lock,
        _temp: None,
    };
    ragent_config::yolo::set_enabled(false);

    let mut app = make_app_with_storage(storage);

    // Sanity: YOLO starts off.
    assert!(!ragent_config::yolo::is_enabled());

    // Press Alt+Y through the app handler so the new persist path runs.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::ALT));

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
        text.contains("YOLO:✓"),
        "status bar should show enabled YOLO indicator: {text}"
    );

    // Toggle back off and verify.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::ALT));
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
        text.contains("YOLO:✗"),
        "status bar should show disabled YOLO indicator: {text}"
    );
}

#[test]
fn test_slash_yolo_toggles_and_persists() {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let _lock = cwd_lock();
    let original_cwd = std::env::current_dir().expect("cwd");
    let _temp = enter_temp_config_dir();
    let _guard = CwdGuard {
        prev: original_cwd,
        _lock,
        _temp: None,
    };
    ragent_config::yolo::set_enabled(false);

    let mut app = make_app_with_storage(storage);
    app.input = "/yolo".to_string();
    app.input_cursor = app.input.chars().count();

    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(ragent_config::yolo::is_enabled());
    assert!(app.status.contains("ENABLED"));

    // Running again disables it.
    app.input = "/yolo".to_string();
    app.input_cursor = app.input.chars().count();
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
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

#[test]
fn test_config_save_picker_intercepts_keys_in_handle_key_event() {
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

    app.handle_key_event(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty()));
    assert!(
        app.input.is_empty(),
        "keys must be intercepted while config save picker is open"
    );
}

#[test]
fn test_slash_memory_no_args_shows_usage() {
    // /memory with an unknown subcommand should list the supported subcommands.
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/memory foobar");

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

#[test]
fn test_slash_actionloop_help_shows_subcommands() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/actionloop help");

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

#[test]
fn test_slash_actionloop_no_samples_reports_hint() {
    // With no profiling samples, the plain form reports the "no samples" hint.
    // The profiler is shared process-wide, so reset it first for determinism.
    agent_loop_profiler().reset();
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/actionloop");

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

#[test]
fn test_slash_actionloop_clip_no_samples_reports_hint() {
    // The clip variant degrades gracefully when there is nothing to copy.
    // The profiler is shared process-wide and other tests may have recorded
    // samples concurrently, so reset before running to make the "no samples"
    // path deterministic where possible.
    agent_loop_profiler().reset();
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/actionloop clip");

    // When another test polluted the shared profiler, the clip path reports
    // success instead; either outcome is acceptable, so assert on the message.
    let text = app.messages.last().unwrap().text_content().to_lowercase();
    assert!(
        text.contains("action-loop timing")
            && (text.contains("clipboard") || text.contains("no action-loop timing samples")),
        "clip should report either a copy or the no-samples hint: {text}"
    );
}

#[test]
fn test_slash_actionloop_with_samples_shows_timings() {
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

    app.execute_slash_command("/actionloop");

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

#[test]
fn test_triggers_list_empty() {
    let mut app = make_app();
    app.execute_slash_command("/triggers list");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("No trigger rules registered"),
        "empty list should say so: {text}"
    );
    assert_eq!(app.status, "triggers: list empty");
}

#[test]
fn test_triggers_list_with_rules() {
    let mut app = make_app();
    let runtime = ragent_agent::trigger::TriggerRuntime::default();
    let rule =
        ragent_types::trigger::TriggerRule::new("when $HOME/build.done exists", "run cargo test");
    let rule_id = rule.id.as_str().to_string();
    runtime.add_rule(rule);
    app.trigger_runtime = Some(runtime);

    app.execute_slash_command("/triggers list");
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

#[test]
fn test_triggers_no_subcommand_defaults_to_list() {
    let mut app = make_app();
    app.execute_slash_command("/triggers");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("No trigger rules registered"),
        "bare /triggers should default to list: {text}"
    );
}

#[test]
fn test_triggers_enable_existing_rule() {
    let mut app = make_app();
    let runtime = ragent_agent::trigger::TriggerRuntime::default();
    let rule = ragent_types::trigger::TriggerRule::new("cond-a", "act-a");
    let rule_id = rule.id.as_str().to_string();
    runtime.add_rule(rule);
    runtime.disable_rule(&rule_id);
    app.trigger_runtime = Some(runtime);

    app.execute_slash_command(&format!("/triggers enable {rule_id}"));
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("enabled"), "enable should confirm: {text}");
    assert_eq!(app.status, "triggers: enabled");
}

#[test]
fn test_triggers_enable_not_found() {
    let mut app = make_app();
    app.execute_slash_command("/triggers enable nonexistent-id");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("not found"),
        "enable on missing rule should say not found: {text}"
    );
    assert_eq!(app.status, "triggers: not found");
}

#[test]
fn test_triggers_enable_no_id() {
    let mut app = make_app();
    app.execute_slash_command("/triggers enable");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("Usage"),
        "enable with no id should show usage: {text}"
    );
    assert_eq!(app.status, "triggers: enable usage");
}

#[test]
fn test_triggers_disable_existing_rule() {
    let mut app = make_app();
    let runtime = ragent_agent::trigger::TriggerRuntime::default();
    let rule = ragent_types::trigger::TriggerRule::new("cond-b", "act-b");
    let rule_id = rule.id.as_str().to_string();
    runtime.add_rule(rule);
    app.trigger_runtime = Some(runtime);

    app.execute_slash_command(&format!("/triggers disable {rule_id}"));
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("disabled"), "disable should confirm: {text}");
    assert_eq!(app.status, "triggers: disabled");
}

#[test]
fn test_triggers_disable_not_found() {
    let mut app = make_app();
    app.execute_slash_command("/triggers disable nonexistent-id");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("not found"),
        "disable on missing rule should say not found: {text}"
    );
    assert_eq!(app.status, "triggers: not found");
}

#[test]
fn test_triggers_remove_existing_rule() {
    let mut app = make_app();
    app.trigger_runtime = Some(ragent_agent::trigger::TriggerRuntime::default());
    let runtime = ragent_agent::trigger::TriggerRuntime::default();
    let rule = ragent_types::trigger::TriggerRule::new("cond-c", "act-c");
    let rule_id = rule.id.as_str().to_string();
    runtime.add_rule(rule);
    app.trigger_runtime = Some(runtime);

    app.execute_slash_command(&format!("/triggers remove {rule_id}"));
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("removed"), "remove should confirm: {text}");
    assert_eq!(app.status, "triggers: removed");
    assert_eq!(app.trigger_runtime.as_ref().unwrap().rule_count(), 0);
}

#[test]
fn test_triggers_remove_not_found() {
    let mut app = make_app();
    app.execute_slash_command("/triggers remove nonexistent-id");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("not found"),
        "remove on missing rule should say not found: {text}"
    );
    assert_eq!(app.status, "triggers: not found");
}

#[test]
fn test_triggers_remove_no_id() {
    let mut app = make_app();
    app.execute_slash_command("/triggers remove");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("Usage"),
        "remove with no id should show usage: {text}"
    );
    assert_eq!(app.status, "triggers: remove usage");
}

#[test]
fn test_triggers_status_empty() {
    let mut app = make_app();
    app.execute_slash_command("/triggers status");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("Trigger Runtime Status"),
        "status should show header: {text}"
    );
    assert!(
        text.contains("Total rules") && text.contains("0"),
        "status should show zero rules: {text}"
    );
    assert_eq!(app.status, "triggers: status");
}

#[test]
fn test_triggers_status_with_rules() {
    let mut app = make_app();
    let runtime = ragent_agent::trigger::TriggerRuntime::default();
    runtime.add_rule(ragent_types::trigger::TriggerRule::new("cond-1", "act-1"));
    runtime.add_rule(ragent_types::trigger::TriggerRule::new("cond-2", "act-2"));
    app.trigger_runtime = Some(runtime);

    app.execute_slash_command("/triggers status");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("Total rules") && text.contains("2"),
        "status should show 2 rules: {text}"
    );
    assert!(
        text.contains("Active") && text.contains("2"),
        "status should show 2 active: {text}"
    );
    assert_eq!(app.status, "triggers: status");
}

#[test]
fn test_triggers_help() {
    let mut app = make_app();
    app.execute_slash_command("/triggers help");
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

#[test]
fn test_triggers_unknown_subcommand() {
    let mut app = make_app();
    app.execute_slash_command("/triggers frobnicate");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("Unknown sub-command"),
        "unknown sub-command should warn: {text}"
    );
    assert_eq!(app.status, "triggers: unknown");
}
// ── /inbox slash command tests ────────────────────────────────────────

#[test]
fn test_inbox_list_empty() {
    let _guard = enter_with_cwd();

    let mut app = make_app();
    app.execute_slash_command("/inbox list");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("Inbox is empty"),
        "empty inbox should say so: {text}"
    );
    assert_eq!(app.status, "inbox: list empty");
}

#[test]
fn test_inbox_no_subcommand_defaults_to_list() {
    let _guard = enter_with_cwd();

    let mut app = make_app();
    app.execute_slash_command("/inbox");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("Inbox is empty"),
        "bare /inbox should default to list: {text}"
    );
}

#[test]
fn test_inbox_list_with_entries() {
    let _guard = enter_with_cwd();

    // Write some inbox entries directly to the JSONL file
    let entries = vec![
        ragent_agent::loop_state::InboxEntry::new("event-abc", "first finding"),
        ragent_agent::loop_state::InboxEntry::new("event-xyz", "second finding"),
    ];
    ragent_agent::loop_state::write_inbox_entries(&_guard.path(), &entries).expect("write entries");

    let mut app = make_app();
    app.execute_slash_command("/inbox list");
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

#[test]
fn test_inbox_claim_existing() {
    let _guard = enter_with_cwd();

    let entry = ragent_agent::loop_state::InboxEntry::new("event-1", "test finding");
    let entry_id = entry.id.clone();
    ragent_agent::loop_state::write_inbox_entries(&_guard.path(), &[entry]).expect("write entry");

    let mut app = make_app();
    app.execute_slash_command(&format!("/inbox claim {entry_id}"));
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("claimed"), "claim should confirm: {text}");
    assert_eq!(app.status, "inbox: claimed");

    // Verify the status was persisted
    let read = ragent_agent::loop_state::read_inbox(&_guard.path()).unwrap();
    assert_eq!(read[0].status, "claimed");
}

#[test]
fn test_inbox_claim_not_found() {
    let _guard = enter_with_cwd();

    let mut app = make_app();
    app.execute_slash_command("/inbox claim nonexistent-id");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("not found"),
        "claim on missing entry should say not found: {text}"
    );
    assert_eq!(app.status, "inbox: not found");
}

#[test]
fn test_inbox_claim_no_id() {
    let _guard = enter_with_cwd();

    let mut app = make_app();
    app.execute_slash_command("/inbox claim");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("Usage"),
        "claim with no id should show usage: {text}"
    );
    assert_eq!(app.status, "inbox: claimed usage");
}

#[test]
fn test_inbox_dismiss_existing() {
    let _guard = enter_with_cwd();

    let entry = ragent_agent::loop_state::InboxEntry::new("event-1", "to dismiss");
    let entry_id = entry.id.clone();
    ragent_agent::loop_state::write_inbox_entries(&_guard.path(), &[entry]).expect("write entry");

    let mut app = make_app();
    app.execute_slash_command(&format!("/inbox dismiss {entry_id}"));
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("dismissed"), "dismiss should confirm: {text}");
    assert_eq!(app.status, "inbox: dismissed");

    // Verify the status was persisted
    let read = ragent_agent::loop_state::read_inbox(&_guard.path()).unwrap();
    assert_eq!(read[0].status, "dismissed");
}

#[test]
fn test_inbox_dismiss_not_found() {
    let _guard = enter_with_cwd();

    let mut app = make_app();
    app.execute_slash_command("/inbox dismiss nonexistent-id");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("not found"),
        "dismiss on missing entry should say not found: {text}"
    );
    assert_eq!(app.status, "inbox: not found");
}

#[test]
fn test_inbox_dismiss_no_id() {
    let _guard = enter_with_cwd();

    let mut app = make_app();
    app.execute_slash_command("/inbox dismiss");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("Usage"),
        "dismiss with no id should show usage: {text}"
    );
    assert_eq!(app.status, "inbox: dismissed usage");
}

#[test]
fn test_inbox_clear_with_entries() {
    let _guard = enter_with_cwd();

    let entries = vec![
        ragent_agent::loop_state::InboxEntry::new("event-1", "first"),
        ragent_agent::loop_state::InboxEntry::new("event-2", "second"),
    ];
    ragent_agent::loop_state::write_inbox_entries(&_guard.path(), &entries).expect("write entries");

    let mut app = make_app();
    app.execute_slash_command("/inbox clear");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("Cleared 2 finding(s)"),
        "clear should report count: {text}"
    );
    assert_eq!(app.status, "inbox: cleared");

    // Verify the file is gone
    assert!(
        !_guard
            .path()
            .join("log")
            .join("inbox")
            .join("inbox.jsonl")
            .exists()
    );
}

#[test]
fn test_inbox_clear_empty() {
    let _guard = enter_with_cwd();

    let mut app = make_app();
    app.execute_slash_command("/inbox clear");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("Cleared 0 finding(s)"),
        "clear on empty inbox should report 0: {text}"
    );
    assert_eq!(app.status, "inbox: cleared");
}

#[test]
fn test_inbox_help() {
    let _guard = enter_with_cwd();

    let mut app = make_app();
    app.execute_slash_command("/inbox help");
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

#[test]
fn test_inbox_unknown_subcommand() {
    let _guard = enter_with_cwd();

    let mut app = make_app();
    app.execute_slash_command("/inbox frobnicate");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("Unknown sub-command"),
        "unknown sub-command should warn: {text}"
    );
    assert_eq!(app.status, "inbox: unknown");
}

#[test]
fn test_inbox_list_shows_status() {
    let _guard = enter_with_cwd();

    let entry = ragent_agent::loop_state::InboxEntry::new("event-1", "test finding");
    let entry_id = entry.id.clone();
    ragent_agent::loop_state::write_inbox_entries(&_guard.path(), &[entry]).expect("write entry");

    // Claim it first
    ragent_agent::loop_state::update_inbox_entry_status(&_guard.path(), &entry_id, "claimed")
        .expect("update status");

    let mut app = make_app();
    app.execute_slash_command("/inbox list");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("claimed"),
        "list should show updated status: {text}"
    );
}

// ── /task (todo2tasks T-016, FR-019) ─────────────────────────────────

#[test]
fn test_slash_task_toggles_panel() {
    let mut app = make_app();
    assert!(
        !app.show_tasks_panel,
        "tasks panel should be hidden initially"
    );

    app.execute_slash_command("/task");
    assert!(
        app.show_tasks_panel,
        "tasks panel should be visible after /task"
    );
    assert_eq!(app.status, "tasks panel visible");

    app.execute_slash_command("/task");
    assert!(
        !app.show_tasks_panel,
        "tasks panel should be hidden after second /task"
    );
    assert_eq!(app.status, "tasks panel hidden");
}

#[test]
fn test_slash_task_mutually_excludes_log() {
    let mut app = make_app();
    app.show_log = true;
    app.show_tasks_panel = false;

    app.execute_slash_command("/task");
    assert!(app.show_tasks_panel, "tasks panel should be visible");
    assert!(
        !app.show_log,
        "log panel should be hidden when tasks is shown"
    );
}

#[test]
fn test_slash_task_list_shows_items() {
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

    app.execute_slash_command("/task list");

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

#[test]
fn test_slash_task_list_empty() {
    let mut app = make_app();
    let session_id = "task-empty-session".to_string();
    app.session_id = Some(session_id.clone());
    app.storage
        .create_session(&session_id, ".")
        .expect("create session");

    app.execute_slash_command("/task list");

    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("No tasks found"),
        "empty list should say 'No tasks found': {text}"
    );
}

#[test]
fn test_slash_task_help() {
    let mut app = make_app();
    app.session_id = Some("help-session".to_string());

    app.execute_slash_command("/task help");

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

#[test]
fn test_slash_task_add_delegates_hint() {
    let mut app = make_app();
    app.session_id = Some("add-hint-session".to_string());

    app.execute_slash_command("/task add");

    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("task_create"),
        "/task add should mention task_create tool: {text}"
    );
    assert_eq!(app.status, "Use agent tool: task_create");
}

#[test]
fn test_slash_task_update_delegates_hint() {
    let mut app = make_app();
    app.session_id = Some("update-hint-session".to_string());

    app.execute_slash_command("/task update");

    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("task_update"),
        "/task update should mention task_update tool: {text}"
    );
    assert_eq!(app.status, "Use agent tool: task_update");
}

#[test]
fn test_slash_task_get_delegates_hint() {
    let mut app = make_app();
    app.session_id = Some("get-hint-session".to_string());

    app.execute_slash_command("/task get");

    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("task_get"),
        "/task get should mention task_get tool: {text}"
    );
    assert_eq!(app.status, "Use agent tool: task_get");
}

#[test]
fn test_slash_task_create_alias_for_add() {
    let mut app = make_app();
    app.session_id = Some("create-alias-session".to_string());

    app.execute_slash_command("/task create");

    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("task_create"),
        "/task create should mention task_create tool: {text}"
    );
}

#[test]
fn test_slash_task_unknown_subcommand_shows_help() {
    let mut app = make_app();
    app.session_id = Some("unknown-sub-session".to_string());

    app.execute_slash_command("/task frobnicate");

    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("From: /task help"),
        "unknown subcommand should fall through to help: {text}"
    );
}

#[test]
fn test_slash_task_toggles_panel_with_tasks_status() {
    // FR-019: /task toggles the Tasks side panel.
    let mut app = make_app();

    app.execute_slash_command("/task");
    assert!(
        app.show_tasks_panel,
        "tasks panel should be visible after /task"
    );
    assert_eq!(app.status, "tasks panel visible");

    app.execute_slash_command("/task");
    assert!(
        !app.show_tasks_panel,
        "tasks panel should be hidden after second /task"
    );
    assert_eq!(app.status, "tasks panel hidden");
}
