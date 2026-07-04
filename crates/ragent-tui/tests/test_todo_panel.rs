//! Tests for the TODO side panel (spec `todopanel`).
//!
//! Covers:
//! - T-010: `render_todo_panel` empty / populated / error states (FR-005,
//!   FR-006, FR-013, NFR-005).
//! - T-011: `InputAction::ToggleTodo` toggling and mutual exclusion with the
//!   log and profile panels (FR-002, FR-003, FR-012).
//!
//! Tests live in `crates/ragent-tui/tests/` per the AGENTS.md test-organization
//! rule (no inline `#[cfg(test)]` modules in `src/`).

use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ragent_agent::{
    StreamConfig, agent,
    event::EventBus,
    permission::PermissionChecker,
    provider,
    session::{SessionManager, processor::SessionProcessor},
    storage::Storage,
    tool,
};
use ragent_tui::{
    App,
    app::ScreenMode,
    input::{InputAction, handle_key},
    layout,
};
use ratatui::{Terminal, backend::TestBackend};

/// Build an [`App`] backed by an in-memory database.
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
        task_manager: std::sync::OnceLock::new(),
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
        cached_config: parking_lot::Mutex::new(None),
        team_context_cache: std::sync::Arc::new(parking_lot::RwLock::new(
            std::collections::HashMap::new(),
        )),
        auto_approve: false,
        system_prompt_cache: parking_lot::RwLock::new(None),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
    });
    let agent_info =
        agent::resolve_agent("general", &Default::default()).expect("resolve general agent");

    App::new(
        event_bus,
        storage,
        provider_registry,
        session_processor,
        agent_info,
        false,
        std::path::PathBuf::new(),
    )
}

/// Render the app into a string buffer of the given terminal size, with the
/// TODO panel visible.
fn render_app_to_string(app: &mut App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, app))
        .expect("render todo panel");

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

// ─────────────────────────────────────────────────────────────────────────────
// T-011: ToggleTodo toggling and mutual exclusion (FR-002, FR-003, FR-012)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_alt_t_maps_to_toggle_todo_action() {
    // FR-002: Alt+T must produce InputAction::ToggleTodo when no modal is
    // active (no permission dialog, no provider setup, no slash menu).
    let mut app = make_app();
    let key = KeyEvent::new(KeyCode::Char('t'), KeyModifiers::ALT);
    let action = handle_key(&mut app, key);
    // InputAction does not derive PartialEq, so we match on the variant
    // explicitly instead of using assert_eq!.
    assert!(
        matches!(action, Some(InputAction::ToggleTodo)),
        "Alt+T should produce InputAction::ToggleTodo, got {action:?}"
    );
}

#[test]
fn test_toggle_todo_flips_show_todo_flag() {
    // FR-002: dispatching ToggleTodo via the full key-event path flips
    // `show_todo`.
    let mut app = make_app();
    assert!(!app.show_todo, "show_todo should start false");
    app.handle_key_event(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::ALT));
    assert!(app.show_todo, "first toggle should set show_todo=true");
    app.handle_key_event(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::ALT));
    assert!(!app.show_todo, "second toggle should set show_todo=false");
}

#[test]
fn test_toggle_todo_mutually_excludes_log_panel() {
    // FR-003 / FR-012: enabling the TODO panel must hide the log panel, and
    // enabling the log panel must hide the TODO panel.
    let mut app = make_app();
    // Start with the log panel visible.
    app.show_log = true;
    app.show_todo = false;

    // Enable TODO panel — log must be dismissed.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::ALT));
    assert!(app.show_todo, "TODO panel should be visible");
    assert!(!app.show_log, "log panel must be hidden when TODO is shown");

    // Re-enable log panel — TODO must be dismissed.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::ALT));
    assert!(app.show_log, "log panel should be visible");
    assert!(
        !app.show_todo,
        "TODO panel must be hidden when log is shown"
    );
}

