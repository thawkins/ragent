//! Tests for the Memory side panel (spec `mempanel`).
//!
//! Covers two spec tasks:
//! - **T-007**: extending mouse hit-testing in `session_ops.rs` (`pane_at`)
//!   and `input_handler.rs` (scroll, scrollbar drag, left-click selection
//!   start, right-click context menu) to recognise clicks inside
//!   `memory_area` so users can select text in and open the context menu on
//!   the Memory panel the same way they can on the Log / Profile / TODO
//!   panels (FR-013).
//! - **T-012**: `Alt+M` toggle flips `show_memory` (FR-003), mutual exclusion
//!   with `show_log` / `show_todo` / `show_profile` (FR-004), `Alt+M` does
//!   not insert `m` into the input buffer (FR-011), `render_memory_panel`
//!   with populated and missing files (FR-015), and scroll-offset bounds
//!   (FR-009).
//!
//! Tests live in `crates/ragent-tui/tests/` per the AGENTS.md test
//! organization rule (no inline `#[cfg(test)]` modules in `src/`).

use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
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
    tool::{Tool, ToolContext, structured_memory::MemoryStoreTool},
};
use ragent_tui::{
    App,
    app::{ContextAction, ScreenMode, ScrollbarDragPane, SelectionPane, TextSelection},
    input::{InputAction, handle_key},
    layout,
};
use serde_json::json;

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
        task_manager: std::sync::OnceLock::new(),
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
    });
    let agent_info =
        agent::resolve_agent("general", &Default::default()).expect("resolve general agent");

    App::new(
        event_bus,
        storage,
        provider_registry,
        session_processor,
        agent_info,
        true,
        std::path::PathBuf::new(),
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Mouse-event constructors
// ────────────────────���────────────────────────────────────────────────────────

const fn mouse_down(col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: col,
        row,
        modifiers: crossterm::event::KeyModifiers::empty(),
    }
}

const fn mouse_drag(col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: col,
        row,
        modifiers: crossterm::event::KeyModifiers::empty(),
    }
}

const fn mouse_up(col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: col,
        row,
        modifiers: crossterm::event::KeyModifiers::empty(),
    }
}

const fn right_click(col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: col,
        row,
        modifiers: crossterm::event::KeyModifiers::empty(),
    }
}

const fn mouse_scroll_up(col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::ScrollUp,
        column: col,
        row,
        modifiers: crossterm::event::KeyModifiers::empty(),
    }
}

const fn mouse_scroll_down(col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: col,
        row,
        modifiers: crossterm::event::KeyModifiers::empty(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Render helper
// ─────────────────────────────────────────────────────────────────────────────

/// Render the app into a string buffer of the given terminal size.
///
/// Mirrors the helper in `test_todo_panel.rs`: draws `layout::render` into a
/// `TestBackend` and flattens the cell buffer into a single string so tests
/// can assert on visible text.
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

/// Serialise `std::env::set_current_dir` across parallel tests.
///
/// `render_memory_panel` resolves the project memory path from
/// `std::env::current_dir()`, so the populated/missing-file render tests need
/// to change the process working directory. `set_current_dir` is process-
/// global, so a static mutex guards against parallel tests trampling each
/// other's cwd. The guard restores the original cwd when dropped.
struct CwdGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    prev: std::path::PathBuf,
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.prev);
    }
}

/// Acquire the cwd mutex and change the process working directory to `dir`,
/// returning a guard that restores the previous cwd on drop.
fn with_cwd(dir: &std::path::Path) -> CwdGuard {
    static CWD_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let lock = CWD_MUTEX.lock().expect("cwd mutex poisoned");
    let prev = std::env::current_dir().expect("current dir");
    std::env::set_current_dir(dir).expect("set_current_dir");
    CwdGuard { _lock: lock, prev }
}

/// Seed a structured memory for the project identified by `dir`.
fn seed_project_memory_with_tags(
    storage: &Storage,
    dir: &std::path::Path,
    content: &str,
    tags: &[String],
) {
    let project = dir.to_string_lossy().to_string();
    storage
        .create_memory(content, "fact", "test", 0.7, &project, "", tags)
        .expect("seed memory");
}

