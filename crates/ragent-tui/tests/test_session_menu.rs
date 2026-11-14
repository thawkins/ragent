//! Session context menu (spec SESSIONMENU, milestone M1).
//!
//! Renders the full layout into a `TestBackend`, opens the menu on a sidebar
//! row with keys or the mouse, and checks the painted menu and the actions it
//! dispatches.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ragent_tui::App;
use ragent_tui::session_menu::{self, MenuAction, MenuTarget};
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
    assert_eq!(menu.target, MenuTarget::Session(a.clone()));
    let actions: Vec<MenuAction> = menu.items.iter().map(|i| i.action.clone()).collect();
    // `Terminal` is listed only when this machine can start one, so the expected
    // list names it conditionally: every other item is unconditional.
    let mut expected = vec![MenuAction::OpenIn];
    let has_terminal = actions.contains(&MenuAction::Terminal);
    expected.extend([
        MenuAction::TogglePin,
        MenuAction::ToggleUnread,
        MenuAction::Rename,
        MenuAction::Fork,
        MenuAction::MoveTo,
        MenuAction::Archive,
        MenuAction::Delete,
    ]);
    // `Terminal` closes the menu so the items above keep their positions.
    if has_terminal {
        expected.push(MenuAction::Terminal);
    }
    assert_eq!(actions, expected);
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
    // The menu's top border is `lines[0]`, so an item's screen line is its
    // position plus the group rules drawn above it, plus one for that border.
    // The optional `Terminal` entry shifts every line after it.
    let items = &menu.items;
    let rule_above = |index: usize| index > 0 && items[index - 1].group != items[index].group;
    let rules_before = |index: usize| (1..=index).filter(|&i| rule_above(i)).count();
    let line_of = |action: &MenuAction| {
        let index = items
            .iter()
            .position(|i| &i.action == action)
            .expect("item present");
        1 + index + rules_before(index)
    };

    // An item's line is `│ label ... hint │`: the label sits just after the
    // border and the accelerator (or `>` for a submenu) just before it.
    for (action, label) in [
        (MenuAction::OpenIn, "Open in"),
        (MenuAction::TogglePin, "Pin"),
        (MenuAction::ToggleUnread, "Mark as unread"),
        (MenuAction::Rename, "Rename"),
        (MenuAction::Fork, "Fork"),
        (MenuAction::MoveTo, "Move to section"),
        (MenuAction::Archive, "Archive"),
        (MenuAction::Delete, "Delete"),
        (MenuAction::Terminal, "Terminal"),
    ] {
        let line = &lines[line_of(&action)];
        let body = line.trim_start_matches('│').trim_end_matches('│');
        let hint = body.trim_end().chars().last().expect("a hint column");
        assert!(
            body.contains(&format!(" {label} ")),
            "{label} on line {}, body {body:?}: {lines:#?}",
            line_of(&action)
        );
        assert!(
            hint == '>' || hint.is_ascii_uppercase(),
            "{label} has an accelerator or submenu marker: {line}"
        );
    }
    // A rule sits between groups: one drawn line after an item whose successor
    // starts a later group.
    let rules: Vec<usize> = (1..items.len())
        .filter(|&i| rule_above(i))
        .map(|i| line_of(&items[i].action) - 1)
        .collect();
    assert_eq!(
        rules.len(),
        usize::from(has_terminal) + 3,
        "three group rules, one more above Terminal: {lines:#?}"
    );
    assert!(
        rules.iter().all(|&l| lines[l].contains("-----")),
        "rule between groups: {lines:#?}"
    );
    let delete = items
        .iter()
        .position(|i| i.action == MenuAction::Delete)
        .expect("a delete item");
    assert!(items[delete].danger, "Delete is drawn as dangerous");
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
    assert_eq!(menu.target, MenuTarget::Session(a.clone()));
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
    let menu = app.sidebar.menu.as_ref().expect("menu");
    let pin = menu
        .items
        .iter()
        .find(|i| i.action == MenuAction::TogglePin)
        .expect("a pin item");
    assert_eq!(pin.label, "Unpin");
}

