//! Additional coverage for the `App` input surfaces that route through
//! `src/app/input_handler.rs`: `handle_key_event`, `handle_mouse_event`, and
//! `execute_context_action` for the side panels (Telemetry / Context / Memory)
//! and the modal overlays (history picker, agents / teams windows, context
//! menu, file-menu hover) that the existing suites do not exercise.
//!
//! Tests only - no production logic is touched.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use ragent_agent::message::{Message, MessagePart, Role};
use ragent_tui::app::{
    ContextAction, ContextMenuState, FileMenuEntry, FileMenuState, HistoryPickerState,
    SelectionPane, TextSelection,
};

#[path = "support/mod.rs"]
mod support;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn kmod(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

const fn mouse(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column: col,
        row,
        modifiers: KeyModifiers::empty(),
    }
}

fn app_with_session(session: &str) -> ragent_tui::App {
    let mut app = support::make_app();
    app.session_id = Some(session.to_string());
    app.current_screen = ragent_tui::app::ScreenMode::Chat;
    app
}

// ===========================================================================
// Mouse wheel scrolling of the Telemetry / Context / Memory panels
// ===========================================================================

#[test]
fn wheel_scrolls_the_telemetry_panel() {
    let mut app = app_with_session("s1");
    app.show_telemetry = true;
    app.telemetry_area = Rect::new(60, 1, 20, 10);
    app.telemetry_scroll_offset = 0;

    app.handle_mouse_event(mouse(MouseEventKind::ScrollUp, 65, 5));
    assert!(
        app.telemetry_scroll_offset > 0,
        "wheel-up over the telemetry pane scrolls it"
    );

    app.handle_mouse_event(mouse(MouseEventKind::ScrollDown, 65, 5));
    assert_eq!(
        app.telemetry_scroll_offset, 0,
        "wheel-down scrolls back down"
    );
}

#[test]
fn wheel_scrolls_the_context_panel() {
    let mut app = app_with_session("s1");
    app.show_context_panel = true;
    app.context_panel_area = Rect::new(60, 1, 20, 10);
    app.context_scroll_offset = 0;

    app.handle_mouse_event(mouse(MouseEventKind::ScrollUp, 65, 5));
    assert!(app.context_scroll_offset > 0);

    app.handle_mouse_event(mouse(MouseEventKind::ScrollDown, 65, 5));
    assert_eq!(app.context_scroll_offset, 0);
}

#[test]
fn wheel_scrolls_the_memory_panel() {
    let mut app = app_with_session("s1");
    app.show_memory = true;
    app.memory_area = Rect::new(60, 1, 20, 10);
    app.memory_scroll_offset = 0;

    app.handle_mouse_event(mouse(MouseEventKind::ScrollUp, 65, 5));
    assert!(app.memory_scroll_offset > 0);

    app.handle_mouse_event(mouse(MouseEventKind::ScrollDown, 65, 5));
    assert_eq!(app.memory_scroll_offset, 0);
}

#[test]
fn wheel_over_the_input_pane_is_a_noop() {
    let mut app = app_with_session("s1");
    app.input_area = Rect::new(0, 40, 80, 3);
    app.scroll_offset = 5;

    app.handle_mouse_event(mouse(MouseEventKind::ScrollUp, 5, 41));
    // The input pane does not scroll the message view.
    assert_eq!(app.scroll_offset, 5);
}

// ===========================================================================
// Right-click context menu per pane, dismissal, and action dispatch
// ===========================================================================

#[test]
fn right_click_in_message_pane_opens_context_menu() {
    let mut app = app_with_session("s1");
    app.message_area = Rect::new(0, 1, 80, 20);

    app.handle_mouse_event(mouse(MouseEventKind::Down(MouseButton::Right), 5, 5));
    let menu = app.context_menu.as_ref().expect("context menu opens");
    assert_eq!(menu.pane, SelectionPane::Messages);
}

#[test]
fn right_click_outside_every_pane_clears_context_menu() {
    let mut app = app_with_session("s1");
    app.message_area = Rect::new(0, 1, 80, 20);
    app.context_menu = Some(ContextMenuState {
        x: 1,
        y: 1,
        pane: SelectionPane::Messages,
        selected: 0,
        items: vec![(ContextAction::Copy, true)],
    });

    // (200, 200) is outside every cached pane.
    app.handle_mouse_event(mouse(MouseEventKind::Down(MouseButton::Right), 200, 200));
    assert!(app.context_menu.is_none());
}