/// Seed a structured memory for the project identified by `dir`.
fn seed_project_memory(storage: &Storage, dir: &std::path::Path, content: &str) {
    seed_project_memory_with_tags(storage, dir, content, &[]);
}

// ═════════════════════════════════════════════════════════════════════════════
// T-007: `pane_at` recognises `memory_area` (FR-013 hit-testing)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn test_pane_at_returns_memory_when_click_inside_memory_area() {
    // FR-013: `App::pane_at` must map clicks inside `memory_area` to
    // `SelectionPane::Memory` so the rest of the mouse pipeline (selection
    // start, context menu, scrollbar drag) can route on that pane.
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    app.show_memory = true;
    app.memory_area = Rect::new(80, 1, 30, 20);

    let pane = app.pane_at(90, 10);
    assert_eq!(pane, Some(SelectionPane::Memory));
}

#[test]
fn test_pane_at_returns_none_for_memory_when_panel_hidden() {
    // When `show_memory` is false, clicks inside the cached `memory_area`
    // rect must NOT be reported as the Memory pane — the panel is not
    // visible so hit-testing should fall through (FR-002 mutual exclusion
    // relies on `show_memory` gating `memory_area`).
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    app.show_memory = false;
    // Leave memory_area non-empty to prove the `show_memory` gate is what
    // blocks the match, not a zeroed rect.
    app.memory_area = Rect::new(80, 1, 30, 20);

    let pane = app.pane_at(90, 10);
    assert_ne!(pane, Some(SelectionPane::Memory));
}

// ─────────────────────────────────────────────────────────────────────────────
// T-007: mouse scroll inside `memory_area` adjusts `memory_scroll_offset`
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_mouse_scroll_up_on_memory_increments_memory_scroll_offset() {
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    app.show_memory = true;
    app.memory_area = Rect::new(80, 1, 30, 20);
    assert_eq!(app.memory_scroll_offset, 0);

    app.handle_mouse_event(mouse_scroll_up(90, 10));
    assert_eq!(app.memory_scroll_offset, 3);
    // Messages pane must be untouched.
    assert_eq!(app.scroll_offset, 0);
}

#[test]
fn test_mouse_scroll_down_on_memory_decrements_memory_scroll_offset() {
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    app.show_memory = true;
    app.memory_area = Rect::new(80, 1, 30, 20);
    app.memory_scroll_offset = 9;

    app.handle_mouse_event(mouse_scroll_down(90, 10));
    assert_eq!(app.memory_scroll_offset, 6);
}

// ─────────────────────────────────────────────────────────────────────────────
// T-007: scrollbar-gutter click on `memory_area` starts a Memory drag
// ────────────────────────────────────────────────────���────────────────────────

#[test]
fn test_drag_starts_on_memory_scrollbar_column() {
    // Clicking the rightmost column of `memory_area` (the scrollbar gutter,
    // matching the Log / Profile / TODO behaviour) should initiate a
    // `ScrollbarDragPane::Memory` drag and clear any active text selection.
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    app.show_memory = true;
    app.memory_area = Rect::new(80, 1, 30, 20);
    app.memory_max_scroll = 50;

    // Pre-existing selection that the scrollbar click must clear.
    app.text_selection = Some(TextSelection {
        pane: SelectionPane::Memory,
        anchor: (82, 5),
        endpoint: (90, 5),
    });

    // Scrollbar sits at column 109 (80 + 30 - 1).
    app.handle_mouse_event(mouse_down(109, 10));
    assert_eq!(app.scrollbar_drag, Some(ScrollbarDragPane::Memory));
    assert!(app.text_selection.is_none());
}

#[test]
fn test_drag_memory_scrollbar_moves_offset() {
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    app.show_memory = true;
    app.memory_area = Rect::new(80, 0, 30, 21);
    app.memory_max_scroll = 60;

    // Click scrollbar (column 109), drag to top → offset = max_scroll.
    app.handle_mouse_event(mouse_down(109, 10));
    assert_eq!(app.scrollbar_drag, Some(ScrollbarDragPane::Memory));

    app.handle_mouse_event(mouse_drag(109, 0));
    assert_eq!(app.memory_scroll_offset, 60);

    // Drag to bottom → offset = 0.
    app.handle_mouse_event(mouse_drag(109, 20));
    assert_eq!(app.memory_scroll_offset, 0);
}

