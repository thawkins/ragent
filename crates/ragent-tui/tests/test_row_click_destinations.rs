//! Tests for row-click destinations (spec `agentui` T-008).
//!
//! Covers the Agents primary-row click (FR-033), the sub-agent row click with
//! its output-view label and selected index (FR-034), the Teammate click that
//! focuses the member and reports `team: focused -> <name>` (FR-038), the lead
//! click that clears focus and reports `focus: lead (you)` (FR-039), the
//! redraw flag every row-click destination sets (NFR-005) and the retention of
//! selection and focus across a close/reopen (FR-065).
//!
//! Every case drives the real mouse router ([`App::handle_mouse_event`]) with
//! the row map the renderer records, so the assertions exercise the same path
//! a real click takes (NFR-001). All state is synthetic; nothing is written
//! into the repository (NFR-012).

mod support;

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;

use ragent_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use ragent_agent::team::{MemberStatus, TeamConfig, TeamMember};
use ragent_tui::app::{OutputViewState, OutputViewTarget};
use ragent_tui::{App, utils};
use support::make_app;

/// The lead session id every Agents case uses.
const LEAD_SESSION: &str = "lead-session-0001";

/// A left-button press at `(col, row)`.
const fn click(col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: col,
        row,
        modifiers: KeyModifiers::empty(),
    }
}

/// A sub-agent [`TaskEntry`] with `agent`'s label and a session id derived from
/// the task id.
fn task(id: &str, parent_session: &str, agent: &str, status: TaskStatus) -> TaskEntry {
    TaskEntry {
        id: id.to_string(),
        parent_session_id: parent_session.to_string(),
        child_session_id: format!("{id}-session"),
        agent_name: agent.to_string(),
        task_prompt: "prompt".to_string(),
        background: true,
        detached: false,
        status,
        result: None,
        error: None,
        created_at: chrono::Utc::now(),
        completed_at: None,
        reported: false,
        waiter_count: 0,
        output_file: None,
        report_status: ReportStatus::default(),
    }
}

/// A teammate in `Working` state with a spawned session.
fn member(agent_id: &str, name: &str) -> TeamMember {
    let mut member = TeamMember::new(name, agent_id, "general");
    member.status = MemberStatus::Working;
    member.session_id = Some(format!("{agent_id}-session"));
    member
}

/// Render `app` with the given window open and return nothing; the renderer
/// records the row map the click cases resolve against.
fn render(app: &mut App) {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("render frame");
}

/// The centre of a row `offset` lines below the top of `area`, at a safe
/// left-side column (away from the `[stop]` / `[x]` controls).
const fn row_point(area: Rect, offset: u16) -> (u16, u16) {
    (area.x.saturating_add(5), area.y.saturating_add(offset))
}

/// The session id an open output view targets, and its label when it is a
/// session view.
fn output_session(view: &OutputViewState) -> (&str, Option<&str>) {
    match &view.target {
        OutputViewTarget::Session { session_id, label } => (session_id, Some(label.as_str())),
        OutputViewTarget::TeamMember {
            session_id,
            agent_id,
            ..
        } => {
            let _agent_id = agent_id;
            (session_id.as_deref().unwrap_or(""), None)
        }
    }
}

// ---------------------------------------------------------------------------
// FR-033: the primary row
// ---------------------------------------------------------------------------

#[test]
fn primary_row_click_opens_the_primary_output_view() {
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.agent_name = "general".to_string();
    app.active_tasks
        .push(task("task-1", LEAD_SESSION, "explore", TaskStatus::Running));
    app.show_agents_window = true;
    render(&mut app);

    let (col, row) = row_point(app.active_agents_area, 1);
    app.handle_mouse_event(click(col, row));

    assert_eq!(
        app.selected_agent_index,
        Some(0),
        "the primary click must record selected index 0 (FR-033)"
    );
    let view = app.output_view.as_ref().expect("output view opens");
    assert!(
        matches!(view.target, OutputViewTarget::Session { .. }),
        "the primary click opens a Session output view (FR-033)"
    );
    let (session, label) = output_session(view);
    assert_eq!(
        session, LEAD_SESSION,
        "the view targets the primary session"
    );
    assert_eq!(
        label,
        Some("primary"),
        "the view is labelled primary (FR-033)"
    );
    assert!(
        app.needs_redraw,
        "opening the view requests a repaint (NFR-005)"
    );
}

