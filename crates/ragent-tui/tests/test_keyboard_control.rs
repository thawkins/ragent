//! Tests for the Agents/Teams window keyboard control surface (spec `agentui`
//! T-011).
//!
//! `test_row_action_confirm.rs` pins the dialog and its trigger points via the
//! mouse and dialog internals; `test_row_action_dispatch.rs` pins the
//! dispatches. This file pins the **keyboard surface** end to end through
//! `App::handle_key_event`, the path a keyboard-only user drives (TC-010,
//! TC-011, TC-013, TC-022):
//!
//! - FR-041: `Space` on a selected sub-agent row suspends it, on a suspended
//!   row resumes it, is consumed (no space reaches the input buffer), and the
//!   request line is attributed to the child session (NFR-008);
//! - FR-042: `Space` on the primary row sets `Cannot suspend primary agent`
//!   and suspends nothing;
//! - FR-043: `Shift+K` on a selected sub-agent row confirms first and the
//!   confirmed answer drives the real `AgentManager` registry entry to
//!   `Terminating` (FR-036's keyboard twin);
//! - FR-044: `Shift+K` on the primary row sets `Cannot kill primary agent`
//!   and terminates nothing;
//! - FR-045: `Space` and `Shift+K` on a focused teammate arm the graceful
//!   shutdown dialog and the confirmed answer drives the real `TeamManager`
//!   (the on-disk store shows `ShuttingDown`);
//! - FR-055: with a window open both keys are consumed before the input
//!   buffer sees them - including the no-selection and no-focus corners;
//! - FR-067: no key path can dispatch a suspend/kill for the primary agent
//!   or a shutdown for the lead.
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
// FR-041: Space on the selected sub-agent row
// ---------------------------------------------------------------------------

/// `Space` on a selected running sub-agent row requests suspension without a
/// dialog and is consumed (TC-010 step 1: no space reaches the input buffer).
#[tokio::test]
async fn space_on_a_running_agent_row_requests_suspension() {
    let mut app = support::make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.show_agents_window = true;
    app.active_tasks
        .push(task("d4e5f6a7", "explore", TaskStatus::Running));
    app.selected_agent_index = Some(1); // the first sub-agent row
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

/// `Space` on a selected suspended sub-agent row requests resumption instead
/// (TC-010 step 3: the toggle's other direction).
#[tokio::test]
async fn space_on_a_suspended_agent_row_requests_resumption() {
    let mut app = support::make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.show_agents_window = true;
    app.active_tasks
        .push(task("e5f6a7b8", "explore", TaskStatus::Suspended));
    app.selected_agent_index = Some(1);
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
// FR-042: the primary agent is protected from Space
// ---------------------------------------------------------------------------

/// `Space` on the primary row refuses with the documented text and dispatches
/// nothing: with no manager bound there is nothing to observe anyway, so the
/// status text and the untouched input buffer are the contract (TC-011).
#[tokio::test]
async fn space_on_the_primary_row_refuses_and_suspends_nothing() {
    let mut app = support::make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.show_agents_window = true;
    app.active_tasks = vec![
        task("a1b2c3d4", "explore", TaskStatus::Running),
        task("b2c3d4e5", "build", TaskStatus::Running),
    ];
    app.selected_agent_index = Some(0); // the primary row
    app.input = "keep".to_string();
    app.input_cursor = 4;

    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;

    assert_eq!(
        app.status, "Cannot suspend primary agent",
        "the documented refusal text (FR-042)"
    );
    assert!(!app.row_action_confirm_open());
    assert_eq!(app.input, "keep", "no character typed (FR-055)");
}

/// `Space` on the primary row does not dispatch a suspend for any task, even
/// with a real manager bound: the refusal short-circuits before the list is
/// consulted (FR-067). The pump proves no spawned suspend ever fires (a
/// fire-and-forget suspend logs only when the manager accepts, so the absence
/// of any request line for the sibling tasks is the observable).
#[tokio::test]
async fn space_on_the_primary_row_reaches_no_task() {
    let mut app = support::make_app();
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
    app.selected_agent_index = Some(0);
    let baseline = app.log_entries.len();

    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;

    assert_eq!(app.status, "Cannot suspend primary agent");
    // Give any wrongly spawned dispatch a chance to run, then confirm it
    // never logged a request for either task (FR-067).
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        app.log_entries.len(),
        baseline,
        "a primary refusal must dispatch nothing"
    );
    assert!(
        !app.log_entries
            .iter()
            .any(|e| e.message.contains("a1b2c3d4") || e.message.contains("b2c3d4e5")),
        "no task may be named by a primary refusal path (FR-067)"
    );
}

// ---------------------------------------------------------------------------
// FR-043: Shift+K on the selected sub-agent row
// ---------------------------------------------------------------------------

/// `Shift+K` on a selected sub-agent row arms the kill dialog naming the task
/// and is consumed (TC-010 step 4, TC-013 step 2); the follow-up answer keys
/// stay absorbed by the dialog (TC-013 step 3).
#[tokio::test]
async fn shift_k_on_a_selected_agent_row_arms_the_kill_dialog() {
    let mut app = support::make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.show_agents_window = true;
    app.active_tasks
        .push(task("c3d4e5f6", "rust-coder", TaskStatus::Running));
    app.selected_agent_index = Some(1);
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
    app.selected_agent_index = Some(1);

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
} // ---------------------------------------------------------------------------
// FR-044: the primary agent is protected from Shift+K
// ---------------------------------------------------------------------------

/// `Shift+K` on the primary row refuses with the documented text, opens no
/// dialog and dispatches nothing even with a real manager bound (TC-011
/// steps 4-6; FR-067).
#[tokio::test]
async fn shift_k_on_the_primary_row_reaches_no_task() {
    let mut app = support::make_app();
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
    app.selected_agent_index = Some(0);
    let baseline = app.log_entries.len();

    app.handle_key_event(key(KeyCode::Char('K'), KeyModifiers::SHIFT))
        .await;

    assert_eq!(
        app.status, "Cannot kill primary agent",
        "the documented refusal text (FR-044)"
    );
    assert!(
        !app.row_action_confirm_open(),
        "the primary row must never reach the dialog"
    );
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        app.log_entries.len(),
        baseline,
        "a primary kill refusal must dispatch nothing (FR-067)"
    );
}