#[test]
fn second_right_click_dismisses_the_context_menu() {
    let mut app = app_with_session("s1");
    app.context_menu = Some(ContextMenuState {
        x: 2,
        y: 2,
        pane: SelectionPane::Messages,
        selected: 0,
        items: vec![(ContextAction::Copy, true)],
    });

    app.handle_mouse_event(mouse(MouseEventKind::Down(MouseButton::Right), 3, 3));
    assert!(app.context_menu.is_none(), "a second right-click dismisses");
}

#[test]
fn left_click_on_a_disabled_context_item_dismisses_menu() {
    let mut app = app_with_session("s1");
    // Menu at (2,2); item rows start at y=3. Item 0 is disabled.
    app.context_menu = Some(ContextMenuState {
        x: 2,
        y: 2,
        pane: SelectionPane::Messages,
        selected: 0,
        items: vec![(ContextAction::Cut, false)],
    });

    app.handle_mouse_event(mouse(MouseEventKind::Down(MouseButton::Left), 3, 3));
    assert!(
        app.context_menu.is_none(),
        "selecting a disabled item closes the menu without acting"
    );
}

#[test]
fn left_click_outside_context_menu_dismisses_it() {
    let mut app = app_with_session("s1");
    app.context_menu = Some(ContextMenuState {
        x: 2,
        y: 2,
        pane: SelectionPane::Messages,
        selected: 0,
        items: vec![(ContextAction::Copy, true)],
    });

    app.handle_mouse_event(mouse(MouseEventKind::Down(MouseButton::Left), 80, 40));
    assert!(app.context_menu.is_none());
}

#[test]
fn execute_context_action_copy_clears_the_menu() {
    let mut app = app_with_session("s1");
    app.message_area = Rect::new(0, 1, 80, 20);
    app.text_selection = Some(TextSelection {
        pane: SelectionPane::Messages,
        anchor: (0, 2),
        endpoint: (3, 2),
    });
    app.context_menu = Some(ContextMenuState {
        x: 2,
        y: 2,
        pane: SelectionPane::Messages,
        selected: 0,
        items: vec![(ContextAction::Copy, true)],
    });
    app.message_content_lines = vec!["hello world".to_string()];

    app.execute_context_action(ContextAction::Copy);
    assert!(app.context_menu.is_none(), "the menu closes on dispatch");
}

#[test]
fn execute_context_action_cut_on_non_input_pane_only_copies() {
    // Cut outside the input pane copies but does not mutate the buffer.
    let mut app = app_with_session("s1");
    app.input = "abcdef".to_string();
    app.input_cursor = 6;
    app.message_area = Rect::new(0, 1, 80, 20);
    app.text_selection = Some(TextSelection {
        pane: SelectionPane::Messages,
        anchor: (0, 2),
        endpoint: (3, 2),
    });
    app.context_menu = Some(ContextMenuState {
        x: 2,
        y: 2,
        pane: SelectionPane::Messages,
        selected: 0,
        items: vec![(ContextAction::Cut, true)],
    });

    app.execute_context_action(ContextAction::Cut);
    assert!(app.context_menu.is_none());
    assert_eq!(app.input, "abcdef", "the input buffer is untouched");
}

#[test]
fn execute_context_action_paste_on_non_input_pane_is_a_noop() {
    let mut app = app_with_session("s1");
    app.input = "keep".to_string();
    app.context_menu = Some(ContextMenuState {
        x: 2,
        y: 2,
        pane: SelectionPane::Messages,
        selected: 0,
        items: vec![(ContextAction::Paste, true)],
    });

    app.execute_context_action(ContextAction::Paste);
    assert!(app.context_menu.is_none());
    assert_eq!(
        app.input, "keep",
        "paste into a non-input pane does nothing"
    );
}

// ===========================================================================
// File-menu mouse hover + click selection
// ===========================================================================

