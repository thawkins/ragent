//! Tests for test_text_selection.rs

//! Tests for text selection and copy.
//!
//! Verifies mouse-driven text selection, highlight tracking,
//! right-click copy, and selection state management.

use std::sync::Arc;

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::{Terminal, backend::TestBackend};

use ragent_core::{
    agent,
    event::EventBus,
    message::{Message, MessagePart, Role, ToolCallState, ToolCallStatus},
    permission::PermissionChecker,
    provider,
    session::{SessionManager, processor::SessionProcessor},
    storage::Storage,
    tool,
};
use ragent_tui::App;
use ragent_tui::app::{
    ContextAction, ContextMenuState, OutputViewState, OutputViewTarget, ScreenMode, SelectionPane,
    TextSelection,
};

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

#[test]
fn test_clicking_active_agents_row_opens_output_view() {
    let mut app = make_app();
    app.session_id = Some("lead-s1".to_string());
    app.show_agents_window = true;
    app.active_agents_area = Rect::new(0, 10, 80, 8);
    app.active_tasks.push(ragent_core::task::TaskEntry {
        id: "task-12345678".to_string(),
        parent_session_id: "lead-s1".to_string(),
        child_session_id: "child-s1".to_string(),
        agent_name: "explore".to_string(),
        task_prompt: "x".to_string(),
        background: true,
        status: ragent_core::task::TaskStatus::Running,
        result: None,
        error: None,
        created_at: chrono::Utc::now(),
        completed_at: None,
        reported: false,
    });

    app.handle_mouse_event(mouse_down(2, 13));
    assert!(app.output_view.is_some());
    assert_eq!(app.selected_agent_session_id.as_deref(), Some("child-s1"));
}

#[test]
fn test_clicking_teams_row_opens_output_view() {
    let mut app = make_app();
    app.session_id = Some("lead-s1".to_string());
    app.show_teams_window = true;
    app.active_team = Some(ragent_core::team::TeamConfig::new("alpha", "lead-s1"));
    let mut member = ragent_core::team::TeamMember::new("writer", "tm-001", "general");
    member.session_id = Some("tm-s1".to_string());
    app.team_members.push(member);
    app.teams_area = Rect::new(0, 20, 80, 8);

    app.handle_mouse_event(mouse_down(2, 23));
    assert!(app.output_view.is_some());
    assert_eq!(app.selected_agent_session_id.as_deref(), Some("tm-s1"));
}

#[test]
fn test_click_outside_output_view_closes_overlay() {
    let mut app = make_app();
    app.output_view = Some(ragent_tui::app::OutputViewState {
        target: ragent_tui::app::OutputViewTarget::Session {
            session_id: "s1".to_string(),
            label: "primary".to_string(),
        },
        scroll_offset: 0,
        max_scroll: 0,
    });
    app.output_view_area = Rect::new(10, 10, 40, 10);

    app.handle_mouse_event(mouse_down(1, 1));
    assert!(app.output_view.is_none());
}

#[test]
fn test_clicking_agents_button_toggles_agents_window() {
    let mut app = make_app();
    app.active_tasks.push(ragent_core::task::TaskEntry {
        id: "task-1".to_string(),
        parent_session_id: "lead-s1".to_string(),
        child_session_id: "child-s1".to_string(),
        agent_name: "explore".to_string(),
        task_prompt: "x".to_string(),
        background: true,
        status: ragent_core::task::TaskStatus::Running,
        result: None,
        error: None,
        created_at: chrono::Utc::now(),
        completed_at: None,
        reported: false,
    });
    app.agents_button_area = Rect::new(2, 30, 9, 3);

    app.handle_mouse_event(mouse_down(3, 31));
    assert!(app.show_agents_window);

    app.handle_mouse_event(mouse_down(3, 31));
    assert!(!app.show_agents_window);
}

#[test]
fn test_clicking_teams_button_toggles_teams_window() {
    let mut app = make_app();
    app.active_team = Some(ragent_core::team::TeamConfig::new("alpha", "lead-s1"));
    app.teams_button_area = Rect::new(12, 30, 9, 3);

    app.handle_mouse_event(mouse_down(13, 31));
    assert!(app.show_teams_window);

    app.handle_mouse_event(mouse_down(13, 31));
    assert!(!app.show_teams_window);
}

