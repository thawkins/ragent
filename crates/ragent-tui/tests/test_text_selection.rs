//! Tests for text selection and copy.
//!
//! Verifies mouse-driven text selection, highlight tracking,
//! right-click copy, and selection state management.

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
use ragent_tui::app::{ContextAction, ContextMenuState, SelectionPane, TextSelection};

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

fn right_click(col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: col,
        row,
        modifiers: crossterm::event::KeyModifiers::empty(),
    }
}

// ---------- Selection creation ----------

#[test]
fn test_left_click_starts_selection_in_messages() {
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    app.log_area = Rect::new(80, 1, 30, 20);

    // Click inside messages (not on scrollbar column 79)
    app.handle_mouse_event(mouse_down(10, 5));
    assert!(app.text_selection.is_some());
    let sel = app.text_selection.as_ref().unwrap();
    assert_eq!(sel.pane, SelectionPane::Messages);
    assert_eq!(sel.anchor, (10, 5));
    assert_eq!(sel.endpoint, (10, 5));
}

#[test]
fn test_left_click_starts_selection_in_log() {
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    app.log_area = Rect::new(80, 1, 30, 20);

    app.handle_mouse_event(mouse_down(90, 10));
    assert!(app.text_selection.is_some());
    let sel = app.text_selection.as_ref().unwrap();
    assert_eq!(sel.pane, SelectionPane::Log);
}

#[test]
fn test_left_click_starts_selection_in_input() {
    let mut app = make_app();
    app.input_area = Rect::new(0, 22, 80, 3);
    // message_area must not contain the click
    app.message_area = Rect::new(0, 1, 80, 20);

    app.handle_mouse_event(mouse_down(10, 23));
    assert!(app.text_selection.is_some());
    let sel = app.text_selection.as_ref().unwrap();
    assert_eq!(sel.pane, SelectionPane::Input);
}

#[test]
fn test_left_click_outside_panes_clears_selection() {
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    // Start a selection
    app.text_selection = Some(TextSelection {
        pane: SelectionPane::Messages,
        anchor: (5, 5),
        endpoint: (10, 5),
    });

    // Click at row 0 (status bar, outside all panes)
    app.handle_mouse_event(mouse_down(10, 0));
    assert!(app.text_selection.is_none());
}

// ---------- Selection dragging ----------

#[test]
fn test_drag_extends_selection() {
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);

    app.handle_mouse_event(mouse_down(5, 3));
    app.handle_mouse_event(mouse_drag(40, 7));

    let sel = app.text_selection.as_ref().unwrap();
    assert_eq!(sel.anchor, (5, 3));
    assert_eq!(sel.endpoint, (40, 7));
}

#[test]
fn test_mouse_up_preserves_selection() {
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);

    app.handle_mouse_event(mouse_down(5, 3));
    app.handle_mouse_event(mouse_drag(40, 7));
    app.handle_mouse_event(mouse_up(40, 7));

    // Selection should still be present after release
    assert!(app.text_selection.is_some());
}

// ---------- Selection normalization ----------

#[test]
fn test_selection_normalized_forward() {
    let sel = TextSelection {
        pane: SelectionPane::Messages,
        anchor: (5, 3),
        endpoint: (10, 5),
    };
    let ((sc, sr), (ec, er)) = sel.normalized();
    assert_eq!((sc, sr), (5, 3));
    assert_eq!((ec, er), (10, 5));
}

#[test]
fn test_selection_normalized_backward() {
    let sel = TextSelection {
        pane: SelectionPane::Messages,
        anchor: (10, 5),
        endpoint: (5, 3),
    };
    let ((sc, sr), (ec, er)) = sel.normalized();
    assert_eq!((sc, sr), (5, 3));
    assert_eq!((ec, er), (10, 5));
}

#[test]
fn test_selection_normalized_same_row() {
    let sel = TextSelection {
        pane: SelectionPane::Messages,
        anchor: (20, 5),
        endpoint: (5, 5),
    };
    let ((sc, sr), (ec, er)) = sel.normalized();
    assert_eq!((sc, sr), (5, 5));
    assert_eq!((ec, er), (20, 5));
}

// ---------- Right-click context menu ----------

