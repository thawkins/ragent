//! Tests for the Agents-window lifecycle and status-line reporting (spec
//! `agentui` T-018, FR-061, FR-063, FR-064, NFR-005).
//!
//! - FR-063: the Agents window closes on the turn whose removal empties it -
//!   via the `SubagentComplete`, `SubagentCancelled`, `SubagentKilled` and
//!   `BackgroundTaskCompleted` event arms and via the registry reconciliation
//!   merge; a row remaining keeps it open and an already-closed window is a
//!   silent no-op;
//! - FR-061: window open/close and row-button actions stay serviced while a
//!   turn is running (`is_processing`), driven through the real mouse router;
//! - FR-064: the protected-agent refusals, the teammate focus transitions and
//!   the focus-cleared notice appear in the status line;
//! - NFR-005: every auto-close, refusal and focus transition sets the redraw
//!   flag so the next frame shows the change.
//!
//! Every case builds synthetic in-memory state through `make_app`; nothing
//! writes into the repository working tree (NFR-012).

#[path = "support/mod.rs"]
mod support;

use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;

use ragent_agent::event::Event;
use ragent_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use ragent_agent::team::{TeamConfig, TeamMember};
use ragent_tui::App;
use support::make_app;

/// The lead session id every case uses.
const LEAD_SESSION: &str = "lead-session-t018";

/// Render one frame through the real entry point so the renderer records the
/// hit rects a real session carries (NFR-001).
fn render(app: &mut App) {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("render frame");
}

