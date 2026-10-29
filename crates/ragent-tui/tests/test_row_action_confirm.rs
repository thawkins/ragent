//! Tests for the destructive row-action confirmation dialog (spec `agentui`
//! T-009).
//!
//! Covers the modal itself - `open_row_action` arming it with `No` selected
//! (FR-069), the render that paints the title/target and caches the three
//! hit-rect areas (NFR-010), the keyboard answer path (`Enter` selects the
//! highlighted option, `Esc` dismisses, every other key is swallowed so
//! nothing reaches the input buffer), the mouse answer path (a click on the
//! painted `[ Yes ]` / `[ No ]` span answers that option), the destructive
//! trigger points routing through the dialog (sub-agent Shift+K with the
//! documented task id and agent name; background `[x]` cancel; teammate
//! `[stop]` suspend with the direct resume exemption, FR-040; teammate `[x]`
//! shutdown; teammate Space/Shift+K shutdown, FR-045), the stale-row answer
//! that mutates nothing (FR-069), and the redraw flag every mutating path
//! sets (NFR-005).
//!
//! The keyboard-only confirmations mirror `test_stop_confirm_dialog.rs` and
//! the mouse cases drive the real `App::handle_mouse_event` router with the
//! rects the renderer records.

#[path = "support/mod.rs"]
mod support;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

use ragent_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use ragent_agent::team::{MemberStatus, TeamConfig, TeamMember};
use ragent_tui::App;
use ragent_tui::app::RowAction;
use support::make_app;

/// The lead session id every case uses.
const LEAD_SESSION: &str = "lead-session-0001";

/// A team name no team store on disk can own, so the FR-060 render hydration
/// never overrides the synthetic member statuses these cases rely on.
const TEAM_NAME: &str = "row-action-confirm";

fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

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

/// A live sub-agent task.
fn task(id: &str, agent: &str, status: TaskStatus) -> TaskEntry {
    TaskEntry {
        id: id.to_string(),
        parent_session_id: LEAD_SESSION.to_string(),
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
        time_bound_secs: None,
    }
}

/// A background shell task view.
fn bg_task(id: &str, command: &str) -> ragent_tui::app::BgTaskView {
    ragent_tui::app::BgTaskView {
        id: id.to_string(),
        session_id: LEAD_SESSION.to_string(),
        command: command.to_string(),
        status: "running".to_string(),
        created_at: chrono::Utc::now(),
        completed_at: None,
    }
}

/// A teammate in `status` with a spawned session id.
fn member(agent_id: &str, name: &str, status: MemberStatus) -> TeamMember {
    let mut member = TeamMember::new(name, agent_id, "general");
    member.status = status;
    member.session_id = Some(format!("{agent_id}-session"));
    member
}

/// Render one frame so the renderer records its hit rects, as a real session
/// does before any click can arrive.
fn render(app: &mut App) {
    let backend = TestBackend::new(170, 44);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("render frame");
}

/// Render one frame and return the buffer content of the region around the
/// confirmation dialog, so its painted title/body/buttons can be asserted.
fn render_dialog_text(app: &mut App) -> String {
    render(app);
    let area = app.row_action_confirm_area;
    let backend_area = Rect::new(0, 0, 170, 44);
    assert!(
        area.x > 0 && area.y > 0,
        "the dialog must be painted inside the dialog region (area {area:?})"
    );
    let _ = backend_area;
    text_of(app, area)
}

/// Flatten the given rect of the last render into text.
fn text_of(app: &mut App, area: Rect) -> String {
    let backend = TestBackend::new(170, 44);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("render frame");
    let buffer = terminal.backend().buffer().clone();
    let mut text = String::new();
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            if let Some(cell) = buffer.cell((x, y)) {
                text.push_str(cell.symbol());
            }
        }
        text.push('\n');
    }
    text
}