#[test]
fn test_drag_does_not_start_on_memory_scrollbar_without_scrollable_content() {
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    app.show_memory = true;
    app.memory_area = Rect::new(80, 1, 30, 20);
    app.memory_max_scroll = 0;

    app.handle_mouse_event(mouse_down(109, 10));
    assert!(app.scrollbar_drag.is_none());
}

// ─────────────────────────────────────────────────────────────────────────────
// T-007: left-click inside `memory_area` starts a Memory text selection
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_left_click_inside_memory_area_starts_memory_selection() {
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    app.show_memory = true;
    app.memory_area = Rect::new(80, 1, 30, 20);

    // Click in the body of the panel (not the scrollbar column 109).
    app.handle_mouse_event(mouse_down(90, 10));
    let sel = app.text_selection.as_ref().expect("selection should start");
    assert_eq!(sel.pane, SelectionPane::Memory);
    assert_eq!(sel.anchor, (90, 10));
    assert_eq!(sel.endpoint, (90, 10));
}

#[test]
fn test_drag_extends_memory_selection_endpoint() {
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    app.show_memory = true;
    app.memory_area = Rect::new(80, 1, 30, 20);

    app.handle_mouse_event(mouse_down(85, 3));
    app.handle_mouse_event(mouse_drag(100, 7));

    let sel = app
        .text_selection
        .as_ref()
        .expect("selection should persist");
    assert_eq!(sel.pane, SelectionPane::Memory);
    assert_eq!(sel.anchor, (85, 3));
    assert_eq!(sel.endpoint, (100, 7));
}

#[test]
fn test_mouse_up_preserves_memory_selection() {
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    app.show_memory = true;
    app.memory_area = Rect::new(80, 1, 30, 20);

    app.handle_mouse_event(mouse_down(85, 3));
    app.handle_mouse_event(mouse_drag(100, 7));
    app.handle_mouse_event(mouse_up(100, 7));

    assert!(
        app.text_selection.is_some(),
        "selection should remain after mouse-up"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// T-007: right-click inside `memory_area` opens a Memory context menu
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_right_click_inside_memory_area_opens_context_menu() {
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    app.show_memory = true;
    app.memory_area = Rect::new(80, 1, 30, 20);

    app.handle_mouse_event(right_click(90, 10));
    let menu = app
        .context_menu
        .as_ref()
        .expect("right-click should open context menu");
    assert_eq!(menu.pane, SelectionPane::Memory);
    // The standard 3-item menu (Cut / Copy / Paste) is always offered; Copy
    // is disabled because there is no selection yet.
    assert_eq!(menu.items.len(), 3);
    assert_eq!(menu.items[0], (ContextAction::Cut, false));
    assert_eq!(menu.items[1], (ContextAction::Copy, false));
}

#[test]
fn test_right_click_with_memory_selection_enables_copy() {
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    app.show_memory = true;
    app.memory_area = Rect::new(80, 1, 30, 20);
    app.text_selection = Some(TextSelection {
        pane: SelectionPane::Memory,
        anchor: (82, 5),
        endpoint: (90, 5),
    });

    app.handle_mouse_event(right_click(85, 7));
    let menu = app.context_menu.as_ref().expect("context menu open");
    assert_eq!(menu.pane, SelectionPane::Memory);
    assert_eq!(menu.items[1], (ContextAction::Copy, true));
}

#[test]
fn test_right_click_outside_memory_area_does_not_open_memory_menu() {
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    app.show_memory = true;
    app.memory_area = Rect::new(80, 1, 30, 20);

    // Click in the status bar (row 0) — outside every pane.
    app.handle_mouse_event(right_click(10, 0));
    assert!(app.context_menu.is_none());
}

// ═════════════════════════════════════════════════════════════════════════════
// T-012: Alt+M toggle flips `show_memory` (FR-003)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn test_alt_m_maps_to_toggle_memory_action() {
    // FR-003: Alt+M must produce InputAction::ToggleMemory when no modal is
    // active (no permission dialog, no provider setup, no slash menu).
    let mut app = make_app();
    let key = KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT);
    let action = handle_key(&mut app, key);
    // InputAction does not derive PartialEq, so we match on the variant
    // explicitly instead of using assert_eq!.
    assert!(
        matches!(action, Some(InputAction::ToggleMemory)),
        "Alt+M should produce InputAction::ToggleMemory, got {action:?}"
    );
}