/// A left-button press at `(col, row)`.
fn click(col: u16, row: u16) -> MouseEvent {
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

/// A live sub-agent task. The id keeps its 8-char short prefix visible: the
/// suspend/kill request lines are logged under `short_id` (the leading 8
/// characters).
fn task(id: &str, agent: &str, status: TaskStatus) -> TaskEntry {
    TaskEntry {
        id: id.to_string(),
        parent_session_id: LEAD_SESSION.to_string(),
        child_session_id: format!("{id}-session"),
        agent_name: agent.to_string(),
        task_prompt: "do work".to_string(),
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

/// A background shell task view.
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

/// An app with the Agents window open, a redraw baseline of "clean" and the
/// given sub-agent rows.
fn app_with_window(tasks: Vec<TaskEntry>) -> App {
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.agent_name = "general".to_string();
    app.active_tasks = tasks;
    app.show_agents_window = true;
    app.needs_redraw = false;
    app
}

/// An app with an active two-member team.
fn app_with_team() -> App {
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.agent_name = "general".to_string();
    app.active_team = Some(TeamConfig::new("alpha", LEAD_SESSION));
    let mut member = TeamMember::new("reviewer", "tm-001", "general");
    member.status = ragent_agent::team::MemberStatus::Working;
    member.session_id = Some("tm-001-session".to_string());
    app.team_members.push(member);
    app.show_teams_window = true;
    app.needs_redraw = false;
    app
}

// ---------------------------------------------------------------------------
// FR-063: the window closes when its list empties
// ---------------------------------------------------------------------------

/// The last `SubagentComplete` closes the Agents window and sets the redraw
/// flag, so the very next frame paints without the window (TC-031).
#[tokio::test]
async fn the_last_subagent_completion_closes_the_agents_window() {
    let mut app = app_with_window(vec![task("a1b2c3d4", "explore", TaskStatus::Running)]);
    render(&mut app);
    assert!(app.show_agents_window, "precondition: the window is open");

    app.handle_event(Event::SubagentComplete {
        session_id: LEAD_SESSION.to_string(),
        task_id: "a1b2c3d4".to_string(),
        child_session_id: "a1b2c3d4-session".to_string(),
        summary: "report body".to_string(),
        success: true,
        duration_ms: 42,
        finish_reason: "stop".to_string(),
    })
    .await;

    assert!(
        !app.show_agents_window,
        "the last row's completion must close the Agents window (FR-063)"
    );
    assert!(
        app.needs_redraw,
        "the auto-close must request a repaint (NFR-005)"
    );

    // The next frame paints the chat without the Agents window and leaves no
    // stale hit rect behind (TC-031 step 2).
    render(&mut app);
    assert!(
        !app.show_agents_window,
        "a closed window stays closed across a repaint"
    );
}

/// A completion while another sub-agent is still tracked keeps the window
/// open (FR-063 only fires on the last removal).
#[tokio::test]
async fn a_completion_with_a_row_remaining_keeps_the_window_open() {
    let mut app = app_with_window(vec![
        task("a1b2c3d4", "explore", TaskStatus::Running),
        task("b2c3d4e5", "build", TaskStatus::Running),
    ]);
    render(&mut app);

    app.handle_event(Event::SubagentComplete {
        session_id: LEAD_SESSION.to_string(),
        task_id: "a1b2c3d4".to_string(),
        child_session_id: "a1b2c3d4-session".to_string(),
        summary: "report body".to_string(),
        success: true,
        duration_ms: 42,
        finish_reason: "stop".to_string(),
    })
    .await;

    assert!(
        app.show_agents_window,
        "with a row remaining the window stays open (FR-063)"
    );
}

/// The last background-shell task's completion closes the window; a
/// sub-agent still tracked keeps it open (FR-063 covers both row kinds).
#[tokio::test]
async fn the_last_background_task_completion_closes_the_window() {
    // Background alone.
    let mut app = app_with_window(Vec::new());
    app.bg_tasks.push(bg_task("bg-1"));
    render(&mut app);
    assert!(app.show_agents_window);

    app.handle_event(Event::BackgroundTaskCompleted {
        session_id: LEAD_SESSION.to_string(),
        task_id: "bg-1".to_string(),
        status: "completed".to_string(),
        exit_code: Some(0),
    })
    .await;
    assert!(
        !app.show_agents_window,
        "an emptied background shell list closes the window (FR-063)"
    );
    assert!(app.needs_redraw, "the auto-close repaints (NFR-005)");

    // Background empties while a sub-agent still runs.
    let mut app = app_with_window(vec![task("a1b2c3d4", "explore", TaskStatus::Running)]);
    app.bg_tasks.push(bg_task("bg-2"));
    render(&mut app);

    app.handle_event(Event::BackgroundTaskCompleted {
        session_id: LEAD_SESSION.to_string(),
        task_id: "bg-2".to_string(),
        status: "completed".to_string(),
        exit_code: Some(0),
    })
    .await;
    assert!(
        app.show_agents_window,
        "a live sub-agent keeps the window open (FR-063)"
    );
}

/// A cancellation or a kill of the last row closes the window (FR-063 runs on
/// every removal path of TC-031's "kill it from the panel" step).
#[tokio::test]
async fn a_cancel_and_a_kill_of_the_last_row_close_the_window() {
    let mut app = app_with_window(vec![task("a1b2c3d4", "explore", TaskStatus::Running)]);
    render(&mut app);
    app.handle_event(Event::SubagentCancelled {
        session_id: LEAD_SESSION.to_string(),
        task_id: "a1b2c3d4".to_string(),
    })
    .await;
    assert!(
        !app.show_agents_window,
        "the last cancellation closes the window (FR-063)"
    );

    let mut app = app_with_window(vec![task("b2c3d4e5", "build", TaskStatus::Running)]);
    render(&mut app);
    app.handle_event(Event::SubagentKilled {
        session_id: LEAD_SESSION.to_string(),
        task_id: "b2c3d4e5".to_string(),
        child_session_id: "b2c3d4e5-session".to_string(),
        force: true,
    })
    .await;
    assert!(
        !app.show_agents_window,
        "the last kill closes the window (FR-063)"
    );
}

/// The registry reconciliation merge - the safety net for dropped events -
/// closes the window when it removes the last row (FR-063, FR-054).
#[test]
fn reconciliation_empting_the_list_closes_the_window() {
    let mut app = app_with_window(vec![task("a1b2c3d4", "explore", TaskStatus::Running)]);
    render(&mut app);

    // The registry reports the only tracked task completed: the merge drops
    // the ghost, the list empties and the window closes with it.
    let snapshot = vec![TaskEntry {
        status: TaskStatus::Completed,
        ..task("a1b2c3d4", "explore", TaskStatus::Running)
    }];
    app.apply_registry_reconciliation(snapshot);

    assert!(
        !app.show_agents_window,
        "a reconciliation that empties the list closes the window (FR-063)"
    );
    assert!(app.needs_redraw, "the auto-close must repaint (NFR-005)");
}

/// Closing an already-closed window is a silent no-op: no redraw is demanded
/// and no state changes (NFR-005).
#[test]
fn an_already_closed_or_occupied_window_is_untouched_by_the_auto_close() {
    // Closed already: silent.
    let mut app = app_with_window(Vec::new());
    app.show_agents_window = false;
    app.close_agents_window_when_empty();
    assert!(!app.show_agents_window);
    assert!(
        !app.needs_redraw,
        "a redundant close on a closed window paints nothing (NFR-005)"
    );

    // Still has a subject: the window stays open.
    let mut app = app_with_window(vec![task("a1b2c3d4", "explore", TaskStatus::Running)]);
    app.close_agents_window_when_empty();
    assert!(
        app.show_agents_window,
        "the auto-close never fires while a subject is tracked (FR-063)"
    );
}

// ---------------------------------------------------------------------------
// FR-061: the panel stays serviceable during a running turn
// ---------------------------------------------------------------------------

/// With `is_processing` true (a turn streaming), the Agents button toggle, the
/// With `is_processing` true (a turn streaming), the Agents button toggle, the
/// `Esc` dismissal and a row `[stop]` control all stay serviced through the real
/// key/mouse router (TC-029 steps 1-4; FR-061).
#[tokio::test]
async fn window_actions_stay_serviceable_during_a_running_turn() {
    let mut app = app_with_window(vec![task("a1b2c3d4", "explore", TaskStatus::Running)]);
    app.is_processing = true;
    render(&mut app);

    // Step 1: the Agents button opens the window mid-turn.
    let (bcol, brow) = centre(app.agents_button_area);
    app.needs_redraw = false;
    app.show_agents_window = false;
    app.handle_mouse_event(click(bcol, brow));
    assert!(
        app.show_agents_window,
        "the button opens the window mid-turn (FR-061)"
    );
    assert!(app.needs_redraw, "the open repaints (NFR-005)");

    // Step 3: keyboard cursor actions on the sub-agent row stay serviced
    // mid-turn: Space dispatches the suspend request (non-destructive
    // direction needs no dialog, FR-035). (The old [stop]-click trigger is
    // retired with the row mouse.)
    render(&mut app);
    app.needs_redraw = false;
    app.move_agents_cursor(1);
    app.handle_key_event(crossterm::event::KeyEvent::new(
        KeyCode::Char(' '),
        KeyModifiers::NONE,
    ))
    .await;
    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("Suspend requested for task a1b2c3d4")),
        "a mid-turn [stop] click dispatches the suspend (FR-061): {:?}",
        app.log_entries.last().map(|e| e.message.clone())
    );
    assert!(app.needs_redraw, "the request repaints (NFR-005)");

    // Step 4: `Esc` dismisses the window mid-turn.
    app.handle_key_event(crossterm::event::KeyEvent::new(
        KeyCode::Esc,
        KeyModifiers::NONE,
    ))
    .await;
    assert!(
        !app.show_agents_window,
        "Esc dismisses the window mid-turn (FR-061)"
    );

    // The streaming state is untouched by every panel action above.
    assert!(
        app.is_processing,
        "panel operations must not disturb the running turn (FR-061)"
    );
}

/// The keyboard surface also stays serviced mid-turn: `Space` on the cursor
/// row dispatches while `is_processing` is true (TC-029; FR-061, FR-041).
#[tokio::test]
async fn keyboard_actions_stay_serviceable_during_a_running_turn() {
    let mut app = app_with_window(vec![task("a1b2c3d4", "explore", TaskStatus::Running)]);
    app.is_processing = true;
    render(&mut app);
    app.agents_cursor_subagent = Some(0);

    app.handle_key_event(crossterm::event::KeyEvent::new(
        KeyCode::Char(' '),
        KeyModifiers::NONE,
    ))
    .await;

    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("Suspend requested for task a1b2c3d4")),
        "a mid-turn Space dispatches the suspend (FR-061)"
    );
    assert!(
        app.is_processing,
        "the keyboard path must not disturb the running turn"
    );
}