/// An app with one running sub-agent and the Agents window open.
fn app_with_task() -> App {
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.agent_name = "general".to_string();
    app.active_tasks
        .push(task("a1b2c3d4", "explore", TaskStatus::Running));
    app.show_agents_window = true;
    app.needs_redraw = false;
    app
}

/// An app with an active team of one working teammate and the Teams window
/// open.
fn app_with_team(app_status: MemberStatus) -> App {
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.agent_name = "general".to_string();
    app.active_team = Some(TeamConfig::new(TEAM_NAME, LEAD_SESSION));
    app.team_members
        .push(member("tm-001", "reviewer", app_status));
    app.show_teams_window = true;
    app.needs_redraw = false;
    app
}

// ---------------------------------------------------------------------------
// Arming and defaults
// ---------------------------------------------------------------------------

/// `open_row_action` arms the dialog with `No` selected and demands a redraw
/// (FR-069, NFR-005); the dialog alone dispatches nothing.
#[tokio::test]
async fn open_row_action_arms_the_dialog_with_no_selected() {
    let mut app = make_app();
    app.needs_redraw = false;

    app.open_row_action(RowAction::KillSubagent {
        task_id: "a1b2c3d4".to_string(),
        agent_name: "explore".to_string(),
    });

    assert!(
        app.row_action_confirm_open(),
        "the dialog must be armed by open_row_action"
    );
    assert!(
        !app.row_action_confirm_is_yes(),
        "No must be the default selection (FR-069)"
    );
    assert!(
        app.needs_redraw,
        "arming the dialog requests a redraw (NFR-005)"
    );
}

/// The dialog paints its action title, the target body and both button spans,
/// and caches the three areas for the mouse router (NFR-010).
#[tokio::test]
async fn render_paints_title_body_and_button_rects() {
    let mut app = app_with_task();
    app.open_row_action(RowAction::KillSubagent {
        task_id: "a1b2c3d4".to_string(),
        agent_name: "explore".to_string(),
    });

    let text = render_dialog_text(&mut app);
    assert!(
        text.contains("Kill sub-agent?"),
        "the dialog title names the action: {text}"
    );
    assert!(
        text.contains("explore [a1b2c3d4]"),
        "the dialog body names the target: {text}"
    );
    assert!(text.contains("[ Yes ]"), "the Yes span is painted: {text}");
    assert!(text.contains("[ No ]"), "the No span is painted: {text}");

    let yes = app.row_action_confirm_yes_area;
    let no = app.row_action_confirm_no_area;
    let dialog = app.row_action_confirm_area;
    assert!(
        dialog.contains(centre(yes).into()) && dialog.contains(centre(no).into()),
        "both button rects must sit inside the cached dialog area"
    );
    assert_ne!(yes, no, "the two button rects must be distinct hit targets");
    assert!(
        yes.x < no.x,
        "Yes paints left of No so the router halves stay ordered"
    );
}

// ---------------------------------------------------------------------------
// Keyboard answering (FR-069, NFR-005)
// ---------------------------------------------------------------------------

/// `Enter` on the default `No` dismisses the dialog and dispatches nothing.
#[tokio::test]
async fn enter_on_the_default_no_dismisses_without_dispatching() {
    let mut app = app_with_task();
    app.open_row_action(RowAction::KillSubagent {
        task_id: "a1b2c3d4".to_string(),
        agent_name: "explore".to_string(),
    });

    app.handle_key_event(key(KeyCode::Enter, KeyModifiers::NONE))
        .await;

    assert!(
        !app.row_action_confirm_open(),
        "Enter on the default No closes the dialog"
    );
    assert!(app.needs_redraw, "the closure requests a redraw (NFR-005)");
    assert!(
        app.active_tasks
            .iter()
            .any(|t| t.id == "a1b2c3d4" && t.status == TaskStatus::Running),
        "No must leave the task untouched (FR-069)"
    );
}

