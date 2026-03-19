//! TUI tests for Teams integration (M4-T9).
//!
//! Covers:
//! - `/team status` output with no active team and with an active team
//! - `/team create` creates a team and updates app state
//! - `/team tasks` when no team is active
//! - `/team cleanup` clears in-memory state
//! - `/team message` with no active team / unknown teammate
//! - Unknown `/team` subcommand shows error
//! - `[T]` badge appears when team members are present
//! - Team events update `active_team` and `team_members`

use std::sync::Arc;
use std::sync::Mutex;

use ragent_core::{
    agent,
    event::{Event, EventBus},
    permission::PermissionChecker,
    provider,
    session::{SessionManager, processor::SessionProcessor},
    storage::Storage,
    team::{MemberStatus, Task, TaskStatus, TeamConfig, TeamMember, TeamStore},
    tool,
};
use ragent_tui::App;
use ragent_tui::app::LogLevel;

static CWD_LOCK: Mutex<()> = Mutex::new(());

/// Build an [`App`] backed by an in-memory database.
fn make_app() -> App {
    let event_bus = Arc::new(EventBus::default());
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let provider_registry = Arc::new(provider::create_default_registry());
    let tool_registry = Arc::new(tool::create_default_registry());
    let permission_checker = Arc::new(tokio::sync::RwLock::new(PermissionChecker::new(vec![])));
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));
    let session_processor = Arc::new(SessionProcessor {
        session_manager,
        provider_registry: provider_registry.clone(),
        tool_registry,
        permission_checker,
        event_bus: event_bus.clone(),
        task_manager: std::sync::OnceLock::new(),
        lsp_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
    });
    let agent_info =
        agent::resolve_agent("general", &Default::default()).expect("resolve general agent");

    App::new(
        event_bus,
        storage,
        provider_registry,
        session_processor,
        agent_info,
        false,
    )
}

fn unique_team_name(prefix: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("unix time")
        .as_nanos();
    format!("{prefix}-{nanos}")
}

// ── /team status ─────────────────────────────────────────────────────────────

#[test]
fn test_team_status_no_active_team() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/team status");

    assert_eq!(app.status, "team: status");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("No active team"),
        "should indicate no active team: {text}"
    );
    assert!(
        text.contains("/team create"),
        "should suggest /team create: {text}"
    );
}

#[test]
fn test_team_status_with_active_team() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    // Inject an active team into app state (as TeammateSpawned event would).
    let team = TeamConfig::new("code-review", "s1");
    app.active_team = Some(team);

    let mut m1 = TeamMember::new("security-reviewer", "tm-001", "general");
    m1.status = MemberStatus::Working;
    m1.current_task_id = Some("task-001".to_string());
    app.team_members.push(m1);

    app.execute_slash_command("/team");

    assert_eq!(app.status, "team: status");
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("code-review"), "should name the team: {text}");
    assert!(
        text.contains("security-reviewer"),
        "should list teammate: {text}"
    );
    assert!(
        text.contains("task-001"),
        "should show current task: {text}"
    );
    assert!(text.contains("1 teammate"), "should show count: {text}");
}

#[test]
fn test_team_status_no_args_defaults_to_status() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    // Without args, `/team` should behave like `/team status`
    app.execute_slash_command("/team");

    assert_eq!(app.status, "team: status");
}

// ── /team create ─────────────────────────────────────────────────────────────

#[test]
fn test_team_create_no_name_shows_usage() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/team create");

    assert!(
        app.status.contains("Usage"),
        "should show usage hint: {}",
        app.status
    );
}

// ── /team open ───────────────────────────────────────────────────────────────

#[test]
fn test_team_open_no_name_shows_usage() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/team open");

    assert!(
        app.status.contains("Usage"),
        "should show usage hint: {}",
        app.status
    );
}

#[test]
fn test_team_open_loads_existing_team() {
    let _cwd_guard = CWD_LOCK.lock().expect("cwd lock");
    let tmp = tempfile::tempdir().expect("tempdir");
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(tmp.path()).unwrap();
    std::fs::create_dir_all(tmp.path().join(".ragent/teams")).unwrap();

    // Create a team and add one member on disk.
    let mut store = TeamStore::create("existing-team", "lead-session", tmp.path(), true)
        .expect("create existing team");
    store
        .add_member(TeamMember::new("worker-a", "tm-001", "general"))
        .expect("add member");

    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.execute_slash_command("/team open existing-team");

    let _ = std::env::set_current_dir(original_dir);

    assert_eq!(
        app.active_team.as_ref().map(|t| t.name.as_str()),
        Some("existing-team")
    );
    assert_eq!(app.team_members.len(), 1, "should load existing members");
    assert!(app.show_teams, "teams panel should be visible");
    assert_eq!(app.status, "team: existing-team");
    assert!(
        app.session_processor.team_manager.get().is_some(),
        "TeamManager should be initialised after /team open"
    );
}