#[test]
fn test_toggle_memory_flips_show_memory_flag() {
    // FR-003: dispatching ToggleMemory via the full key-event path flips
    // `show_memory` on each press.
    let mut app = make_app();
    assert!(!app.show_memory, "show_memory should start false");
    app.handle_key_event(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT));
    assert!(app.show_memory, "first Alt+M should set show_memory=true");
    app.handle_key_event(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT));
    assert!(
        !app.show_memory,
        "second Alt+M should set show_memory=false"
    );
}

#[test]
fn test_toggle_memory_status_message_reflects_state() {
    // FR-014: the status bar message reflects the new panel state.
    let mut app = make_app();
    app.handle_key_event(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT));
    assert_eq!(app.status, "memory panel visible");
    app.handle_key_event(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT));
    assert_eq!(app.status, "memory panel hidden");
}

// ═════════════════════════════════════════════════════════════════════════════
// T-012: Mutual exclusion with log / todo / profile panels (FR-004)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn test_toggle_memory_mutually_excludes_log_panel() {
    // FR-004: enabling the Memory panel must hide the log panel, and
    // enabling the log panel must hide the Memory panel.
    let mut app = make_app();
    app.show_log = true;
    app.show_memory = false;

    // Enable Memory panel — log must be dismissed.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT));
    assert!(app.show_memory, "Memory panel should be visible");
    assert!(
        !app.show_log,
        "log panel must be hidden when Memory is shown"
    );

    // Re-enable log panel — Memory must be dismissed.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::ALT));
    assert!(app.show_log, "log panel should be visible");
    assert!(
        !app.show_memory,
        "Memory panel must be hidden when log is shown"
    );
}

#[test]
fn test_toggle_memory_mutually_excludes_todo_panel() {
    // FR-004: enabling the Memory panel must hide the TODO panel, and
    // enabling the TODO panel must hide the Memory panel.
    let mut app = make_app();
    app.show_todo = true;
    app.show_memory = false;

    app.handle_key_event(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT));
    assert!(app.show_memory);
    assert!(!app.show_todo, "TODO panel must hide when Memory is shown");

    // Re-enable TODO panel — Memory must be dismissed.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::ALT));
    assert!(app.show_todo);
    assert!(
        !app.show_memory,
        "Memory panel must hide when TODO is shown"
    );
}

#[test]
fn test_toggle_memory_mutually_excludes_profile_panel() {
    // FR-004: enabling the Memory panel must hide the profile panel, and
    // enabling the profile panel must hide the Memory panel. Profile toggle
    // routes through `set_profile_panel_enabled` which dismisses every other
    // side panel.
    let mut app = make_app();
    app.show_profile = true;
    app.show_memory = false;

    app.handle_key_event(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT));
    assert!(app.show_memory);
    assert!(
        !app.show_profile,
        "profile panel must hide when Memory is shown"
    );

    // Re-enable profile panel — Memory must be dismissed.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::ALT));
    assert!(app.show_profile);
    assert!(
        !app.show_memory,
        "Memory panel must hide when profile is shown"
    );
}

#[test]
fn test_toggle_memory_clears_memory_selection_and_context_menu_on_hide() {
    // FR-005: when the Memory panel becomes hidden, any active Memory-pane
    // text selection or context menu is cleared.
    let mut app = make_app();
    app.show_memory = true;
    app.memory_area = Rect::new(0, 1, 80, 20);
    app.text_selection = Some(TextSelection {
        pane: SelectionPane::Memory,
        anchor: (5, 5),
        endpoint: (10, 5),
    });

    // Drive the toggle through the same InputAction dispatch the Alt+M key
    // handler uses. We dismiss any context menu first because the
    // context-menu key router in `handle_key` swallows all keys until the
    // menu is dismissed, which would prevent the Alt+M mapping from running.
    app.context_menu = None;
    app.handle_key_event(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT));
    assert!(!app.show_memory);
    assert!(
        app.text_selection.is_none(),
        "Memory selection must be cleared when panel is hidden"
    );
}

