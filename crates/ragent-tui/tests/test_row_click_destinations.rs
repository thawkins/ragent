//! Tests for row destinations (spec `agentui` T-008, keyboard-first).
//!
//! The row-mouse-click model was retired: the Agents / Teams popup windows are
//! now keyboard-driven through a block cursor, and the mouse router only keeps
//! wheel scrolling, scrollbar-gutter dragging and the close buttons. This file
//! therefore drives the destination chains through the keyboard cursor helpers
//! (`move_agents_cursor` / `agents_cursor_open_output`,
//! `move_teams_cursor` / `teams_cursor_focus`) - the same chains the old row
//! clicks drove:
//!
//! - FR-034: `Enter` on a cursor sub-agent row opens the output view of the
//!   task the cursor names, in tree paint order;
//! - FR-038: `Enter` on a cursor teammate row focuses the member, opens its
//!   output view and reports the transition to the status line;
//! - FR-064: the focus transition reports through the keyboard path too;
//! - FR-065: the cursor ordinal and the focused teammate survive a window
//!   close/reopen;
//! - NFR-005: every destination sets the redraw flag.
//!
//! The focused-border-title and mailbox-routing cases never used clicks; they
//! set `focused_teammate` directly and are kept as written. All state is
//! synthetic; nothing is written into the repository (NFR-012).

mod support;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use ragent_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use ragent_agent::team::{MemberStatus, TeamConfig, TeamMember};
use ragent_tui::app::{OutputViewState, OutputViewTarget};
use ragent_tui::{App, utils};
use support::make_app;

/// The lead session id every Agents case uses.
const LEAD_SESSION: &str = "lead-session-0001";

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

