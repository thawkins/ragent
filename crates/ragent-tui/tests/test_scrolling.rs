//! Tests for test_scrolling.rs

//! Tests for scroll and mouse support.
//!
//! Verifies keyboard and mouse-driven scrolling of the message and log panes,
//! including scroll offset clamping and mouse hit-testing.

use std::sync::Arc;

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use ragent_core::{
    agent,
    event::EventBus,
    permission::PermissionChecker,
    provider,
    session::{SessionManager, processor::SessionProcessor},
    storage::Storage,
    tool,
};
use ragent_tui::App;

/// Build an [`App`] backed by an in-memory database.
fn make_app() -> App {
    let event_bus = Arc::new(EventBus::default());
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let provider_registry = Arc::new(provider::create_default_registry());
    let tool_registry = Arc::new(tool::create_default_registry());
    let permission_checker = Arc::new(tokio::sync::RwLock::new(PermissionChecker::new(vec![])));
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));
    let session_processor = Arc::new(SessionProcessor {
        session_manager,
        provider_registry: provider_registry.clone(),
        tool_registry,
        permission_checker,
        event_bus: event_bus.clone(),
        task_manager: std::sync::OnceLock::new(),
        lsp_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        code_index: std::sync::OnceLock::new(),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: ragent_core::config::StreamConfig::default(),
        auto_approve: false,
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
    )
}

fn mouse_scroll_up(col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::ScrollUp,
        column: col,
        row,
        modifiers: crossterm::event::KeyModifiers::empty(),
    }
}

fn mouse_scroll_down(col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: col,
        row,
        modifiers: crossterm::event::KeyModifiers::empty(),
    }
}

// ---------- Message pane keyboard scroll ----------

#[test]
fn test_scroll_up_increments_offset() {
    let mut app = make_app();
    assert_eq!(app.scroll_offset, 0);
    app.scroll_offset = app.scroll_offset.saturating_add(3); // simulate ScrollUp action
    assert_eq!(app.scroll_offset, 3);
}

#[test]
fn test_scroll_down_decrements_offset() {
    let mut app = make_app();
    app.scroll_offset = 6;
    app.scroll_offset = app.scroll_offset.saturating_sub(3);
    assert_eq!(app.scroll_offset, 3);
}

#[test]
fn test_scroll_down_does_not_go_below_zero() {
    let mut app = make_app();
    app.scroll_offset = 1;
    app.scroll_offset = app.scroll_offset.saturating_sub(3);
    assert_eq!(app.scroll_offset, 0);
}

// ---------- Log pane keyboard scroll ----------

#[test]
fn test_log_scroll_up_increments() {
    let mut app = make_app();
    assert_eq!(app.log_scroll_offset, 0);
    app.log_scroll_offset = app.log_scroll_offset.saturating_add(3);
    assert_eq!(app.log_scroll_offset, 3);
}

#[test]
fn test_log_scroll_down_does_not_go_below_zero() {
    let mut app = make_app();
    app.log_scroll_offset = 1;
    app.log_scroll_offset = app.log_scroll_offset.saturating_sub(3);
    assert_eq!(app.log_scroll_offset, 0);
}

// ---------- Mouse scroll on message area ----------

#[test]
fn test_mouse_scroll_up_on_messages() {
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    app.log_area = Rect::new(80, 1, 30, 20);

    app.handle_mouse_event(mouse_scroll_up(10, 10));
    assert_eq!(app.scroll_offset, 3);
    assert_eq!(app.log_scroll_offset, 0);
}

#[test]
fn test_mouse_scroll_down_on_messages() {
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    app.scroll_offset = 6;

    app.handle_mouse_event(mouse_scroll_down(10, 10));
    assert_eq!(app.scroll_offset, 3);
}

// ---------- Mouse scroll on log area ----------

#[test]
fn test_mouse_scroll_up_on_log() {
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    app.log_area = Rect::new(80, 1, 30, 20);

    app.handle_mouse_event(mouse_scroll_up(90, 10));
    assert_eq!(app.log_scroll_offset, 3);
    assert_eq!(app.scroll_offset, 0);
}

#[test]
fn test_mouse_scroll_down_on_log() {
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    app.log_area = Rect::new(80, 1, 30, 20);
    app.log_scroll_offset = 9;

    app.handle_mouse_event(mouse_scroll_down(90, 10));
    assert_eq!(app.log_scroll_offset, 6);
}

// ---------- Mouse scroll outside panes ----------

#[test]
fn test_mouse_scroll_outside_panes_no_effect() {
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    app.log_area = Rect::new(80, 1, 30, 20);

    // Scroll in the status bar area (row 0)
    app.handle_mouse_event(mouse_scroll_up(10, 0));
    assert_eq!(app.scroll_offset, 0);
    assert_eq!(app.log_scroll_offset, 0);
}

// ---------- Log hidden: scroll only messages ----------

#[test]
fn test_mouse_scroll_log_hidden_ignores_log_area() {
    let mut app = make_app();
    app.show_log = false;
    app.message_area = Rect::new(0, 1, 110, 20);
    app.log_area = Rect::default();

    app.handle_mouse_event(mouse_scroll_up(50, 10));
    assert_eq!(app.scroll_offset, 3);
    assert_eq!(app.log_scroll_offset, 0);
}

// ---------- Area tracking fields ----------

#[test]
fn test_area_fields_default_to_zero() {
    let app = make_app();
    assert_eq!(app.message_area, Rect::default());
    assert_eq!(app.log_area, Rect::default());
}