#[test]
fn test_toggle_memory_clears_memory_context_menu_on_hide() {
    // FR-005 (context-menu half): driving ToggleMemory to hide the panel
    // clears an active Memory-pane context menu.
    let mut app = make_app();
    app.show_memory = true;
    app.memory_area = Rect::new(0, 1, 80, 20);
    app.context_menu = Some(ragent_tui::app::ContextMenuState {
        x: 5,
        y: 5,
        pane: SelectionPane::Memory,
        selected: 0,
        items: vec![
            (ContextAction::Cut, false),
            (ContextAction::Copy, true),
            (ContextAction::Paste, false),
        ],
    });

    // The context-menu key router in `handle_key` swallows all keys until
    // the menu is dismissed, so we dismiss it first and then toggle.
    app.context_menu = None;
    app.handle_key_event(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT));
    assert!(!app.show_memory);
    assert!(
        app.context_menu.is_none(),
        "Memory context menu must be cleared when panel is hidden"
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// T-012: Alt+M does not insert `m` into the input buffer (FR-011)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn test_alt_m_does_not_insert_m_into_input() {
    // FR-011: pressing Alt+M must toggle the Memory panel and must NOT insert
    // the character `m` into the chat input buffer. The Alt+M mapping in
    // `input.rs` is placed before the generic `KeyCode::Char(c)` insertion
    // handler (NFR-002) so the event is consumed entirely by the toggle.
    let mut app = make_app();
    assert!(app.input.is_empty(), "input should start empty");

    app.handle_key_event(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT));
    assert!(
        app.input.is_empty(),
        "Alt+M must not insert 'm' into input; got {:?}",
        app.input
    );
    assert!(app.show_memory, "Alt+M should have toggled show_memory on");
}

#[test]
fn test_alt_m_does_not_insert_m_when_panel_already_visible() {
    // FR-011 regression guard: toggling the panel off via Alt+M must also
    // not leak an `m` into the input buffer.
    let mut app = make_app();
    app.show_memory = true;

    app.handle_key_event(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT));
    assert!(
        app.input.is_empty(),
        "Alt+M must not insert 'm' when hiding the panel; got {:?}",
        app.input
    );
    assert!(
        !app.show_memory,
        "Alt+M should have toggled show_memory off"
    );
}

// ════════════════════��════════════════════════════════════════════════════════
// T-012: render_memory_panel with populated and missing files (FR-015)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn test_render_memory_panel_sets_memory_area_rect() {
    // FR-002 / FR-013: while the Memory panel is visible, `app.memory_area`
    // must be populated with the panel's rect so mouse hit-testing works.
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    app.show_memory = true;
    app.show_log = false;
    app.show_profile = false;
    app.show_todo = false;
    app.current_screen = ScreenMode::Chat;

    let _ = render_app_to_string(&mut app, 120, 40);
    assert!(
        app.memory_area.area() > 0,
        "memory_area should be set to a non-empty rect when panel is visible"
    );
}

#[test]
fn test_render_memory_panel_hidden_clears_memory_area() {
    // FR-002: when the Memory panel is not visible, `memory_area` should be
    // reset to an empty rect by the layout split so hit-testing never
    // targets a hidden panel.
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    app.show_memory = false;
    app.current_screen = ScreenMode::Chat;

    let _ = render_app_to_string(&mut app, 120, 40);
    assert_eq!(
        app.memory_area.area(),
        0,
        "memory_area should be empty when panel is hidden"
    );
}

#[test]
fn test_render_memory_panel_with_populated_project_memory() {
    // FR-015 (positive case): when the project has structured memories, the
    // panel renders their content.
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    let body = "Unique marker line: zebra-tango-mango";
    seed_project_memory(&app.storage, dir.path(), body);
    app.show_memory = true;
    app.show_log = false;
    app.show_profile = false;
    app.show_todo = false;
    app.current_screen = ScreenMode::Chat;

    let text = render_app_to_string(&mut app, 140, 40);
    assert!(
        text.contains("Memory"),
        "panel border/title should be rendered; got:
{text}"
    );
    assert!(
        text.contains("zebra-tango-mango"),
        "panel should render the project memory content; got:
{text}"
    );
}