#[test]
fn primary_row_click_without_a_session_opens_nothing() {
    let mut app = make_app();
    app.session_id = None;
    app.active_tasks
        .push(task("task-1", "", "explore", TaskStatus::Running));
    app.show_agents_window = true;
    render(&mut app);

    let (col, row) = row_point(app.active_agents_area, 1);
    app.handle_mouse_event(click(col, row));
    assert!(
        app.output_view.is_none(),
        "without a session there is nothing to open (FR-033)"
    );
    assert_eq!(app.selected_agent_index, None, "no selection is recorded");
}

// ---------------------------------------------------------------------------
// FR-034: sub-agent rows, including a tree-order mapping case
// ---------------------------------------------------------------------------

#[test]
fn sub_agent_row_click_opens_the_task_painted_on_that_row() {
    // Insertion order differs from tree paint order on purpose: `task-9` is a
    // child of `task-1`, so the painted tree lists it before `task-2` even
    // though `task-2` was appended first (FR-034, FR-069).
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.agent_name = "general".to_string();
    app.active_tasks = vec![
        task("task-1", LEAD_SESSION, "explore", TaskStatus::Running),
        task(
            "task-9",
            "task-1-session",
            "rust-coder",
            TaskStatus::Running,
        ),
        task("task-2", LEAD_SESSION, "plan", TaskStatus::Running),
    ];
    app.show_agents_window = true;
    render(&mut app);

    // Painted rows: 0 header, 1 primary, 2 task-1, 3 task-9, 4 task-2.
    let (col, row) = row_point(app.active_agents_area, 3);
    app.handle_mouse_event(click(col, row));

    let view = app.output_view.as_ref().expect("output view opens");
    let (session, _label) = output_session(view);
    assert_eq!(
        session, "task-9-session",
        "the click must open the nested task the user sees on that row (FR-034)"
    );
    // The Flat index of `task-9` is 1, so the recorded selection is 2 and the
    // keyboard path (`active_tasks.get(selected - 1)`) hits the same task.
    assert_eq!(
        app.selected_agent_index,
        Some(2),
        "the selection indexes the flat task list (FR-034)"
    );
    assert_eq!(
        app.active_tasks
            .get(app.selected_agent_index.unwrap() - 1)
            .map(|t| t.id.as_str()),
        Some("task-9"),
        "Space/Shift+K on the recorded index must reach the clicked task"
    );
}

#[test]
fn sub_agent_row_click_label_carries_agent_and_short_task_id() {
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.agent_name = "general".to_string();
    app.active_tasks.push(task(
        "task-42",
        LEAD_SESSION,
        "explore",
        TaskStatus::Running,
    ));
    app.show_agents_window = true;
    render(&mut app);

    let (col, row) = row_point(app.active_agents_area, 2);
    app.handle_mouse_event(click(col, row));

    let view = app.output_view.as_ref().expect("output view opens");
    if let OutputViewTarget::Session { label, .. } = &view.target {
        assert_eq!(
            label,
            &format!("explore [{}]", utils::short_id("task-42")),
            "the label carries the agent name and short task id (FR-034)"
        );
    } else {
        panic!("a sub-agent click opens a Session output view (FR-034)");
    }
}

#[test]
fn stale_row_click_after_a_task_is_removed_opens_nothing() {
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.agent_name = "general".to_string();
    app.active_tasks
        .push(task("task-1", LEAD_SESSION, "explore", TaskStatus::Running));
    app.show_agents_window = true;
    render(&mut app);

    // The task list changes between the render that recorded the row map and
    // the click (a stale hit-rects case, FR-069).
    app.active_tasks.clear();
    let (col, row) = row_point(app.active_agents_area, 2);
    app.status = "sentinel".to_string();
    app.handle_mouse_event(click(col, row));

    assert!(
        app.output_view.is_none(),
        "a stale row click must open nothing (FR-069)"
    );
    assert_eq!(app.selected_agent_index, None, "no selection is recorded");
    assert_eq!(
        app.status, "sentinel",
        "a stale row click leaves the status untouched (FR-069)"
    );
}

#[test]
fn header_row_click_is_consumed_without_selecting() {
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.active_tasks
        .push(task("task-1", LEAD_SESSION, "explore", TaskStatus::Running));
    app.show_agents_window = true;
    render(&mut app);

    let (col, row) = row_point(app.active_agents_area, 0);
    app.status = "sentinel".to_string();
    app.handle_mouse_event(click(col, row));

    assert!(
        app.output_view.is_none(),
        "the header row opens nothing (FR-070)"
    );
    assert_eq!(app.selected_agent_index, None);
}

