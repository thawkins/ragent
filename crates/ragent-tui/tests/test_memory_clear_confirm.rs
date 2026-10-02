//! Tests for the `/memory clear` confirmation dialog.
//!
//! Covers the `Yes`/`No` gate added to `/memory clear`:
//! - the slash command opens the dialog and removes nothing,
//! - `Yes` clears only the current project's memories,
//! - `No` and `Esc` leave memory unchanged,
//! - the dialog defaults to `No` so a stray `Enter` cannot clear memory.
//!
//! Tests live in `crates/ragent-tui/tests/` per the AGENTS.md test
//! organization rule (no inline `#[cfg(test)]` modules in `src/`).

use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
use tempfile::TempDir;

use ragent_agent::{
    StreamConfig, agent,
    event::EventBus,
    permission::PermissionChecker,
    provider,
    session::{SessionManager, processor::SessionProcessor},
    storage::Storage,
    tool,
};
use ragent_tui::{App, app::ScreenMode, layout};

// ─────────────────────────────────────────────────────────────────────────────
// App fixture (mirrors `test_memory_panel.rs`)
// ─────────────────────────────────────────────────────────────────────────────

/// Build an [`App`] backed by an in-memory database.
fn make_app() -> App {
    let event_bus = Arc::new(EventBus::default());
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
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
        connector_session: tokio::sync::RwLock::new(None),
        connector_statuses: tokio::sync::RwLock::new(None),
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

/// Render the app into a flattened string for the given terminal size.
fn render_app_to_string(app: &mut App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, app))
        .expect("render app");

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

/// Seed a structured memory attributed to `dir`.
fn seed_project_memory(storage: &Storage, dir: &std::path::Path, content: &str) {
    storage
        .create_memory(
            content,
            "fact",
            "test",
            0.7,
            &dir.to_string_lossy(),
            "",
            &[],
        )
        .expect("seed memory");
}

/// Serialise `std::env::set_current_dir` across parallel tests.
struct CwdGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    prev: std::path::PathBuf,
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.prev);
    }
}

/// Change the process working directory to `dir`, returning a restoring guard.
fn with_cwd(dir: &std::path::Path) -> CwdGuard {
    static CWD_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let lock = CWD_MUTEX.lock().expect("cwd mutex poisoned");
    let prev = std::env::current_dir().expect("current dir");
    std::env::set_current_dir(dir).expect("set_current_dir");
    CwdGuard { _lock: lock, prev }
}

/// Count memories stored against `dir`.
fn project_memory_count(app: &App, dir: &std::path::Path) -> u64 {
    app.storage
        .count_memories_for_project(dir)
        .expect("count memories")
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

/// `/memory clear` must open the dialog and remove nothing.
#[tokio::test]
async fn test_memory_clear_opens_confirmation_without_deleting() {
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    seed_project_memory(&app.storage, dir.path(), "keep me: alpha");
    app.current_screen = ScreenMode::Chat;

    app.execute_slash_command("/memory clear").await;

    assert!(
        app.memory_clear_confirm_open,
        "/memory clear must open the confirmation dialog"
    );
    assert!(
        !app.memory_clear_confirm_is_yes(),
        "the dialog must default to No"
    );
    assert_eq!(
        project_memory_count(&app, dir.path()),
        1,
        "opening the dialog must not delete anything"
    );

    let text = render_app_to_string(&mut app, 140, 40);
    assert!(
        text.contains("Clear this project's memory?"),
        "the dialog title must be rendered"
    );
    assert!(text.contains("Yes"), "the dialog must offer Yes");
    assert!(text.contains("No"), "the dialog must offer No");
}

/// `Esc` dismisses the dialog and leaves memory untouched.
#[tokio::test]
async fn test_memory_clear_esc_keeps_memories() {
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    seed_project_memory(&app.storage, dir.path(), "keep me: alpha");
    app.current_screen = ScreenMode::Chat;

    app.execute_slash_command("/memory clear").await;
    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        .await;

    assert!(
        !app.memory_clear_confirm_open,
        "Esc must close the confirmation dialog"
    );
    assert_eq!(
        project_memory_count(&app, dir.path()),
        1,
        "Esc must leave memory unchanged"
    );
}

/// `No` (the default selection) dismisses without clearing.
#[tokio::test]
async fn test_memory_clear_no_keeps_memories() {
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    seed_project_memory(&app.storage, dir.path(), "keep me: alpha");
    app.current_screen = ScreenMode::Chat;

    app.execute_slash_command("/memory clear").await;
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .await;

    assert!(
        !app.memory_clear_confirm_open,
        "Enter on the default No must close the dialog"
    );
    assert_eq!(
        project_memory_count(&app, dir.path()),
        1,
        "the default No selection must leave memory unchanged"
    );
}

/// `Yes` clears the current project's memories.
#[tokio::test]
async fn test_memory_clear_yes_deletes_project_memories() {
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    seed_project_memory(&app.storage, dir.path(), "delete me: alpha");
    seed_project_memory(&app.storage, dir.path(), "delete me: beta");
    app.current_screen = ScreenMode::Chat;

    app.execute_slash_command("/memory clear").await;
    // Right arrow moves the selection from No to Yes.
    app.handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE))
        .await;
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .await;

    assert!(
        !app.memory_clear_confirm_open,
        "Yes must close the confirmation dialog"
    );
    assert_eq!(
        project_memory_count(&app, dir.path()),
        0,
        "Yes must clear every memory for this project"
    );
}