#[test]
fn test_render_memory_panel_with_missing_files_shows_placeholder() {
    // FR-015 (missing case): when a project has no structured memories, the
    // panel renders a placeholder and does NOT abort rendering.
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    app.show_memory = true;
    app.show_log = false;
    app.show_profile = false;
    app.show_todo = false;
    app.current_screen = ScreenMode::Chat;

    let text = render_app_to_string(&mut app, 140, 40);
    assert!(
        text.contains("Memory"),
        "panel border/title should be rendered even with no memories"
    );
    assert!(
        text.contains("(no memories for this project)"),
        "missing memories should render the placeholder; got:
{text}"
    );
}

#[test]
fn test_render_memory_panel_does_not_panic_with_no_home_dir() {
    // FR-015 robustness: the panel must not panic when the user memory path
    // cannot be resolved. `dirs::home_dir()` returning None is handled by a
    // dedicated "(no user memory)" branch in `render_memory_panel`.
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    app.show_memory = true;
    app.current_screen = ScreenMode::Chat;

    // Rendering should complete without panicking regardless of home dir.
    let _ = render_app_to_string(&mut app, 120, 40);
}

#[test]
fn test_render_memory_panel_re_reads_files_on_every_render() {
    // FR-010: the panel re-reads the SQLite store on every render so external
    // changes are reflected without restarting the TUI.
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    app.show_memory = true;
    app.current_screen = ScreenMode::Chat;

    let first = render_app_to_string(&mut app, 140, 40);
    assert!(first.contains("(no memories for this project)"));

    seed_project_memory(
        &app.storage,
        dir.path(),
        "Fresh content marker: kiwi-rewrite",
    );
    let second = render_app_to_string(&mut app, 140, 40);
    assert!(
        second.contains("kiwi-rewrite"),
        "second render should pick up the newly stored memory; got:
{second}"
    );
}

#[test]
fn test_render_memory_panel_sets_max_scroll_when_content_overflows() {
    // FR-009: when the rendered content exceeds the visible height,
    // `memory_max_scroll` is set to a positive value.
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    app.show_memory = true;
    app.current_screen = ScreenMode::Chat;

    // Seed many memories to force content overflow.
    for i in 0..60u32 {
        seed_project_memory(
            &app.storage,
            dir.path(),
            &format!("overflow line number {i}"),
        );
    }

    let _ = render_app_to_string(&mut app, 120, 20);
    assert!(
        app.memory_max_scroll > 0,
        "memory_max_scroll should be > 0 when content overflows the panel, got {}",
        app.memory_max_scroll
    );
}

#[test]
fn test_log_scroll_down_on_memory_does_not_underflow_below_zero() {
    // FR-009 bounds: LogScrollDown decrements `memory_scroll_offset` with
    // saturating subtraction, so it must never underflow past 0. The key
    // binding for LogScrollDown is Ctrl+PageDown (see `input.rs`).
    let mut app = make_app();
    app.show_memory = true;
    app.show_log = false;
    app.show_profile = false;
    app.show_todo = false;
    app.memory_scroll_offset = 0;

    app.handle_key_event(KeyEvent::new(KeyCode::PageDown, KeyModifiers::CONTROL));
    assert_eq!(
        app.memory_scroll_offset, 0,
        "LogScrollDown at offset 0 must saturate at 0, not underflow"
    );
}

#[test]
fn test_log_scroll_up_on_memory_increments_offset() {
    // FR-009 bounds: LogScrollUp increments `memory_scroll_offset` by 3
    // when the Memory panel is the visible side panel. The key binding for
    // LogScrollUp is Ctrl+PageUp (see `input.rs`).
    let mut app = make_app();
    app.show_memory = true;
    app.show_log = false;
    app.show_profile = false;
    app.show_todo = false;
    assert_eq!(app.memory_scroll_offset, 0);

    app.handle_key_event(KeyEvent::new(KeyCode::PageUp, KeyModifiers::CONTROL));
    assert_eq!(
        app.memory_scroll_offset, 3,
        "LogScrollUp should increment memory_scroll_offset by 3"
    );
}

