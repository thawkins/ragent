//! Session context menu (spec SESSIONMENU, milestone M1).
//!
//! Renders the full layout into a `TestBackend`, opens the menu on a sidebar
//! row with keys or the mouse, and checks the painted menu and the actions it
//! dispatches.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ragent_tui::App;
use ragent_tui::session_menu::{self, MenuAction};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

#[path = "support/mod.rs"]
mod support;

const WIDTH: u16 = 120;
const HEIGHT: u16 = 30;

/// An app with the sidebar on and two sessions: `(app, a, b)`, `b` focused.
async fn app_with_sidebar() -> (App, String, String) {
    let mut app = support::make_app();
    app.sidebar.visible = true;
    app.execute_slash_command("/session list").await;
    let a = app.session.session_id.clone().expect("session A");
    std::thread::sleep(std::time::Duration::from_millis(5));
    app.execute_slash_command("/session new").await;
    let b = app.session.session_id.clone().expect("session B");
    (app, a, b)
}

/// Draws the whole layout and returns it as text rows.
fn draw(app: &mut App) -> Vec<String> {
    let mut terminal = Terminal::new(TestBackend::new(WIDTH, HEIGHT)).expect("terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("draw");
    let buffer = terminal.backend().buffer().clone();
    (0..HEIGHT)
        .map(|y| {
            (0..WIDTH)
                .map(|x| buffer[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect()
}

/// Screen row of the sidebar row showing session `id`.
fn row_of_session(app: &mut App, id: &str) -> usize {
    let label = format!("untitled {}", &id[id.len() - 8..]);
    let rows = draw(app);
    let width = usize::from(app.sidebar.area.width);
    rows.iter()
        .position(|r| r.chars().take(width).collect::<String>().contains(&label))
        .expect("session row")
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

/// Focuses the sidebar with its cursor on session `id`'s row.
fn put_cursor_on(app: &mut App, id: &str) -> usize {
    let row = row_of_session(app, id);
    app.sidebar.focused = true;
    // Rows inside the border start one line below the sidebar top.
    app.sidebar.cursor = row - 1;
    row
}

#[tokio::test]
async fn test_session_menu_m_opens_menu_with_accelerators_and_rules() {
    let (mut app, a, _b) = app_with_sidebar().await;
    let row = put_cursor_on(&mut app, &a);

    app.handle_key_event(key(KeyCode::Char('m'))).await;

    let menu = app.sidebar.menu.clone().expect("menu open");
    assert_eq!(menu.session_id, a);
    let actions: Vec<MenuAction> = menu.items.iter().map(|i| i.action.clone()).collect();
    assert_eq!(
        actions,
        [
            MenuAction::OpenIn,
            MenuAction::TogglePin,
            MenuAction::ToggleUnread,
            MenuAction::Rename,
            MenuAction::Fork,
            MenuAction::MoveTo,
            MenuAction::Archive,
            MenuAction::Delete
        ]
    );
    let rows = draw(&mut app);
    let area = app.sidebar.menu.as_ref().expect("menu").area;
    assert_eq!(usize::from(area.y), row + 1, "menu opens below its row");
    let lines: Vec<String> = rows[usize::from(area.y)..usize::from(area.y + area.height)]
        .iter()
        .map(|r| {
            r.chars()
                .skip(usize::from(area.x))
                .take(usize::from(area.width))
                .collect()
        })
        .collect();
    assert!(
        lines[3].contains("Pin") && lines[3].contains(" P "),
        "{lines:#?}"
    );
    for (line, label, accel) in [
        (1, "Open in", " > "),
        (4, "Mark as unread", " U "),
        (5, "Rename", " R "),
        (6, "Fork", " F "),
        (8, "Move to section", " > "),
        (10, "Archive", " A "),
        (11, "Delete", " D "),
    ] {
        assert!(
            lines[line].contains(label) && lines[line].contains(accel),
            "{label}: {lines:#?}"
        );
    }
    assert!(
        [2, 7, 9].iter().all(|&l| lines[l].contains("-----")),
        "rule between groups: {lines:#?}"
    );
    assert!(menu.items[7].danger, "Delete is drawn as dangerous");
}

#[tokio::test]
async fn test_session_menu_shift_f10_opens_and_esc_closes() {
    let (mut app, a, _b) = app_with_sidebar().await;
    put_cursor_on(&mut app, &a);

    app.handle_key_event(KeyEvent::new(KeyCode::F(10), KeyModifiers::SHIFT))
        .await;
    assert!(app.sidebar.menu.is_some(), "Shift+F10 opens the menu");

    app.handle_key_event(key(KeyCode::Esc)).await;
    assert!(app.sidebar.menu.is_none(), "Esc closes the menu");
    assert!(app.sidebar.focused, "the sidebar keeps focus");
}

#[tokio::test]
async fn test_session_menu_not_opened_on_header_rows() {
    let (mut app, _a, _b) = app_with_sidebar().await;
    let _ = draw(&mut app);
    app.sidebar.focused = true;
    app.sidebar.cursor = 0; // `+ New session`

    app.handle_key_event(key(KeyCode::Char('m'))).await;
    assert!(app.sidebar.menu.is_none());

    // Right-click on the `+ New session` row (screen row 1) opens nothing.
    app.handle_mouse_event(mouse(MouseEventKind::Down(MouseButton::Right), 5, 1));
    assert!(app.sidebar.menu.is_none());
}

#[tokio::test]
async fn test_session_menu_right_click_opens_for_clicked_row() {
    let (mut app, a, b) = app_with_sidebar().await;
    let row = row_of_session(&mut app, &a);

    app.handle_mouse_event(mouse(
        MouseEventKind::Down(MouseButton::Right),
        5,
        row as u16,
    ));

    let menu = app.sidebar.menu.as_ref().expect("menu open");
    assert_eq!(menu.session_id, a);
    assert_eq!(
        app.sidebar.cursor,
        row - 1,
        "cursor moves to the clicked row"
    );
    assert_eq!(
        app.session.session_id.as_deref(),
        Some(b.as_str()),
        "right-click does not switch sessions"
    );
}

#[tokio::test]
async fn test_session_menu_accelerator_pins_like_p() {
    let (mut app, a, _b) = app_with_sidebar().await;
    put_cursor_on(&mut app, &a);

    app.handle_key_event(key(KeyCode::Char('m'))).await;
    app.handle_key_event(key(KeyCode::Char('P'))).await;

    assert!(
        app.sidebar.menu.is_none(),
        "running an item closes the menu"
    );
    assert!(
        app.storage.pinned_session_ids().expect("pins").contains(&a),
        "P pins the session"
    );

    let _ = draw(&mut app); // reloads the cached pin set
    let row = row_of_session(&mut app, &a);
    app.sidebar.focused = true;
    app.sidebar.cursor = row - 1;
    app.handle_key_event(key(KeyCode::Char('m'))).await;
    let label = &app.sidebar.menu.as_ref().expect("menu").items[1].label;
    assert_eq!(label, "Unpin");
}

#[tokio::test]
async fn test_session_menu_enter_on_archive_archives_background_session() {
    let (mut app, a, b) = app_with_sidebar().await;
    put_cursor_on(&mut app, &a);

    app.handle_key_event(key(KeyCode::Char('m'))).await;
    app.handle_key_event(key(KeyCode::End)).await;
    app.handle_key_event(key(KeyCode::Up)).await;
    app.handle_key_event(key(KeyCode::Enter)).await;

    let row = app.storage.get_session(&a).expect("read").expect("row A");
    assert!(row.archived_at.is_some(), "A is archived");
    assert_eq!(app.session.session_id.as_deref(), Some(b.as_str()));
}

#[tokio::test]
async fn test_session_menu_click_on_rename_prefills_input() {
    let (mut app, a, _b) = app_with_sidebar().await;
    put_cursor_on(&mut app, &a);
    app.handle_key_event(key(KeyCode::Char('m'))).await;
    let _ = draw(&mut app);
    let area = app.sidebar.menu.as_ref().expect("menu").area;

    // Line 5 inside the border is `Rename`.
    app.handle_mouse_event(mouse(
        MouseEventKind::Down(MouseButton::Left),
        area.x + 2,
        area.y + 5,
    ));

    assert!(app.sidebar.menu.is_none());
    assert_eq!(app.session.session_id.as_deref(), Some(a.as_str()));
    assert!(
        app.session.input.starts_with("/session rename"),
        "{:?}",
        app.session.input
    );
}

#[tokio::test]
async fn test_session_menu_click_outside_closes_without_acting() {
    let (mut app, a, b) = app_with_sidebar().await;
    put_cursor_on(&mut app, &a);
    app.handle_key_event(key(KeyCode::Char('m'))).await;
    let _ = draw(&mut app);

    app.handle_mouse_event(mouse(
        MouseEventKind::Down(MouseButton::Left),
        WIDTH - 2,
        HEIGHT - 2,
    ));

    assert!(app.sidebar.menu.is_none());
    assert_eq!(app.session.session_id.as_deref(), Some(b.as_str()));
    assert!(
        app.storage
            .get_session(&a)
            .expect("read")
            .expect("row A")
            .archived_at
            .is_none()
    );
}

#[tokio::test]
async fn test_session_menu_flips_above_row_near_bottom() {
    let (mut app, a, _b) = app_with_sidebar().await;
    let _ = draw(&mut app);
    let row_y = HEIGHT - 2;
    session_menu::open(&mut app, &a, row_y);

    let _ = draw(&mut app);

    let area = app.sidebar.menu.as_ref().expect("menu").area;
    assert_eq!(area.y + area.height, row_y, "menu ends just above its row");
}

/// The state marker of the row showing session `id`: the last column
/// before the sidebar's right border.
fn marker_of(app: &mut App, id: &str) -> char {
    let row = row_of_session(app, id);
    let width = usize::from(app.sidebar.area.width);
    let text: String = draw(app)[row].chars().take(width).collect();
    text.chars().rev().nth(1).expect("marker column")
}

#[tokio::test]
async fn test_session_menu_u_marks_unread_until_focused() {
    let (mut app, a, _b) = app_with_sidebar().await;
    put_cursor_on(&mut app, &a);

    app.handle_key_event(key(KeyCode::Char('m'))).await;
    app.handle_key_event(key(KeyCode::Char('u'))).await;

    assert!(
        app.storage
            .unread_session_ids()
            .expect("unread")
            .contains(&a)
    );
    assert_eq!(marker_of(&mut app, &a), 'o', "unread marker shown");

    // The menu now offers the reverse.
    put_cursor_on(&mut app, &a);
    app.handle_key_event(key(KeyCode::Char('m'))).await;
    assert_eq!(
        app.sidebar.menu.as_ref().expect("menu").items[2].label,
        "Mark as read"
    );
    app.handle_key_event(key(KeyCode::Esc)).await;

    // Focusing the session reads it.
    app.switch_to_session(&a).expect("focus A");
    assert!(app.storage.unread_session_ids().expect("unread").is_empty());
}

#[tokio::test]
async fn test_session_menu_unread_mark_shows_for_a_session_not_open() {
    let (mut app, a, _b) = app_with_sidebar().await;
    app.storage.set_session_unread(&a, true).expect("mark");
    // A is still open in the background; close it so only storage knows.
    app.switch_to_session(&a).expect("focus A");
    app.storage
        .set_session_unread(&a, true)
        .expect("mark again");
    app.execute_slash_command("/session close").await;
    app.sidebar.dirty = true;

    assert_eq!(marker_of(&mut app, &a), 'o');
}

#[tokio::test]
async fn test_session_menu_f_forks_and_focuses_the_copy() {
    let (mut app, a, b) = app_with_sidebar().await;
    app.storage
        .create_message(&ragent_types::message::Message::user_text(
            &a,
            "forked prompt",
        ))
        .expect("message");
    put_cursor_on(&mut app, &a);
    // Renamed after the cursor is placed: the sidebar still shows the
    // cached `untitled` label the cursor was found by.
    app.storage.update_session(&a, "Original").expect("title");

    app.handle_key_event(key(KeyCode::Char('m'))).await;
    app.handle_key_event(key(KeyCode::Char('f'))).await;

    let fork = app.session.session_id.clone().expect("fork focused");
    assert!(fork != a && fork != b, "a new session is focused");
    let row = app.storage.get_session(&fork).expect("get").expect("row");
    assert_eq!(row.title, "Original (fork)");
    let texts: Vec<String> = app
        .storage
        .get_messages(&fork)
        .expect("messages")
        .iter()
        .map(ragent_types::message::Message::text_content)
        .collect();
    assert_eq!(texts, ["forked prompt"]);
    assert!(
        app.background
            .iter()
            .any(|v| v.session_id.as_deref() == Some(b.as_str())),
        "the previous session stays open"
    );
}

#[tokio::test]
async fn test_session_menu_fork_refuses_a_running_session() {
    let (mut app, _a, b) = app_with_sidebar().await;
    app.session.is_processing = true;

    app.execute_slash_command("/session fork").await;

    assert_eq!(app.session.session_id.as_deref(), Some(b.as_str()));
    assert!(app.status.contains("running"), "{}", app.status);
    app.session.is_processing = false;
}

#[tokio::test]
async fn test_session_menu_delete_asks_and_no_keeps_the_session() {
    let (mut app, a, _b) = app_with_sidebar().await;
    put_cursor_on(&mut app, &a);

    app.handle_key_event(key(KeyCode::Char('m'))).await;
    app.handle_key_event(key(KeyCode::Char('d'))).await;

    assert!(app.sidebar.menu.is_none());
    assert!(app.row_action_confirm_open(), "Delete asks first");
    assert!(!app.row_action_confirm_is_yes(), "the default answer is No");
    let rows = draw(&mut app);
    assert!(
        rows.iter().any(|r| r.contains("Delete session?")),
        "dialog title painted"
    );
    assert!(rows.iter().any(|r| r.contains("cannot be undone")));

    app.handle_key_event(key(KeyCode::Enter)).await;

    assert!(!app.row_action_confirm_open());
    assert!(app.storage.get_session(&a).expect("get").is_some(), "kept");
}

#[tokio::test]
async fn test_session_menu_delete_yes_removes_background_session() {
    let (mut app, a, b) = app_with_sidebar().await;
    put_cursor_on(&mut app, &a);

    app.handle_key_event(KeyEvent::new(KeyCode::Char('D'), KeyModifiers::SHIFT))
        .await;
    app.handle_key_event(key(KeyCode::Left)).await;
    assert!(app.row_action_confirm_is_yes());
    app.handle_key_event(key(KeyCode::Enter)).await;

    assert!(
        app.storage.get_session(&a).expect("get").is_none(),
        "deleted"
    );
    assert!(app.background.position(&a).is_none(), "closed");
    assert_eq!(app.session.session_id.as_deref(), Some(b.as_str()));
    let width = usize::from(app.sidebar.area.width);
    let label = format!("untitled {}", &a[a.len() - 8..]);
    assert!(
        !draw(&mut app)
            .iter()
            .any(|r| r.chars().take(width).collect::<String>().contains(&label)),
        "gone from the sidebar"
    );
}

#[tokio::test]
async fn test_session_menu_delete_focused_session_focuses_another() {
    let (mut app, a, b) = app_with_sidebar().await;

    app.execute_slash_command(&format!("/session delete {b}"))
        .await;
    assert!(app.row_action_confirm_open());
    app.handle_key_event(key(KeyCode::Right)).await;
    app.handle_key_event(key(KeyCode::Enter)).await;

    assert!(app.storage.get_session(&b).expect("get").is_none());
    assert_eq!(app.session.session_id.as_deref(), Some(a.as_str()));
}

#[tokio::test]
async fn test_session_menu_delete_refuses_a_running_session() {
    let (mut app, _a, b) = app_with_sidebar().await;
    app.session.is_processing = true;

    app.execute_slash_command(&format!("/session delete {b}"))
        .await;

    assert!(
        !app.row_action_confirm_open(),
        "no dialog for a running session"
    );
    assert!(app.status.contains("running"), "{}", app.status);
    app.session.is_processing = false;
}

/// Opens the menu on session `id` and moves its cursor to the item with
/// `action`.
async fn menu_on(app: &mut App, id: &str, action: &MenuAction) {
    put_cursor_on(app, id);
    app.handle_key_event(key(KeyCode::Char('m'))).await;
    let menu = app.sidebar.menu.as_mut().expect("menu");
    menu.cursor = menu
        .items
        .iter()
        .position(|i| &i.action == action)
        .expect("item present");
}

/// The labels of the open submenu.
fn submenu_labels(app: &App) -> Vec<String> {
    app.sidebar
        .menu
        .as_ref()
        .and_then(|m| m.submenu.as_ref())
        .expect("submenu open")
        .items
        .iter()
        .map(|i| i.label.trim().to_string())
        .collect()
}

#[tokio::test]
async fn test_session_menu_move_to_section_submenu_moves_session() {
    let (mut app, a, b) = app_with_sidebar().await;
    let work = app.storage.create_section("Work").expect("section");
    app.sidebar.dirty = true;
    menu_on(&mut app, &a, &MenuAction::MoveTo).await;

    app.handle_key_event(key(KeyCode::Right)).await;
    assert_eq!(
        submenu_labels(&app),
        ["* Project (default)", "Work", "New section..."]
    );
    let _ = draw(&mut app);
    let menu = app.sidebar.menu.as_ref().expect("menu");
    let sub = menu.submenu.as_ref().expect("submenu");
    assert_eq!(
        sub.area.x,
        menu.area.x + menu.area.width,
        "opens to the right"
    );

    app.handle_key_event(key(KeyCode::Down)).await;
    app.handle_key_event(key(KeyCode::Enter)).await;

    assert!(app.sidebar.menu.is_none());
    assert_eq!(app.storage.session_sections().expect("placed")[&a], work.id);
    let rows = draw(&mut app);
    let header = rows
        .iter()
        .position(|r| r.contains("Work v"))
        .expect("section header");
    let a_row = row_of_session(&mut app, &a);
    let b_row = row_of_session(&mut app, &b);
    assert!(
        header < a_row && a_row < b_row,
        "A sits under its section, above its project's sessions: {rows:#?}"
    );
}

#[tokio::test]
async fn test_session_menu_new_section_prefills_and_creates() {
    let (mut app, a, _b) = app_with_sidebar().await;
    menu_on(&mut app, &a, &MenuAction::MoveTo).await;
    app.handle_key_event(key(KeyCode::Right)).await;
    app.handle_key_event(key(KeyCode::End)).await;
    app.handle_key_event(key(KeyCode::Enter)).await;

    assert_eq!(app.session.session_id.as_deref(), Some(a.as_str()));
    assert_eq!(app.session.input, "/session section new ");

    app.execute_slash_command("/session section new Reviews")
        .await;

    let sections = app.storage.list_sections().expect("list");
    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].name, "Reviews");
    assert_eq!(
        app.storage.session_sections().expect("placed")[&a],
        sections[0].id
    );
}

#[tokio::test]
async fn test_session_menu_esc_closes_submenu_only() {
    let (mut app, a, _b) = app_with_sidebar().await;
    menu_on(&mut app, &a, &MenuAction::MoveTo).await;
    app.handle_key_event(key(KeyCode::Enter)).await;
    assert!(
        app.sidebar.menu.as_ref().expect("menu").submenu.is_some(),
        "Enter on a submenu item opens it"
    );

    app.handle_key_event(key(KeyCode::Esc)).await;
    let menu = app.sidebar.menu.as_ref().expect("menu still open");
    assert!(menu.submenu.is_none());

    app.handle_key_event(key(KeyCode::Left)).await;
    assert!(
        app.sidebar.menu.is_none(),
        "Left on the top level closes it"
    );
}

#[tokio::test]
async fn test_session_menu_pinned_wins_over_section() {
    let (mut app, a, _b) = app_with_sidebar().await;
    let work = app.storage.create_section("Work").expect("section");
    app.storage
        .set_session_section(&a, Some(&work.id))
        .expect("move");
    app.storage.set_session_pinned(&a, true).expect("pin");
    app.sidebar.dirty = true;

    let rows = draw(&mut app);
    let pinned = rows
        .iter()
        .position(|r| r.contains("Pinned v"))
        .expect("pinned");
    let a_row = row_of_session(&mut app, &a);
    assert_eq!(a_row, pinned + 1, "listed under Pinned");
    assert!(
        !rows.iter().any(|r| r.contains("Work v")),
        "an empty section shows no header"
    );
}

#[tokio::test]
async fn test_session_menu_open_in_file_manager_records_launch() {
    let (mut app, a, _b) = app_with_sidebar().await;
    app.sidebar.launch_log = Some(Vec::new());
    menu_on(&mut app, &a, &MenuAction::OpenIn).await;
    app.handle_key_event(key(KeyCode::Right)).await;
    let index = submenu_labels(&app)
        .iter()
        .position(|l| l == "File manager")
        .expect("file manager offered");
    app.sidebar
        .menu
        .as_mut()
        .and_then(|m| m.submenu.as_mut())
        .expect("submenu")
        .cursor = index;

    app.handle_key_event(key(KeyCode::Enter)).await;

    let log = app.sidebar.launch_log.as_ref().expect("log");
    assert_eq!(log.len(), 1);
    let dir = std::path::PathBuf::from(
        app.storage
            .get_session(&a)
            .expect("get")
            .expect("row A")
            .directory,
    );
    assert_eq!(log[0].cwd, dir);
    assert_eq!(log[0].args, [dir.display().to_string()]);
    assert!(app.status.starts_with("opened "), "{}", app.status);
}

#[tokio::test]
async fn test_session_menu_section_slash_commands() {
    let (mut app, a, b) = app_with_sidebar().await;

    app.execute_slash_command("/session section new Alpha")
        .await;
    let placed = app.storage.session_sections().expect("placed");
    let alpha = placed[&b].clone();

    app.execute_slash_command("/session section rename Beta")
        .await;
    assert_eq!(app.storage.list_sections().expect("list")[0].name, "Beta");

    app.execute_slash_command("/session section --none").await;
    assert!(app.storage.session_sections().expect("placed").is_empty());

    app.execute_slash_command("/session section beta").await;
    assert_eq!(app.storage.session_sections().expect("placed")[&b], alpha);

    app.execute_slash_command("/session section nope").await;
    assert!(app.status.contains("no section named"), "{}", app.status);

    app.execute_slash_command("/session section delete Beta")
        .await;
    assert_eq!(app.storage.list_sections().expect("list"), Vec::new());
    assert!(app.storage.session_sections().expect("placed").is_empty());
    assert!(
        !app.storage
            .session_sections()
            .expect("placed")
            .contains_key(&a),
        "A was never moved"
    );
}
