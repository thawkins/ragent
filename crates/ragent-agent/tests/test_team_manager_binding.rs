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

// -- Rebinding (review P0-4) -------------------------------------------------

use ragent_agent::team::{
    Mailbox, MemberStatus, MessageType, TeamManager, TeamManagerSlot, TeamMember, TeamStatus,
};
use ragent_agent::tool::{TeamManagerInterface, Tool, ToolContext};

/// A manager over a fresh team `name`, led by `lead-sess`, in `workspace`.
fn manager_for(
    processor: &Arc<ragent_agent::session::processor::SessionProcessor>,
    workspace: &std::path::Path,
    name: &str,
) -> Arc<TeamManager> {
    let store = TeamStore::create(name, "lead-sess", workspace, true).expect("create team");
    TeamManager::new(
        name,
        "lead-sess",
        store.dir.clone(),
        Arc::clone(processor),
        Arc::new(EventBus::new(16)),
    )
    .into_arc()
}

#[tokio::test]
async fn test_team_binding_slot_set_replace_clear() {
    let (processor, tmp) = fixture();
    let slot = TeamManagerSlot::default();
    let a = manager_for(&processor, tmp.path(), "slot-a");
    let b = manager_for(&processor, tmp.path(), "slot-b");
    assert!(slot.set(Arc::clone(&a)).is_ok());
    assert!(slot.set(Arc::clone(&b)).is_err(), "set does not overwrite");
    let old = slot.replace(Arc::clone(&b)).expect("replaced");
    assert_eq!(old.team_name, "slot-a");
    assert_eq!(slot.get().expect("bound").team_name, "slot-b");
    assert!(slot.clear().is_some());
    assert!(slot.get().is_none());
}

#[tokio::test]
async fn test_team_binding_slot_reports_a_gone_team_as_stale() {
    let (processor, tmp) = fixture();
    let slot = TeamManagerSlot::default();
    let a = manager_for(&processor, tmp.path(), "stale-a");
    assert!(slot.set(Arc::clone(&a)).is_ok());
    assert!(slot.stale().is_none(), "a live team is not stale");

    TeamStore::update(&a.team_dir, |c| {
        c.status = TeamStatus::Disbanded;
        Ok(())
    })
    .expect("disband");
    assert!(slot.stale().is_some(), "a disbanded team is stale");

    std::fs::remove_dir_all(&a.team_dir).expect("remove");
    assert!(slot.stale().is_some(), "a deleted team is stale");
}

#[tokio::test]
async fn test_team_binding_rebinds_from_a_cleaned_up_team() {
    let (processor, tmp) = fixture();
    let first = manager_for(&processor, tmp.path(), "first");
    assert!(processor.team_manager.set(Arc::clone(&first)).is_ok());
    std::fs::remove_dir_all(&first.team_dir).expect("cleanup first");

    processor.ensure_team_manager(Some(&lead()), tmp.path(), &model());
    assert_eq!(
        processor.team_manager.get().expect("bound").team_name,
        "bind-a"
    );
}

#[tokio::test]
async fn test_team_binding_rebinds_when_the_lead_moves_to_another_team() {
    let (processor, tmp) = fixture();
    let other = manager_for(&processor, tmp.path(), "other");
    assert!(processor.team_manager.set(Arc::clone(&other)).is_ok());

    processor.ensure_team_manager(Some(&lead()), tmp.path(), &model());
    assert_eq!(
        processor.team_manager.get().expect("bound").team_name,
        "bind-a"
    );
}

#[tokio::test]
async fn test_team_binding_spawn_into_another_team_is_refused() {
    let (processor, tmp) = fixture();
    let other = manager_for(&processor, tmp.path(), "other");
    let err = other
        .spawn_teammate(
            "bind-a",
            "bob",
            "general",
            "one thing",
            None,
            None,
            tmp.path(),
        )
        .await
        .expect_err("wrong team refused");
    assert!(
        err.to_string().contains("bound to team 'other'"),
        "err: {err}"
    );
    let roster = TeamStore::load(&other.team_dir)
        .expect("load")
        .config
        .members;
    assert!(roster.is_empty(), "nothing written into the bound team");
}

#[tokio::test]
async fn test_team_binding_shutdown_tool_ignores_a_manager_of_another_team() {
    let (processor, tmp) = fixture();
    // `bind-a` has a working teammate tm-001; the bound runtime drives `other`,
    // which has its own tm-001.
    let a_dir = TeamStore::load_by_name("bind-a", tmp.path())
        .expect("a")
        .dir;
    let other = manager_for(&processor, tmp.path(), "other");
    for dir in [&a_dir, &other.team_dir] {
        TeamStore::update(dir, |c| {
            let mut m = TeamMember::new("worker", "tm-001", "general");
            m.status = MemberStatus::Working;
            c.members.push(m);
            Ok(())
        })
        .expect("add member");
    }

    let ctx = ToolContext {
        session_id: "lead-sess".to_string(),
        working_dir: tmp.path().to_path_buf(),
        event_bus: Arc::new(EventBus::new(16)),
        storage: None,
        agent_manager: None,
        active_model: None,
        provider_registry: None,
        team_context: Some(Arc::new(lead())),
        team_manager: Some(Arc::clone(&other) as Arc<dyn TeamManagerInterface>),
        code_index: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        bg_service: None,
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        tool_registry: ToolContext::default_tool_registry(),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        allowed_roots: Vec::new(),
    };
    ragent_agent::tool::team_shutdown_teammate::TeamShutdownTeammateTool
        .execute(
            serde_json::json!({"team_name": "bind-a", "teammate": "tm-001"}),
            &ctx,
        )
        .await
        .expect("shutdown");

    let status = |dir: &std::path::Path| {
        TeamStore::load(dir)
            .expect("load")
            .config
            .member_by_id("tm-001")
            .map(|m| m.status)
    };
    assert_eq!(
        status(&a_dir),
        Some(MemberStatus::ShuttingDown),
        "named team's member"
    );
    assert_eq!(
        status(&other.team_dir),
        Some(MemberStatus::Working),
        "other team untouched"
    );
    let inbox = Mailbox::open(&a_dir, "tm-001")
        .expect("open")
        .peek_unread()
        .expect("peek");
    assert!(
        inbox
            .iter()
            .any(|m| m.message_type == MessageType::ShutdownRequest),
        "the shutdown request is delivered on the disk-only path"
    );
}