// ── /team close ──────────────────────────────────────────────────────────────

#[test]
fn test_team_close_no_active_team() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/team close");

    assert_eq!(app.status, "No active team to close");
}

#[test]
fn test_team_close_clears_active_team_state() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.active_team = Some(TeamConfig::new("my-team", "s1"));
    app.team_members.push(TeamMember::new("bob", "tm-001", "general"));
    app.show_teams = true;

    app.execute_slash_command("/team close");

    assert!(app.active_team.is_none(), "active team should be cleared");
    assert!(app.team_members.is_empty(), "members should be cleared");
    assert!(!app.show_teams, "teams panel should be hidden");
    assert_eq!(app.status, "team closed");
}

// ── /team delete ─────────────────────────────────────────────────────────────

#[test]
fn test_team_delete_no_name_shows_usage() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/team delete");

    assert!(
        app.status.contains("Usage"),
        "should show usage hint: {}",
        app.status
    );
}

#[test]
fn test_team_delete_removes_existing_team() {
    let _cwd_guard = CWD_LOCK.lock().expect("cwd lock");
    let tmp = tempfile::tempdir().expect("tempdir");
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(tmp.path()).unwrap();
    std::fs::create_dir_all(tmp.path().join(".ragent/teams")).unwrap();

    let team_name = unique_team_name("delete-me");
    let _store = TeamStore::create(&team_name, "lead-session", tmp.path(), true)
        .expect("create team");
    let team_path = tmp.path().join(".ragent/teams").join(&team_name);
    assert!(team_path.exists(), "team dir should exist before delete");

    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.execute_slash_command(&format!("/team delete {team_name}"));

    let _ = std::env::set_current_dir(original_dir);

    assert!(
        !team_path.exists(),
        "team dir should be deleted (status: {}, path: {})",
        app.status,
        team_path.display()
    );
    assert_eq!(app.status, "team deleted");
}

#[test]
fn test_team_delete_active_team_clears_session_state() {
    let _cwd_guard = CWD_LOCK.lock().expect("cwd lock");
    let tmp = tempfile::tempdir().expect("tempdir");
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(tmp.path()).unwrap();
    std::fs::create_dir_all(tmp.path().join(".ragent/teams")).unwrap();

    let team_name = unique_team_name("active-delete");
    let _store = TeamStore::create(&team_name, "lead-session", tmp.path(), true)
        .expect("create team");
    let team_path = tmp.path().join(".ragent/teams").join(&team_name);

    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.active_team = Some(TeamConfig::new(&team_name, "s1"));
    app.team_members.push(TeamMember::new("bob", "tm-001", "general"));
    app.show_teams = true;

    app.execute_slash_command(&format!("/team delete {team_name}"));

    let _ = std::env::set_current_dir(original_dir);

    assert!(
        !team_path.exists(),
        "team dir should be deleted (status: {}, path: {})",
        app.status,
        team_path.display()
    );
    assert!(app.active_team.is_none(), "active team should be cleared");
    assert!(app.team_members.is_empty(), "members should be cleared");
    assert!(!app.show_teams, "teams panel should be hidden");
    assert_eq!(app.status, "team deleted");
}

#[test]
fn test_team_delete_active_team_blocked_when_teammates_working() {
    let _cwd_guard = CWD_LOCK.lock().expect("cwd lock");
    let tmp = tempfile::tempdir().expect("tempdir");
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(tmp.path()).unwrap();
    std::fs::create_dir_all(tmp.path().join(".ragent/teams")).unwrap();

    let team_name = unique_team_name("busy-delete");
    let _store = TeamStore::create(&team_name, "lead-session", tmp.path(), true)
        .expect("create team");
    let team_path = tmp.path().join(".ragent/teams").join(&team_name);

    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.active_team = Some(TeamConfig::new(&team_name, "s1"));
    let mut m = TeamMember::new("worker", "tm-001", "general");
    m.status = MemberStatus::Working;
    app.team_members.push(m);
    app.show_teams = true;

    app.execute_slash_command(&format!("/team delete {team_name}"));

    let _ = std::env::set_current_dir(original_dir);

    assert!(
        app.status.contains("still active"),
        "should refuse delete while active: {}",
        app.status
    );
    assert!(team_path.exists(), "team dir should remain");
    assert!(app.active_team.is_some(), "active team should remain");
}

