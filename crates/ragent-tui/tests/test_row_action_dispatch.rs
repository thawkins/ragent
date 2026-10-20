//! Tests for the row action dispatch layer (spec `agentui` T-010).
//!
//! `test_row_action_confirm.rs` pins the yes/no dialog and the trigger-point
//! routing; this file pins what the confirmed (or non-destructive) action
//! actually dispatches:
//!
//! - FR-035: a `[stop]` click on a running sub-agent row requests
//!   suspension, on a suspended row requests resumption - with no dialog in
//!   either direction;
//! - FR-036: the confirmed `[x]` answer terminates the task through the real
//!   `AgentManager` (the registry entry moves to `Terminating`);
//! - FR-037: the confirmed background-shell answer cancels the task through
//!   the real `BackgroundTaskService` (the service publishes the `cancelled`
//!   status event);
//! - FR-040: the confirmed teammate answers suspend/shutdown through the
//!   real `TeamManager` (the on-disk team store shows the new status) and the
//!   resume direction restores an already-suspended member without a dialog;
//! - FR-068: every dispatch is spawned on the async runtime and returns
//!   control to the caller immediately (the tests drive `handle_mouse_event`
//!   on the render thread and still pump afterwards);
//! - NFR-008: every panel-triggered request is logged as a panel line
//!   attributed to the affected child or teammate session id;
//! - FR-069: a confirmed answer whose row vanished dispatches nothing.
//!
//! Real managers are bound to the test `App` (`AgentManager`,
//! `BackgroundTaskService`, `TeamManager`) so the dispatch is observed
//! end-to-end rather than through mocks. Every scratch artefact lives under
//! the git-ignored `target/temp/agentui-t010` tree (NFR-012).

#[path = "support/mod.rs"]
mod support;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{Terminal, backend::TestBackend};

use ragent_agent::background::BackgroundTaskService;
use ragent_agent::storage::Storage;
use ragent_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use ragent_agent::team::{MemberStatus, TeamConfig, TeamMember, TeamStore};
use ragent_tui::App;
use support::make_app;

/// The lead session id every case uses.
const LEAD_SESSION: &str = "lead-session-t010";

/// A team name the on-disk stores use; the scratch keeps it isolated.
const TEAM_NAME: &str = "row-action-dispatch";

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
fn centre(rect: ratatui::layout::Rect) -> (u16, u16) {
    (
        rect.x + rect.width / 2,
        rect.y + rect.height.saturating_sub(1) / 2,
    )
}

/// A left-button press at the centre of a recorded hit rect.
fn click_at(pos: (u16, u16)) -> MouseEvent {
    click(pos.0, pos.1)
}

/// A plain key press (`test_row_action_confirm.rs` shape).
fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

/// A live sub-agent task (`test_row_action_confirm.rs` shape).
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
    }
}

/// A background shell task view.
fn bg_task_view(id: &str, command: &str) -> ragent_tui::app::BgTaskView {
    ragent_tui::app::BgTaskView {
        id: id.to_string(),
        session_id: LEAD_SESSION.to_string(),
        command: command.to_string(),
        status: "running".to_string(),
        created_at: chrono::Utc::now(),
        completed_at: None,
    }
}

/// A team member with a stable synthetic session id.
fn member(agent_id: &str, name: &str, status: MemberStatus) -> TeamMember {
    let mut member = TeamMember::new(name, agent_id, "general");
    member.status = status;
    member.session_id = Some(format!("{agent_id}-session"));
    member
}

/// A unique scratch directory under `target/temp/agentui-t010` carrying its
/// own `.ragent/teams` so the team store resolves inside the scratch, never
/// in the repository (NFR-012).
fn scratch_dir(tag: &str) -> PathBuf {
    let unique = format!(
        "{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/agentui-t010")
        .join(unique);
    std::fs::create_dir_all(dir.join(".ragent/teams")).expect("scratch dir");
    dir
}