/// Toggle then `Enter` accepts: the dialog closes and the pending action is
/// dispatched.
#[tokio::test]
async fn enter_after_toggling_accepts_the_pending_action() {
    let mut app = app_with_task();
    app.open_row_action(RowAction::KillSubagent {
        task_id: "a1b2c3d4".to_string(),
        agent_name: "explore".to_string(),
    });

    // Left toggles No -> Yes (the queue-clear dialog's horizontal-move rule).
    app.handle_key_event(key(KeyCode::Left, KeyModifiers::NONE))
        .await;
    assert!(
        app.row_action_confirm_is_yes(),
        "Left moves the selection to Yes"
    );
    assert!(
        app.row_action_confirm_open(),
        "toggling keeps the dialog open"
    );

    app.handle_key_event(key(KeyCode::Enter, KeyModifiers::NONE))
        .await;
    assert!(!app.row_action_confirm_open(), "Yes closes the dialog");
}

/// `Esc` dismisses the dialog and dispatches nothing.
#[tokio::test]
async fn esc_dismisses_the_dialog() {
    let mut app = app_with_task();
    app.open_row_action(RowAction::SuspendTeammate {
        agent_id: "tm-001".to_string(),
        name: "reviewer".to_string(),
    });

    app.handle_key_event(key(KeyCode::Esc, KeyModifiers::NONE))
        .await;

    assert!(!app.row_action_confirm_open(), "Esc closes the dialog");
}

/// While the dialog is open every other key is swallowed: nothing reaches the
/// input buffer and the dialog stays armed.
#[tokio::test]
async fn other_keys_are_swallowed_while_the_dialog_is_open() {
    let mut app = app_with_task();
    app.input = "hello".to_string();
    app.input_cursor = 5;
    app.open_row_action(RowAction::KillSubagent {
        task_id: "a1b2c3d4".to_string(),
        agent_name: "explore".to_string(),
    });

    app.handle_key_event(key(KeyCode::Char('q'), KeyModifiers::NONE))
        .await;
    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;
    app.handle_key_event(key(KeyCode::Char('k'), KeyModifiers::SHIFT))
        .await;

    assert!(
        app.row_action_confirm_open(),
        "an unrelated key must not dismiss the dialog"
    );
    assert_eq!(
        app.input, "hello",
        "modal keys must not leak into the input buffer (FR-055 style gate)"
    );
}

// ---------------------------------------------------------------------------
// Mouse answering (T-009: "with keyboard and mouse answering")
// ---------------------------------------------------------------------------

/// A left-click on the painted `[ No ]` span dismisses the dialog without
/// dispatching (FR-069), and a click inside the dialog area that carries no
/// button is consumed without mutating the surface beneath (FR-070).
#[tokio::test]
async fn mouse_click_on_no_dismisses_without_dispatching() {
    let mut app = app_with_task();
    app.open_row_action(RowAction::KillSubagent {
        task_id: "a1b2c3d4".to_string(),
        agent_name: "explore".to_string(),
    });
    render(&mut app);
    let (col, row) = centre(app.row_action_confirm_no_area);
    app.needs_redraw = false;

    app.handle_mouse_event(click(col, row));

    assert!(
        !app.row_action_confirm_open(),
        "a No click closes the dialog"
    );
    assert!(app.needs_redraw, "answering the dialog paints the change");
    assert!(
        app.active_tasks
            .iter()
            .any(|t| t.id == "a1b2c3d4" && t.status == TaskStatus::Running),
        "No must leave the task untouched (FR-069)"
    );
}

/// A left-click on the painted `[ Yes ]` span accepts and dispatches.
#[tokio::test]
async fn mouse_click_on_yes_accepts() {
    let mut app = app_with_task();
    app.open_row_action(RowAction::KillSubagent {
        task_id: "a1b2c3d4".to_string(),
        agent_name: "explore".to_string(),
    });
    render(&mut app);
    let (col, row) = centre(app.row_action_confirm_yes_area);

    app.handle_mouse_event(click(col, row));

    assert!(
        !app.row_action_confirm_open(),
        "a Yes click closes the dialog"
    );
}

