//! Tests for the GCF status-bar indicator and the Alt+G toggle.
//!
//! Covers:
//! - Alt+G toggles GCF encoding through the real persist path
//!   (`ragent_config::gcf::toggle_persist`) and reports the new state.
//! - The GCF icon renders on the second status-bar line, LEFT of the
//!   codeindex icon, with the enabled (`✓`) / disabled (`✗`) marker.
//! - The `g` keystroke is never inserted into the input buffer (NFR-002).

use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ragent_agent::{
    event::EventBus, permission::PermissionChecker, provider, session::SessionManager,
    session::processor::SessionProcessor, storage::Storage, tool,
};
use ragent_tui::{App, layout};
use ratatui::{Terminal, backend::TestBackend};

/// Build an [`App`] backed by an in-memory database (mirrors the pattern used
/// by the slash-command tests).
fn make_app() -> App {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
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
        skill_registry_cache: parking_lot::Mutex::new(None),
        active_loops: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        active_loop_specs: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        loop_telemetry_recorded: std::sync::atomic::AtomicBool::new(false),
        active_loop_interrupts: parking_lot::RwLock::new(std::collections::HashMap::new()),
        active_loop_captures: tokio::sync::RwLock::new(std::collections::HashMap::new()),
    });
    let agent_info =
        ragent_agent::agent::resolve_agent("general", &Default::default()).expect("agent");
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
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.prev);
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

/// Create a tempdir, chdir into it with a project-local config primed for
/// GCF-off, and return the cwd-restoring guard. The tempdir is removed only
/// after cwd has been restored.
fn enter_isolated_config_project() -> (CwdGuard, tempfile::TempDir) {
    let lock = cwd_lock();
    let prev = std::env::current_dir().expect("current dir");
    let temp = tempfile::TempDir::new().expect("tempdir");
    std::env::set_current_dir(temp.path()).expect("set cwd");
    let ragent_dir = temp.path().join(".ragent");
    std::fs::create_dir_all(&ragent_dir).expect("create .ragent");
    std::fs::write(
        ragent_dir.join("ragent.json"),
        r#"{"gcf": {"enabled": false}}"#,
    )
    .expect("write project config");
    (CwdGuard { prev, lock }, temp)
}

/// Render the app and return the full terminal buffer text.
fn render_to_text(app: &mut App) -> String {
    let backend = TestBackend::new(140, 30);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| layout::render(frame, app))
        .expect("draw");
    let cells = terminal.backend().buffer().content.clone();
    cells.iter().map(ratatui::buffer::Cell::symbol).collect()
}

#[test]
fn test_alt_g_toggles_gcf_and_status_bar_indicator() {
    let (_guard, _temp) = enter_isolated_config_project();
    ragent_config::gcf::set_enabled(false);

    // Warm the config cache BEFORE the toggle so the real project-config
    // resolution path (which mirrors the live app) is exercised.
    // Touch the config loader once so a malformed-config failure surfaces as
    // an explicit expect here rather than a silent toggle failure below.
    ragent_config::Config::load().expect("config load");

    let mut app = make_app();

    // Sanity: GCF starts off.
    assert!(!ragent_config::gcf::is_enabled());

    // Press Alt+G through the app handler so the persist path runs.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::ALT));

    // Handler should have toggled and persisted GCF on.
    assert!(ragent_config::gcf::is_enabled());
    assert!(app.status.contains("GCF encoding enabled"));

    // Status bar should show the enabled GCF icon next to the codeindex icon.
    let text = render_to_text(&mut app);
    assert!(
        text.contains("🗜") && text.contains("✓"),
        "status bar should show enabled GCF icon: {text}"
    );

    // Toggle back off and verify.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::ALT));
    assert!(!ragent_config::gcf::is_enabled());
    assert!(app.status.contains("GCF encoding disabled"));

    let text = render_to_text(&mut app);
    assert!(
        text.contains("🗜") && text.contains("✗"),
        "status bar should show disabled GCF icon: {text}"
    );
}

#[test]
fn test_alt_g_never_inserts_g_into_input_buffer() {
    let (_guard, _temp) = enter_isolated_config_project();
    ragent_config::gcf::set_enabled(false);

    let mut app = make_app();

    app.handle_key_event(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::ALT));

    assert!(
        app.input.is_empty(),
        "Alt+G must not insert `g` into the input buffer; input was: {:?}",
        app.input
    );
}

#[test]
fn test_gcf_indicator_sits_left_of_codeindex_icon() {
    let (_guard, _temp) = enter_isolated_config_project();
    ragent_config::gcf::set_enabled(true);

    let mut app = make_app();
    let text = render_to_text(&mut app);

    // Both icons must be present on line 2, with GCF appearing before
    // (left of) the codeindex magnifying-glass icon.
    let gcf_pos = text.find("🗜").expect("GCF icon must render when enabled");
    let codeindex_pos = text
        .find("🔍")
        .expect("codeindex icon must render on line 2");
    assert!(
        gcf_pos < codeindex_pos,
        "GCF icon must sit LEFT of the codeindex icon; text: {text}"
    );

    ragent_config::gcf::set_enabled(false);
}