/// Render one frame so the renderer records its hit rects.
fn render(app: &mut App) {
    let backend = TestBackend::new(170, 44);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("render frame");
}

/// Pump the async runtime so the spawned dispatch futures run, up to about
/// two seconds, stopping as soon as `done` observes its condition.
async fn pump_until(mut done: impl FnMut() -> bool) {
    for _ in 0..200 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("the awaited dispatch condition never became true");
}

/// Whether the TUI log panel carries the attributed request line for a
/// sub-agent action (NFR-008): the entry must be stamped with the child
/// session id and name the task.
fn log_has_attribution(app: &App, session_id: &str, needle: &str) -> bool {
    app.log_entries
        .iter()
        .any(|e| e.session_id.as_deref() == Some(session_id) && e.message.contains(needle))
}

/// An Agents-window app with one live row per subject.
fn app_with_agents_window() -> App {
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.agent_name = "general".to_string();
    app.show_agents_window = true;
    app.needs_redraw = false;
    app
}

/// A Teams-window app over the given member statuses.
fn app_with_teams_window(status: MemberStatus) -> App {
    let mut app = make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.agent_name = "general".to_string();
    app.active_team = Some(TeamConfig::new(TEAM_NAME, LEAD_SESSION));
    app.team_members.push(member("tm-001", "reviewer", status));
    app.show_teams_window = true;
    app.needs_redraw = false;
    app
}

// ---------------------------------------------------------------------------
// FR-035: the [stop] toggle on a sub-agent row
// ---------------------------------------------------------------------------

/// Space on the cursor-selected running sub-agent row requests suspension
/// without a dialog, and the request is logged attributed to the child session
/// (NFR-008). The runtime-bound `AgentManager` accepts the request
/// asynchronously; the row stays painted until the lifecycle event or the
/// reconciliation pass converges it (FR-054). (Old [stop]-click trigger
/// retired with the row mouse.)
#[tokio::test]
async fn space_on_a_running_subagent_requests_suspension() {
    let mut app = app_with_agents_window();
    app.active_tasks
        .push(task("a1b2c3d4", "explore", TaskStatus::Running));
    let manager = Arc::new(ragent_agent::task::AgentManager::new(
        app.event_bus.clone(),
        app.session_processor.clone(),
        4,
        300,
    ));
    manager
        .seed_completed_for_test(task("a1b2c3d4", "explore", TaskStatus::Running))
        .await;
    let _ = app
        .session_processor
        .agent_manager
        .set(Arc::clone(&manager));
    render(&mut app);
    app.needs_redraw = false;

    app.move_agents_cursor(1);
    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;

    assert!(
        !app.row_action_confirm_open(),
        "the suspend direction is not destructive and opens no dialog (FR-035)"
    );
    assert!(
        log_has_attribution(
            &app,
            "a1b2c3d4-session",
            "Suspend requested for task a1b2c3d4"
        ),
        "the request must be attributed to the child session (NFR-008)"
    );
    assert!(
        app.needs_redraw,
        "logging the request repaints the log panel (NFR-005)"
    );
}

/// Space on the cursor-selected suspended sub-agent row requests resumption
/// instead (FR-035's other direction), again without a dialog.
#[tokio::test]
async fn space_on_a_suspended_subagent_requests_resumption() {
    let mut app = app_with_agents_window();
    app.active_tasks
        .push(task("b2c3d4e5", "explore", TaskStatus::Suspended));
    render(&mut app);
    app.needs_redraw = false;

    app.move_agents_cursor(1);
    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;

    assert!(!app.row_action_confirm_open(), "resume opens no dialog");
    assert!(
        log_has_attribution(
            &app,
            "b2c3d4e5-session",
            "Resume requested for task b2c3d4e5"
        ),
        "the resume request must be attributed to the child session (NFR-008)"
    );
}

// ---------------------------------------------------------------------------
// FR-036: the confirmed kill dispatches through the real manager
// ---------------------------------------------------------------------------

