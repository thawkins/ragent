//! Session sidebar (spec MULTISESSION, milestone M5).
//!
//! Renders the full layout into a `TestBackend` and drives the sidebar with
//! real key and mouse events. Sessions live in the in-memory storage
//! `make_app` builds, so nothing is written into the repository.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ragent_agent::event::Event;
use ragent_tui::App;
use ragent_tui::sessions_sidebar::{self, SidebarState};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::Color;

#[path = "support/mod.rs"]
mod support;

/// An app with the sidebar on and two sessions: `(app, a, b)`, `b` focused
/// and newer than `a`.
async fn app_with_sidebar() -> (App, String, String) {
    let mut app = support::make_app();
    app.sidebar.visible = true;
    app.execute_slash_command("/session list").await;
    let a = app.session.session_id.clone().expect("session A");
    // Distinct `updated_at` values keep the order deterministic.
    std::thread::sleep(std::time::Duration::from_millis(5));
    app.execute_slash_command("/session new").await;
    let b = app.session.session_id.clone().expect("session B");
    (app, a, b)
}

/// Draws the whole layout and returns it as text rows.
fn draw(app: &mut App, width: u16, height: u16) -> (Vec<String>, Terminal<TestBackend>) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("draw");
    let buffer = terminal.backend().buffer().clone();
    let rows = (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect();
    (rows, terminal)
}

/// The sidebar columns of each row (the first `app.sidebar.area.width`).
fn sidebar_rows(app: &mut App) -> Vec<String> {
    let (rows, _) = draw(app, 120, 30);
    let width = usize::from(app.sidebar.area.width);
    rows.into_iter()
        .map(|r| r.chars().take(width).collect::<String>())
        .collect()
}

/// Index of the first sidebar row containing `needle`.
fn row_of(rows: &[String], needle: &str) -> Option<usize> {
    rows.iter().position(|r| r.contains(needle))
}

/// The `untitled <tail>` label the sidebar shows for an untitled session.
fn untitled(id: &str) -> String {
    format!("untitled {}", &id[id.len() - 8..])
}

/// The label of the launch project's section.
fn launch_label(app: &App) -> String {
    let root = ragent_storage::project_root(&app.cwd_path.display().to_string());
    std::path::Path::new(&root)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or(root)
}

fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