#[test]
fn test_team_create_sets_active_team() {
    let _cwd_guard = CWD_LOCK.lock().expect("cwd lock");
    let tmp = tempfile::tempdir().expect("tempdir");
    let original_dir = std::env::current_dir().unwrap();
    // Change into the temp dir so the project-local team is created there.
    std::env::set_current_dir(tmp.path()).unwrap();
    // Create the .ragent directory so project-local teams can be stored.
    std::fs::create_dir_all(tmp.path().join(".ragent/teams")).unwrap();

    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/team create my-test-team");

    std::env::set_current_dir(original_dir).unwrap();

    assert!(
        app.active_team.is_some(),
        "active_team should be set after create"
    );
    assert_eq!(app.active_team.as_ref().unwrap().name, "my-test-team");
    assert!(app.show_teams, "show_teams should be enabled");
    assert!(app.team_members.is_empty(), "no teammates yet");
    assert!(
        app.session_processor.team_manager.get().is_some(),
        "TeamManager should be initialised after /team create"
    );
    assert!(
        app.status.contains("my-test-team"),
        "status should mention team name: {}",
        app.status
    );
    // Log entry should mention team creation.
    assert!(
        app.log_entries.iter().any(|e| e.message.contains("my-test-team")),
        "log should mention team name"
    );
}

// ── /team tasks ───────────────────────────────────────────────────────────────

#[test]
fn test_team_tasks_no_active_team() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/team tasks");

    assert_eq!(app.status, "no active team");
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("No active team"), "should indicate no team: {text}");
}

#[test]
fn test_team_tasks_renders_table_with_status() {
    let _cwd_guard = CWD_LOCK.lock().expect("cwd lock");
    let tmp = tempfile::tempdir().expect("tempdir");
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(tmp.path()).unwrap();
    std::fs::create_dir_all(tmp.path().join(".ragent/teams")).unwrap();

    let team_name = unique_team_name("tasks-table");
    let store = TeamStore::create(&team_name, "lead-session", tmp.path(), true).expect("create");
    let task_store = store.task_store().expect("task store");

    let mut t1 = Task::new("task-001", "Write docs");
    t1.status = TaskStatus::Pending;
    task_store.add_task(t1).expect("add task 1");

    let mut t2 = Task::new("task-002", "Run tests");
    t2.status = TaskStatus::InProgress;
    t2.assigned_to = Some("tm-001".to_string());
    task_store.add_task(t2).expect("add task 2");

    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.active_team = Some(TeamConfig::new(&team_name, "s1"));

    app.execute_slash_command("/team tasks");

    let _ = std::env::set_current_dir(original_dir);

    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("ID"), "should include table header: {text}");
    assert!(text.contains("Status"), "should include status column: {text}");
    assert!(text.contains("task-001"), "should include task id: {text}");
    assert!(text.contains("pending"), "should show pending status: {text}");
    assert!(text.contains("task-002"), "should include second task: {text}");
    assert!(text.contains("in-progress"), "should show in-progress status: {text}");
    assert!(text.contains("tm-001"), "should show assignee: {text}");
}

// ── /team clear ───────────────────────────────────────────────────────────────

#[test]
fn test_team_clear_no_active_team() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/team clear");

    assert_eq!(app.status, "no active team");
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("No active team"), "should indicate no team: {text}");
}

#[test]
fn test_team_clear_removes_tasks_for_active_team() {
    let _cwd_guard = CWD_LOCK.lock().expect("cwd lock");
    let tmp = tempfile::tempdir().expect("tempdir");
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(tmp.path()).unwrap();
    std::fs::create_dir_all(tmp.path().join(".ragent/teams")).unwrap();

    let team_name = unique_team_name("clear-tasks");
    let store = TeamStore::create(&team_name, "lead-session", tmp.path(), true).expect("create");
    let task_store = store.task_store().expect("task store");
    task_store
        .add_task(Task::new("task-001", "Task to clear"))
        .expect("add task");
    let tasks_path = store.dir.join("tasks.json");
    assert!(tasks_path.exists(), "tasks.json should exist before clear");

    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.active_team = Some(TeamConfig::new(&team_name, "s1"));

    app.execute_slash_command("/team clear");

    let _ = std::env::set_current_dir(original_dir);

    assert!(!tasks_path.exists(), "tasks.json should be removed after clear");
    assert_eq!(app.status, "team tasks cleared");
}