#[test]
fn test_toggle_todo_mutually_excludes_profile_panel() {
    // FR-003 / FR-012: enabling the TODO panel must hide the profile panel,
    // and enabling the profile panel must hide the TODO panel.
    let mut app = make_app();
    app.show_profile = true;
    app.show_todo = false;

    app.handle_key_event(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::ALT));
    assert!(app.show_todo);
    assert!(
        !app.show_profile,
        "profile panel must hide when TODO is shown"
    );

    // Profile toggle uses set_profile_panel_enabled via ToggleProfile.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::ALT));
    assert!(app.show_profile);
    assert!(!app.show_todo, "TODO panel must hide when profile is shown");
}

#[test]
fn test_toggle_todo_status_message_reflects_state() {
    let mut app = make_app();
    app.handle_key_event(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::ALT));
    assert_eq!(app.status, "todo panel visible");
    app.handle_key_event(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::ALT));
    assert_eq!(app.status, "todo panel hidden");
}

// ────────────────────────────────────────────────────────────���────────────────
// T-010: render_todo_panel empty / populated / error (FR-005, FR-006, FR-013,
//        NFR-005)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_render_todo_panel_empty_shows_placeholder() {
    // FR-005: with zero TODO items the panel shows "No TODO items" in dark
    // gray. We render with a session id that has no todos and assert the
    // placeholder text appears in the buffer.
    let mut app = make_app();
    app.session_id = Some("empty-session".to_string());
    app.show_todo = true;
    app.show_log = false;
    app.show_profile = false;
    app.current_screen = ScreenMode::Chat;

    let text = render_app_to_string(&mut app, 120, 40);
    assert!(
        text.contains("No TODO items"),
        "empty TODO panel should render 'No TODO items' placeholder; got:\n{text}"
    );
    assert!(
        text.contains("TODO"),
        "TODO panel border/title should be rendered"
    );
}

#[test]
fn test_render_todo_panel_populated_shows_items() {
    // FR-006 / FR-013: with TODO items the panel renders one line per item
    // formatted as `[<STATUS>] <title>` ordered by created_at ascending.
    let mut app = make_app();
    let session_id = "todo-session".to_string();
    app.session_id = Some(session_id.clone());
    app.show_todo = true;
    app.show_log = false;
    app.show_profile = false;
    app.current_screen = ScreenMode::Chat;

    // The todos table has a FOREIGN KEY on session_id → sessions(id), so we
    // must create the session row before inserting todos.
    app.storage
        .create_session(&session_id, ".")
        .expect("create session row");

    // Insert three todos with distinct created_at timestamps. create_todo
    // uses the current time, so ordering is insertion order; the titles are
    // crafted so the expected ascending order is "first", "second", "third".
    app.storage
        .create_todo("t1", &session_id, "first task", "pending", "")
        .expect("create todo t1");
    app.storage
        .create_todo("t2", &session_id, "second task", "in_progress", "")
        .expect("create todo t2");
    app.storage
        .create_todo("t3", &session_id, "third task", "done", "")
        .expect("create todo t3");

    let text = render_app_to_string(&mut app, 120, 40);
    assert!(
        text.contains("[PENDING] first task"),
        "should render [PENDING] first task; got:\n{text}"
    );
    assert!(
        text.contains("[IN_PROGRESS] second task"),
        "should render [IN_PROGRESS] second task; got:\n{text}"
    );
    assert!(
        text.contains("[DONE] third task"),
        "should render [DONE] third task; got:\n{text}"
    );
}

#[test]
fn test_render_todo_panel_unknown_status_uses_dark_gray() {
    // FR-007: unrecognised statuses fall back to DarkGray. We can't inspect
    // colour directly from the TestBackend buffer without pulling in extra
    // helpers, so this test just confirms the row still renders with the
    // uppercased status prefix and does not panic.
    let mut app = make_app();
    let session_id = "weird-session".to_string();
    app.session_id = Some(session_id.clone());
    app.show_todo = true;
    app.current_screen = ScreenMode::Chat;

    app.storage
        .create_session(&session_id, ".")
        .expect("create session row");
    app.storage
        .create_todo("w1", &session_id, "weird task", "banana", "")
        .expect("create weird todo");

    let text = render_app_to_string(&mut app, 120, 40);
    assert!(
        text.contains("[BANANA] weird task"),
        "unknown status should render uppercased prefix; got:\n{text}"
    );
}