fn click(column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

#[tokio::test]
async fn test_sessions_sidebar_lists_sessions_under_project() {
    let (mut app, a, b) = app_with_sidebar().await;
    let rows = sidebar_rows(&mut app);
    let header = format!("v {} (2)", launch_label(&app));

    assert!(row_of(&rows, "SESSIONS").is_some(), "{rows:#?}");
    let new_row = row_of(&rows, "+ New session").expect("new session row");
    let header_row = row_of(&rows, &header).expect("project header");
    let b_row = row_of(&rows, &untitled(&b)).expect("row for B");
    let a_row = row_of(&rows, &untitled(&a)).expect("row for A");
    assert!(new_row < header_row && header_row < b_row && b_row < a_row);
}

#[tokio::test]
async fn test_sessions_sidebar_highlights_focused_session() {
    let (mut app, _a, b) = app_with_sidebar().await;
    let (rows, terminal) = draw(&mut app, 120, 30);
    let y = row_of(&rows, &untitled(&b)).expect("row for B");
    let x = rows[y].find(&untitled(&b)).expect("title column");
    let cell = &terminal.backend().buffer()[(x as u16, y as u16)];
    assert_eq!(cell.fg, Color::Cyan);
}

#[tokio::test]
async fn test_sessions_sidebar_hides_when_narrow_or_turned_off() {
    let (mut app, _a, _b) = app_with_sidebar().await;
    let (rows, _) = draw(&mut app, 90, 30);
    assert!(
        row_of(&rows, "SESSIONS").is_none(),
        "hidden below 100 columns"
    );

    app.sidebar.visible = false;
    let (rows, _) = draw(&mut app, 120, 30);
    assert!(
        row_of(&rows, "SESSIONS").is_none(),
        "hidden when turned off"
    );
}

#[tokio::test]
async fn test_sessions_sidebar_pinned_section_comes_first() {
    let (mut app, a, b) = app_with_sidebar().await;
    app.execute_slash_command("/session pin").await;
    let rows = sidebar_rows(&mut app);

    let pinned = row_of(&rows, "v Pinned (1)").expect("pinned header");
    let project = row_of(&rows, &format!("v {} (1)", launch_label(&app))).expect("project");
    let b_row = row_of(&rows, &untitled(&b)).expect("B");
    let a_row = row_of(&rows, &untitled(&a)).expect("A");
    assert!(pinned < b_row && b_row < project && project < a_row);
}

#[tokio::test]
async fn test_sessions_sidebar_left_collapses_a_section() {
    let (mut app, a, _b) = app_with_sidebar().await;
    let _ = sidebar_rows(&mut app);
    app.handle_key_event(key(KeyCode::Char('s'), KeyModifiers::ALT))
        .await;
    assert!(app.sidebar.focused, "Alt+S focuses the visible sidebar");
    app.handle_key_event(key(KeyCode::Down, KeyModifiers::NONE))
        .await;
    app.handle_key_event(key(KeyCode::Left, KeyModifiers::NONE))
        .await;

    let rows = sidebar_rows(&mut app);
    assert!(row_of(&rows, &format!("> {} (2)", launch_label(&app))).is_some());
    assert!(row_of(&rows, &untitled(&a)).is_none(), "sessions hidden");
}

#[tokio::test]
async fn test_sessions_sidebar_filter_keeps_matching_titles() {
    let (mut app, a, b) = app_with_sidebar().await;
    app.storage
        .update_session(&a, "alpha parser")
        .expect("title A");
    app.storage
        .update_session(&b, "beta lexer")
        .expect("title B");
    app.sidebar.dirty = true;
    app.sidebar.focused = true;
    for code in [
        KeyCode::Char('/'),
        KeyCode::Char('a'),
        KeyCode::Char('l'),
        KeyCode::Char('p'),
        KeyCode::Enter,
    ] {
        app.handle_key_event(key(code, KeyModifiers::NONE)).await;
    }

    let rows = sidebar_rows(&mut app);
    assert!(row_of(&rows, "SESSIONS /alp").is_some(), "{rows:#?}");
    assert!(row_of(&rows, "alpha parser").is_some());
    assert!(row_of(&rows, "beta lexer").is_none());
}

#[tokio::test]
async fn test_sessions_sidebar_marks_waiting_session_and_badges_status_bar() {
    let (mut app, a, _b) = app_with_sidebar().await;
    app.handle_event(Event::PermissionRequested {
        session_id: a.clone(),
        request_id: "p1".to_string(),
        permission: "file:write".to_string(),
        description: "write".to_string(),
        options: vec!["once".to_string()],
    })
    .await;

    let (rows, _) = draw(&mut app, 120, 30);
    let a_row = row_of(&rows, &untitled(&a)).expect("A");
    assert!(
        rows[a_row].contains("! untitled"),
        "row was: {}",
        rows[a_row]
    );
    assert!(
        rows.iter().any(|r| r.contains("1 need you")),
        "status bar badge missing"
    );
}

#[tokio::test]
async fn test_sessions_sidebar_ctrl_down_and_alt_r_switch_sessions() {
    let (mut app, a, b) = app_with_sidebar().await;
    let _ = sidebar_rows(&mut app);

    app.handle_key_event(key(KeyCode::Down, KeyModifiers::CONTROL))
        .await;
    assert_eq!(app.session.session_id.as_deref(), Some(a.as_str()));

    app.handle_key_event(key(KeyCode::Char('r'), KeyModifiers::ALT))
        .await;
    assert_eq!(app.session.session_id.as_deref(), Some(b.as_str()));
}

#[tokio::test]
async fn test_sessions_sidebar_ctrl_n_opens_a_session() {
    let (mut app, _a, _b) = app_with_sidebar().await;
    app.handle_key_event(key(KeyCode::Char('n'), KeyModifiers::CONTROL))
        .await;
    assert_eq!(app.background.len(), 2);
}

#[tokio::test]
async fn test_sessions_sidebar_alt_s_cycles_show_focus_hide() {
    let mut app = support::make_app();
    app.handle_key_event(key(KeyCode::Char('s'), KeyModifiers::ALT))
        .await;
    assert!(app.sidebar.visible && app.sidebar.focused);
    app.handle_key_event(key(KeyCode::Char('s'), KeyModifiers::ALT))
        .await;
    assert!(!app.sidebar.visible && !app.sidebar.focused);
}

#[tokio::test]
async fn test_sessions_sidebar_enter_on_row_switches_session() {
    let (mut app, a, _b) = app_with_sidebar().await;
    let rows = sidebar_rows(&mut app);
    let a_row = row_of(&rows, &untitled(&a)).expect("A");
    app.sidebar.focused = true;
    // Rows inside the border start one line below the sidebar top.
    app.sidebar.cursor = a_row - 1;

    app.handle_key_event(key(KeyCode::Enter, KeyModifiers::NONE))
        .await;

    assert_eq!(app.session.session_id.as_deref(), Some(a.as_str()));
    assert!(!app.sidebar.focused, "focus returns to the input");
}

#[tokio::test]
async fn test_sessions_sidebar_click_switches_and_drag_resizes() {
    let (mut app, a, _b) = app_with_sidebar().await;
    let rows = sidebar_rows(&mut app);
    let a_row = row_of(&rows, &untitled(&a)).expect("A");

    app.handle_mouse_event(click(5, a_row as u16));
    assert_eq!(app.session.session_id.as_deref(), Some(a.as_str()));

    let border = app.sidebar.area.width - 1;
    app.handle_mouse_event(click(border, 3));
    app.handle_mouse_event(MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: 39,
        row: 3,
        modifiers: KeyModifiers::NONE,
    });
    app.handle_mouse_event(MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: 39,
        row: 3,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.sidebar.width, 40);
    assert_eq!(SidebarState::load(&app.storage).width, 40, "width saved");
}

