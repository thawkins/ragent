//! Tests for the Agents/Teams window open-close routing (spec `agentui` T-007).
//!
//! Covers the Agents button toggle with mutual exclusion (FR-028), the no-op
//! click when the Agents window has no subject (FR-029), the Teams button
//! toggle and its exclusion (FR-030), the no-op click when no team is active
//! (FR-031), the Close-button dismissal that leaves task, member and input
//! state untouched (FR-032), and the redraw flag every mutating path sets
//! (NFR-005).
//!
//! Every case drives the real mouse router ([`App::handle_mouse_event`]) with
//! the hit rects the renderer records, so the assertions exercise the same
//! path a real click takes (NFR-001).

mod support;

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;

use ragent_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use ragent_agent::team::{MemberStatus, TeamConfig, TeamMember};
use ragent_tui::App;
use support::make_app;

/// The lead session id used by every case.
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

/// The centre cell of a recorded hit rect.
fn centre(rect: Rect) -> (u16, u16) {
    (
        rect.x + rect.width / 2,
        rect.y + rect.height.saturating_sub(1) / 2,
    )
}

/// A live sub-agent task so the Agents window has a subject.
fn running_task(id: &str) -> TaskEntry {
    TaskEntry {
        id: id.to_string(),
        parent_session_id: LEAD_SESSION.to_string(),
        child_session_id: format!("{id}-session"),
        agent_name: "explore".to_string(),
        task_prompt: "x".to_string(),
        background: true,
        detached: false,
        status: TaskStatus::Running,
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

/// A background shell task so the Agents window has a subject.
fn bg_task(id: &str) -> ragent_tui::app::BgTaskView {
    ragent_tui::app::BgTaskView {
        id: id.to_string(),
        session_id: LEAD_SESSION.to_string(),
        command: "sleep 30".to_string(),
        status: "running".to_string(),
        created_at: chrono::Utc::now(),
        completed_at: None,
    }
}

/// A teammate in `Working` state.
fn member(name: &str, agent_id: &str) -> TeamMember {
    let mut member = TeamMember::new(name, agent_id, "general");
    member.status = MemberStatus::Working;
    member.session_id = Some(format!("{agent_id}-session"));
    member
}

/// A bare app with an explicit redraw baseline of "clean".
fn app() -> App {
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.agent_name = "general".to_string();
    app.needs_redraw = false;
    app
}

/// An app with one running sub-agent task, so the Agents button is enabled.
fn app_with_task() -> App {
    let mut app = app();
    app.active_tasks.push(running_task("task-1"));
    app
}

/// An app with an active team holding one teammate.
fn app_with_team() -> App {
    let mut app = app();
    app.active_team = Some(TeamConfig::new("alpha", LEAD_SESSION));
    app.team_members.push(member("reviewer", "tm-001"));
    app
}

/// Render one frame so the renderer records its hit rects, as a real session
/// does before any click can arrive.
fn render(app: &mut App) {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("render frame");
}

// ---------------------------------------------------------------------------
// FR-028 / FR-029: Agents button
// ---------------------------------------------------------------------------

#[test]
fn agents_button_toggles_the_window_and_requests_a_redraw() {
    let mut app = app_with_task();
    render(&mut app);
    let (col, row) = centre(app.agents_button_area);

    app.handle_mouse_event(click(col, row));
    assert!(app.show_agents_window, "the first click opens the window");
    assert!(app.needs_redraw, "opening the window requests a redraw");

    app.needs_redraw = false;
    app.handle_mouse_event(click(col, row));
    assert!(!app.show_agents_window, "the second click closes it");
    assert!(app.needs_redraw, "closing the window requests a redraw");
}

#[test]
fn agents_button_click_is_a_no_op_without_a_subject() {
    let mut app = app();
    render(&mut app);
    let (col, row) = centre(app.agents_button_area);

    app.handle_mouse_event(click(col, row));
    assert!(
        !app.show_agents_window,
        "a click with nothing to show is inert"
    );
    assert!(
        !app.needs_redraw,
        "a refused click must not request a redraw (NFR-005)"
    );
}

#[test]
fn agents_button_click_keeps_an_open_window_open_when_the_subject_vanishes() {
    let mut app = app();
    render(&mut app);
    app.show_agents_window = true;
    app.needs_redraw = false;
    let (col, row) = centre(app.agents_button_area);

    app.handle_mouse_event(click(col, row));
    assert!(
        app.show_agents_window,
        "with no subject the window keeps its current open state"
    );
    assert!(!app.needs_redraw, "a no-op click paints nothing");
}

// ---------------------------------------------------------------------------
// FR-028 / FR-030: mutual exclusion
// ---------------------------------------------------------------------------

#[test]
fn opening_the_agents_window_closes_the_teams_window() {
    let mut app = app_with_task();
    app.active_team = Some(TeamConfig::new("alpha", LEAD_SESSION));
    render(&mut app);
    app.show_teams_window = true;
    let (col, row) = centre(app.agents_button_area);

    app.handle_mouse_event(click(col, row));
    assert!(app.show_agents_window, "the Agents window opens");
    assert!(!app.show_teams_window, "the Teams window is closed");
}

#[test]
fn opening_the_teams_window_closes_the_agents_window() {
    let mut app = app_with_team();
    app.active_tasks.push(running_task("task-1"));
    render(&mut app);
    app.show_agents_window = true;
    let (col, row) = centre(app.teams_button_area);

    app.handle_mouse_event(click(col, row));
    assert!(app.show_teams_window, "the Teams window opens");
    assert!(!app.show_agents_window, "the Agents window is closed");
}

// ---------------------------------------------------------------------------
// FR-030 / FR-031: Teams button
// ---------------------------------------------------------------------------

#[test]
fn teams_button_toggles_the_window_and_requests_a_redraw() {
    let mut app = app_with_team();
    render(&mut app);
    let (col, row) = centre(app.teams_button_area);

    app.handle_mouse_event(click(col, row));
    assert!(app.show_teams_window, "the first click opens the window");
    assert!(app.needs_redraw, "opening the window requests a redraw");

    app.needs_redraw = false;
    app.handle_mouse_event(click(col, row));
    assert!(!app.show_teams_window, "the second click closes it");
    assert!(app.needs_redraw, "closing the window requests a redraw");
}

#[test]
fn teams_button_click_is_a_no_op_without_an_active_team() {
    let mut app = app();
    render(&mut app);
    let (col, row) = centre(app.teams_button_area);

    app.handle_mouse_event(click(col, row));
    assert!(!app.show_teams_window, "a click with no team is inert");
    assert!(
        !app.needs_redraw,
        "a refused click must not request a redraw (NFR-005)"
    );
}

#[test]
fn teams_button_click_keeps_an_open_window_open_when_the_team_is_gone() {
    let mut app = app();
    render(&mut app);
    app.show_teams_window = true;
    app.needs_redraw = false;
    let (col, row) = centre(app.teams_button_area);

    app.handle_mouse_event(click(col, row));
    assert!(
        app.show_teams_window,
        "with no team the window keeps its current open state"
    );
    assert!(!app.needs_redraw, "a no-op click paints nothing");
}

// ---------------------------------------------------------------------------
// FR-032: Close button
// ---------------------------------------------------------------------------

#[test]
fn agents_close_button_dismisses_the_window_and_leaves_state_untouched() {
    let mut app = app_with_task();
    app.active_tasks.push(running_task("task-2"));
    app.input = "hello".to_string();
    let tasks_before = app.active_tasks.len();
    app.show_agents_window = true;
    render(&mut app);

    let (col, row) = centre(app.agents_close_button_area);
    app.handle_mouse_event(click(col, row));

    assert!(
        !app.show_agents_window,
        "the Close button dismisses the window"
    );
    assert!(app.needs_redraw, "dismissing requests a redraw");
    assert_eq!(
        app.active_tasks.len(),
        tasks_before,
        "closing the window must not touch the task list"
    );
    assert_eq!(
        app.input, "hello",
        "closing the window must not touch the input"
    );
}

#[test]
fn teams_close_button_dismisses_the_window_and_leaves_state_untouched() {
    let mut app = app_with_team();
    app.input = "draft".to_string();
    app.show_teams_window = true;
    render(&mut app);

    let (col, row) = centre(app.teams_close_button_area);
    app.handle_mouse_event(click(col, row));

    assert!(
        !app.show_teams_window,
        "the Close button dismisses the window"
    );
    assert!(app.needs_redraw, "dismissing requests a redraw");
    assert_eq!(
        app.team_members.len(),
        1,
        "closing the window must not touch the member list"
    );
    assert_eq!(
        app.input, "draft",
        "closing the window must not touch the input"
    );
}

#[test]
fn closing_an_already_closed_window_is_inert() {
    let mut app = app_with_task();
    render(&mut app);
    // Both windows are closed, so the renderer zeroed the Close rects.
    assert_eq!(app.agents_close_button_area, Rect::default());
    assert_eq!(app.teams_close_button_area, Rect::default());

    app.handle_mouse_event(click(0, 0));
    assert!(!app.show_agents_window);
    assert!(!app.show_teams_window);
    assert!(
        !app.needs_redraw,
        "clicking a closed window's absent Close button paints nothing"
    );
}

// ---------------------------------------------------------------------------
// Router helpers directly (the state machine behind every click path)
// ---------------------------------------------------------------------------

#[test]
fn routing_helpers_agree_with_the_buttons_enabled_state() {
    let mut app = app();
    assert!(!app.agents_window_available());
    assert!(!app.teams_window_available());

    app.active_tasks.push(running_task("task-1"));
    assert!(app.agents_window_available());

    app.active_tasks.clear();
    app.bg_tasks.push(bg_task("bg-1"));
    assert!(
        app.agents_window_available(),
        "a background shell task alone enables the window"
    );

    app.bg_tasks.clear();
    assert!(!app.agents_window_available());

    app.active_team = Some(TeamConfig::new("alpha", LEAD_SESSION));
    assert!(app.teams_window_available());
}

#[test]
fn toggling_through_the_helpers_is_idempotent_when_the_subject_is_absent() {
    let mut app = app();
    app.toggle_agents_window();
    assert!(!app.show_agents_window);
    app.toggle_teams_window();
    assert!(!app.show_teams_window);
    app.close_agents_window();
    app.close_teams_window();
    assert!(!app.needs_redraw, "no refused or redundant path paints");
}