// ── /team message ─────────────────────────────────────────────────────────────

#[test]
fn test_team_message_no_active_team() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/team message alice hello");

    assert_eq!(app.status, "No active team");
}

#[test]
fn test_team_message_unknown_teammate() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.active_team = Some(TeamConfig::new("my-team", "s1"));

    app.execute_slash_command("/team message ghost hello");

    assert!(
        app.status.contains("ghost") && app.status.contains("not found"),
        "should report unknown teammate: {}",
        app.status
    );
}

#[test]
fn test_team_message_missing_text_shows_usage() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/team message");

    assert!(
        app.status.contains("Usage"),
        "should show usage: {}",
        app.status
    );
}

// ── /team cleanup ─────────────────────────────────────────────────────────────

#[test]
fn test_team_cleanup_no_active_team() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/team cleanup");

    assert!(
        app.status.contains("No active team"),
        "should warn when no team active: {}",
        app.status
    );
}

#[test]
fn test_team_cleanup_clears_state() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.active_team = Some(TeamConfig::new("my-team", "s1"));
    app.team_members.push(TeamMember::new("bob", "tm-001", "general"));
    app.show_teams = true;

    // Team dir does not exist on disk — cleanup should still clear in-memory state.
    app.execute_slash_command("/team cleanup");

    assert!(app.active_team.is_none(), "active_team should be cleared");
    assert!(app.team_members.is_empty(), "team_members should be cleared");
    assert!(!app.show_teams, "show_teams should be disabled");
    assert_eq!(app.status, "team cleaned up");
}

#[test]
fn test_team_cleanup_blocked_when_teammates_active() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.active_team = Some(TeamConfig::new("busy-team", "s1"));

    let mut m = TeamMember::new("worker", "tm-001", "general");
    m.status = MemberStatus::Working;
    app.team_members.push(m);
    app.show_teams = true;

    app.execute_slash_command("/team cleanup");

    // Should refuse because a teammate is still working.
    assert!(
        app.status.contains("still active"),
        "should block cleanup when teammates active: {}",
        app.status
    );
    assert!(app.active_team.is_some(), "team should not be removed");
}

// ── /team unknown subcommand ──────────────────────────────────────────────────

#[test]
fn test_team_unknown_subcommand_shows_error() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/team frobnicate");

    assert!(
        app.status.contains("Unknown /team subcommand") || app.status.contains("frobnicate"),
        "should report unknown subcommand: {}",
        app.status
    );
    assert!(
        app.log_entries.iter().any(|e| e.level == LogLevel::Warn),
        "should log a warning"
    );
}

// ── [T] badge — team_members drives badge rendering ───────────────────────────

#[test]
fn test_team_members_drives_t_badge_set() {
    // The [T] badge is rendered in layout_active_agents by checking
    // whether the task's child_session_id is in the teammate_ids set.
    // This test verifies that the set is built from team_members correctly.
    let mut app = make_app();
    let mut m = TeamMember::new("security-reviewer", "tm-001", "general");
    m.session_id = Some("sess-tm-001".to_string());
    app.team_members.push(m);

    let teammate_ids: std::collections::HashSet<String> = app
        .team_members
        .iter()
        .filter_map(|m| m.session_id.clone())
        .collect();

    assert!(
        teammate_ids.contains("sess-tm-001"),
        "teammate session ID should be in badge set"
    );
}

// ── Event handling — TeammateSpawned ─────────────────────────────────────────

#[test]
fn test_event_teammate_spawned_adds_member_and_shows_panel() {
    let mut app = make_app();
    let sid = "sess-lead".to_string();
    app.session_id = Some(sid.clone());

    let event = Event::TeammateSpawned {
        session_id: sid.clone(),
        team_name: "alpha".to_string(),
        teammate_name: "writer".to_string(),
        agent_id: "tm-001".to_string(),
    };
    app.handle_event(event);

    assert!(app.show_teams, "show_teams should be enabled");
    assert_eq!(app.team_members.len(), 1);
    assert_eq!(app.team_members[0].name, "writer");
    assert_eq!(app.team_members[0].agent_id, "tm-001");

    // Log should mention the spawn.
    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("writer")),
        "log should mention teammate name"
    );
}