/// With the dialog open a click that lands outside it is swallowed so nothing
/// on the surface beneath it reacts (FR-070: the modal owns the mouse).
#[tokio::test]
async fn mouse_click_outside_the_dialog_is_swallowed() {
    let mut app = app_with_team(MemberStatus::Working);
    app.open_row_action(RowAction::ShutdownTeammate {
        agent_id: "tm-001".to_string(),
        name: "reviewer".to_string(),
    });
    render(&mut app);
    let area = app.row_action_confirm_area;
    app.status = "sentinel".to_string();
    app.needs_redraw = false;

    // The input-area left edge is far from the centred dialog at a
    // 120-column terminal.
    app.handle_mouse_event(click(2, 43));

    assert!(
        app.row_action_confirm_open(),
        "a click outside the dialog must not alter it"
    );
    assert!(
        !app.needs_redraw,
        "a swallowed click paints nothing (area outside {area:?})"
    );
    assert_eq!(app.status, "sentinel", "nothing beneath reacts (FR-070)");
}

// ---------------------------------------------------------------------------
// Destructive trigger points route through the dialog
// ---------------------------------------------------------------------------

/// `Shift+K` on the cursor-selected sub-agent row opens the dialog naming the
/// task; key #2 is consumed by the dialog so no second kill can fire behind it
/// (FR-036, FR-069). (Old selected_agent_index trigger retired.)
#[tokio::test]
async fn shift_k_on_a_selected_agent_row_opens_the_kill_dialog() {
    let mut app = app_with_task();
    render(&mut app);
    app.agents_cursor_subagent = Some(0); // the only sub-agent row

    app.handle_key_event(key(KeyCode::Char('K'), KeyModifiers::SHIFT))
        .await;

    assert!(
        app.row_action_confirm_open(),
        "Shift+K confirms the kill first"
    );
    match app.row_action_confirm.as_ref() {
        Some(pending) => match &pending.action {
            RowAction::KillSubagent {
                task_id,
                agent_name,
            } => {
                assert_eq!(task_id, "a1b2c3d4", "the dialog names the task");
                assert_eq!(agent_name, "explore", "the dialog names the agent");
            }
            other => panic!("expected KillSubagent, got {other:?}"),
        },
        None => unreachable!(),
    }
    assert!(
        app.needs_redraw,
        "arming the dialog requests a redraw (NFR-005)"
    );
}

/// The keyboard cursor cannot reach the primary row at all: it never lands
/// below the sub-agent list's bottom, never above its first row, so the old
/// "primary refusal" case is retired in favour of the range invariant (FR-067).
#[tokio::test]
async fn the_agents_cursor_range_covers_subagent_rows_only() {
    let mut app = app_with_task();
    render(&mut app);

    // Down from no selection starts on the first sub-agent row.
    app.move_agents_cursor(1);
    assert_eq!(app.agents_cursor_subagent, Some(0));
    // The cursor ordinal 0 names the sub-agent, never the primary/lead (FR-067).
    assert_eq!(
        app.agents_subagent_task_ids.first().map(String::as_str),
        Some("a1b2c3d4"),
        "the first cursor row must be a sub-agent (FR-067)"
    );
}