/// The confirmed `[x]` answer terminates the task through the real
/// `AgentManager`: the registry entry moves to `Terminating`, the dialog
/// closes and the request line is attributed (NFR-008). The dispatch is
/// spawned on the async runtime and the render-thread call returned
/// immediately (FR-068) - the effect is only visible after pumping.
#[tokio::test]
async fn kill_answer_terminates_the_task_through_the_manager() {
    let mut app = app_with_agents_window();
    app.active_tasks
        .push(task("c3d4e5f6", "rust-coder", TaskStatus::Running));
    let manager = Arc::new(ragent_agent::task::AgentManager::new(
        app.event_bus.clone(),
        app.session_processor.clone(),
        4,
        300,
    ));
    manager
        .seed_completed_for_test(task("c3d4e5f6", "rust-coder", TaskStatus::Running))
        .await;
    let _ = app
        .session_processor
        .agent_manager
        .set(Arc::clone(&manager));
    render(&mut app);

    // Arm the dialog from the keyboard cursor (Del) and answer Yes by mouse.
    app.move_agents_cursor(1);
    app.handle_key_event(key(KeyCode::Delete, KeyModifiers::NONE))
        .await;
    assert!(app.row_action_confirm_open(), "the kill confirms first");
    render(&mut app);
    app.handle_mouse_event(click_at(centre(app.row_action_confirm_yes_area)));
    assert!(!app.row_action_confirm_open(), "Yes closes the dialog");
    assert!(
        log_has_attribution(&app, "c3d4e5f6-session", "Kill requested for task c3d4e5f6"),
        "the kill request must be attributed to the child session (NFR-008)"
    );

    // Pump until the spawned kill reaches the manager's registry.
    for _ in 0..200 {
        match manager.get_task("c3d4e5f6").await {
            Some(entry) if entry.status == TaskStatus::Terminating => break,
            _ => tokio::time::sleep(Duration::from_millis(10)).await,
        }
    }
    let entry = manager
        .get_task("c3d4e5f6")
        .await
        .expect("the seeded task entry");
    assert_eq!(
        entry.status,
        TaskStatus::Terminating,
        "the confirmed answer must drive the real manager kill path (FR-036)"
    );
}

// ---------------------------------------------------------------------------
// FR-037: the confirmed cancel dispatches through the real service
// ---------------------------------------------------------------------------

/// The confirmed background-shell cancel answer cancels the task through
/// the real `BackgroundTaskService`: a real `sleep` process is spawned, then
/// cancelled, and the service publishes the `cancelled` status event. The
/// cancellation is requested attributed to the row's session (NFR-008).
///
/// The trigger is armed directly through `open_row_action`: the keyboard
/// cursor never covers background rows (they are service-terminated, not
/// cursorable), so the old bg `[stop]`-click trigger is retired; the dialog
/// and the dispatch are unchanged (FR-037).
#[tokio::test]
async fn background_cancel_answer_cancels_through_the_service() {
    let mut app = app_with_agents_window();
    let workdir = scratch_dir("bg");
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    storage
        .create_session(LEAD_SESSION, ".")
        .expect("seed the lead session row");
    let bg_service = Arc::new(BackgroundTaskService::new(storage, app.event_bus.clone()));
    let _ = app.session_processor.bg_service.set(bg_service.clone());
    let task_id = bg_service
        .spawn(LEAD_SESSION, "sleep 30", &workdir)
        .await
        .expect("spawn background sleep");
    app.bg_tasks.push(bg_task_view(&task_id, "sleep 30"));

    // Subscribe before the dispatch so the service's status event is captured.
    let mut rx = app.event_bus.subscribe();
    render(&mut app);
    app.open_row_action(ragent_tui::app::RowAction::CancelBackground {
        task_id: task_id.clone(),
        command: "sleep 30".to_string(),
    });
    assert!(
        app.row_action_confirm_open(),
        "the background cancel confirms first (FR-037)"
    );
    render(&mut app);
    app.handle_mouse_event(click_at(centre(app.row_action_confirm_yes_area)));
    assert!(!app.row_action_confirm_open(), "Yes closes the dialog");
    assert!(
        log_has_attribution(&app, LEAD_SESSION, "requested for background task"),
        "the cancel request must be attributed to the row session (NFR-008)"
    );

    // Pump until the service publishes the cancelled status.
    pump_until(|| {
        while let Ok(event) = rx.try_recv() {
            if let ragent_agent::event::Event::BackgroundTaskUpdated {
                task_id: ref id,
                ref status,
                ..
            } = event
            {
                if id == &task_id && status == "cancelled" {
                    return true;
                }
            }
        }
        false
    })
    .await;

    let _ = std::fs::remove_dir_all(&workdir);
}

