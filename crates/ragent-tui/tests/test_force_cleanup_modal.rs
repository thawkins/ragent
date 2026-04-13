//! Tests for test_force_cleanup_modal.rs

use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ragent_core::{
    agent,
    event::EventBus,
    permission::PermissionChecker,
    provider,
    session::{SessionManager, processor::SessionProcessor},
    storage::Storage,
    team::{MemberStatus, TeamConfig, TeamMember, TeamStore},
    tool,
};
use ragent_tui::App;

use std::sync::Mutex;
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
        mcp_client: std::sync::OnceLock::new(),
        code_index: std::sync::OnceLock::new(),
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

#[test]
fn test_forcecleanup_modal_confirm_flow() {
    let _cwd_guard = CWD_LOCK.lock().expect("cwd lock");
    let tmp = tempfile::tempdir().expect("tempdir");
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(tmp.path()).unwrap();
    std::fs::create_dir_all(tmp.path().join(".ragent/teams")).unwrap();

    let _store =
        TeamStore::create("force-team", "lead-session", tmp.path(), true).expect("create team");
    // Add a member to the store so the forcecleanup has something to deactivate
    let mut store = TeamStore::load_by_name("force-team", tmp.path()).expect("load store");
    store
        .add_member(TeamMember::new("alice", "tm-001", "general"))
        .expect("add member");

    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.active_team = Some(TeamConfig::new("force-team", "s1"));
    let mut m = TeamMember::new("alice", "tm-001", "general");
    m.status = MemberStatus::Working;
    app.team_members.push(m);

    app.execute_slash_command("/team forcecleanup");

    assert!(
        app.pending_forcecleanup.is_some(),
        "pending modal should be set"
    );
    let last_msg = app.messages.last().unwrap().text_content();
    assert!(last_msg.contains("Active teammates") || last_msg.contains("Press Enter"));
    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("forcecleanup confirmation required"))
    );

    // Press Enter to confirm
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(
        app.pending_forcecleanup.is_none(),
        "pending modal should be cleared after confirm"
    );
    // After confirming, team should be cleaned up (status updated)
    assert!(
        app.status.contains("team force"),
        "expected status to reflect force cleanup: {}",
        app.status
    );

    let _ = std::env::set_current_dir(original_dir);
}

#[test]
fn test_forcecleanup_modal_cancel_flow() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.active_team = Some(TeamConfig::new("force-team", "s1"));
    let mut m = TeamMember::new("bob", "tm-002", "general");
    m.status = MemberStatus::Working;
    app.team_members.push(m);

    app.execute_slash_command("/team forcecleanup");
    assert!(
        app.pending_forcecleanup.is_some(),
        "pending modal should be set"
    );

    // Press Esc to cancel
    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    assert!(
        app.pending_forcecleanup.is_none(),
        "pending modal should be cleared after cancel"
    );
    let last_msg = app.messages.last().unwrap().text_content();
    assert!(last_msg.contains("Force-cleanup cancelled"));
    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("forcecleanup cancelled"))
    );
}
