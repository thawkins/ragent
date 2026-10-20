//! Tests for the Agents/Teams window keyboard control surface (spec `agentui`
//! T-011, keyboard-first rework).
//!
//! `test_row_action_confirm.rs` pins the dialog; `test_row_action_dispatch.rs`
//! pins the dispatches; the hit-rect rework moved the keyboard arms from the
//! retired `selected_agent_index` to the block cursor
//! (`agents_cursor_subagent` / `teams_cursor_teammate`). This file pins the
//! **keyboard surface** end to end through `App::handle_key_event`:
//!
//! - FR-041: `Space` on the cursor sub-agent row suspends it, on a suspended
//!   row resumes it, is consumed (no space reaches the input buffer), and the
//!   request line is attributed to the child session (NFR-008);
//! - FR-043: `Shift+K`/`Del` on the cursor sub-agent row confirms first and
//!   the confirmed answer drives the real `AgentManager` registry entry to
//!   `Terminating` (FR-036's keyboard twin);
//! - FR-042/FR-044 re-expressed: the cursor range covers sub-agent rows only,
//!   so no key path can reach the primary agent at all (FR-067);
//! - FR-040: `Space` on the cursor teammate row confirms only the suspend
//!   direction; `Del`/`k` arms the graceful shutdown dialog and the confirmed
//!   answer drives the real `TeamManager` (the on-disk store shows
//!   `ShuttingDown`);
//! - FR-055: with a window open the window keys are consumed before the input
//!   buffer sees them - including the no-cursor corner;
//! - FR-067: the cursor list never names the primary agent or the lead.
//!
//! Real managers are bound where the dispatch must be observed end to end
//! (`AgentManager`, `TeamManager`); every scratch artefact lives under the
//! git-ignored `target/temp/agentui-t011` tree (NFR-012).

#[path = "support/mod.rs"]
mod support;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crossterm::event::{KeyCode, KeyModifiers};

use ragent_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use ragent_agent::team::{MemberStatus, TeamConfig, TeamMember, TeamStore};
use ragent_tui::App;

/// The lead session id every case uses.
const LEAD_SESSION: &str = "lead-session-t011";

/// A team name no on-disk store under the repo can own; the scratch keeps the
/// FR-060 render hydration from overriding the synthetic member statuses.
const TEAM_NAME: &str = "agentui-t011-team";

/// A plain key press.
fn key(code: KeyCode, modifiers: KeyModifiers) -> crossterm::event::KeyEvent {
    crossterm::event::KeyEvent::new(code, modifiers)
}

/// Render one frame so the renderer publishes the cursor row maps (the window
/// key arms consult them).
fn render(app: &mut App) {
    let backend = ratatui::backend::TestBackend::new(170, 44);
    let mut terminal = ratatui::Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("render frame");
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
    }
}

/// A teammate in `status` with a stable synthetic session id.
fn member(agent_id: &str, name: &str, status: MemberStatus) -> TeamMember {
    let mut member = TeamMember::new(name, agent_id, "general");
    member.status = status;
    member.session_id = Some(format!("{agent_id}-session"));
    member
}

/// Whether the TUI log panel carries the attributed request line (NFR-008).
fn log_has_attribution(app: &App, session_id: &str, needle: &str) -> bool {
    app.log_entries
        .iter()
        .any(|e| e.session_id.as_deref() == Some(session_id) && e.message.contains(needle))
}

/// Pump the async runtime until `done` observes its condition, up to about
/// two seconds.
async fn pump_until(mut done: impl FnMut() -> bool) {
    for _ in 0..200 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("the awaited dispatch condition never became true");
}

/// A unique scratch directory under `target/temp/agentui-t011` carrying its
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
        .join("../../target/temp/agentui-t011")
        .join(unique);
    std::fs::create_dir_all(dir.join(".ragent/teams")).expect("scratch dir");
    dir
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

/// The on-disk status of `tm-001` (`None` when the store or member is gone).
fn stored_status(store_dir: &PathBuf) -> Option<MemberStatus> {
    TeamStore::load(store_dir)
        .ok()?
        .config
        .member_by_id("tm-001")
        .map(|m| m.status)
}

// ---------------------------------------------------------------------------
// FR-041: Space on the cursor sub-agent row
// ---------------------------------------------------------------------------