// ---------------------------------------------------------------------------
// FR-040: the teammate controls dispatch through the real manager
// ---------------------------------------------------------------------------

/// The on-disk status of `tm-001` (`None` when the store or member is gone).
fn stored_status(store_dir: &PathBuf) -> Option<MemberStatus> {
    TeamStore::load(store_dir)
        .ok()?
        .config
        .member_by_id("tm-001")
        .map(|m| m.status)
}

/// Bind a real `TeamManager` over a real store in the scratch and register
/// `tm-001` in it; returns the store dir for later reloads.
fn bind_team_manager(app: &mut App, status: MemberStatus) -> PathBuf {
    let workdir = scratch_dir("team");
    let store =
        TeamStore::create(TEAM_NAME, LEAD_SESSION, &workdir, true).expect("create team store");
    {
        let mut config = store.config.clone();
        let mut store_member = TeamMember::new("reviewer", "tm-001", "general");
        store_member.status = status;
        store_member.session_id = Some("tm-001-session".to_string());
        config.members.push(store_member);
        TeamStore {
            dir: store.dir.clone(),
            config,
        }
        .save()
        .expect("save store member");
    }
    let manager = Arc::new(ragent_agent::team::TeamManager::new(
        TEAM_NAME,
        LEAD_SESSION,
        store.dir.clone(),
        app.session_processor.clone(),
        app.event_bus.clone(),
    ));
    let _ = app.session_processor.team_manager.set(manager);
    store.dir
}

/// Space on the cursor-selected teammate suspends the member through the
/// real `TeamManager`: the on-disk store shows `Suspended` and the request
/// line is attributed to the member session (NFR-008). (Old [stop]-click
/// trigger retired with the row mouse.)
#[tokio::test]
async fn teammate_space_answer_suspends_through_the_manager() {
    let mut app = app_with_teams_window(MemberStatus::Working);
    let store_dir = bind_team_manager(&mut app, MemberStatus::Working);
    render(&mut app);

    app.move_teams_cursor(1);
    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;
    assert!(
        app.row_action_confirm_open(),
        "suspending a live teammate confirms first (FR-040)"
    );
    render(&mut app);
    app.handle_mouse_event(click_at(centre(app.row_action_confirm_yes_area)));
    assert!(!app.row_action_confirm_open(), "Yes closes the dialog");
    assert!(
        log_has_attribution(&app, "tm-001-session", "tm-001 suspend requested"),
        "the suspend request must be attributed to the member session (NFR-008)"
    );

    pump_until(|| stored_status(&store_dir) == Some(MemberStatus::Suspended)).await;
    assert_eq!(
        stored_status(&store_dir),
        Some(MemberStatus::Suspended),
        "the confirmed answer must drive the real manager suspend path (FR-040)"
    );
    // Cleanup removes the whole scratch root (the store dir sits three
    // levels below it: <scratch>/.ragent/teams/<team>).
    let scratch = store_dir.ancestors().nth(3).expect("scratch root");
    let _ = std::fs::remove_dir_all(scratch);
}