#[test]
fn test_right_click_opens_context_menu_and_keeps_selection() {
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    app.message_content_lines = vec![
        "You: hello world".to_string(),
        "Assistant: hi there".to_string(),
    ];

    app.text_selection = Some(TextSelection {
        pane: SelectionPane::Messages,
        anchor: (1, 1),
        endpoint: (10, 1),
    });

    app.handle_mouse_event(right_click(5, 5));
    assert!(app.text_selection.is_some());
    let menu = app.context_menu.as_ref().expect("context menu should open");
    assert_eq!(menu.pane, SelectionPane::Messages);
    assert_eq!(menu.items.len(), 3);
    assert_eq!(menu.items[0], (ContextAction::Cut, false));
    assert_eq!(menu.items[1], (ContextAction::Copy, true));
}

#[test]
fn test_right_click_with_no_selection_opens_disabled_menu() {
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    assert!(app.text_selection.is_none());
    app.handle_mouse_event(right_click(5, 5));
    assert!(app.text_selection.is_none());
    let menu = app.context_menu.as_ref().expect("context menu should open");
    assert_eq!(menu.pane, SelectionPane::Messages);
    assert_eq!(menu.items[0], (ContextAction::Cut, false));
    assert_eq!(menu.items[1], (ContextAction::Copy, false));
}

#[test]
fn test_right_click_with_selection_in_other_pane_disables_copy_for_clicked_pane() {
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    app.log_area = Rect::new(80, 1, 30, 20);
    app.show_log = true;
    app.text_selection = Some(TextSelection {
        pane: SelectionPane::Messages,
        anchor: (1, 1),
        endpoint: (5, 1),
    });

    app.handle_mouse_event(right_click(90, 5));

    let menu = app.context_menu.as_ref().expect("context menu should open");
    assert_eq!(menu.pane, SelectionPane::Log);
    assert_eq!(menu.items[1], (ContextAction::Copy, false));
}

#[test]
fn test_context_copy_keeps_selection() {
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    app.message_content_lines = vec!["You: hello world".to_string()];
    app.text_selection = Some(TextSelection {
        pane: SelectionPane::Messages,
        anchor: (1, 1),
        endpoint: (5, 1),
    });
    app.context_menu = Some(ContextMenuState {
        x: 1,
        y: 1,
        pane: SelectionPane::Messages,
        selected: 1,
        items: vec![
            (ContextAction::Cut, false),
            (ContextAction::Copy, true),
            (ContextAction::Paste, false),
        ],
    });

    app.execute_context_action(ContextAction::Copy);

    assert!(app.text_selection.is_some());
}

// ---------- extract_text_from_lines ----------

#[test]
fn test_extract_single_line() {
    let lines = vec!["Hello, world!".to_string()];
    // inner_x=1, inner_y=1 (inside border)
    // select columns 1..6 at row 1 → "Hello,"
    let text = App::extract_text_from_lines(&lines, 1, 1, 1, 1, 6, 1);
    assert_eq!(text, "Hello,");
}

#[test]
fn test_extract_multi_line() {
    let lines = vec![
        "Line one text".to_string(),
        "Line two text".to_string(),
        "Line three text".to_string(),
    ];
    // inner_x=1, inner_y=5
    // select from (1,5) to (8,7) → all of line 0, all of line 1, "Line thr" of line 2
    let text = App::extract_text_from_lines(&lines, 1, 5, 1, 5, 8, 7);
    assert_eq!(text, "Line one text\nLine two text\nLine thr");
}

#[test]
fn test_extract_partial_single_line() {
    let lines = vec!["ABCDEFGHIJ".to_string()];
    // inner_x=2, inner_y=0
    // select col 4..7 → characters at positions 2..5 → "CDEF"
    let text = App::extract_text_from_lines(&lines, 2, 0, 4, 0, 7, 0);
    assert_eq!(text, "CDEF");
}

// ---------- Scrollbar click does NOT start selection ----------

#[test]
fn test_scrollbar_click_clears_selection() {
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    app.message_max_scroll = 50;

    // Pre-set a selection
    app.text_selection = Some(TextSelection {
        pane: SelectionPane::Messages,
        anchor: (5, 5),
        endpoint: (10, 5),
    });

    // Click on scrollbar column (79)
    app.handle_mouse_event(mouse_down(79, 10));
    assert!(app.text_selection.is_none());
    assert!(app.scrollbar_drag.is_some());
}

// ---------- New click replaces old selection ----------