/// A teammate row-click also stays serviced mid-turn: the Teams button and the
/// keyboard cursor keep operating while `is_processing` is true (TC-029
/// step 2's Teams-button half; FR-061, FR-038).
#[test]
fn teams_window_stays_serviceable_during_a_running_turn() {
    let mut app = app_with_team();
    app.is_processing = true;
    render(&mut app);

    // The Teams button is reachable mid-turn: it toggles, so the click closes
    // the open window and the next click reopens it.
    let (bcol, brow) = centre(app.teams_button_area);
    app.handle_mouse_event(click(bcol, brow));
    assert!(
        !app.show_teams_window,
        "the Teams button keeps operating mid-turn (FR-061)"
    );
    app.handle_mouse_event(click(bcol, brow));
    assert!(
        app.show_teams_window,
        "the second click reopens it (FR-061)"
    );

    // The teammate keyboard cursor still focuses mid-turn: ordinal 0 = tm-001.
    render(&mut app);
    app.move_teams_cursor(1);
    app.teams_cursor_focus();
    assert_eq!(
        app.focused_teammate.as_deref(),
        Some("tm-001"),
        "a teammate focus stays serviced mid-turn (FR-061)"
    );
    assert!(
        app.is_processing,
        "the panel actions must not disturb the running turn"
    );
}