/// Arming the background-shell cancel directly through `open_row_action` (the
/// keyboard cursor never covers background rows, so the trigger is
/// service-level; the confirm flow itself is unchanged, FR-037).
#[tokio::test]
async fn background_cancel_opens_the_cancel_dialog() {
    let mut app = app_with_task();
    app.active_tasks.clear();
    app.bg_tasks = vec![bg_task("cc112233", "sleep 30")];
    app.needs_redraw = false;
    app.open_row_action(RowAction::CancelBackground {
        task_id: "cc112233".to_string(),
        command: "sleep 30".to_string(),
    });

    assert!(
        app.row_action_confirm_open(),
        "the background cancel confirms first"
    );
    match app.row_action_confirm.as_ref().map(|p| &p.action) {
        Some(RowAction::CancelBackground { task_id, command }) => {
            assert_eq!(task_id, "cc112233");
            assert_eq!(command, "sleep 30");
        }
        other => panic!("expected CancelBackground, got {other:?}"),
    }
    assert!(app.needs_redraw, "arming the dialog requests a redraw");
}

/// Space on the cursor-selected working teammate opens the suspend dialog
/// naming the member (FR-040's suspend direction confirms; the old row click
/// trigger is retired).
#[tokio::test]
async fn teammate_space_on_a_working_member_opens_the_suspend_dialog() {
    let mut app = app_with_team(MemberStatus::Working);
    render(&mut app);
    // The cursor list covers the painted teammate rows; ordinal 0 = tm-001.
    app.move_teams_cursor(1);
    assert_eq!(app.teams_cursor_teammate, Some(0));
    app.needs_redraw = false;

    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;

    assert!(
        app.row_action_confirm_open(),
        "teammate suspend confirms first"
    );
    match app.row_action_confirm.as_ref().map(|p| &p.action) {
        Some(RowAction::SuspendTeammate { agent_id, name }) => {
            assert_eq!(agent_id, "tm-001");
            assert_eq!(name, "reviewer");
        }
        other => panic!("expected SuspendTeammate, got {other:?}"),
    }
    assert!(app.needs_redraw, "arming the dialog requests a redraw");
}

/// Space on an ALREADY SUSPENDED teammate resumes it directly: no dialog opens
/// (FR-040 confirms only the suspend direction; FR-035 rule).
#[tokio::test]
async fn teammate_space_on_a_suspended_member_resumes_without_a_dialog() {
    let mut app = app_with_team(MemberStatus::Suspended);
    render(&mut app);
    app.move_teams_cursor(1);
    app.needs_redraw = false;

    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;

    assert!(
        !app.row_action_confirm_open(),
        "a resume must not open the confirmation dialog"
    );
    assert!(
        app.needs_redraw,
        "the resume request line repaints the log panel (NFR-005)"
    );
}

/// `Del` on the cursor-selected teammate row opens the shutdown dialog
/// (FR-040).
#[tokio::test]
async fn teammate_del_opens_the_shutdown_dialog() {
    let mut app = app_with_team(MemberStatus::Working);
    render(&mut app);
    app.move_teams_cursor(1);
    app.needs_redraw = false;

    app.handle_key_event(key(KeyCode::Delete, KeyModifiers::NONE))
        .await;

    assert!(
        app.row_action_confirm_open(),
        "teammate shutdown confirms first"
    );
    match app.row_action_confirm.as_ref().map(|p| &p.action) {
        Some(RowAction::ShutdownTeammate { agent_id, name }) => {
            assert_eq!(agent_id, "tm-001");
            assert_eq!(name, "reviewer");
        }
        other => panic!("expected ShutdownTeammate, got {other:?}"),
    }
    assert!(app.needs_redraw, "arming the dialog requests a redraw");
}