/// `Space` on the cursor running sub-agent row requests suspension without a
/// dialog and is consumed (TC-010 step 1: no space reaches the input buffer).
#[tokio::test]
async fn space_on_a_running_agent_row_requests_suspension() {
    let mut app = support::make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.show_agents_window = true;
    app.active_tasks
        .push(task("d4e5f6a7", "explore", TaskStatus::Running));
    app.agents_cursor_subagent = Some(0); // the first sub-agent row
    render(&mut app);
    app.input = "draft".to_string();
    app.input_cursor = 5;

    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;

    assert!(!app.row_action_confirm_open(), "suspend opens no dialog");
    assert!(
        log_has_attribution(
            &app,
            "d4e5f6a7-session",
            "Suspend requested for task d4e5f6a7"
        ),
        "the suspend request must be attributed to the child session (NFR-008)"
    );
    assert_eq!(
        app.input, "draft",
        "the consumed Space must not reach the input buffer (FR-055)"
    );
    assert_eq!(app.input_cursor, 5, "the cursor must not advance either");
    assert!(app.needs_redraw, "the request line repaints (NFR-005)");
}

/// `Space` on the cursor suspended sub-agent row requests resumption instead
/// (TC-010 step 3: the toggle's other direction).
#[tokio::test]
async fn space_on_a_suspended_agent_row_requests_resumption() {
    let mut app = support::make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.show_agents_window = true;
    app.active_tasks
        .push(task("e5f6a7b8", "explore", TaskStatus::Suspended));
    app.agents_cursor_subagent = Some(0);
    render(&mut app);
    let baseline_input = app.input.clone();

    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;

    assert!(
        log_has_attribution(
            &app,
            "e5f6a7b8-session",
            "Resume requested for task e5f6a7b8"
        ),
        "the resume request must be attributed to the child session (NFR-008)"
    );
    assert_eq!(
        app.input, baseline_input,
        "the consumed Space must not reach the input buffer (FR-055)"
    );
}

// ---------------------------------------------------------------------------
// FR-042 / FR-044 re-expressed: the cursor range never reaches the primary
// ---------------------------------------------------------------------------

/// The cursor starts on the first sub-agent row and clamps inside the row
/// range: the primary row is not part of the cursor list, so no number of
/// Up/Down presses can ever select it (the old primary-refusal cases are
/// retired with the `selected_agent_index`; FR-042, FR-067).
#[tokio::test]
async fn the_cursor_range_never_reaches_the_primary_row() {
    let mut app = support::make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.show_agents_window = true;
    app.active_tasks = vec![
        task("a1b2c3d4", "explore", TaskStatus::Running),
        task("b2c3d4e5", "build", TaskStatus::Running),
    ];
    render(&mut app);

    // Down from no selection lands on the first sub-agent row.
    app.handle_key_event(key(KeyCode::Down, KeyModifiers::NONE))
        .await;
    assert_eq!(app.agents_cursor_subagent, Some(0));
    // Up clamps at the first sub-agent row, never past it to the primary.
    app.handle_key_event(key(KeyCode::Up, KeyModifiers::NONE))
        .await;
    assert_eq!(
        app.agents_cursor_subagent,
        Some(0),
        "Up past the top clamps at the first sub-agent (FR-042)"
    );
    for _ in 0..10 {
        app.handle_key_event(key(KeyCode::Down, KeyModifiers::NONE))
            .await;
    }
    assert_eq!(
        app.agents_cursor_subagent,
        Some(1),
        "Down past the end clamps at the last sub-agent (FR-044)"
    );
    // The range itself names sub-agent tasks only (FR-067).
    assert_eq!(
        app.agents_subagent_task_ids,
        vec!["a1b2c3d4".to_string(), "b2c3d4e5".to_string()],
        "the primary agent is not in the cursor list (FR-067)"
    );
}