// ---------------------------------------------------------------------------
// FR-038 / FR-039: the Teams window focus paths
// ---------------------------------------------------------------------------

fn app_with_team() -> App {
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.agent_name = "general".to_string();
    app.active_team = Some(TeamConfig::new("alpha", LEAD_SESSION));
    app.team_members = vec![member("tm-001", "reviewer"), member("tm-002", "tester")];
    app.show_teams_window = true;
    app.needs_redraw = false;
    app
}

#[test]
fn teammate_row_click_focuses_the_teammate_and_reports_it() {
    let mut app = app_with_team();
    render(&mut app);

    // Painted rows: 0 header, 1 lead, 2 tm-001.
    let (col, row) = row_point(app.teams_rows_area, 2);
    app.handle_mouse_event(click(col, row));

    assert_eq!(
        app.focused_teammate.as_deref(),
        Some("tm-001"),
        "the click focuses the teammate (FR-038)"
    );
    assert_eq!(
        app.status, "team: focused -> reviewer",
        "the status line reports the focus transition (FR-038, FR-064)"
    );
    let view = app
        .output_view
        .as_ref()
        .expect("focused teammate's output view");
    assert!(
        matches!(view.target, OutputViewTarget::TeamMember { .. }),
        "the click opens the teammate's output view (FR-038)"
    );
    assert!(app.needs_redraw, "a focus change repaints (NFR-005)");
}

#[test]
fn lead_row_click_clears_focus_and_reports_it() {
    let mut app = app_with_team();
    app.focused_teammate = Some("tm-001".to_string());

    render(&mut app);
    let (col, row) = row_point(app.teams_rows_area, 1);
    app.needs_redraw = false;
    app.handle_mouse_event(click(col, row));

    assert!(
        app.focused_teammate.is_none(),
        "the lead click clears the focused teammate (FR-039)"
    );
    assert_eq!(
        app.status, "focus: lead (you)",
        "the status line reports the cleared focus (FR-039, FR-064)"
    );
    assert!(app.needs_redraw, "a focus change repaints (NFR-005)");
}

#[test]
fn stale_teammate_click_is_ignored_without_mutating_state() {
    let mut app = app_with_team();
    render(&mut app);

    // The member list changes after the render that recorded the focus marker:
    // the click is stale (FR-069).
    app.team_members.clear();
    let (col, row) = row_point(app.teams_rows_area, 2);
    app.status = "sentinel".to_string();
    app.handle_mouse_event(click(col, row));

    assert_eq!(
        app.focused_teammate, None,
        "a stale click must not focus anything (FR-069)"
    );
    assert_eq!(
        app.status, "sentinel",
        "a stale click leaves the status untouched (FR-069)"
    );
}

// ---------------------------------------------------------------------------
// FR-065: selection and focus survive a close/reopen
// ---------------------------------------------------------------------------

#[test]
fn agents_selection_survives_a_close_and_reopen() {
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.agent_name = "general".to_string();
    app.active_tasks
        .push(task("task-1", LEAD_SESSION, "explore", TaskStatus::Running));
    app.show_agents_window = true;
    render(&mut app);

    // Select the sub-agent row by clicking it (FR-034).
    let (col, row) = row_point(app.active_agents_area, 2);
    app.handle_mouse_event(click(col, row));
    let selected = app.selected_agent_index;
    assert_eq!(selected, Some(1), "the click selects the sub-agent row");

    // Close the window with its Close button (TC-032 step 1: the click path a
    // real user takes).
    let (close_col, close_row) = (
        app.agents_close_button_area.x + app.agents_close_button_area.width / 2,
        app.agents_close_button_area.y + app.agents_close_button_area.height / 2,
    );
    app.handle_mouse_event(click(close_col, close_row));
    assert!(!app.show_agents_window);
    assert_eq!(
        app.selected_agent_index, selected,
        "closing the window preserves the selection (FR-065)"
    );

    // Reopen and confirm Space would still act on the recorded selection.
    app.toggle_agents_window();
    assert!(app.show_agents_window);
    assert_eq!(
        app.selected_agent_index, selected,
        "reopening preserves the selection (FR-065)"
    );
    render(&mut app);
    assert_eq!(
        app.selected_agent_index, selected,
        "the rendered reopen keeps it too"
    );
}