/// `Space` and `Shift+K` on the cursor-selected teammate route through the
/// suspend direction (Space confirms only the suspend; the old
/// focused_teammate Space shutdown case is retired - the cursor model confirms
/// the destructive direction via `Del`, FR-045's re-expression).
#[tokio::test]
async fn teammate_keyboard_keys_consume_the_key_either_way() {
    let mut app = app_with_team(MemberStatus::Working);
    render(&mut app);
    app.move_teams_cursor(1);

    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;
    assert!(app.row_action_confirm_open(), "Space confirms the suspend");
    match app.row_action_confirm.as_ref().map(|p| &p.action) {
        Some(RowAction::SuspendTeammate { agent_id, name }) => {
            assert_eq!(agent_id, "tm-001");
            assert_eq!(name, "reviewer");
        }
        other => panic!("expected SuspendTeammate from Space, got {other:?}"),
    }

    // The dialog consumes the follow-up key so the destructive key cannot
    // re-arm or fire behind it.
    app.handle_key_event(key(KeyCode::Char('K'), KeyModifiers::SHIFT))
        .await;
    assert!(
        app.row_action_confirm_open(),
        "the dialog stays open while it is being answered"
    );
    assert_eq!(
        app.input, "",
        "the consumed key must not leak into the input buffer"
    );
}

/// `Space` on the cursor-selected RUNNING sub-agent row suspends it without a
/// dialog: the suspend direction is not destructive (FR-035 parity with the
/// mouse; the old selected_agent_index trigger is retired).
#[tokio::test]
async fn space_on_a_running_agent_row_suspends_without_a_dialog() {
    let mut app = app_with_task();
    app.move_agents_cursor(1);

    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;

    assert!(
        !app.row_action_confirm_open(),
        "a non-destructive suspend opens no dialog"
    );
}

// ---------------------------------------------------------------------------
// Stale-row answers dispatch nothing (FR-069)
// ---------------------------------------------------------------------------

/// Answering `Yes` after the row vanished closes the dialog and mutates
/// nothing (FR-069).
#[tokio::test]
async fn answering_yes_after_the_row_vanishes_mutates_nothing() {
    let mut app = app_with_task();
    app.open_row_action(RowAction::KillSubagent {
        task_id: "a1b2c3d4".to_string(),
        agent_name: "explore".to_string(),
    });
    // The subject disappears between open and answer.
    app.active_tasks.clear();
    app.status = "sentinel".to_string();

    // Toggle to Yes and confirm.
    app.handle_key_event(key(KeyCode::Left, KeyModifiers::NONE))
        .await;
    app.handle_key_event(key(KeyCode::Enter, KeyModifiers::NONE))
        .await;

    assert!(!app.row_action_confirm_open());
    assert_eq!(
        app.status, "sentinel",
        "a stale answer must not touch the status line or any list (FR-069)"
    );
}

/// The dialog closes on `Yes` even when the target transitioned to a
/// background shell task identity; the same cancel service runs either way.
#[tokio::test]
async fn the_kill_answer_follows_a_row_that_became_a_background_task() {
    let mut app = app_with_task();
    app.open_row_action(RowAction::KillSubagent {
        task_id: "a1b2c3d4".to_string(),
        agent_name: "explore".to_string(),
    });
    // The task registry replaced the agent task with a bg task of the same id.
    app.active_tasks.clear();
    app.bg_tasks = vec![bg_task("a1b2c3d4", "sleep 30")];

    app.handle_key_event(key(KeyCode::Left, KeyModifiers::NONE))
        .await;
    app.handle_key_event(key(KeyCode::Enter, KeyModifiers::NONE))
        .await;

    assert!(!app.row_action_confirm_open(), "the answer still resolves");
}

// ---------------------------------------------------------------------------
// FR-028 / FR-032 - the dialog states which option performs and which cancels
// ---------------------------------------------------------------------------

