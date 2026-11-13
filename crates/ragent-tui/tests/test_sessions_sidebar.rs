//! Session sidebar (spec MULTISESSION, milestone M5; spec `projects`).
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

/// The attention marker of a painted sidebar row: the last column inside the
/// right border.
fn state_marker(row: &str) -> char {
    row.chars().rev().nth(1).expect("row has columns")
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
    let header = format!("{} v", launch_label(&app));

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

    let pinned = row_of(&rows, "Pinned v").expect("pinned header");
    let project = row_of(&rows, &format!("{} v", launch_label(&app))).expect("project");
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
    // New session, Search, (blank), Pinned, (blank), then the project
    // heading: the cursor skips the blank rows, so three steps.
    for _ in 0..3 {
        app.handle_key_event(key(KeyCode::Down, KeyModifiers::NONE))
            .await;
    }
    app.handle_key_event(key(KeyCode::Left, KeyModifiers::NONE))
        .await;

    let rows = sidebar_rows(&mut app);
    assert!(
        row_of(&rows, &format!("{} > (2)", launch_label(&app))).is_some(),
        "{rows:#?}"
    );
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
    let width = usize::from(app.sidebar.area.width);
    let sidebar_row: String = rows[a_row].chars().take(width).collect();
    assert_eq!(state_marker(&sidebar_row), '!', "row was: {sidebar_row}");
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
    app.sidebar.cursor = project_heading(&app);
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
    assert_eq!(state_marker(&rows[a_row]), 'x', "row was: {}", rows[a_row]);

    app.switch_to_session(&a).expect("focus A");
    let rows = sidebar_rows(&mut app);
    let a_row = row_of(&rows, &untitled(&a)).expect("A");
    assert_ne!(state_marker(&rows[a_row]), 'x', "cleared once seen");
}

// -- spec `projects`: project headings, branch markers, request counts ------

/// An existing directory under the git-ignored `target/temp/` tree, unique to
/// `name`, so a fixture project is not painted as `(missing)`.
fn fixture_dir(name: &str) -> std::path::PathBuf {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/sessions_sidebar")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir); // INTENTIONAL: may not exist yet
    std::fs::create_dir_all(&dir).expect("fixture dir");
    dir.canonicalize().expect("canonical")
}

/// Index of the heading row for project `id` in the sidebar's row list.
fn heading_index(app: &mut App, id: &str) -> usize {
    sessions_sidebar::refresh_if_dirty(app);
    sessions_sidebar::build_rows(app)
        .iter()
        .position(
            |row| matches!(row, sessions_sidebar::SidebarRow::Header { key, .. } if key == id),
        )
        .expect("project heading")
}

#[tokio::test]
async fn test_sessions_sidebar_heading_shows_the_project_name() {
    let (mut app, _a, _b) = app_with_sidebar().await;
    app.execute_slash_command("/project rename Storefront")
        .await;

    let rows = sidebar_rows(&mut app);

    assert!(row_of(&rows, "Storefront v").is_some(), "{rows:#?}");
    assert!(
        row_of(&rows, &format!("{} v", launch_label(&app))).is_none(),
        "the directory name is no longer the heading"
    );
}

#[tokio::test]
async fn test_sessions_sidebar_lists_a_project_with_no_sessions_and_its_remote() {
    let (mut app, _a, _b) = app_with_sidebar().await;
    let dir = fixture_dir("empty_project").join("toolbox");
    std::fs::create_dir_all(&dir).expect("dir");
    let project = app
        .storage
        .ensure_project(&dir.display().to_string())
        .expect("project");
    app.storage
        .set_project_remote(
            &project.id,
            Some(&ragent_storage::ProjectRemoteRow {
                kind: "gitlab".to_string(),
                host: "gitlab.com".to_string(),
                path: "group/toolbox".to_string(),
                url: "https://gitlab.com/group/toolbox.git".to_string(),
                remote_name: "origin".to_string(),
                default_branch: None,
            }),
        )
        .expect("remote");
    app.sidebar.dirty = true;

    let rows = sidebar_rows(&mut app);

    let heading = row_of(&rows, "toolbox v").expect("empty project is listed");
    assert!(
        rows[heading].contains("[gitlab]"),
        "row was: {}",
        rows[heading]
    );
    assert!(
        !rows[heading].contains("(missing)"),
        "row was: {}",
        rows[heading]
    );
}