// ---------------------------------------------------------------------------
// FR-045: Space and Shift+K on the focused teammate
// ---------------------------------------------------------------------------

/// `Space` on a focused teammate arms the graceful-shutdown dialog naming the
/// member and the confirmed answer drives the real `TeamManager`: the
/// on-disk store shows `ShuttingDown` (TC-022 steps 1-3).
#[tokio::test]
async fn space_on_a_focused_teammate_shuts_it_down_through_the_manager() {
    let mut app = support::make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.active_team = Some(TeamConfig::new(TEAM_NAME, LEAD_SESSION));
    app.team_members
        .push(member("tm-001", "reviewer", MemberStatus::Working));
    app.show_teams_window = true;
    let store_dir = bind_team_manager(&mut app, MemberStatus::Working);
    app.focused_teammate = Some("tm-001".to_string());
    app.input = "draft".to_string();
    app.input_cursor = 5;

    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;

    assert!(app.row_action_confirm_open(), "Space confirms the shutdown");
    match app.row_action_confirm.as_ref().map(|p| &p.action) {
        Some(ragent_tui::app::RowAction::ShutdownTeammate { agent_id, name }) => {
            assert_eq!(agent_id, "tm-001");
            assert_eq!(name, "reviewer");
        }
        other => panic!("expected ShutdownTeammate from Space, got {other:?}"),
    }
    assert_eq!(app.input, "draft", "no space reaches the buffer (FR-055)");

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

/// `Shift+K` on a focused teammate arms the same shutdown dialog; the dialog
/// absorbs the follow-up `Shift+K` so no second action can re-arm behind it
/// (TC-022 step 4).
#[tokio::test]
async fn shift_k_on_a_focused_teammate_arms_the_shutdown_dialog() {
    let mut app = support::make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.active_team = Some(TeamConfig::new(TEAM_NAME, LEAD_SESSION));
    app.team_members
        .push(member("tm-001", "reviewer", MemberStatus::Working));
    app.show_teams_window = true;
    app.focused_teammate = Some("tm-001".to_string());

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

/// With the Teams window open but no focused teammate both keys are a
/// consumed no-op (FR-069 stale-focus rule): the dialog never arms and the
/// input buffer stays empty.
#[tokio::test]
async fn teams_keys_without_a_focused_teammate_are_consumed_no_ops() {
    let mut app = support::make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.active_team = Some(TeamConfig::new(TEAM_NAME, LEAD_SESSION));
    app.team_members
        .push(member("tm-001", "reviewer", MemberStatus::Working));
    app.show_teams_window = true;
    app.focused_teammate = Some("tm-gone".to_string()); // a stale focus
    app.needs_redraw = false;

    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;
    assert!(
        !app.row_action_confirm_open(),
        "a focus whose member is gone arms nothing (FR-069)"
    );
    app.handle_key_event(key(KeyCode::Char('K'), KeyModifiers::SHIFT))
        .await;
    assert!(!app.row_action_confirm_open());
    assert_eq!(app.input, "", "neither key leaks a character (FR-055)");
}

// ---------------------------------------------------------------------------
// FR-055: window keys take precedence over the input buffer
// ---------------------------------------------------------------------------

/// With the Agents window open and no selection, `Space` and `Shift+K` are
/// still consumed by the window: nothing reaches the input buffer (the
/// no-selection corner of FR-055; TC-013 step 4).
#[tokio::test]
async fn agent_window_keys_with_no_selection_never_touch_the_input() {
    let mut app = support::make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.show_agents_window = true;
    app.active_tasks
        .push(task("a1b2c3d4", "explore", TaskStatus::Running));
    app.selected_agent_index = None;
    app.input = "draft".to_string();
    app.input_cursor = 5;

    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;
    app.handle_key_event(key(KeyCode::Char('K'), KeyModifiers::SHIFT))
        .await;

    assert!(!app.row_action_confirm_open(), "no selection arms nothing");
    assert_eq!(
        app.input, "draft",
        "Space and Shift+K must be consumed while the window is open (FR-055)"
    );
    assert_eq!(app.input_cursor, 5);
}

/// Even an out-of-range selected index is consumed without leaking and
/// without reaching a task (a stale index must route nowhere, FR-069).
#[tokio::test]
async fn an_out_of_range_selected_index_is_consumed_harmlessly() {
    let mut app = support::make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.show_agents_window = true;
    app.selected_agent_index = Some(99); // beyond the live rows
    app.input = "draft".to_string();
    app.input_cursor = 5;

    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;
    app.handle_key_event(key(KeyCode::Char('K'), KeyModifiers::SHIFT))
        .await;

    assert_eq!(app.input, "draft", "the stale index consumes both keys");
    assert!(
        !app.log_entries
            .iter()
            .any(|e| e.message.contains("requested")),
        "a stale index must dispatch nothing (FR-069)"
    );
}

// ---------------------------------------------------------------------------
// FR-067: no key path terminates the lead
// ---------------------------------------------------------------------------

/// The Teams-window key arm never names the lead: a focus whose agent id is
/// `lead` is refused before the dialog can arm (FR-067) - both keys stay
/// consumed no-ops.
#[tokio::test]
async fn teammate_keys_never_target_the_lead() {
    let mut app = support::make_app();
    app.session_id = Some(LEAD_SESSION.to_string());
    app.active_team = Some(TeamConfig::new(TEAM_NAME, LEAD_SESSION));
    app.team_members
        .push(member("lead", "general", MemberStatus::Idle));
    app.show_teams_window = true;
    app.focused_teammate = Some("lead".to_string());

    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;
    app.handle_key_event(key(KeyCode::Char('K'), KeyModifiers::SHIFT))
        .await;

    assert!(
        !app.row_action_confirm_open(),
        "a focus on the lead must never arm a shutdown (FR-067)"
    );
    assert_eq!(
        app.input, "",
        "neither key leaks a character into the buffer (FR-055)"
    );
}