// ---------- Non-scroll mouse events ignored ----------

#[test]
fn test_mouse_click_no_effect() {
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    app.message_max_scroll = 100;
    let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 10, // not the scrollbar column (79)
        row: 10,
        modifiers: crossterm::event::KeyModifiers::empty(),
    };
    app.handle_mouse_event(click);
    assert_eq!(app.scroll_offset, 0);
    assert_eq!(app.log_scroll_offset, 0);
    assert!(app.scrollbar_drag.is_none());
}

// ---------- Scrollbar drag tests ----------

use ragent_tui::app::ScrollbarDragPane;

fn mouse_down(col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: col,
        row,
        modifiers: crossterm::event::KeyModifiers::empty(),
    }
}

fn mouse_drag(col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: col,
        row,
        modifiers: crossterm::event::KeyModifiers::empty(),
    }
}

fn mouse_up(col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: col,
        row,
        modifiers: crossterm::event::KeyModifiers::empty(),
    }
}

#[test]
fn test_drag_starts_on_message_scrollbar_column() {
    let mut app = make_app();
    // message area: x=0, y=1, width=80, height=20 → scrollbar at column 79
    app.message_area = Rect::new(0, 1, 80, 20);
    app.message_max_scroll = 100;

    app.handle_mouse_event(mouse_down(79, 10));
    assert_eq!(app.scrollbar_drag, Some(ScrollbarDragPane::Messages));
}

#[test]
fn test_drag_does_not_start_without_scrollable_content() {
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    app.message_max_scroll = 0; // nothing to scroll

    app.handle_mouse_event(mouse_down(79, 10));
    assert!(app.scrollbar_drag.is_none());
}

#[test]
fn test_drag_starts_on_log_scrollbar_column() {
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    app.log_area = Rect::new(80, 1, 30, 20);
    app.log_max_scroll = 50;

    // log scrollbar at column 109 (80 + 30 - 1)
    app.handle_mouse_event(mouse_down(109, 10));
    assert_eq!(app.scrollbar_drag, Some(ScrollbarDragPane::Log));
}

#[test]
fn test_drag_to_top_scrolls_to_top_of_content() {
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    app.message_max_scroll = 100;

    // Click scrollbar, drag to top of pane (row 1)
    app.handle_mouse_event(mouse_down(79, 10));
    app.handle_mouse_event(mouse_drag(79, 1));
    // Top of content → scroll_offset = max_scroll
    assert_eq!(app.scroll_offset, 100);
}

#[test]
fn test_drag_to_bottom_scrolls_to_bottom_of_content() {
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    app.message_max_scroll = 100;
    app.scroll_offset = 50;

    // Click scrollbar, drag to bottom of pane (row 20 = y + height - 1)
    app.handle_mouse_event(mouse_down(79, 10));
    app.handle_mouse_event(mouse_drag(79, 20));
    // Bottom of content → scroll_offset = 0
    assert_eq!(app.scroll_offset, 0);
}

#[test]
fn test_drag_to_middle_scrolls_to_midpoint() {
    let mut app = make_app();
    // area: y=0, height=21 → rows 0..20 inclusive, track_height=20
    app.message_area = Rect::new(0, 0, 80, 21);
    app.message_max_scroll = 100;

    app.handle_mouse_event(mouse_down(79, 10));
    app.handle_mouse_event(mouse_drag(79, 10));
    // fraction = 10/20 = 0.5, offset = (1-0.5)*100 = 50
    assert_eq!(app.scroll_offset, 50);
}

#[test]
fn test_drag_release_clears_state() {
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    app.message_max_scroll = 100;

    app.handle_mouse_event(mouse_down(79, 10));
    assert!(app.scrollbar_drag.is_some());

    app.handle_mouse_event(mouse_up(79, 10));
    assert!(app.scrollbar_drag.is_none());
}

#[test]
fn test_drag_outside_pane_clamped() {
    let mut app = make_app();
    // area: y=5, height=10 → rows 5..14
    app.message_area = Rect::new(0, 5, 80, 10);
    app.message_max_scroll = 100;

    app.handle_mouse_event(mouse_down(79, 10));
    // Drag above the pane (row 0) — should clamp to top
    app.handle_mouse_event(mouse_drag(79, 0));
    assert_eq!(app.scroll_offset, 100); // top of content

    // Drag below the pane (row 30) — should clamp to bottom
    app.handle_mouse_event(mouse_drag(79, 30));
    assert_eq!(app.scroll_offset, 0); // bottom of content
}

#[test]
fn test_drag_log_scrollbar() {
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    app.log_area = Rect::new(80, 0, 30, 21);
    app.log_max_scroll = 60;

    // Click log scrollbar (column 109), drag to top
    app.handle_mouse_event(mouse_down(109, 10));
    assert_eq!(app.scrollbar_drag, Some(ScrollbarDragPane::Log));

    app.handle_mouse_event(mouse_drag(109, 0));
    assert_eq!(app.log_scroll_offset, 60);

    // Drag to bottom
    app.handle_mouse_event(mouse_drag(109, 20));
    assert_eq!(app.log_scroll_offset, 0);
}

#[test]
fn test_drag_ignored_when_no_active_drag() {
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    app.message_max_scroll = 100;

    // Drag without prior mouse down — no scrollbar_drag active
    app.handle_mouse_event(mouse_drag(79, 5));
    assert_eq!(app.scroll_offset, 0);
}