#[tokio::test]
async fn test_sessions_sidebar_row_shows_branch_marker_and_request_count() {
    let (mut app, a, b) = app_with_sidebar().await;
    let project_id = app
        .storage
        .get_session(&a)
        .expect("get")
        .expect("row")
        .project_id;
    let directory = app.cwd_path.display().to_string();
    let with_branch = |id: &str, title: &str, worktree: Option<&str>, count: Option<i64>| {
        app.storage
            .create_session_in_project(
                id,
                &project_id,
                &directory,
                &ragent_storage::SessionCheckout {
                    branch: Some("feature/x".to_string()),
                    worktree: worktree.map(str::to_string),
                    pr_count: count,
                },
            )
            .expect("session");
        app.storage.update_session(id, title).expect("title");
    };
    with_branch("s-counted", "counted", Some("/nonexistent/tree"), Some(16));
    with_branch("s-zero", "zero requests", None, Some(0));
    with_branch("s-unknown", "unknown count", None, None);
    app.sidebar.dirty = true;

    let (rows, terminal) = draw(&mut app, 120, 30);

    let counted = row_of(&rows, "counted").expect("counted row");
    assert!(rows[counted].contains("Y16 "), "row was: {}", rows[counted]);
    let x = rows[counted].find("Y16").expect("marker column") as u16;
    assert_eq!(
        terminal.backend().buffer()[(x, counted as u16)].fg,
        Color::Magenta,
        "a session that owns its worktree is highlighted"
    );
    for title in ["zero requests", "unknown count"] {
        let y = row_of(&rows, title).expect("row");
        assert!(rows[y].contains(" Y "), "row was: {}", rows[y]);
        assert!(
            !rows[y].contains("Y0"),
            "a zero or unknown count shows no number: {}",
            rows[y]
        );
    }
    for id in [&a, &b] {
        let y = row_of(&rows, &untitled(id)).expect("row");
        assert!(
            !rows[y].contains(" Y"),
            "a session with no branch has no marker: {}",
            rows[y]
        );
    }
}

#[tokio::test]
async fn test_sessions_sidebar_state_marker_is_at_the_right_edge() {
    let (mut app, a, _b) = app_with_sidebar().await;
    app.handle_event(Event::PermissionRequested {
        session_id: a.clone(),
        request_id: "p1".to_string(),
        permission: "file:write".to_string(),
        description: "write".to_string(),
        options: vec!["once".to_string()],
    })
    .await;

    let rows = sidebar_rows(&mut app);
    let a_row = row_of(&rows, &untitled(&a)).expect("A");
    let row = &rows[a_row];

    assert_eq!(state_marker(row), '!');
    assert!(
        row.find('!').expect("marker") > row.find("untitled").expect("title"),
        "the marker follows the title: {row}"
    );
    assert!(
        row.starts_with("\u{2502}  untitled"),
        "no marker before the title: {row}"
    );
}

#[tokio::test]
async fn test_sessions_sidebar_narrow_width_drops_the_age_before_the_title() {
    let (mut app, a, _b) = app_with_sidebar().await;
    app.storage
        .update_session(&a, "a session title")
        .expect("title");
    app.sidebar.dirty = true;

    let wide = sidebar_rows(&mut app);
    let y = row_of(&wide, "a session title").expect("row");
    assert!(
        wide[y].contains(" now "),
        "age shown when wide: {}",
        wide[y]
    );

    app.sidebar.width = sessions_sidebar::MIN_WIDTH;
    let narrow = sidebar_rows(&mut app);
    let y = row_of(&narrow, "a session ti").expect("row");
    assert!(
        !narrow[y].contains("now"),
        "the age is dropped first: {}",
        narrow[y]
    );
    assert!(
        narrow[y].contains("a session ti"),
        "the title keeps the room: {}",
        narrow[y]
    );
}