// ---------------------------------------------------------------------------
// FR-064: status-line reporting for refusals and focus transitions
// ---------------------------------------------------------------------------

/// The keyboard cursor refusals/status reporting stay live while a turn
/// streams (FR-064; the protected-agent refusals are re-expressed by the
/// cursor range - the cursor never reaches the primary row, so the reported
/// text is the consumed no-op message instead, T-011's keyboard twin).
#[tokio::test]
async fn cursor_reports_to_the_status_line_during_a_running_turn() {
    let mut app = app_with_window(vec![task("a1b2c3d4", "explore", TaskStatus::Running)]);
    app.is_processing = true;

    app.handle_key_event(crossterm::event::KeyEvent::new(
        KeyCode::Char(' '), // no cursor yet: consumed no-op with its status text
        KeyModifiers::NONE,
    ))
    .await;
    assert_eq!(
        app.status, "agents: selected row is gone",
        "the window key reports to the status line (FR-064)"
    );

    // Moving the cursor also reports the landed row on the status line.
    app.handle_key_event(crossterm::event::KeyEvent::new(
        KeyCode::Down,
        KeyModifiers::NONE,
    ))
    .await;
    assert!(
        app.status.starts_with("agents: "),
        "a cursor move reports the selection (FR-064), got {}",
        app.status
    );
    assert!(
        app.is_processing,
        "the keyboard path must not disturb the running turn"
    );
}

/// An `Alt+Down` focus transition reports the newly focused teammate to the
/// status line, and cycling past the ends reports the cleared focus
/// (FR-064, FR-046).
#[tokio::test]
async fn focus_transitions_report_to_the_status_line() {
    let mut app = app_with_team();

    app.cycle_focused_teammate(true);

    assert_eq!(
        app.focused_teammate.as_deref(),
        Some("tm-001"),
        "the first cycle focuses the only member (FR-046)"
    );
    assert_eq!(
        app.status, "team: focused -> reviewer",
        "the focus transition reports to the status line (FR-064)"
    );

    // Cycling past the end clears the focus and reports it (FR-064).
    app.cycle_focused_teammate(true);
    assert!(app.focused_teammate.is_none());
    assert_eq!(
        app.status, "team: focus cleared",
        "the focus-cleared notice reports to the status line (FR-064)"
    );
}

/// The keyboard cursor focus transition reports to the status line too
/// (FR-064; the keyboard path is FR-038's cursor arm).
#[test]
fn a_teammate_focus_reports_to_the_status_line() {
    let mut app = app_with_team();
    render(&mut app);

    app.move_teams_cursor(1);
    app.teams_cursor_focus();

    assert_eq!(
        app.status, "team: focused -> reviewer",
        "the keyboard focus transition reports to the status line (FR-064)"
    );
}