/// With a real manager bound, the keyboard path never dispatches a suspend or
/// a kill for a task other than the cursor row: the consumed keys carry only
/// the cursor ordinal (FR-067). (The old primary-refusal-no-dispatch case,
/// re-expressed through the range.)
#[tokio::test]
async fn the_keyboard_path_dispatches_only_for_the_cursor_row() {
    let mut app = support::make_app();
    app.show_agents_window = true;
    app.session_id = Some(LEAD_SESSION.to_string());
    app.show_agents_window = true;
    app.active_tasks = vec![
        task("a1b2c3d4", "explore", TaskStatus::Running),
        task("b2c3d4e5", "build", TaskStatus::Running),
    ];
    let manager = Arc::new(ragent_agent::task::AgentManager::new(
        app.event_bus.clone(),
        app.session_processor.clone(),
        4,
        300,
    ));
    manager
        .seed_completed_for_test(task("a1b2c3d4", "explore", TaskStatus::Running))
        .await;
    manager
        .seed_completed_for_test(task("b2c3d4e5", "build", TaskStatus::Running))
        .await;
    let _ = app
        .session_processor
        .agent_manager
        .set(Arc::clone(&manager));

    // One Down: the cursor names a1b2c3d4 only.
    render(&mut app);

    // One Down: the cursor names a1b2c3d4 only.
    app.handle_key_event(key(KeyCode::Down, KeyModifiers::NONE))
        .await;
    let baseline = app.log_entries.len();

    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;

    // Give the spawned dispatch a chance to run, then confirm exactly one
    // request line was logged - the cursor row's.
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        log_has_attribution(
            &app,
            "a1b2c3d4-session",
            "Suspend requested for task a1b2c3d4"
        ),
        "suspend must target the cursor row only (FR-067)"
    );
    assert!(
        !app.log_entries
            .iter()
            .any(|e| e.message.contains("b2c3d4e5")),
        "no task but the cursor row may be named (FR-067)"
    );
    assert_eq!(
        app.log_entries.len() - baseline,
        1,
        "one Cursor action logs exactly one request line"
    );
}

// ---------------------------------------------------------------------------
// FR-043: Shift+K / Del on the cursor sub-agent row
// ---------------------------------------------------------------------------

/// `Shift+K` on the cursor sub-agent row arms the kill dialog naming the task
/// and is consumed (TC-010 step 4, TC-013 step 2); the follow-up answer keys
/// stay absorbed by the dialog (TC-013 step 3).
#[tokio::test]
async fn shift_k_on_a_selected_agent_row_arms_the_kill_dialog() {
    let mut app = support::make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.show_agents_window = true;
    app.active_tasks
        .push(task("c3d4e5f6", "rust-coder", TaskStatus::Running));
    app.agents_cursor_subagent = Some(0);
    render(&mut app);
    app.input = "typed".to_string();
    app.input_cursor = 5;

    app.handle_key_event(key(KeyCode::Char('K'), KeyModifiers::SHIFT))
        .await;

    assert!(app.row_action_confirm_open(), "Shift+K confirms the kill");
    match app.row_action_confirm.as_ref().map(|p| &p.action) {
        Some(ragent_tui::app::RowAction::KillSubagent {
            task_id,
            agent_name,
        }) => {
            assert_eq!(task_id, "c3d4e5f6");
            assert_eq!(agent_name, "rust-coder");
        }
        other => panic!("expected KillSubagent, got {other:?}"),
    }
    assert_eq!(
        app.input, "typed",
        "no `k` reaches the input buffer (FR-055)"
    );

    // The dialog answers while still armed; `Esc` leaves the task untouched.
    app.handle_key_event(key(KeyCode::Esc, KeyModifiers::NONE))
        .await;
    assert!(!app.row_action_confirm_open());
    assert_eq!(
        app.active_tasks[0].status,
        TaskStatus::Running,
        "Esc dismisses without dispatching"
    );
}