#[tokio::test]
async fn test_sessions_sidebar_search_row_starts_the_filter() {
    let (mut app, a, b) = app_with_sidebar().await;
    app.storage
        .update_session(&a, "alpha parser")
        .expect("title A");
    app.storage
        .update_session(&b, "beta lexer")
        .expect("title B");
    app.sidebar.dirty = true;
    let rows = sidebar_rows(&mut app);
    let search = row_of(&rows, "/ Search").expect("search row");
    assert_eq!(search, row_of(&rows, "+ New session").expect("new") + 1);

    app.sidebar.focused = true;
    app.sidebar.cursor = 1;
    app.handle_key_event(key(KeyCode::Enter, KeyModifiers::NONE))
        .await;
    assert!(
        app.sidebar.filter_editing,
        "Enter on Search starts the filter"
    );
    for c in ['b', 'e', 't'] {
        app.handle_key_event(key(KeyCode::Char(c), KeyModifiers::NONE))
            .await;
    }

    let rows = sidebar_rows(&mut app);
    assert!(row_of(&rows, "/ bet_").is_some(), "{rows:#?}");
    assert!(row_of(&rows, "beta lexer").is_some());
    assert!(row_of(&rows, "alpha parser").is_none());
}

#[tokio::test]
#[allow(clippy::await_holding_lock)] // current-thread runtime; the lock pins the roots
async fn test_sessions_sidebar_n_on_a_heading_opens_a_session_in_that_project() {
    // The permitted roots depend on process-global state (see
    // `support::allow_session_root`).
    let _env = crate::global_env_lock();
    let (mut app, _a, b) = app_with_sidebar().await;
    let dir = fixture_dir("n_on_heading").join("other");
    std::fs::create_dir_all(&dir).expect("dir");
    support::allow_session_root(&app, &dir);
    let project = app
        .storage
        .ensure_project(&dir.display().to_string())
        .expect("project");
    app.sidebar.dirty = true;
    app.sidebar.cursor = heading_index(&mut app, &project.id);
    app.sidebar.focused = true;

    app.handle_key_event(key(KeyCode::Char('n'), KeyModifiers::NONE))
        .await;

    let id = app.session.session_id.clone().expect("focused session");
    assert_ne!(id, b, "a new session is focused");
    let row = app.storage.get_session(&id).expect("get").expect("row");
    assert_eq!(row.project_id, project.id);
    assert_eq!(std::path::PathBuf::from(row.directory), dir);
    assert_eq!(app.cwd_path, dir, "the TUI works in the project");
    assert!(!app.sidebar.focused, "focus returns to the input");
}

#[tokio::test]
async fn test_sessions_sidebar_b_on_a_heading_prefills_the_branch_command() {
    let (mut app, a, _b) = app_with_sidebar().await;
    let project_id = app
        .storage
        .get_session(&a)
        .expect("get")
        .expect("row")
        .project_id;
    app.sidebar.cursor = heading_index(&mut app, &project_id);
    app.sidebar.focused = true;

    app.handle_key_event(key(KeyCode::Char('b'), KeyModifiers::NONE))
        .await;

    assert_eq!(
        app.session.input,
        format!("/session new --project {project_id} --branch ")
    );
    assert!(!app.sidebar.focused);
}

#[tokio::test]
async fn test_sessions_sidebar_project_update_reloads_the_rows() {
    let (mut app, a, _b) = app_with_sidebar().await;
    let project_id = app
        .storage
        .get_session(&a)
        .expect("get")
        .expect("row")
        .project_id;
    let _ = sidebar_rows(&mut app);
    assert!(!app.sidebar.dirty);
    // What the request-count service does when a stored count changes.
    app.storage
        .rename_project(&project_id, "Fresh Name")
        .expect("rename");

    app.handle_event(Event::ProjectUpdated { project_id }).await;

    assert!(app.sidebar.dirty);
    assert!(row_of(&sidebar_rows(&mut app), "Fresh Name v").is_some());
}

#[tokio::test]
#[allow(clippy::await_holding_lock)] // current-thread runtime; the lock pins the roots
async fn test_sessions_sidebar_n_on_a_heading_outside_permitted_roots_is_refused() {
    let _env = crate::global_env_lock();
    let (mut app, _a, b) = app_with_sidebar().await;
    let dir = fixture_dir("n_outside_roots").join("other");
    std::fs::create_dir_all(&dir).expect("dir");
    // Pin the permitted roots: the user-global config may allow this
    // checkout's own tree, which holds the fixture.
    let elsewhere = fixture_dir("n_outside_roots_allowed");
    support::allow_session_root(&app, &elsewhere);
    let project = app
        .storage
        .ensure_project(&dir.display().to_string())
        .expect("project");
    app.sidebar.dirty = true;
    app.sidebar.cursor = heading_index(&mut app, &project.id);
    app.sidebar.focused = true;

    app.handle_key_event(key(KeyCode::Char('n'), KeyModifiers::NONE))
        .await;

    assert!(app.status.contains("outside"), "status: {}", app.status);
    assert_eq!(app.session.session_id.as_deref(), Some(b.as_str()));
    assert!(
        app.storage
            .list_project_sessions(&project.id, false)
            .expect("list")
            .is_empty(),
        "no session was created in the project"
    );
}