#[tokio::test]
async fn test_session_menu_enter_on_archive_archives_background_session() {
    let (mut app, a, b) = app_with_sidebar().await;
    put_cursor_on(&mut app, &a);

    app.handle_key_event(key(KeyCode::Char('m'))).await;
    let archive = app
        .sidebar
        .menu
        .as_ref()
        .expect("menu")
        .items
        .iter()
        .position(|i| i.action == MenuAction::Archive)
        .expect("an archive item");
    // Walk to `Archive` by name, because the optional trailing `Terminal`
    // entry changes what `End` lands on.
    for _ in 0..archive {
        app.handle_key_event(key(KeyCode::Down)).await;
    }
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
    let rename = app
        .sidebar
        .menu
        .as_ref()
        .expect("menu")
        .items
        .iter()
        .position(|i| i.action == MenuAction::Rename)
        .expect("a rename item");

    // The menu's border and the group rules above an item are drawn lines too,
    // so the row is derived rather than assumed: the item's position plus one
    // for the top border, plus one per rule above it.
    let rules = app.sidebar.menu.as_ref().expect("menu").items[..rename]
        .windows(2)
        .filter(|w| w[0].group != w[1].group)
        .count();
    app.handle_mouse_event(mouse(
        MouseEventKind::Down(MouseButton::Left),
        area.x + 2,
        area.y + 1 + rename as u16 + rules as u16,
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

#[tokio::test]
async fn test_session_menu_open_request_first_when_bound() {
    let (mut app, a, _b) = app_with_sidebar().await;
    app.storage
        .set_session_request(&ragent_storage::SessionRequest {
            session_id: a.clone(),
            forge: "github".to_string(),
            host: "github.com".to_string(),
            repo: "acme/app".to_string(),
            number: 12,
            title: "Add menu".to_string(),
            web_url: "https://github.com/acme/app/pull/12".to_string(),
            head_branch: "menu".to_string(),
            state: "open".to_string(),
        })
        .expect("bind");
    app.sidebar.launch_log = Some(Vec::new());
    put_cursor_on(&mut app, &a);

    app.handle_key_event(key(KeyCode::Char('m'))).await;
    let menu = app.sidebar.menu.clone().expect("menu open");
    assert_eq!(menu.items[0].action, MenuAction::OpenRequest);
    assert_eq!(menu.items[0].label, "Open PR");
    assert_eq!(menu.items[0].accel, Some('g'));

    app.handle_key_event(key(KeyCode::Char('g'))).await;

    assert!(app.sidebar.menu.is_none(), "the menu closes");
    let log = app.sidebar.launch_log.as_ref().expect("log");
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].args, ["https://github.com/acme/app/pull/12"]);
}

#[tokio::test]
async fn test_session_menu_no_open_request_when_unbound() {
    let (mut app, a, _b) = app_with_sidebar().await;
    put_cursor_on(&mut app, &a);
    app.handle_key_event(key(KeyCode::Char('m'))).await;
    let menu = app.sidebar.menu.clone().expect("menu open");
    assert!(
        menu.items
            .iter()
            .all(|i| i.action != MenuAction::OpenRequest)
    );
}

#[tokio::test]
async fn test_session_menu_autofix_toggle_and_sidebar_glyphs() {
    let (mut app, a, _b) = app_with_sidebar().await;
    let request = ragent_storage::SessionRequest {
        session_id: a.clone(),
        forge: "gitlab".to_string(),
        host: "gitlab.example.com".to_string(),
        repo: "group/app".to_string(),
        number: 4,
        title: "Fix".to_string(),
        web_url: "https://gitlab.example.com/group/app/-/merge_requests/4".to_string(),
        head_branch: "fix".to_string(),
        state: "open".to_string(),
    };
    app.storage.set_session_request(&request).expect("bind");
    app.sidebar.dirty = true;
    let row = row_of_session(&mut app, &a);
    let width = usize::from(app.sidebar.area.width);
    let text: String = draw(&mut app)[row].chars().take(width).collect();
    assert!(text.contains(" #1 "), "bound glyph with count: {text}");

    put_cursor_on(&mut app, &a);
    app.handle_key_event(key(KeyCode::Char('m'))).await;
    let menu = app.sidebar.menu.clone().expect("menu open");
    assert_eq!(menu.items[0].label, "Open MR");
    assert_eq!(menu.items[1].action, MenuAction::ToggleAutofix);
    assert_eq!(menu.items[1].label, "Auto-fix: off");
    assert_eq!(menu.items[1].accel, Some('x'));

    app.handle_key_event(key(KeyCode::Char('x'))).await;
    assert!(app.sidebar.menu.is_none());
    assert!(
        app.storage
            .session_autofix(&a)
            .expect("read")
            .expect("bound")
            .enabled
    );
    assert!(
        app.status.contains("Auto-fix on for MR !4"),
        "{}",
        app.status
    );

    put_cursor_on(&mut app, &a);
    app.handle_key_event(key(KeyCode::Char('m'))).await;
    assert_eq!(
        app.sidebar.menu.as_ref().expect("menu").items[1].label,
        "Auto-fix: on"
    );
    app.handle_key_event(key(KeyCode::Esc)).await;

    // Paused waiting on the user.
    app.storage.set_autofix_paused(&a, true).expect("pause");
    app.sidebar.dirty = true;
    let text: String = draw(&mut app)[row].chars().take(width).collect();
    assert!(text.contains(" #!1 "), "paused glyph: {text}");
}

#[tokio::test]
async fn test_session_menu_delete_refuses_while_a_sub_agent_runs() {
    let (mut app, a, _b) = app_with_sidebar().await;
    let child = format!("{a}-child");
    app.storage
        .create_session_with(
            &child,
            "/nonexistent/menu",
            Some(&a),
            ragent_storage::SessionKind::Subagent,
        )
        .expect("child");
    app.active_tasks.push(ragent_agent::task::TaskEntry {
        id: "t1".to_string(),
        parent_session_id: a.clone(),
        child_session_id: child,
        agent_name: "explore".to_string(),
        task_prompt: "x".to_string(),
        background: true,
        detached: false,
        status: ragent_agent::task::TaskStatus::Running,
        result: None,
        error: None,
        created_at: chrono::Utc::now(),
        completed_at: None,
        reported: false,
        waiter_count: 0,
        output_file: None,
        report_status: ragent_agent::task::ReportStatus::default(),
        time_bound_secs: None,
    });

    app.execute_slash_command(&format!("/session delete {a}"))
        .await;

    assert!(!app.row_action_confirm_open(), "no dialog");
    assert!(app.status.contains("still running"), "{}", app.status);
    assert!(app.storage.get_session(&a).expect("get").is_some());
}

#[tokio::test]
async fn test_session_menu_scrolls_to_keep_cursor_visible() {
    let (mut app, a, _b) = app_with_sidebar().await;
    // More sections than the screen has rows.
    for n in 0..40 {
        app.storage
            .create_section(&format!("Section {n:02}"))
            .expect("section");
    }
    menu_on(&mut app, &a, &MenuAction::MoveTo).await;
    app.handle_key_event(key(KeyCode::Enter)).await;
    draw(&mut app);
    app.handle_key_event(key(KeyCode::End)).await;
    let rows = draw(&mut app);
    assert!(
        rows.iter().any(|r| r.contains("New section...")),
        "the last item is scrolled into view"
    );
    let sub = app
        .sidebar
        .menu
        .as_ref()
        .and_then(|m| m.submenu.as_ref())
        .expect("submenu");
    assert!(sub.scroll > 0, "the submenu scrolled");
    let last = sub.items.len() - 1;
    assert_eq!(sub.cursor, last);
    // A click on the painted last line picks the last item.
    let row = rows
        .iter()
        .position(|r| r.contains("New section..."))
        .expect("row");
    let column = sub.area.x + 2;
    app.handle_mouse_event(mouse(
        MouseEventKind::Down(MouseButton::Left),
        column,
        u16::try_from(row).expect("row"),
    ));
    assert!(
        app.session.input.starts_with("/session section new"),
        "the clicked item ran: {}",
        app.session.input
    );
}

/// A GitHub request bound to session `id`.
fn github_request(id: &str, number: u64) -> ragent_storage::SessionRequest {
    ragent_storage::SessionRequest {
        session_id: id.to_string(),
        forge: "github".to_string(),
        host: "github.com".to_string(),
        repo: "acme/app".to_string(),
        number,
        title: format!("Change {number}"),
        web_url: format!("https://github.com/acme/app/pull/{number}"),
        head_branch: format!("b{number}"),
        state: "open".to_string(),
    }
}

#[tokio::test]
async fn test_session_menu_requests_submenu_lists_history_and_opens_url() {
    let (mut app, a, _b) = app_with_sidebar().await;
    app.storage
        .set_session_request(&github_request(&a, 12))
        .expect("bind 12");
    std::thread::sleep(std::time::Duration::from_millis(5));
    app.storage
        .set_session_request(&github_request(&a, 15))
        .expect("rebind 15");
    app.sidebar.dirty = true;
    app.sidebar.launch_log = Some(Vec::new());
    let row = row_of_session(&mut app, &a);
    let width = usize::from(app.sidebar.area.width);
    let text: String = draw(&mut app)[row].chars().take(width).collect();
    assert!(text.contains(" #2 "), "two requests so far: {text}");

    menu_on(&mut app, &a, &MenuAction::Requests).await;
    let label = app.sidebar.menu.as_ref().expect("menu").items
        [app.sidebar.menu.as_ref().expect("menu").cursor]
        .label
        .clone();
    assert_eq!(label, "PR/MRs (2)");
    app.handle_key_event(key(KeyCode::Enter)).await;
    assert_eq!(
        submenu_labels(&app),
        ["PR #15 [open] Change 15", "PR #12 [open] Change 12"],
        "newest first"
    );

    // Clicking an entry opens its URL.
    let _ = draw(&mut app);
    let sub = app
        .sidebar
        .menu
        .as_ref()
        .and_then(|m| m.submenu.as_ref())
        .expect("submenu")
        .area;
    app.handle_mouse_event(mouse(
        MouseEventKind::Down(MouseButton::Left),
        sub.x + 2,
        sub.y + 2,
    ));

    assert!(app.sidebar.menu.is_none(), "the menu closes");
    let log = app.sidebar.launch_log.as_ref().expect("log");
    assert_eq!(log.len(), 1, "{}", app.status);
    assert_eq!(log[0].args, ["https://github.com/acme/app/pull/12"]);
}

#[tokio::test]
async fn test_session_menu_requests_item_absent_without_history() {
    let (mut app, a, _b) = app_with_sidebar().await;
    put_cursor_on(&mut app, &a);
    app.handle_key_event(key(KeyCode::Char('m'))).await;
    let menu = app.sidebar.menu.as_ref().expect("menu open");
    assert!(menu.items.iter().all(|i| i.action != MenuAction::Requests));
}

#[tokio::test]
async fn test_session_menu_unbound_session_keeps_grey_request_count() {
    let (mut app, a, _b) = app_with_sidebar().await;
    app.storage
        .set_session_request(&github_request(&a, 3))
        .expect("bind");
    app.storage.clear_session_request(&a).expect("unbind");
    app.sidebar.dirty = true;
    let row = row_of_session(&mut app, &a);
    let width = usize::from(app.sidebar.area.width);
    let text: String = draw(&mut app)[row].chars().take(width).collect();
    assert!(text.contains(" #1 "), "history survives unbinding: {text}");
}

#[tokio::test]
async fn test_session_menu_terminal_opens_a_terminal_at_the_session_directory() {
    let (mut app, a, _b) = app_with_sidebar().await;
    // Session `A` stays open in the background, and an open view's directory
    // outranks the stored one, so the fixture has to move both.
    let worktree = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/temp/worktree");
    std::fs::create_dir_all(&worktree).expect("worktree directory");
    set_session_directory(&mut app, &a, &worktree);
    app.sidebar.launch_log = Some(Vec::new());

    if !has_terminal() {
        return;
    }

    menu_on(&mut app, &a, &MenuAction::Terminal).await;
    app.handle_key_event(key(KeyCode::Enter)).await;

    let log = app.sidebar.launch_log.as_ref().expect("log");
    assert_eq!(log.len(), 1, "one terminal launched");
    assert_eq!(
        log[0].cwd, worktree,
        "the terminal starts at the session's worktree"
    );
    assert!(
        app.status.contains("opened") && app.status.contains("terminal"),
        "status reports the launch: {}",
        app.status
    );
    assert!(!app.notice_open, "a present worktree raises no dialog");
}

#[tokio::test]
async fn test_session_menu_terminal_reports_a_missing_worktree_in_a_dialog() {
    let (mut app, a, _b) = app_with_sidebar().await;
    // A deleted worktree is a session directory that is no longer a directory.
    let missing = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/temp/gone-worktree-does-not-exist");
    assert!(!missing.exists(), "the fixture path must not exist");
    set_session_directory(&mut app, &a, &missing);
    app.sidebar.launch_log = Some(Vec::new());

    if !has_terminal() {
        return;
    }

    menu_on(&mut app, &a, &MenuAction::Terminal).await;
    app.handle_key_event(key(KeyCode::Enter)).await;

    // The menu focuses its target before acting, so the active view holds it.
    assert!(app.notice_open, "the missing worktree raises the dialog");
    assert_eq!(app.notice_title, " Worktree missing ");
    assert!(
        app.notice_message.contains(&missing.display().to_string()),
        "the dialog names the path: {}",
        app.notice_message
    );
    assert!(
        app.sidebar.launch_log.as_ref().is_some_and(Vec::is_empty),
        "no terminal is started for a missing worktree"
    );

    // The dialog paints and names its single `OK` button.
    let painted = draw(&mut app).join("\n");
    assert!(painted.contains("OK"), "an OK button is drawn: {painted}");
    assert!(
        painted.contains("Worktree missing"),
        "the title is drawn: {painted}"
    );

    // `Enter` closes it and nothing else happens.
    app.handle_key_event(key(KeyCode::Enter)).await;
    assert!(!app.notice_open, "Enter closes the notice");
}

/// Whether this machine can start a terminal, so a build environment without
/// one skips the terminal tests rather than failing them.
fn has_terminal() -> bool {
    ragent_tui::open_in::launch_spec(
        ragent_tui::open_in::OpenTarget::Terminal,
        &std::path::PathBuf::from("/"),
        &ragent_tui::open_in::LaunchEnv::current(),
    )
    .is_some()
}

/// Points session `id` at `directory`: its open view when it has one, and its
/// stored row, which a session without a view falls back to.
fn set_session_directory(app: &mut App, id: &str, directory: &std::path::Path) {
    if app.session.session_id.as_deref() == Some(id) {
        app.session.directory = directory.to_path_buf();
    }
    if let Some(index) = app.background.position(id)
        && let Some(mut view) = app.background.remove(index)
    {
        view.directory = directory.to_path_buf();
        app.background.insert_front(view);
    }
    let conn = app.storage.conn_lock_for_test().expect("connection");
    conn.execute(
        "UPDATE sessions SET directory = ?1 WHERE id = ?2",
        [directory.display().to_string(), id.to_string()],
    )
    .expect("store the session directory");
}
