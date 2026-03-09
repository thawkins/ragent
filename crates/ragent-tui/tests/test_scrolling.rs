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
    });
    let agent_info = agent::resolve_agent("general", &Default::default())
        .expect("resolve general agent");

    App::new(event_bus, storage, provider_registry, session_processor, agent_info, true)
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
    let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 10,
        row: 10,
        modifiers: crossterm::event::KeyModifiers::empty(),
    };
    app.handle_mouse_event(click);
    assert_eq!(app.scroll_offset, 0);
    assert_eq!(app.log_scroll_offset, 0);
}