fn file_menu_with_two_entries() -> FileMenuState {
    FileMenuState {
        matches: vec![
            FileMenuEntry {
                display: "src/a.rs".to_string(),
                path: std::path::PathBuf::from("src/a.rs"),
                is_dir: false,
            },
            FileMenuEntry {
                display: "src/b.rs".to_string(),
                path: std::path::PathBuf::from("src/b.rs"),
                is_dir: false,
            },
        ],
        selected: 0,
        scroll_offset: 0,
        query: "a".to_string(),
        current_dir: None,
    }
}

#[test]
fn mouse_move_over_file_menu_updates_hovered_row() {
    let mut app = app_with_session("s1");
    app.input_area = Rect::new(0, 10, 60, 3);
    app.file_menu = Some(file_menu_with_two_entries());

    // Popup geometry: height = (2 items + 1 + 2).min(10) = 5, top = 10 - 5 = 5.
    // Content rows begin at popup_y + 1 = 6.
    app.handle_mouse_event(mouse(MouseEventKind::Moved, 2, 7));
    let menu = app.file_menu.as_ref().expect("menu still open");
    assert_eq!(menu.selected, 1, "hovering the second row selects it");
}

#[test]
fn left_click_outside_file_menu_popup_dismisses_it() {
    let mut app = app_with_session("s1");
    app.input_area = Rect::new(0, 10, 60, 3);
    app.file_menu = Some(file_menu_with_two_entries());

    app.handle_mouse_event(mouse(MouseEventKind::Down(MouseButton::Left), 200, 60));
    assert!(app.file_menu.is_none());
}

// ===========================================================================
// Drag: scrollbar drag and text-selection endpoint update
// ===========================================================================

#[test]
fn drag_updates_text_selection_endpoint() {
    let mut app = app_with_session("s1");
    app.message_area = Rect::new(0, 1, 80, 20);
    app.text_selection = Some(TextSelection {
        pane: SelectionPane::Messages,
        anchor: (1, 2),
        endpoint: (1, 2),
    });

    app.handle_mouse_event(mouse(MouseEventKind::Drag(MouseButton::Left), 9, 6));
    let sel = app.text_selection.as_ref().expect("selection remains");
    assert_eq!(sel.endpoint, (9, 6));
}

#[test]
fn left_mouse_up_clears_scrollbar_drag() {
    let mut app = app_with_session("s1");
    app.scrollbar_drag = Some(ragent_tui::app::ScrollbarDragPane::Messages);

    app.handle_mouse_event(mouse(MouseEventKind::Up(MouseButton::Left), 5, 5));
    assert!(app.scrollbar_drag.is_none());
}

// ===========================================================================
// History picker keyboard navigation
// ===========================================================================

#[tokio::test]
async fn history_picker_down_and_enter_restores_entry() {
    let mut app = app_with_session("s1");
    app.history_picker = Some(HistoryPickerState {
        entries: vec!["first".to_string(), "second".to_string()],
        selected: 0,
        scroll_offset: 0,
    });

    app.handle_key_event(key(KeyCode::Down)).await;
    assert_eq!(app.history_picker.as_ref().unwrap().selected, 1);

    app.handle_key_event(key(KeyCode::Enter)).await;
    assert!(app.history_picker.is_none(), "Enter closes the picker");
    assert_eq!(app.input, "second");
    assert_eq!(app.input_cursor, app.input.chars().count());
}

#[tokio::test]
async fn history_picker_escape_and_q_close_without_change() {
    for close in [KeyCode::Esc, KeyCode::Char('q')] {
        let mut app = app_with_session("s1");
        app.input = "keep".to_string();
        app.history_picker = Some(HistoryPickerState {
            entries: vec!["x".to_string()],
            selected: 0,
            scroll_offset: 0,
        });
        app.handle_key_event(key(close)).await;
        assert!(
            app.history_picker.is_none(),
            "key {close:?} closes the picker"
        );
        assert_eq!(app.input, "keep", "the input is untouched");
    }
}

#[tokio::test]
async fn history_picker_up_clamps_at_top() {
    let mut app = app_with_session("s1");
    app.history_picker = Some(HistoryPickerState {
        entries: vec!["a".to_string(), "b".to_string()],
        selected: 0,
        scroll_offset: 0,
    });
    app.handle_key_event(key(KeyCode::Up)).await;
    assert_eq!(app.history_picker.as_ref().unwrap().selected, 0);
}

