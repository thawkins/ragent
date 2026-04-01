use std::path::PathBuf;
use tempfile::TempDir;
use std::sync::Arc;

use ragent_core::team::TeamStore;
use ragent_core::team::manager::TeamManager;
use ragent_core::event::EventBus;
use ragent_core::session::processor::SessionProcessor;
use ragent_core::session::SessionManager;

#[tokio::test]
async fn test_reconcile_spawning_members_updates_config() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join(".ragent/teams")).unwrap();

    let name = "recon-test".to_string();
    let sid = "lead-session".to_string();

    // Create a team store and add spawning members (simulate blueprint run without manager)
    let store = TeamStore::create(&name, &sid, &project, true).expect("create store");
    let mut loaded = TeamStore::load_by_name(&name, &project).expect("load");
    let member = ragent_core::team::config::TeamMember::new("auto-1", "tm-001", "general");
    loaded.add_member(member).expect("add member");

    // Ensure member is Spawning on disk
    let loaded2 = TeamStore::load_by_name(&name, &project).expect("load2");
    assert!(loaded2.config.members.iter().any(|m| m.name == "auto-1" && m.status == ragent_core::team::config::MemberStatus::Spawning));

    // Prepare minimal SessionProcessor with SessionManager so TeamManager can create child sessions
    let event_bus = Arc::new(EventBus::new(32));
    let db = project.join("ragent.db");
    let storage = Arc::new(ragent_core::storage::Storage::open(&db).unwrap());
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));

    let processor = Arc::new(SessionProcessor {
        session_manager: session_manager.clone(),
        provider_registry: Arc::new(ragent_core::provider::create_default_registry()),
        tool_registry: Arc::new(ragent_core::tool::create_default_registry()),
        permission_checker: Arc::new(tokio::sync::RwLock::new(ragent_core::permission::PermissionChecker::new(vec![]))),
        event_bus: event_bus.clone(),
        task_manager: std::sync::OnceLock::new(),
        lsp_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
    });

    // Create TeamManager and set it into the processor OnceLock
    let team_dir = TeamStore::load_by_name(&name, &project).unwrap().dir;
    let manager = Arc::new(TeamManager::new(name.clone(), sid.clone(), team_dir.clone(), processor.clone(), event_bus.clone()));
    let _ = processor.team_manager.set(manager.clone());

    // Run reconciliation
    manager.reconcile_spawning_members();

    // Allow background tasks to run
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Reload store and assert member now has a session_id (spawned)
    let final_store = TeamStore::load_by_name(&name, &project).expect("final");
    let member = final_store.config.member_by_name("auto-1").expect("member present");
    assert!(member.session_id.is_some(), "reconciled member should have session_id");
}