#[test]
fn test_new_click_replaces_selection() {
    let mut app = make_app();
    app.message_area = Rect::new(0, 1, 80, 20);

    app.handle_mouse_event(mouse_down(5, 5));
    app.handle_mouse_event(mouse_drag(20, 5));
    assert_eq!(app.text_selection.as_ref().unwrap().anchor, (5, 5));

    // New click starts fresh selection
    app.handle_mouse_event(mouse_down(30, 10));
    let sel = app.text_selection.as_ref().unwrap();
    assert_eq!(sel.anchor, (30, 10));
    assert_eq!(sel.endpoint, (30, 10));
}

#[test]
fn test_context_cut_on_input_removes_selected_range_only() {
    let mut app = make_app();
    app.input_area = Rect::new(0, 22, 40, 3);
    app.input = "abcdef".to_string();
    app.input_cursor = app.input.chars().count();
    app.text_selection = Some(TextSelection {
        pane: SelectionPane::Input,
        // Select "cd" from "> abcdef" in inner row y=23.
        anchor: (5, 23),
        endpoint: (6, 23),
    });
    app.context_menu = Some(ContextMenuState {
        x: 1,
        y: 1,
        pane: SelectionPane::Input,
        selected: 0,
        items: vec![(ContextAction::Cut, true)],
    });

    app.execute_context_action(ContextAction::Cut);

    assert_eq!(app.input, "abef");
    assert_eq!(app.input_cursor, 2);
}

#[test]
fn test_extract_text_from_lines_unicode_uses_character_columns() {
    let lines = vec!["a💡b".to_string()];
    let text = App::extract_text_from_lines(&lines, 0, 0, 1, 0, 1, 0);
    assert_eq!(text, "💡");
}

#[test]
fn test_context_cut_on_wrapped_unicode_input_removes_single_character() {
    let mut app = make_app();
    app.input_area = Rect::new(0, 22, 6, 4); // inner width = 4
    app.input = "ab💡cd".to_string();
    app.input_cursor = app.input.chars().count();
    app.text_selection = Some(TextSelection {
        pane: SelectionPane::Input,
        // Wrapped display lines for "> ab💡cd" (inner_w=4):
        // row 23: "> ab", row 24: "💡cd"
        anchor: (1, 24),
        endpoint: (1, 24),
    });
    app.context_menu = Some(ContextMenuState {
        x: 1,
        y: 1,
        pane: SelectionPane::Input,
        selected: 0,
        items: vec![(ContextAction::Cut, true)],
    });

    app.execute_context_action(ContextAction::Cut);

    assert_eq!(app.input, "abcd");
    assert_eq!(app.input_cursor, 2);
}


#[test]
fn test_right_click_uses_home_input_geometry_for_file_menu_popup() {
    let mut app = make_app();
    app.home_input_area = Rect::new(20, 15, 40, 3);
    app.input_area = Rect::default();
    app.input = "@src".to_string();
    app.input_cursor = app.input.chars().count();
    app.file_menu = Some(ragent_tui::app::FileMenuState {
        matches: vec![ragent_tui::app::FileMenuEntry {
            display: "src/main.rs".to_string(),
            path: std::path::PathBuf::from("src/main.rs"),
            is_dir: false,
        }],
        selected: 0,
        scroll_offset: 0,
        query: "src".to_string(),
        current_dir: None,
    });

    // Click inside file menu popup based on home_input_area geometry.
    // item_count=1 => height=4 (includes hint row), popup starts at y=11, first item row=12.
    app.handle_mouse_event(mouse_down(20, 12));

    assert!(
        app.file_menu.is_none(),
        "click should accept file entry and close menu"
    );
    assert!(app.input.contains("src/main.rs"));
}

#[test]
fn test_left_click_outside_file_menu_closes_popup() {
    let mut app = make_app();
    app.home_input_area = Rect::new(20, 15, 40, 3);
    app.input = "@".to_string();
    app.input_cursor = 1;
    app.file_menu = Some(ragent_tui::app::FileMenuState {
        matches: vec![ragent_tui::app::FileMenuEntry {
            display: "src/main.rs".to_string(),
            path: std::path::PathBuf::from("src/main.rs"),
            is_dir: false,
        }],
        selected: 0,
        scroll_offset: 0,
        query: "src".to_string(),
        current_dir: None,
    });

    // Popup is above y=15, so y=20 is outside.
    app.handle_mouse_event(mouse_down(20, 20));
    assert!(app.file_menu.is_none(), "outside click should dismiss popup");
}