#[test]
fn test_scrollbar_drag_clamps_offset_within_bounds() {
    // FR-009 bounds: dragging the scrollbar computes the offset from the
    // drag fraction and clamps it to [0, max_scroll]. Dragging to the middle
    // of a 100-line scrollable region yields ~50, never negative and never
    // greater than max_scroll.
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    app.show_memory = true;
    app.memory_area = Rect::new(80, 0, 30, 21);
    app.memory_max_scroll = 100;

    // Click scrollbar (column 109) at the vertical middle (row 10 of a
    // 21-row pane → track rows 0..20, middle ≈ row 10).
    app.handle_mouse_event(mouse_down(109, 10));
    app.handle_mouse_event(mouse_drag(109, 10));

    let offset = app.memory_scroll_offset;
    assert!(
        offset <= app.memory_max_scroll,
        "offset {offset} must not exceed max_scroll {}",
        app.memory_max_scroll
    );
    // fraction = 10/20 = 0.5 → offset = (1-0.5)*100 = 50.
    assert_eq!(offset, 50, "middle drag should yield offset 50");

    // Drag above the pane → clamps to top (offset = max_scroll).
    app.handle_mouse_event(mouse_drag(109, 0));
    assert_eq!(app.memory_scroll_offset, 100);

    // Drag below the pane → clamps to bottom (offset = 0).
    app.handle_mouse_event(mouse_drag(109, 30));
    assert_eq!(app.memory_scroll_offset, 0);
}

#[tokio::test]
async fn test_memory_store_tool_content_appears_in_memory_panel() {
    // Regression for the bug where `memory_store` wrote memories under the
    // directory basename ("myproject") while the TUI memory panel and the
    // `/memory show` slash command queried by the full working directory path
    // ("/home/user/myproject"). The mismatch made freshly stored memories
    // invisible in the panel.
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    app.show_memory = true;
    app.show_log = false;
    app.show_profile = false;
    app.show_todo = false;
    app.current_screen = ScreenMode::Chat;

    let tool = MemoryStoreTool;
    let ctx = ToolContext {
        session_id: "memory-panel-regression-sess".to_string(),
        working_dir: dir.path().to_path_buf(),
        event_bus: Arc::new(EventBus::new(16)),
        storage: Some(app.storage.clone()),
        task_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
        code_index: None,
        bg_service: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
    };
    tool.execute(
        json!({
            "content": "memory_store panel visibility marker: banana-kangaroo-sapphire",
            "category": "fact",
            "confidence": 0.85,
            "tags": ["regression", "memory-panel"],
            "source": "memory_store"
        }),
        &ctx,
    )
    .await
    .expect("memory_store tool should execute");

    let text = render_app_to_string(&mut app, 140, 40);
    assert!(
        text.contains("Memory"),
        "panel border/title should be rendered; got:\n{text}"
    );
    assert!(
        text.contains("banana-kangaroo-sapphire"),
        "panel must render a memory written by memory_store using the same project key as the panel; got:\n{text}"
    );
    assert!(
        text.contains("Structured memories: 1"),
        "panel should report the single stored memory; got:\n{text}"
    );
}

#[test]
fn test_render_memory_panel_shows_tags_for_structured_memories() {
    // The Memory panel should render the comma-separated tags stored for each
    // structured memory alongside the content preview.
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    seed_project_memory_with_tags(
        &app.storage,
        dir.path(),
        "Memory with tags marker: alpha-bravo",
        &["rust".to_string(), "tui".to_string()],
    );
    app.show_memory = true;
    app.show_log = false;
    app.show_profile = false;
    app.show_todo = false;
    app.current_screen = ScreenMode::Chat;

    let text = render_app_to_string(&mut app, 140, 40);
    assert!(
        text.contains("alpha-bravo"),
        "panel should render the memory content; got:\n{text}"
    );
    assert!(
        text.contains("tags: rust, tui"),
        "panel should render the associated tags; got:\n{text}"
    );
}

#[test]
fn test_render_memory_panel_omits_tags_line_when_memory_has_no_tags() {
    // Memories without tags should not produce a 'tags:' line in the panel.
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    seed_project_memory(
        &app.storage,
        dir.path(),
        "No-tags memory marker: charlie-delta",
    );
    app.show_memory = true;
    app.show_log = false;
    app.show_profile = false;
    app.show_todo = false;
    app.current_screen = ScreenMode::Chat;

    let text = render_app_to_string(&mut app, 140, 40);
    assert!(
        text.contains("charlie-delta"),
        "panel should render the memory content; got:\n{text}"
    );
    assert!(
        !text.contains("tags:"),
        "panel should not render a tags line for tag-less memory; got:\n{text}"
    );
}
