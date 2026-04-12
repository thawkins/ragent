//! Integration test for live team reconciliation.
#![allow(missing_docs)]

use std::sync::Arc;

use ragent_core::event::EventBus;
use ragent_core::session::SessionManager;
use ragent_core::session::processor::SessionProcessor;
use ragent_core::team::TeamStore;
use ragent_core::team::manager::TeamManager;

#[tokio::test]
async fn test_reconcile_against_existing_team_dir() {
    // This test targets the project's .ragent teams directory if present.
    let cwd = std::env::current_dir().unwrap();
    let team_dir = cwd.join(".ragent").join("teams");
    if !team_dir.exists() {
        // Skip if no live teams available.
        eprintln!("No .ragent/teams found, skipping live reconcile test");
        return;
    }

    // Pick the most recent team directory (if any)
    let mut latest = None;
    for e in std::fs::read_dir(&team_dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            latest = Some(p);
            break;
        }
    }
    let team_dir = match latest {
        Some(p) => p,
        None => {
            eprintln!("No team subdirs found, skipping");
            return;
        }
    };

    let store = TeamStore::load(&team_dir).expect("load team");
    eprintln!("Testing reconcile for team: {}", store.config.name);

    let event_bus = Arc::new(EventBus::new(32));
    let db = std::env::temp_dir().join("ragent_live_test.db");
    let storage = Arc::new(ragent_core::storage::Storage::open(&db).unwrap());
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));

    let processor = Arc::new(SessionProcessor {
        session_manager: session_manager.clone(),
        provider_registry: Arc::new(ragent_core::provider::create_default_registry()),
        tool_registry: Arc::new(ragent_core::tool::create_default_registry()),
        permission_checker: Arc::new(tokio::sync::RwLock::new(
            ragent_core::permission::PermissionChecker::new(vec![]),
        )),
        event_bus: event_bus.clone(),
        task_manager: std::sync::OnceLock::new(),
        lsp_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
    });

    let manager = Arc::new(TeamManager::new(
        store.config.name.clone(),
        "lead-test",
        team_dir.clone(),
        processor.clone(),
        event_bus.clone(),
    ));
    // Do NOT set processor.team_manager to mimic TUI timing; call reconcile directly
    manager.reconcile_spawning_members();
    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

    let refreshed = TeamStore::load(&team_dir).unwrap();
    eprintln!("After reconcile, members:\n{:?}", refreshed.config.members);
}