/// The row-action dialog's footer carries both hint lines: the toggle keys and
/// which option performs versus cancels (spec `teamssetup` FR-028, FR-032), so
/// `Space` `Enter` on the pre-selected `No` can never silently cancel an action
/// the operator believed they had confirmed.
#[tokio::test]
async fn row_action_dialog_footer_states_the_toggle_and_the_outcomes() {
    let mut app = app_with_task();
    app.open_row_action(RowAction::KillSubagent {
        task_id: "a1b2c3d4".to_string(),
        agent_name: "explore".to_string(),
    });

    let text = render_dialog_text(&mut app);
    assert!(
        text.contains("toggle the selection"),
        "the footer names the toggle, so an un-toggled Enter is not a silent no-op: {text}"
    );
    assert!(
        text.contains("Enter on Yes performs"),
        "the footer says the Yes option performs the action: {text}"
    );
    assert!(
        text.contains("Enter on No cancels"),
        "the footer says the default No option cancels the action: {text}"
    );
    assert!(
        text.contains("Esc cancels"),
        "the footer names the escape route: {text}"
    );
    // The full hints fit inside the painted dialog, so nothing is clipped.
    let inner_w = app.row_action_confirm_area.width.saturating_sub(2);
    for hint in [
        ragent_tui::app::ROW_ACTION_TOGGLE_HINT,
        ragent_tui::app::ROW_ACTION_OUTCOME_HINT,
    ] {
        assert!(
            hint.chars().count() as u16 <= inner_w,
            "hint {hint:?} must fit the {inner_w}-column inner width"
        );
    }
}

/// Every row verb's dialog carries the same footer: the hints belong to the
/// modal shape, not to one action (spec `teamssetup` FR-028, FR-032).
#[tokio::test]
async fn every_row_action_dialog_carries_the_footer_hints() {
    let actions = [
        RowAction::KillSubagent {
            task_id: "a1b2c3d4".to_string(),
            agent_name: "explore".to_string(),
        },
        RowAction::CancelBackground {
            task_id: "bg-1".to_string(),
            command: "sleep 30".to_string(),
        },
        RowAction::SuspendTeammate {
            agent_id: "tm-001".to_string(),
            name: "reviewer".to_string(),
        },
        RowAction::ShutdownTeammate {
            agent_id: "tm-001".to_string(),
            name: "reviewer".to_string(),
        },
        RowAction::ResumeTeammate {
            agent_id: "tm-001".to_string(),
            name: "reviewer".to_string(),
        },
    ];

    for action in actions {
        let mut app = app_with_task();
        app.open_row_action(action.clone());
        let text = render_dialog_text(&mut app);
        assert!(
            text.contains(ragent_tui::app::ROW_ACTION_TOGGLE_HINT),
            "{action:?} dialog must state the toggle hint: {text}"
        );
        assert!(
            text.contains(ragent_tui::app::ROW_ACTION_OUTCOME_HINT),
            "{action:?} dialog must state which option performs and which cancels: {text}"
        );
    }
}

/// The shared `Yes`/`No` dialogs (queue clear, memory clear) state the same
/// contract: `Enter` on `Yes` performs, `Enter` on `No` cancels, `Esc` cancels.
/// `No` is pre-selected, so an un-toggled `Enter` must read as a cancel.
#[tokio::test]
async fn shared_yes_no_dialog_footer_states_the_outcomes() {
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.input_queue.push_back(ragent_tui::app::QueuedInput {
        text: "queued".to_string(),
        image_paths: Vec::new(),
    });
    app.queue_menu_open = true;
    app.queue_menu_select_clear();
    assert!(
        app.queue_clear_confirm_open,
        "precondition: selecting Clear on a non-empty queue arms the dialog"
    );

    // Paint the dialog first: its cached area is only known after a render.
    render(&mut app);
    let area = app.queue_clear_confirm_area;
    assert!(
        area.width > 0 && area.height > 0,
        "the shared dialog must be painted (area {area:?})"
    );
    let text = text_of(&mut app, area);
    assert!(
        text.contains("Enter on Yes performs"),
        "the shared dialog's footer says Yes performs the action: {text}"
    );
    assert!(
        text.contains("Enter on No cancels"),
        "the shared dialog's footer says the default No cancels: {text}"
    );
    assert!(
        text.contains("Esc cancels"),
        "the shared dialog's footer names the escape route: {text}"
    );
    assert!(
        !text.contains("Enter select"),
        "the old footer left the outcome unstated: {text}"
    );
}