/// Index of the first project heading (the first heading after `Pinned`).
fn project_heading(app: &App) -> usize {
    sessions_sidebar::build_rows(app)
        .iter()
        .position(|row| {
            matches!(row, sessions_sidebar::SidebarRow::Header { label, .. } if label != "Pinned")
        })
        .expect("a project heading")
}

#[tokio::test]
async fn test_sessions_sidebar_blank_line_above_pinned_and_projects() {
    let (mut app, _a, _b) = app_with_sidebar().await;
    let _ = sidebar_rows(&mut app);
    let rows = sessions_sidebar::build_rows(&app);
    use sessions_sidebar::SidebarRow;
    assert!(matches!(rows[0], SidebarRow::NewSession));
    assert!(matches!(rows[1], SidebarRow::Search));
    assert!(matches!(rows[2], SidebarRow::Spacer));
    assert!(
        matches!(&rows[3], SidebarRow::Header { label, count: 0, .. } if label == "Pinned"),
        "Pinned is listed with nothing pinned: {rows:#?}"
    );
    assert!(matches!(rows[4], SidebarRow::Spacer));
    let project = project_heading(&app);
    assert!(matches!(rows[project - 1], SidebarRow::Spacer));

    // The cursor never rests on a blank row, moving either way.
    app.sidebar.focused = true;
    app.sidebar.cursor = 1;
    app.handle_key_event(key(KeyCode::Down, KeyModifiers::NONE))
        .await;
    assert_eq!(app.sidebar.cursor, 3, "Down skips the blank row");
    app.handle_key_event(key(KeyCode::Up, KeyModifiers::NONE))
        .await;
    assert_eq!(app.sidebar.cursor, 1, "Up skips it too");
}

#[tokio::test]
async fn test_sessions_sidebar_drag_session_onto_pinned_pins_it() {
    let (mut app, a, _b) = app_with_sidebar().await;
    let rows = sidebar_rows(&mut app);
    let area = app.sidebar.area;
    let screen_row = |index: usize| area.y + 1 + u16::try_from(index).expect("row");
    let session_index = sessions_sidebar::build_rows(&app)
        .iter()
        .position(|r| matches!(r, sessions_sidebar::SidebarRow::Session { id, .. } if *id == a))
        .expect("session row");
    let pinned_index = 3;
    let mouse = |kind, row| MouseEvent {
        kind,
        column: area.x + 2,
        row,
        modifiers: KeyModifiers::NONE,
    };
    assert!(rows.len() > session_index, "{rows:#?}");

    app.handle_mouse_event(mouse(
        MouseEventKind::Down(MouseButton::Left),
        screen_row(session_index),
    ));
    app.handle_mouse_event(mouse(
        MouseEventKind::Drag(MouseButton::Left),
        screen_row(pinned_index),
    ));
    app.handle_mouse_event(mouse(
        MouseEventKind::Up(MouseButton::Left),
        screen_row(pinned_index),
    ));

    assert!(
        app.storage.pinned_session_ids().expect("pins").contains(&a),
        "dropped on Pinned, so pinned"
    );
    assert!(app.sidebar.drag_session.is_none());

    // A drop anywhere else does nothing.
    let b_index = sessions_sidebar::build_rows(&app)
        .iter()
        .position(|r| matches!(r, sessions_sidebar::SidebarRow::Session { id, .. } if *id != a))
        .expect("other session");
    app.handle_mouse_event(mouse(
        MouseEventKind::Down(MouseButton::Left),
        screen_row(b_index),
    ));
    app.handle_mouse_event(mouse(
        MouseEventKind::Up(MouseButton::Left),
        screen_row(project_heading(&app)),
    ));
    assert_eq!(app.storage.pinned_session_ids().expect("pins").len(), 1);
}