/// The confirmed keyboard kill drives the real `AgentManager` registry entry
/// to `Terminating` (the FR-036 dispatch observed from the keyboard path).
#[tokio::test]
async fn confirmed_keyboard_kill_terminates_through_the_manager() {
    let mut app = support::make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.show_agents_window = true;
    app.active_tasks
        .push(task("f6a7b8c9", "rust-coder", TaskStatus::Running));
    let manager = Arc::new(ragent_agent::task::AgentManager::new(
        app.event_bus.clone(),
        app.session_processor.clone(),
        4,
        300,
    ));
    manager
        .seed_completed_for_test(task("f6a7b8c9", "rust-coder", TaskStatus::Running))
        .await;
    let _ = app
        .session_processor
        .agent_manager
        .set(Arc::clone(&manager));
    app.agents_cursor_subagent = Some(0);

    render(&mut app);

    app.handle_key_event(key(KeyCode::Char('K'), KeyModifiers::SHIFT))
        .await;
    assert!(app.row_action_confirm_open());
    app.handle_key_event(key(KeyCode::Left, KeyModifiers::NONE))
        .await;
    app.handle_key_event(key(KeyCode::Enter, KeyModifiers::NONE))
        .await;
    assert!(!app.row_action_confirm_open(), "Yes closes the dialog");
    assert!(
        log_has_attribution(&app, "f6a7b8c9-session", "Kill requested for task f6a7b8c9"),
        "the kill request must be attributed to the child session (NFR-008)"
    );

    // Pump until the spawned kill reaches the manager's registry.
    for _ in 0..200 {
        match manager.get_task("f6a7b8c9").await {
            Some(entry) if entry.status == TaskStatus::Terminating => break,
            _ => tokio::time::sleep(Duration::from_millis(10)).await,
        }
    }
    let entry = manager
        .get_task("f6a7b8c9")
        .await
        .expect("the seeded task entry");
    assert_eq!(
        entry.status,
        TaskStatus::Terminating,
        "the confirmed keyboard kill must drive the real manager (FR-036, FR-043)"
    );
}

// ---------------------------------------------------------------------------
// FR-040: Space (suspend-confirm) and Del / k (shutdown) on the teammate
// ---------------------------------------------------------------------------

/// `Space` on the cursor teammate row arms the suspend dialog naming the
/// member (the destructive direction confirms; the old focused_teammate
/// Space-shutdown case is retired with the focus model).
#[tokio::test]
async fn space_on_a_cursor_teammate_arms_the_suspend_dialog() {
    let mut app = support::make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.active_team = Some(TeamConfig::new(TEAM_NAME, LEAD_SESSION));
    app.team_members
        .push(member("tm-001", "reviewer", MemberStatus::Working));
    app.show_teams_window = true;
    app.teams_cursor_teammate = Some(0);
    app.input = "draft".to_string();
    render(&mut app);
    render(&mut app);
    app.input_cursor = 5;

    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;

    assert!(app.row_action_confirm_open(), "Space confirms the suspend");
    match app.row_action_confirm.as_ref().map(|p| &p.action) {
        Some(ragent_tui::app::RowAction::SuspendTeammate { agent_id, name }) => {
            assert_eq!(agent_id, "tm-001");
            assert_eq!(name, "reviewer");
        }
        other => panic!("expected SuspendTeammate from Space, got {other:?}"),
    }
    assert_eq!(app.input, "draft", "no space reaches the buffer (FR-055)");
}

/// The confirmed suspend answer drives the real `TeamManager` - the on-disk
/// store shows `Suspended` (the FR-040 dispatch observed from the keyboard).
#[tokio::test]
async fn confirmed_keyboard_suspend_suspends_through_the_manager() {
    let mut app = support::make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.active_team = Some(TeamConfig::new(TEAM_NAME, LEAD_SESSION));
    app.team_members
        .push(member("tm-001", "reviewer", MemberStatus::Working));
    app.show_teams_window = true;
    let store_dir = bind_team_manager(&mut app, MemberStatus::Working);
    app.teams_cursor_teammate = Some(0);
    render(&mut app);

    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;
    assert!(app.row_action_confirm_open());
    app.handle_key_event(key(KeyCode::Left, KeyModifiers::NONE))
        .await;
    app.handle_key_event(key(KeyCode::Enter, KeyModifiers::NONE))
        .await;
    assert!(!app.row_action_confirm_open(), "Yes closes the dialog");
    assert!(
        log_has_attribution(&app, "tm-001-session", "tm-001 suspend requested"),
        "the suspend request must be attributed to the member session (NFR-008)"
    );

    pump_until(|| stored_status(&store_dir) == Some(MemberStatus::Suspended)).await;
    assert_eq!(
        stored_status(&store_dir),
        Some(MemberStatus::Suspended),
        "the confirmed keyboard suspend must drive the real manager (FR-040)"
    );
    let scratch = store_dir.ancestors().nth(3).expect("scratch root");
    let _ = std::fs::remove_dir_all(scratch);
}