#[test]
fn test_render_todo_panel_does_not_mutate_todos() {
    // FR-011: rendering the panel is read-only with respect to the todos
    // table. Re-rendering must not change the stored rows.
    let mut app = make_app();
    let session_id = "immutable-session".to_string();
    app.session_id = Some(session_id.clone());
    app.show_todo = true;
    app.current_screen = ScreenMode::Chat;

    app.storage
        .create_session(&session_id, ".")
        .expect("create session row");
    app.storage
        .create_todo("i1", &session_id, "immutable task", "pending", "")
        .expect("create immutable todo");

    let before = app
        .storage
        .get_todos(&session_id, None)
        .expect("read todos before render");
    // Render twice.
    let _ = render_app_to_string(&mut app, 120, 40);
    let _ = render_app_to_string(&mut app, 120, 40);
    let after = app
        .storage
        .get_todos(&session_id, None)
        .expect("read todos after render");

    assert_eq!(before.len(), after.len(), "row count must not change");
    assert_eq!(after[0].title, "immutable task");
    assert_eq!(after[0].status, "pending");
}

#[test]
fn test_render_todo_panel_no_session_shows_empty_placeholder() {
    // When there is no active session id, get_todos is not called and the
    // panel falls back to the empty placeholder (no panic).
    let mut app = make_app();
    app.session_id = None;
    app.show_todo = true;
    app.current_screen = ScreenMode::Chat;

    let text = render_app_to_string(&mut app, 120, 40);
    assert!(
        text.contains("No TODO items"),
        "no-session TODO panel should render 'No TODO items' placeholder; got:\n{text}"
    );
}

#[test]
fn test_render_todo_panel_sets_todo_area_rect() {
    // FR-015: while the TODO panel is visible, `app.todo_area` must be
    // populated with the panel's rect so mouse hit-testing works.
    let mut app = make_app();
    app.session_id = Some("area-session".to_string());
    app.show_todo = true;
    app.current_screen = ScreenMode::Chat;

    let _ = render_app_to_string(&mut app, 120, 40);
    assert!(
        app.todo_area.area() > 0,
        "todo_area should be set to a non-empty rect when panel is visible"
    );
}

#[test]
fn test_render_todo_panel_hidden_clears_todo_area() {
    // When the TODO panel is not visible, todo_area should be reset to an
    // empty rect by the layout split.
    let mut app = make_app();
    app.session_id = Some("hidden-session".to_string());
    app.show_todo = false;
    app.current_screen = ScreenMode::Chat;

    let _ = render_app_to_string(&mut app, 120, 40);
    assert_eq!(
        app.todo_area.area(),
        0,
        "todo_area should be empty when panel is hidden"
    );
}

#[test]
fn test_render_todo_panel_scrollbar_appears_when_overflowing() {
    // FR-008: when rendered rows exceed visible height, a vertical scrollbar
    // is rendered on the right edge. We use a short terminal height and many
    // todos to force overflow, then assert that `todo_max_scroll` is greater
    // than zero (the scrollbar only renders when total_lines > visible_height,
    // which implies max_scroll > 0 for single-line rows).
    let mut app = make_app();
    let session_id = "overflow-session".to_string();
    app.session_id = Some(session_id.clone());
    app.show_todo = true;
    app.current_screen = ScreenMode::Chat;

    app.storage
        .create_session(&session_id, ".")
        .expect("create session row");

    // Insert more todos than the panel height can show.
    for i in 0..40 {
        app.storage
            .create_todo(
                &format!("o{i}"),
                &session_id,
                &format!("task {i}"),
                "pending",
                "",
            )
            .expect("create overflow todo");
    }

    let _ = render_app_to_string(&mut app, 120, 20);
    assert!(
        app.todo_max_scroll > 0,
        "todo_max_scroll should be > 0 when content overflows the panel, got {}",
        app.todo_max_scroll
    );
}