#[test]
fn test_clicking_disabled_buttons_does_not_open_windows() {
    let mut app = make_app();
    app.agents_button_area = Rect::new(2, 30, 9, 3);
    app.teams_button_area = Rect::new(12, 30, 9, 3);

    app.handle_mouse_event(mouse_down(3, 31));
    app.handle_mouse_event(mouse_down(13, 31));

    assert!(!app.show_agents_window);
    assert!(!app.show_teams_window);
}

#[test]
fn test_clicking_close_button_closes_windows() {
    let mut app = make_app();
    app.show_agents_window = true;
    app.show_teams_window = true;
    app.agents_close_button_area = Rect::new(90, 30, 10, 3);
    app.teams_close_button_area = Rect::new(90, 34, 10, 3);

    app.handle_mouse_event(mouse_down(91, 31));
    assert!(!app.show_agents_window);
    assert!(app.show_teams_window);

    app.handle_mouse_event(mouse_down(91, 35));
    assert!(!app.show_teams_window);
}

#[test]
fn test_output_view_can_load_messages_for_non_current_session() {
    let mut app = make_app();
    app.session_id = Some("lead-s1".to_string());
    app.storage
        .create_session("child-s1", "/tmp")
        .expect("create child session");
    app.storage
        .create_message(&ragent_core::message::Message::user_text(
            "child-s1",
            "hello from child",
        ))
        .expect("create message");

    let msgs = app
        .storage
        .get_messages("child-s1")
        .expect("read child messages");
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].text_content(), "hello from child");
}

#[test]
fn test_output_view_overlay_renders_non_current_session_message() {
    let mut app = make_app();
    app.current_screen = ScreenMode::Chat;
    app.session_id = Some("lead-s1".to_string());
    app.storage
        .create_session("child-s1", "/tmp")
        .expect("create child session");
    app.storage
        .create_message(&ragent_core::message::Message::user_text(
            "child-s1",
            "hello from child",
        ))
        .expect("create child message");
    app.output_view = Some(OutputViewState {
        target: OutputViewTarget::Session {
            session_id: "child-s1".to_string(),
            label: "explore [abcd1234]".to_string(),
        },
        scroll_offset: 0,
        max_scroll: 0,
    });

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, &mut app))
        .expect("render frame");

    let buffer = terminal.backend().buffer().clone();
    let mut all_text = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            let cell = buffer.cell((x, y)).expect("cell");
            all_text.push_str(cell.symbol());
        }
        all_text.push('\n');
    }

    assert!(
        all_text.contains("hello from child"),
        "output overlay should render child-session message, got:\n{all_text}"
    );
}

#[test]
fn test_output_view_overlay_renders_tool_calls_for_non_current_session() {
    let mut app = make_app();
    app.current_screen = ScreenMode::Chat;
    app.session_id = Some("lead-s1".to_string());
    app.storage
        .create_session("tm-s1", "/tmp")
        .expect("create teammate session");

    let tool_msg = Message::new(
        "tm-s1",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "bash".to_string(),
            call_id: "c1".to_string(),
            state: ToolCallState {
                status: ToolCallStatus::Completed,
                input: serde_json::json!({"command":"echo hi"}),
                output: Some(serde_json::json!({"line_count": 2})),
                error: None,
                duration_ms: Some(12),
            },
        }],
    );
    app.storage
        .create_message(&tool_msg)
        .expect("create tool-call message");

    app.output_view = Some(OutputViewState {
        target: OutputViewTarget::Session {
            session_id: "tm-s1".to_string(),
            label: "writer [abcd1234]".to_string(),
        },
        scroll_offset: 0,
        max_scroll: 0,
    });

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, &mut app))
        .expect("render frame");

    let buffer = terminal.backend().buffer().clone();
    let mut all_text = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            let cell = buffer.cell((x, y)).expect("cell");
            all_text.push_str(cell.symbol());
        }
        all_text.push('\n');
    }

    assert!(
        all_text.contains("Bash") || all_text.contains("bash"),
        "output overlay should include tool name, got:\n{all_text}"
    );
    assert!(
        all_text.contains("echo hi"),
        "output overlay should include tool input summary, got:\n{all_text}"
    );
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
    app.input_area = Rect::new(20, 15, 40, 3);
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

    // Click inside file menu popup based on input_area geometry.
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
    app.input_area = Rect::new(20, 15, 40, 3);
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
    assert!(
        app.file_menu.is_none(),
        "outside click should dismiss popup"
    );
}