#[test]
fn test_event_teammate_spawned_deduplicates() {
    let mut app = make_app();
    let sid = "sess-lead".to_string();
    app.session_id = Some(sid.clone());

    let event = Event::TeammateSpawned {
        session_id: sid.clone(),
        team_name: "alpha".to_string(),
        teammate_name: "writer".to_string(),
        agent_id: "tm-001".to_string(),
    };
    app.handle_event(event.clone());
    app.handle_event(event);

    assert_eq!(
        app.team_members.len(),
        1,
        "duplicate TeammateSpawned should not add member twice"
    );
}

// ── Event handling — TeammateIdle ────────────────────────────────────────────

#[test]
fn test_event_teammate_idle_updates_status() {
    let mut app = make_app();
    let sid = "sess-lead".to_string();
    app.session_id = Some(sid.clone());

    let mut m = TeamMember::new("tester", "tm-002", "general");
    m.status = MemberStatus::Working;
    app.team_members.push(m);

    let event = Event::TeammateIdle {
        session_id: sid.clone(),
        team_name: "alpha".to_string(),
        agent_id: "tm-002".to_string(),
    };
    app.handle_event(event);

    assert_eq!(app.team_members[0].status, MemberStatus::Idle);
    assert!(
        app.log_entries.iter().any(|e| e.message.contains("idle")),
        "log should record idle event"
    );
}

// ── Event handling — TeamTaskClaimed / Completed ─────────────────────────────

#[test]
fn test_event_team_task_claimed_sets_current_task() {
    let mut app = make_app();
    let sid = "s1".to_string();
    app.session_id = Some(sid.clone());
    app.team_members.push(TeamMember::new("tm-a", "tm-001", "general"));

    app.handle_event(Event::TeamTaskClaimed {
        session_id: sid.clone(),
        team_name: "t".to_string(),
        agent_id: "tm-001".to_string(),
        task_id: "task-007".to_string(),
    });

    assert_eq!(app.team_members[0].status, MemberStatus::Working);
    assert_eq!(
        app.team_members[0].current_task_id.as_deref(),
        Some("task-007")
    );
}

#[test]
fn test_event_team_task_completed_clears_current_task() {
    let mut app = make_app();
    let sid = "s1".to_string();
    app.session_id = Some(sid.clone());
    let mut m = TeamMember::new("tm-a", "tm-001", "general");
    m.current_task_id = Some("task-007".to_string());
    m.status = MemberStatus::Working;
    app.team_members.push(m);

    app.handle_event(Event::TeamTaskCompleted {
        session_id: sid.clone(),
        team_name: "t".to_string(),
        agent_id: "tm-001".to_string(),
        task_id: "task-007".to_string(),
    });

    assert!(
        app.team_members[0].current_task_id.is_none(),
        "current_task_id should be cleared after completion"
    );
}

// ── Event handling — TeamCleanedUp ────────────────────────────────────────────

#[test]
fn test_event_team_cleaned_up_resets_state() {
    let mut app = make_app();
    let sid = "s1".to_string();
    app.session_id = Some(sid.clone());
    app.active_team = Some(TeamConfig::new("gone-team", "s1"));
    app.team_members.push(TeamMember::new("a", "tm-001", "general"));
    app.show_teams = true;

    app.handle_event(Event::TeamCleanedUp {
        session_id: sid,
        team_name: "gone-team".to_string(),
    });

    assert!(app.active_team.is_none());
    assert!(app.team_members.is_empty());
    assert!(!app.show_teams);
}

// ── Event handling — TeammateMessage ─────────────────────────────────────────

#[test]
fn test_event_teammate_message_logs_preview() {
    let mut app = make_app();
    let sid = "s1".to_string();
    app.session_id = Some(sid.clone());

    app.handle_event(Event::TeammateMessage {
        session_id: sid,
        team_name: "alpha".to_string(),
        from: "writer".to_string(),
        to: "lead".to_string(),
        preview: "here is my result".to_string(),
    });

    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("writer") && e.message.contains("here is my result")),
        "log should contain message preview"
    );
}
