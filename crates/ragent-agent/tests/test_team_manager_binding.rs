//! The session processor binds a `TeamManager` for the team its session leads
//! (review P0-5). Before, only the TUI created one, so `team_spawn` in `run`,
//! `serve` and ACP mode never started a teammate.

use std::sync::Arc;

use ragent_agent::agent::ModelRef;
use ragent_agent::event::EventBus;
use ragent_agent::task::AgentManager;
use ragent_agent::team::TeamStore;
use ragent_agent::tool::TeamContext;

#[path = "manager_support/mod.rs"]
mod manager_support;
use manager_support::stub_processor;

fn model() -> ModelRef {
    ModelRef {
        provider_id: "stub".to_string(),
        model_id: "stub-model".to_string(),
    }
}

/// A processor with an `AgentManager` wired (as every entry point does) and a
/// team `bind-a` led by session `lead-sess` in a fresh workspace.
fn fixture() -> (
    Arc<ragent_agent::session::processor::SessionProcessor>,
    tempfile::TempDir,
) {
    let bus = Arc::new(EventBus::new(64));
    let processor = stub_processor(bus.clone());
    let agents = Arc::new(AgentManager::new(bus, Arc::clone(&processor), 4, 300));
    assert!(processor.agent_manager.set(agents).is_ok());
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(tmp.path().join(".ragent")).expect(".ragent");
    TeamStore::create("bind-a", "lead-sess", tmp.path(), true).expect("create team");
    (processor, tmp)
}

fn lead() -> TeamContext {
    TeamContext {
        team_name: "bind-a".to_string(),
        agent_id: "lead".to_string(),
        is_lead: true,
    }
}

#[tokio::test]
async fn test_team_binding_lead_session_gets_a_manager() {
    let (processor, tmp) = fixture();
    assert!(processor.team_manager.get().is_none());
    processor.ensure_team_manager(Some(&lead()), tmp.path(), &model());
    let manager = processor.team_manager.get().expect("manager bound");
    assert_eq!(manager.team_name, "bind-a");
    manager.shutdown_all().await.expect("shutdown");
}

#[tokio::test]
async fn test_team_binding_teammate_or_no_team_binds_nothing() {
    let (processor, tmp) = fixture();
    processor.ensure_team_manager(None, tmp.path(), &model());
    let teammate = TeamContext {
        is_lead: false,
        agent_id: "tm-001".to_string(),
        ..lead()
    };
    processor.ensure_team_manager(Some(&teammate), tmp.path(), &model());
    assert!(processor.team_manager.get().is_none());
}

#[tokio::test]
async fn test_team_binding_without_agent_manager_is_a_no_op() {
    let bus = Arc::new(EventBus::new(64));
    let processor = stub_processor(bus);
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(tmp.path().join(".ragent")).expect(".ragent");
    TeamStore::create("bind-a", "lead-sess", tmp.path(), true).expect("create team");
    processor.ensure_team_manager(Some(&lead()), tmp.path(), &model());
    assert!(processor.team_manager.get().is_none());
}