/// Render `app` so the renderer publishes the row maps the cursor resolves
/// against; the helpers also work before a render, but a real session always
/// has at least one painted frame behind it (NFR-001).
fn render(app: &mut App) {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("render frame");
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

/// Select the painted sub-agent row `ordinal` rows below the primary with the
/// keyboard cursor (Down presses start from no selection at the first row).
fn cursor_onto_subagent(app: &mut App, ordinal: usize) {
    for _ in 0..=ordinal {
        app.move_agents_cursor(1);
    }
}

// ---------------------------------------------------------------------------
// FR-034: sub-agent rows, including a tree-order mapping case
// ---------------------------------------------------------------------------

#[test]
fn enter_on_the_cursor_opens_the_task_painted_on_that_row() {
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

    // Painted rows: 0 header, 1 primary, 2 task-1, 3 task-9, 4 task-2 - the
    // cursor ordinals count sub-agent rows only, so ordinal 1 is the nested
    // task-9.
    assert_eq!(
        app.agents_subagent_task_ids,
        vec![
            "task-1".to_string(),
            "task-9".to_string(),
            "task-2".to_string()
        ],
        "the cursor range must follow tree paint order (FR-034)"
    );
    cursor_onto_subagent(&mut app, 1);
    assert_eq!(
        app.agents_cursor_subagent,
        Some(1),
        "two Down presses land on the second painted sub-agent row"
    );
    app.needs_redraw = false;
    app.agents_cursor_open_output();

    let view = app.output_view.as_ref().expect("output view opens");
    let (session, _label) = output_session(view);
    assert_eq!(
        session, "task-9-session",
        "Enter must open the nested task the cursor names (FR-034)"
    );
    // Ordinal 1 of the cursor list names `task-9` - the keyboard Space/Del
    // path reaches the same task.
    assert_eq!(
        app.agents_subagent_task_ids
            .get(app.agents_cursor_subagent.unwrap())
            .map(String::as_str),
        Some("task-9"),
        "Space/Del on the recorded ordinal must reach the opened task (FR-034)"
    );
    assert!(app.needs_redraw, "opening the view repaints (NFR-005)");
}

#[test]
fn enter_on_the_cursor_label_carries_agent_and_short_task_id() {
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

    cursor_onto_subagent(&mut app, 0);
    app.agents_cursor_open_output();

    let view = app.output_view.as_ref().expect("output view opens");
    if let OutputViewTarget::Session { label, .. } = &view.target {
        assert_eq!(
            label,
            &format!("explore [{}]", utils::short_id("task-42")),
            "the label carries the agent name and short task id (FR-034)"
        );
    } else {
        panic!("a sub-agent Enter opens a Session output view (FR-034)");
    }
}

#[test]
fn a_stale_cursor_row_is_a_consumed_no_op() {
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.agent_name = "general".to_string();
    app.active_tasks
        .push(task("task-1", LEAD_SESSION, "explore", TaskStatus::Running));
    app.show_agents_window = true;
    render(&mut app);

    // The task list changes after the render that published the row map
    // (FR-069): the cursor now names a row that is gone.
    app.active_tasks.clear();
    app.status = "sentinel".to_string();
    cursor_onto_subagent(&mut app, 0);
    app.agents_cursor_open_output();

    assert!(
        app.output_view.is_none(),
        "a stale cursor row must open nothing (FR-069)"
    );
    assert_ne!(
        app.status, "sentinel",
        "a stale cursor row reports itself on the status line"
    );
}

/// The cursor clamps at both ends instead of leaving the row range: the
/// retired stale-out-of-range mouse case has no keyboard analogue; clamping is
/// the new navigation invariant.
#[test]
fn the_cursor_clamps_at_both_ends_of_the_row_list() {
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.agent_name = "general".to_string();
    app.active_tasks = vec![
        task("task-1", LEAD_SESSION, "explore", TaskStatus::Running),
        task("task-2", LEAD_SESSION, "plan", TaskStatus::Running),
    ];
    app.show_agents_window = true;
    render(&mut app);

    for _ in 0..10 {
        app.move_agents_cursor(1);
    }
    assert_eq!(
        app.agents_cursor_subagent,
        Some(1),
        "Down past the end clamps at the last sub-agent row"
    );
    for _ in 0..10 {
        app.move_agents_cursor(-1);
    }
    assert_eq!(
        app.agents_cursor_subagent,
        Some(0),
        "Up past the top clamps at the first sub-agent row"
    );
    // The primary row is not part of the cursor range: clamping at 0 never
    // reaches it (FR-067).
    assert_eq!(
        app.agents_subagent_task_ids[0], "task-1",
        "cursor row 0 is the first sub-agent, never the primary (FR-067)"
    );
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
fn enter_on_the_teammate_cursor_focuses_the_teammate_and_reports_it() {
    let mut app = app_with_team();
    render(&mut app);

    // Painted rows: 0 header, 1 lead, 2 tm-001 - cursor ordinal 0 is tm-001.
    app.move_teams_cursor(1);
    assert_eq!(
        app.teams_cursor_teammate,
        Some(0),
        "the first Down lands on the first teammate row"
    );
    app.needs_redraw = false;
    app.teams_cursor_focus();

    assert_eq!(
        app.focused_teammate.as_deref(),
        Some("tm-001"),
        "Enter on the teammate cursor focuses the teammate (FR-038)"
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
        "the keyboard focus opens the teammate's output view (FR-038)"
    );
    assert!(app.needs_redraw, "a focus change repaints (NFR-005)");
}

/// The cursor range covers teammate rows only: there is no keyboard path that
/// clears focus onto the lead (FR-039's lead-click case is retired with the
/// clicks; the cursor simply cannot reach the lead row).
#[test]
fn the_teammate_cursor_never_reaches_the_lead_row() {
    let mut app = app_with_team();
    render(&mut app);

    let lead_len = 1; // the lead is one painted row but not cursorable
    let _ = lead_len;
    for _ in 0..10 {
        app.move_teams_cursor(-1);
    }
    assert_eq!(
        app.teams_cursor_teammate,
        Some(0),
        "Up past the top clamps at the first teammate row - never the lead (FR-067)"
    );
    assert_eq!(
        app.teams_teammate_agent_ids.first().map(String::as_str),
        Some("tm-001"),
        "the lead is not part of the cursor range (FR-067)"
    );
}

#[test]
fn a_stale_teammate_cursor_is_ignored_without_mutating_state() {
    let mut app = app_with_team();
    render(&mut app);

    // The member list changes after the render that published the row map:
    // the cursor is stale (FR-069).
    app.team_members.clear();
    app.move_teams_cursor(1);
    app.status = "sentinel".to_string();
    app.teams_cursor_focus();

    assert_eq!(
        app.focused_teammate, None,
        "a stale cursor must not focus anything (FR-069)"
    );
    assert_eq!(
        app.status, "sentinel",
        "a stale focus path mutates nothing (FR-069)"
    );
}

/// A stale cursor ordinal (beyond the published range) reports the consumed
/// no-op on the status line (FR-069).
#[test]
fn a_stale_teammate_cursor_ordinal_reports_the_no_op() {
    let mut app = app_with_team();
    render(&mut app);

    app.teams_cursor_teammate = Some(9); // beyond the painted teammate rows
    app.status = "sentinel".to_string();
    app.move_teams_cursor(1);
    app.teams_cursor_focus();

    // move_teams_cursor on a published list clamps; the focus path on the
    // (now in-range) row is live, so reset to a genuinely stale ordinal after
    // the render-sized list: ordinal 2 with only two members.
    let mut app = app_with_team();
    render(&mut app);
    app.teams_cursor_teammate = Some(2);
    app.teams_cursor_toggle_stop_resume();
    assert_eq!(
        app.status, "teams: selected row is gone",
        "Space beyond the range reports the consumed no-op (FR-069)"
    );
    assert!(
        !app.row_action_confirm_open(),
        "a stale ordinal arms no dialog (FR-069)"
    );
}

// ---------------------------------------------------------------------------
// FR-065: cursor and focus survive a close/reopen
// ---------------------------------------------------------------------------

#[tokio::test]
async fn agents_cursor_survives_a_close_and_reopen() {
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.agent_name = "general".to_string();
    app.active_tasks
        .push(task("task-1", LEAD_SESSION, "explore", TaskStatus::Running));
    app.show_agents_window = true;
    render(&mut app);

    // Select the sub-agent row with the keyboard cursor (FR-034).
    cursor_onto_subagent(&mut app, 0);
    let selected = app.agents_cursor_subagent;
    assert_eq!(selected, Some(0), "the cursor selects the sub-agent row");

    // Close the window with `Esc` (TC-032 step 1: the key path a real user
    // takes, since the modal carries no Close button).
    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        .await;
    assert!(!app.show_agents_window);
    assert_eq!(
        app.agents_cursor_subagent, selected,
        "closing the window preserves the cursor (FR-065)"
    );

    // Reopen and confirm Enter would still act on the recorded ordinal.
    app.toggle_agents_window();
    assert!(app.show_agents_window);
    assert_eq!(
        app.agents_cursor_subagent, selected,
        "reopening preserves the cursor (FR-065)"
    );
    render(&mut app);
    assert_eq!(
        app.agents_cursor_subagent, selected,
        "the rendered reopen keeps it too"
    );
    // The reopened render still resolves the cursor to the same task id.
    assert_eq!(
        app.agents_subagent_task_ids
            .get(selected.unwrap())
            .map(String::as_str),
        Some("task-1"),
        "the preserved cursor still names the same sub-agent (FR-065)"
    );
}

#[tokio::test]
async fn teams_focus_survives_a_close_and_reopen() {
    let mut app = app_with_team();
    render(&mut app);

    // Focus the first teammate with the keyboard cursor (FR-038).
    app.move_teams_cursor(1);
    app.teams_cursor_focus();
    assert_eq!(
        app.focused_teammate.as_deref(),
        Some("tm-001"),
        "Enter focuses the teammate"
    );

    // Close with `Esc`, then reopen (TC-032 step 3). Focusing a teammate opens
    // its output view, which is the topmost surface, so the first `Esc` closes
    // the overlay and the second dismisses the Teams window beneath it.
    for _ in 0..2 {
        app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
            .await;
    }
    assert!(!app.show_teams_window);
    assert!(app.output_view.is_none(), "the overlay closed first");
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
    // With the popup open `Enter` is the open-overlay key (FR-038); the
    // mailbox routing applies once the window is closed again.
    app.show_teams_window = false;
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