// ===========================================================================
// Agents / Teams window Space and Shift+K shortcuts
// ===========================================================================

#[tokio::test]
async fn space_on_the_primary_agent_row_is_refused() {
    let mut app = app_with_session("s1");
    app.show_agents_window = true;
    app.selected_agent_index = Some(0);

    app.handle_key_event(key(KeyCode::Char(' '))).await;
    assert_eq!(app.status, "Cannot suspend primary agent");
}

#[tokio::test]
async fn shift_k_on_the_primary_agent_row_is_refused() {
    let mut app = app_with_session("s1");
    app.show_agents_window = true;
    app.selected_agent_index = Some(0);

    app.handle_key_event(kmod(KeyCode::Char('K'), KeyModifiers::SHIFT))
        .await;
    assert_eq!(app.status, "Cannot kill primary agent");
}

#[tokio::test]
async fn space_in_the_teams_window_with_a_focused_teammate_does_not_panic() {
    let mut app = app_with_session("s1");
    app.show_teams_window = true;
    app.focused_teammate = Some("tm-001".to_string());

    app.handle_key_event(key(KeyCode::Char(' '))).await;
    // No panic: with no live TeamManager the shutdown is a best-effort no-op.
    assert!(app.show_teams_window);
}

// ===========================================================================
// Run-cost banner dismissal on keypress
// ===========================================================================

#[tokio::test]
async fn non_character_key_dismisses_run_cost_banner_and_is_consumed() {
    let mut app = app_with_session("s1");
    app.run_cost_banner = Some("banner".to_string());
    app.run_cost_banner_at = Some(std::time::Instant::now());

    // A non-character key only clears the banner and is consumed.
    app.handle_key_event(key(KeyCode::Esc)).await;
    assert!(app.run_cost_banner.is_none());
    assert!(app.run_cost_banner_at.is_none());
}

#[tokio::test]
async fn printable_char_dismisses_banner_and_is_typed() {
    let mut app = app_with_session("s1");
    app.input.clear();
    app.run_cost_banner = Some("banner".to_string());
    app.run_cost_banner_at = Some(std::time::Instant::now());

    app.handle_key_event(key(KeyCode::Char('x'))).await;
    assert!(app.run_cost_banner.is_none(), "banner cleared");
    assert!(
        app.input.contains('x'),
        "the first keystroke still reaches the input, got: {:?}",
        app.input
    );
}

// ===========================================================================
// Memory-view overlay key routing (PageUp/Down, Up/Down, Esc)
// ===========================================================================

#[tokio::test]
async fn memory_view_keys_route_to_scroll_actions() {
    let mut app = app_with_session("s1");
    app.memory_view = Some(ragent_tui::app::MemoryViewState {
        row: ragent_storage::storage::MemoryRow {
            id: 1,
            content: "line".to_string(),
            category: "fact".to_string(),
            source: "test".to_string(),
            confidence: 0.7,
            project: "p".to_string(),
            session_id: "s1".to_string(),
            created_at: String::new(),
            updated_at: String::new(),
            access_count: 0,
            last_accessed: None,
        },
        scroll_offset: 0,
        max_scroll: 50,
        line_cache: ragent_tui::app::OutputViewLineCache {
            wrapped_lines: Vec::new(),
            content_lines: Vec::new(),
            wrapped_count: 0,
            cache_width: 0,
            source_generation: 0,
        },
    });

    app.handle_key_event(key(KeyCode::PageUp)).await;
    assert_eq!(app.memory_view.as_ref().unwrap().scroll_offset, 5);

    app.handle_key_event(key(KeyCode::PageDown)).await;
    assert_eq!(app.memory_view.as_ref().unwrap().scroll_offset, 0);

    app.handle_key_event(key(KeyCode::Esc)).await;
    assert!(app.memory_view.is_none(), "Esc closes the memory view");
}

// ===========================================================================
// message helper keeps imports exercised
// ===========================================================================

#[test]
fn assistant_message_text_content_is_readable() {
    let msg = Message::new(
        "s1",
        Role::Assistant,
        vec![MessagePart::Text {
            text: "body".to_string(),
        }],
    );
    assert_eq!(msg.text_content(), "body");
}