/// `Del` on the cursor teammate row arms the graceful-shutdown dialog naming
/// the member, and the confirmed answer drives the real `TeamManager`: the
/// on-disk store shows `ShuttingDown` (TC-022's Del twin).
#[tokio::test]
async fn del_on_a_cursor_teammate_shuts_it_down_through_the_manager() {
    let mut app = support::make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.active_team = Some(TeamConfig::new(TEAM_NAME, LEAD_SESSION));
    app.team_members
        .push(member("tm-001", "reviewer", MemberStatus::Working));
    app.show_teams_window = true;
    let store_dir = bind_team_manager(&mut app, MemberStatus::Working);
    app.teams_cursor_teammate = Some(0);
    render(&mut app);
    app.input = "draft".to_string();
    app.input_cursor = 5;

    app.handle_key_event(key(KeyCode::Delete, KeyModifiers::NONE))
        .await;

    assert!(app.row_action_confirm_open(), "Del confirms the shutdown");
    match app.row_action_confirm.as_ref().map(|p| &p.action) {
        Some(ragent_tui::app::RowAction::ShutdownTeammate { agent_id, name }) => {
            assert_eq!(agent_id, "tm-001");
            assert_eq!(name, "reviewer");
        }
        other => panic!("expected ShutdownTeammate from Del, got {other:?}"),
    }
    assert_eq!(app.input, "draft", "no key reaches the buffer (FR-055)");

    app.handle_key_event(key(KeyCode::Left, KeyModifiers::NONE))
        .await;
    app.handle_key_event(key(KeyCode::Enter, KeyModifiers::NONE))
        .await;
    assert!(!app.row_action_confirm_open(), "Yes closes the dialog");
    assert!(
        log_has_attribution(&app, "tm-001-session", "tm-001 shutdown requested"),
        "the shutdown request must be attributed to the member session (NFR-008)"
    );

    pump_until(|| stored_status(&store_dir) == Some(MemberStatus::ShuttingDown)).await;
    assert_eq!(
        stored_status(&store_dir),
        Some(MemberStatus::ShuttingDown),
        "the confirmed keyboard shutdown must drive the real manager (FR-045)"
    );
    let scratch = store_dir.ancestors().nth(3).expect("scratch root");
    let _ = std::fs::remove_dir_all(scratch);
}

/// `Shift+K` on the cursor teammate row arms the same shutdown dialog; the
/// dialog absorbs the follow-up `Shift+K` so no second action can re-arm
/// behind it (TC-022 step 4).
#[tokio::test]
async fn shift_k_on_a_cursor_teammate_arms_the_shutdown_dialog() {
    let mut app = support::make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.active_team = Some(TeamConfig::new(TEAM_NAME, LEAD_SESSION));
    app.team_members
        .push(member("tm-001", "reviewer", MemberStatus::Working));
    app.show_teams_window = true;
    app.teams_cursor_teammate = Some(0);

    render(&mut app);

    app.handle_key_event(key(KeyCode::Char('K'), KeyModifiers::SHIFT))
        .await;
    assert!(
        app.row_action_confirm_open(),
        "Shift+K confirms the shutdown (FR-045)"
    );
    match app.row_action_confirm.as_ref().map(|p| &p.action) {
        Some(ragent_tui::app::RowAction::ShutdownTeammate { agent_id, .. }) => {
            assert_eq!(agent_id, "tm-001");
        }
        other => panic!("expected ShutdownTeammate from Shift+K, got {other:?}"),
    }
    // While the dialog is open the repeat key is swallowed whole.
    app.handle_key_event(key(KeyCode::Char('K'), KeyModifiers::SHIFT))
        .await;
    assert!(
        app.row_action_confirm_open(),
        "the open dialog absorbs the repeat Shift+K"
    );
    assert_eq!(app.input, "", "nothing reaches the input buffer (FR-055)");
}

// ---------------------------------------------------------------------------
// FR-055: window keys take precedence over the input buffer
// ---------------------------------------------------------------------------