/// Space on the cursor-selected already-suspended teammate resumes it directly
/// - no dialog, no confirmation - and the store returns to `Idle` (FR-040).
#[tokio::test]
async fn teammate_space_on_a_suspended_row_resumes_without_a_dialog() {
    let mut app = app_with_teams_window(MemberStatus::Suspended);
    let store_dir = bind_team_manager(&mut app, MemberStatus::Suspended);
    render(&mut app);

    app.move_teams_cursor(1);
    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;
    assert!(
        !app.row_action_confirm_open(),
        "the resume direction opens no dialog (FR-040)"
    );
    assert!(
        log_has_attribution(&app, "tm-001-session", "tm-001 resume requested"),
        "the resume request must be attributed to the member session (NFR-008)"
    );

    pump_until(|| stored_status(&store_dir) == Some(MemberStatus::Idle)).await;
    // Cleanup removes the whole scratch root (the store dir sits three
    // levels below it: <scratch>/.ragent/teams/<team>).
    let scratch = store_dir.ancestors().nth(3).expect("scratch root");
    let _ = std::fs::remove_dir_all(scratch);
}

/// The confirmed teammate `Del` answer requests graceful shutdown through the
/// real `TeamManager`: the store shows `ShuttingDown` (the teammate is
/// expected to ack) and the request line is attributed (NFR-008). (Old [x]-
/// click trigger retired with the row mouse.)
#[tokio::test]
async fn teammate_del_answer_requests_graceful_shutdown() {
    let mut app = app_with_teams_window(MemberStatus::Working);
    let store_dir = bind_team_manager(&mut app, MemberStatus::Working);
    render(&mut app);

    app.move_teams_cursor(1);
    app.handle_key_event(key(KeyCode::Delete, KeyModifiers::NONE))
        .await;
    assert!(
        app.row_action_confirm_open(),
        "shutting a teammate down confirms first (FR-040)"
    );
    render(&mut app);
    app.handle_mouse_event(click_at(centre(app.row_action_confirm_yes_area)));
    assert!(!app.row_action_confirm_open(), "Yes closes the dialog");
    assert!(
        log_has_attribution(&app, "tm-001-session", "tm-001 shutdown requested"),
        "the shutdown request must be attributed to the member session (NFR-008)"
    );

    pump_until(|| stored_status(&store_dir) == Some(MemberStatus::ShuttingDown)).await;
    assert_eq!(
        stored_status(&store_dir),
        Some(MemberStatus::ShuttingDown),
        "graceful shutdown marks the member ShuttingDown on disk (FR-040)"
    );
    // Cleanup removes the whole scratch root (the store dir sits three
    // levels below it: <scratch>/.ragent/teams/<team>).
    let scratch = store_dir.ancestors().nth(3).expect("scratch root");
    let _ = std::fs::remove_dir_all(scratch);
}

// ---------------------------------------------------------------------------
// FR-069: a confirmed answer with a vanished subject dispatches nothing
// ---------------------------------------------------------------------------

/// Answering `Yes` after the teammate row vanished closes the dialog and
/// records no request line at all (FR-069): the stale-row re-check in the
/// accept path drops the action before any dispatch.
#[tokio::test]
async fn stale_teammate_answer_dispatches_nothing() {
    let mut app = app_with_teams_window(MemberStatus::Working);
    // Arm the dialog directly (the trigger points are T-009's concern).
    app.open_row_action(ragent_tui::app::RowAction::SuspendTeammate {
        agent_id: "tm-001".to_string(),
        name: "reviewer".to_string(),
    });
    let baseline = app.log_entries.len();
    // The subject disappears between open and answer.
    app.team_members.clear();

    app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE))
        .await;
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .await;

    assert!(
        !app.row_action_confirm_open(),
        "the answer closes the dialog"
    );
    assert_eq!(
        app.log_entries.len(),
        baseline,
        "a stale answer must not log any request (FR-069)"
    );
    assert!(
        !app.log_entries.iter().any(|e| e.message.contains("tm-001")),
        "no request line may name the vanished teammate (FR-069)"
    );
}