#[tokio::test]
async fn test_sessions_sidebar_collapsed_sections_persist() {
    let (mut app, _a, _b) = app_with_sidebar().await;
    let _ = sidebar_rows(&mut app);
    app.sidebar.focused = true;
    app.sidebar.cursor = 1;
    app.handle_key_event(key(KeyCode::Left, KeyModifiers::NONE))
        .await;

    let loaded = SidebarState::load(&app.storage);
    assert_eq!(loaded.collapsed.len(), 1);
    assert!(loaded.visible);
}

#[test]
fn test_sessions_sidebar_relative_age() {
    let now = chrono::DateTime::parse_from_rfc3339("2026-11-12T12:00:00Z")
        .expect("now")
        .with_timezone(&chrono::Utc);
    let age = |t: &str| sessions_sidebar::relative_age(t, now);
    assert_eq!(age("2026-11-12T11:59:30Z"), "now");
    assert_eq!(age("2026-11-12T11:55:00Z"), "5m");
    assert_eq!(age("2026-11-12T09:00:00Z"), "3h");
    assert_eq!(age("2026-11-08T12:00:00Z"), "4d");
    assert_eq!(age("2026-10-29T12:00:00Z"), "2w");
    assert_eq!(age("not a time"), "");
}

#[tokio::test]
async fn test_sessions_sidebar_marks_failed_background_run_until_focused() {
    let (mut app, a, _b) = app_with_sidebar().await;
    app.handle_event(Event::AgentError {
        session_id: a.clone(),
        error: "provider unreachable".to_string(),
    })
    .await;

    let rows = sidebar_rows(&mut app);
    let a_row = row_of(&rows, &untitled(&a)).expect("A");
    assert!(
        rows[a_row].contains("x untitled"),
        "row was: {}",
        rows[a_row]
    );

    app.switch_to_session(&a).expect("focus A");
    let rows = sidebar_rows(&mut app);
    let a_row = row_of(&rows, &untitled(&a)).expect("A");
    assert!(!rows[a_row].contains("x untitled"), "cleared once seen");
}