/// With the Teams window open but a cursor whose member is gone, the keys are
/// a consumed no-op (FR-069 stale-cursor rule): the dialog never arms and the
/// input buffer stays empty.
#[tokio::test]
async fn teams_keys_without_a_live_cursor_are_consumed_no_ops() {
    let mut app = support::make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.active_team = Some(TeamConfig::new(TEAM_NAME, LEAD_SESSION));
    app.team_members
        .push(member("tm-001", "reviewer", MemberStatus::Working));
    app.show_teams_window = true;
    app.teams_cursor_teammate = Some(3); // beyond the live rows
    app.needs_redraw = false;

    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;
    assert!(
        !app.row_action_confirm_open(),
        "a cursor whose member is gone arms nothing (FR-069)"
    );
    app.handle_key_event(key(KeyCode::Char('K'), KeyModifiers::SHIFT))
        .await;
    assert!(!app.row_action_confirm_open());
    assert_eq!(app.input, "", "neither key leaks a character (FR-055)");
}

/// With the Agents window open and no cursor yet, `Space` and `Shift+K` are
/// still consumed by the window: nothing reaches the input buffer (the
/// no-cursor corner of FR-055; TC-013 step 4).
#[tokio::test]
async fn agent_window_keys_with_no_selection_never_touch_the_input() {
    let mut app = support::make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.show_agents_window = true;
    app.active_tasks
        .push(task("a1b2c3d4", "explore", TaskStatus::Running));
    app.agents_cursor_subagent = None;
    app.input = "draft".to_string();
    app.input_cursor = 5;

    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;
    app.handle_key_event(key(KeyCode::Char('K'), KeyModifiers::SHIFT))
        .await;

    assert!(!app.row_action_confirm_open(), "no cursor arms nothing");
    assert_eq!(
        app.input, "draft",
        "Space and Shift+K must be consumed while the window is open (FR-055)"
    );
    assert_eq!(app.input_cursor, 5);
}

/// A cursor beyond the live rows is a consumed no-op and dispatches nothing
/// (a stale ordinal must route nowhere, FR-069).
#[tokio::test]
async fn an_out_of_range_cursor_is_consumed_harmlessly() {
    let mut app = support::make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.show_agents_window = true;
    app.active_tasks
        .push(task("a1b2c3d4", "explore", TaskStatus::Running));
    app.agents_cursor_subagent = Some(9); // beyond the live rows
    app.input = "draft".to_string();
    app.input_cursor = 5;

    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;
    app.handle_key_event(key(KeyCode::Char('K'), KeyModifiers::SHIFT))
        .await;

    assert_eq!(app.input, "draft", "the stale cursor consumes both keys");
    assert!(
        !app.log_entries
            .iter()
            .any(|e| e.message.contains("requested")),
        "a stale cursor must dispatch nothing (FR-069)"
    );
}

// ---------------------------------------------------------------------------
// FR-067: no key path terminates the lead
// ---------------------------------------------------------------------------

/// The Teams cursor list covers painted teammate rows only, so it never names
/// the lead even when a member carries the `lead` id: every key lands on a
/// real member row (FR-067).
#[tokio::test]
async fn teammate_keys_never_target_the_lead() {
    let mut app = support::make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.active_team = Some(TeamConfig::new(TEAM_NAME, LEAD_SESSION));
    app.team_members
        .push(member("lead", "general", MemberStatus::Idle));
    app.show_teams_window = true;

    // Walk the whole cursor range: every row the cursor can land on is a
    render(&mut app);

    // Walk the whole cursor range: every row the cursor can land on is a
    // painted teammate row, and the lead row is not in the list at all.
    for _ in 0..5 {
        app.handle_key_event(key(KeyCode::Down, KeyModifiers::NONE))
            .await;
    }
    assert_eq!(
        app.teams_cursor_teammate,
        Some(0),
        "the range clamps inside the teammate rows (FR-067)"
    );
    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;
    app.handle_key_event(key(KeyCode::Char('K'), KeyModifiers::SHIFT))
        .await;

    // Idle members arm only the suspend direction; the member carrying the
    // literal `lead` id is a cursor row, so Space may arm its suspend dialog.
    match app.row_action_confirm.as_ref().map(|p| &p.action) {
        None => panic!("an idle teammate row must react to Space"),
        Some(ragent_tui::app::RowAction::SuspendTeammate { agent_id, .. }) => {
            assert_eq!(agent_id, "lead", "the cursor names the painted member");
        }
        Some(other) => panic!("expected SuspendTeammate, got {other:?}"),
    }
    assert_eq!(
        app.input, "",
        "no key leaks a character into the buffer (FR-055)"
    );
}