#[test]
fn teams_focus_survives_a_close_and_reopen() {
    let mut app = app_with_team();
    render(&mut app);

    // Focus the first teammate (FR-038).
    let (col, row) = row_point(app.teams_rows_area, 2);
    app.handle_mouse_event(click(col, row));
    assert_eq!(
        app.focused_teammate.as_deref(),
        Some("tm-001"),
        "the click focuses the teammate"
    );

    // Close with the Close button, then reopen (TC-032 step 3).
    let (close_col, close_row) = (
        app.teams_close_button_area.x + app.teams_close_button_area.width / 2,
        app.teams_close_button_area.y + app.teams_close_button_area.height / 2,
    );
    app.handle_mouse_event(click(close_col, close_row));
    assert!(!app.show_teams_window);
    assert_eq!(
        app.focused_teammate.as_deref(),
        Some("tm-001"),
        "closing the window preserves the focused teammate (FR-065)"
    );

    app.toggle_teams_window();
    assert!(app.show_teams_window);
    assert_eq!(
        app.focused_teammate.as_deref(),
        Some("tm-001"),
        "reopening preserves the focused teammate (FR-065)"
    );
}
// ---------------------------------------------------------------------------
// FR-058: focus routes the input while a teammate is focused
// ---------------------------------------------------------------------------

/// Flatten one full screen row of `buffer` into a string.
fn flat_row(buffer: &ratatui::buffer::Buffer, y: u16) -> String {
    let mut out = String::new();
    for x in 0..buffer.area.width {
        match buffer.cell((x, y)) {
            Some(cell) => out.push_str(cell.symbol()),
            None => out.push('?'),
        }
    }
    out
}

/// The column of `needle` in `row`, or `None` when the needle is absent.
///
/// Buffer rows are flatten per *column*, but `str::find` reports the byte
/// offset of the first matched character, so the column index is recovered by
/// counting the characters in front of the match.
fn row_offset(row: &str, needle: &str) -> Option<usize> {
    row.find(needle).map(|byte| row[..byte].chars().count())
}

#[test]
fn focused_teammate_titles_the_input_border_in_bold_yellow() {
    use ratatui::style::Color;

    let mut app = app_with_team();
    app.focused_teammate = Some("tm-001".to_string());
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, &mut app))
        .expect("render frame");
    let buffer = terminal.backend().buffer().clone();

    let input = app.input_area;
    assert!(
        input.height >= 3,
        "the input widget paints a bordered block"
    );
    let border = flat_row(&buffer, input.y);
    let needle = "-> reviewer (focused)";
    assert!(
        border.contains(needle),
        "the input border must carry the focused title (FR-058), got {border:?}"
    );
    let Some(col) = row_offset(&border, needle) else {
        panic!("{needle} not on the border row");
    };
    let cell = buffer
        .cell((col as u16, input.y))
        .cloned()
        .unwrap_or_else(|| panic!("title cell off the buffer"));
    let style = cell.style();
    assert_eq!(
        style.fg,
        Some(Color::Yellow),
        "the focused title must be yellow (FR-058)"
    );
    assert!(
        style.add_modifier.is_empty()
            || (style.add_modifier - ratatui::style::Modifier::empty())
                .contains(ratatui::style::Modifier::BOLD),
        "the focused title must be bold (FR-058)"
    );
}

#[test]
fn unfocused_input_border_shows_the_plain_input_title() {
    let mut app = app_with_team();
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, &mut app))
        .expect("render frame");
    let buffer = terminal.backend().buffer().clone();

    let border = flat_row(&buffer, app.input_area.y);
    assert!(
        border.contains(" Input "),
        "without focus the border reads plain Input, got {border:?}"
    );
    assert!(
        !border.contains("(focused)"),
        "an unfocused border never carries the focus title (FR-058)"
    );
}

#[tokio::test]
async fn focused_input_is_routed_to_the_teammate_mailbox() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let mut app = app_with_team();
    app.focused_teammate = Some("tm-001".to_string());
    app.input = "please report your progress".to_string();
    // The route decision happens before the provider is consulted: the lead
    // path would stop at the no-provider guard WITHOUT consuming the input.
    let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
    app.handle_key_event(key).await;

    assert!(
        app.input.is_empty(),
        "the focused teammate consumes the submission (FR-058 routing)"
    );
    assert!(
        app.status.starts_with("Failed to load team"),
        "the message went to the teammate mailbox path (FR-058), got {}",
        app.status
    );
    assert_eq!(
        app.focused_teammate.as_deref(),
        Some("tm-001"),
        "the submission keeps the focus (FR-058)"
    );
}
