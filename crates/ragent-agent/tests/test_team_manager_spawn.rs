//! Integration coverage for the `TeamManager` spawn path, the team config
//! types, the per-agent mailbox, and the `team_spawn` tool's argument
//! validation.
//!
//! These tests build a real `TeamManager` over a stub `SessionProcessor` (see
//! `manager_support`) with **no** provider. Any spawned teammate agent loop
//! therefore errors on the first model call; the tests assert the *on-disk*
//! team state (a `tm-NNN` member in `Working`, a session id, the memory scope)
//! rather than a successful reply, and the manager's `Drop` aborts the
//! background loops.

use std::sync::Arc;

use ragent_agent::event::EventBus;
use ragent_agent::session::processor::SessionProcessor;
use ragent_agent::team::config::TEAM_CONFIG_SCHEMA_VERSION;
use ragent_agent::team::{
    MAX_MESSAGE_BYTES, Mailbox, MailboxMessage, MemberStatus, MemoryScope, MessageType, TeamConfig,
    TeamManager, TeamMember, TeamSettings, TeamStore, resolve_memory_dir,
};
use ragent_agent::tool::{TeamManagerInterface, Tool, ToolContext, create_default_registry};
use serde_json::json;
use tempfile::TempDir;

#[path = "manager_support/mod.rs"]
mod manager_support;
use manager_support::team_manager;

// -- helpers -----------------------------------------------------------------

/// The project root (`<tmp>/ws`) that owns the team's `.ragent/` directory.
fn workspace_of(team_dir: &std::path::Path) -> std::path::PathBuf {
    // team_dir = <ws>/.ragent/teams/<name>
    team_dir
        .parent()
        .and_then(std::path::Path::parent)
        .and_then(std::path::Path::parent)
        .expect("team dir has ws ancestor")
        .to_path_buf()
}

fn load_config(team_dir: &std::path::Path) -> TeamConfig {
    TeamStore::load(team_dir).expect("load team").config
}

/// Build a `TeamManager` that shares a **strong** `Arc` to its processor, so
/// the spawn path can upgrade the `Weak` and create a real child session.
///
/// `manager_support::team_manager` keeps only a `Weak` reference to a processor
/// that is dropped before it returns, so its spawn path always fails with
/// "session processor unavailable". This helper retains the `Arc` for the test
/// body (returned alongside the manager).
fn live_team_manager(
    team_name: &str,
    lead_session: &str,
) -> (Arc<TeamManager>, TempDir, Arc<SessionProcessor>) {
    let bus = Arc::new(EventBus::new(256));
    let tmp = tempfile::tempdir().expect("tempdir");
    let workspace = tmp.path().join("ws");
    std::fs::create_dir_all(workspace.join(".ragent")).expect("create .ragent");

    let mut store =
        TeamStore::create(team_name, "old-lead", &workspace, true).expect("create team");
    store
        .add_member(TeamMember::new("alice", "tm-001", "general"))
        .expect("add member");
    let team_dir = store.dir.clone();
    drop(store);

    let processor = manager_support::stub_processor(bus.clone());
    // FR-008: the spawn loop records member status through the manager, so the
    // manager must carry the self handle `into_arc` installs.
    let manager = TeamManager::into_arc(TeamManager::new(
        team_name,
        lead_session,
        team_dir,
        Arc::clone(&processor),
        bus,
    ));
    (manager, tmp, processor)
}

/// Build a `ToolContext` wired to `manager` with a working directory from
/// which the team is discoverable.
fn tool_ctx(
    bus: Arc<EventBus>,
    working_dir: std::path::PathBuf,
    manager: Arc<dyn TeamManagerInterface>,
) -> ToolContext {
    ToolContext {
        session_id: "spawn-tool-test".to_string(),
        working_dir,
        event_bus: bus,
        storage: None,
        agent_manager: None,
        active_model: None,
        provider_registry: None,
        team_context: None,
        team_manager: Some(manager),
        code_index: None,
        bg_service: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        allowed_roots: Vec::new(),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        tool_registry: ToolContext::default_tool_registry(),
    }
}

// -- spawn path --------------------------------------------------------------

#[tokio::test]
async fn spawn_records_working_member_with_session_on_disk() {
    let (manager, _tmp, _proc) = live_team_manager("spawn-team", "lead-1");
    let wd = std::env::current_dir().expect("cwd");

    let agent_id = manager
        .spawn_teammate_internal("bob", "general", "do one thing", None, None, &wd)
        .await
        .expect("spawn should allocate an agent id");
    assert_eq!(agent_id, "tm-002", "next id after the seeded tm-001");

    let config = load_config(&manager.team_dir);
    let member = config
        .member_by_name("bob")
        .expect("bob must be recorded in config.json");
    assert_eq!(member.agent_id, "tm-002");
    assert_eq!(member.status, MemberStatus::Working);
    assert!(
        member.session_id.is_some(),
        "the child session id must be persisted"
    );
}

#[tokio::test]
async fn spawn_reuses_the_existing_member_record() {
    let (manager, _tmp, _proc) = live_team_manager("reuse-team", "lead-1");
    let wd = std::env::current_dir().expect("cwd");

    // "alice" already exists as tm-001 (seeded by the support module).
    let id = manager
        .spawn_teammate_internal("alice", "general", "again", None, None, &wd)
        .await
        .expect("respawn of a known name reuses its record");
    assert_eq!(id, "tm-001", "no fresh id for an existing member");

    let config = load_config(&manager.team_dir);
    let alice = config.member_by_name("alice").expect("alice present");
    assert_eq!(alice.status, MemberStatus::Working);
    assert!(alice.session_id.is_some());
    // No duplicate member was appended.
    assert_eq!(
        config.members.iter().filter(|m| m.name == "alice").count(),
        1
    );
}

#[tokio::test]
async fn spawn_enforces_max_teammates_limit() {
    let (manager, _tmp, _bus) = team_manager("limit-team", "lead-1", true);
    // Lower the configured cap to 1; the seeded "alice" already counts.
    let mut store = TeamStore::load(&manager.team_dir).expect("load");
    store.config.settings.max_teammates = 1;
    store.save().expect("persist lowered cap");

    let wd = std::env::current_dir().expect("cwd");
    let err = manager
        .spawn_teammate_internal("bob", "general", "work", None, None, &wd)
        .await
        .expect_err("the cap must reject the spawn");
    assert!(
        err.to_string().contains("maximum"),
        "unexpected error: {err}"
    );

    // The rejected spawn left no new member behind.
    let config = load_config(&manager.team_dir);
    assert!(config.member_by_name("bob").is_none());
}

// -- config types ------------------------------------------------------------

#[test]
fn memory_scope_resolution_and_unsafe_names() {
    let wd = std::path::PathBuf::from("/work/project");
    assert!(resolve_memory_dir(MemoryScope::None, "alice", &wd).is_none());
    assert_eq!(
        resolve_memory_dir(MemoryScope::Project, "alice", &wd),
        Some(wd.join(".ragent").join("agent-memory").join("alice"))
    );
    // User scope resolves to a per-name directory (never None for a safe name).
    let user = resolve_memory_dir(MemoryScope::User, "alice", &wd);
    assert!(user.is_some());
    assert_eq!(user.unwrap().file_name().unwrap(), "alice");

    for bad in [
        "",
        ".",
        "..",
        "a/b",
        "../evil",
        "/abs",
        "a b",
        &"x".repeat(200),
    ] {
        assert!(
            resolve_memory_dir(MemoryScope::Project, bad, &wd).is_none(),
            "unsafe agent name must not resolve: {bad:?}"
        );
    }
}

#[test]
fn config_validate_rejects_empty_and_duplicates() {
    let mut empty_name = TeamConfig::new("", "lead");
    empty_name.name = String::new();
    assert!(empty_name.validate().is_err());

    let mut empty_lead = TeamConfig::new("team", "lead");
    empty_lead.lead_session_id = String::new();
    assert!(empty_lead.validate().is_err());

    let mut empty_member = TeamConfig::new("team", "lead");
    empty_member
        .members
        .push(TeamMember::new("x", "", "general"));
    assert!(empty_member.validate().is_err());

    let mut dup = TeamConfig::new("team", "lead");
    dup.members.push(TeamMember::new("a", "tm-001", "general"));
    dup.members.push(TeamMember::new("b", "tm-001", "general"));
    assert!(dup.validate().is_err());

    TeamConfig::new("team", "lead").validate().expect("valid");
}

#[test]
fn config_migrate_backfills_schema_and_timestamp() {
    let mut config = TeamConfig::new("team", "lead");
    config.schema_version = 0;
    config.updated_at = None;
    config.migrate();
    assert_eq!(config.schema_version, TEAM_CONFIG_SCHEMA_VERSION);
    assert!(config.updated_at.is_some());
}

#[test]
fn config_member_lookups_and_active_members() {
    let mut config = TeamConfig::new("team", "lead");
    let mut stopped = TeamMember::new("stopped", "tm-002", "general");
    stopped.status = MemberStatus::Stopped;
    let mut failed = TeamMember::new("failed", "tm-003", "general");
    failed.status = MemberStatus::Failed;
    config
        .members
        .push(TeamMember::new("alice", "tm-001", "general"));
    config.members.push(stopped);
    config.members.push(failed);

    assert_eq!(config.member_by_id("tm-001").unwrap().name, "alice");
    assert!(config.member_by_id("tm-999").is_none());
    assert_eq!(config.member_by_name("failed").unwrap().agent_id, "tm-003");
    assert!(config.member_by_name("nobody").is_none());

    // active_members excludes only Stopped (Failed is still listed here).
    let active: Vec<&str> = config.active_members().map(|m| m.name.as_str()).collect();
    assert_eq!(active, vec!["alice", "failed"]);

    config.member_by_id_mut("tm-001").unwrap().status = MemberStatus::Idle;
    assert_eq!(
        config.member_by_id("tm-001").unwrap().status,
        MemberStatus::Idle
    );
}

#[test]
fn team_settings_defaults() {
    let settings = TeamSettings::default();
    assert_eq!(settings.max_teammates, 8);
    assert!(!settings.require_plan_approval);
    assert!(settings.auto_claim_tasks);
}

// -- mailbox -----------------------------------------------------------------

#[test]
fn mailbox_push_read_peek_drain_and_mark() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    // Empty mailbox reads are empty.
    let empty = Mailbox::open(dir, "tm-010").expect("open");
    assert_eq!(empty.read_all().expect("read_all").len(), 0);
    assert_eq!(empty.peek_unread().expect("peek").len(), 0);
    assert_eq!(empty.drain_unread().expect("drain").len(), 0);

    let mbox = Mailbox::open(dir, "tm-001").expect("open");
    // A non-lead sender needs no authenticated session id.
    mbox.push(MailboxMessage::new(
        "tm-002",
        "tm-001",
        MessageType::Message,
        "hello",
    ))
    .expect("push");
    mbox.push(MailboxMessage::new(
        "tm-003",
        "tm-001",
        MessageType::Broadcast,
        "all",
    ))
    .expect("push");

    assert_eq!(mbox.read_all().expect("read_all").len(), 2);
    // Peek does not mark read.
    assert_eq!(mbox.peek_unread().expect("peek").len(), 2);
    assert_eq!(mbox.peek_unread().expect("peek again").len(), 2);

    // mark_read transitions one message; idempotent second call returns false.
    let first_id = mbox.read_all().expect("read")[0].message_id.clone();
    assert!(mbox.mark_read(&first_id).expect("mark_read"));
    assert!(!mbox.mark_read(&first_id).expect("mark_read again"));
    assert_eq!(mbox.peek_unread().expect("peek").len(), 1);

    // acknowledge aliases mark_read.
    let second = mbox.read_all().expect("read")[1].message_id.clone();
    assert!(mbox.acknowledge(&second).expect("acknowledge"));
    assert_eq!(mbox.peek_unread().expect("peek").len(), 0);

    // New unread message, drained (read+mark) in one call.
    mbox.push(MailboxMessage::new(
        "tm-004",
        "tm-001",
        MessageType::Message,
        "third",
    ))
    .expect("push");
    let drained = mbox.drain_unread().expect("drain");
    assert_eq!(drained.len(), 1);
    assert_eq!(drained[0].content, "third");
    assert_eq!(mbox.peek_unread().expect("peek").len(), 0);

    // mark_all_read across ids returns the count transitioned.
    mbox.push(MailboxMessage::new(
        "tm-005",
        "tm-001",
        MessageType::Message,
        "a",
    ))
    .expect("push");
    mbox.push(MailboxMessage::new(
        "tm-006",
        "tm-001",
        MessageType::Message,
        "b",
    ))
    .expect("push");
    let ids: Vec<String> = mbox
        .read_all()
        .expect("read")
        .into_iter()
        .map(|m| m.message_id)
        .collect();
    assert_eq!(mbox.mark_all_read(&ids).expect("mark_all_read"), 2);
    assert_eq!(mbox.mark_all_read(&[]).expect("empty ids"), 0);
}

#[test]
fn mailbox_message_validation_and_sender_session() {
    // Non-lead sender validates with no session id.
    let ok = MailboxMessage::new("tm-002", "tm-001", MessageType::Message, "x");
    ok.validate().expect("non-lead sender is fine");

    // A claimed "lead" sender without the authenticated session id is rejected.
    let forged = MailboxMessage::new("lead", "tm-001", MessageType::Message, "x");
    assert!(forged.validate().is_err());
    let forged = forged.with_sender_session("lead-session");
    assert_eq!(forged.sender_session_id.as_deref(), Some("lead-session"));
    forged.validate().expect("lead with session id is fine");

    // Empty from / to are rejected.
    assert!(
        MailboxMessage::new("", "tm-001", MessageType::Message, "x")
            .validate()
            .is_err()
    );
    assert!(
        MailboxMessage::new("tm-002", "", MessageType::Message, "x")
            .validate()
            .is_err()
    );

    // A forged lead message is refused at the push boundary.
    let tmp = tempfile::tempdir().expect("tempdir");
    let mbox = Mailbox::open(tmp.path(), "tm-001").expect("open");
    assert!(
        mbox.push(MailboxMessage::new(
            "lead",
            "tm-001",
            MessageType::Message,
            "x"
        ))
        .is_err()
    );

    // Correlated message carries a correlation id.
    let correlated =
        MailboxMessage::new_correlated("tm-002", "lead", MessageType::PlanRequest, "plan", "cid");
    assert_eq!(correlated.correlation_id.as_deref(), Some("cid"));
}

#[test]
fn mailbox_rejects_oversized_message() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mbox = Mailbox::open(tmp.path(), "tm-001").expect("open");
    let oversized = "x".repeat(MAX_MESSAGE_BYTES + 1);
    let err = mbox
        .push(MailboxMessage::new(
            "tm-002",
            "tm-001",
            MessageType::Message,
            oversized,
        ))
        .expect_err("oversized content must be refused");
    assert!(err.to_string().contains("byte limit"), "err: {err}");
}

// -- team_spawn tool ---------------------------------------------------------

fn spawn_tool() -> Arc<dyn Tool> {
    create_default_registry()
        .get("team_spawn")
        .expect("team_spawn is registered")
}

#[tokio::test]
async fn team_spawn_tool_rejects_missing_required_args() {
    let bus = Arc::new(EventBus::new(64));
    let (manager, _tmp, _bus) = team_manager("tool-missing-team", "lead-1", true);
    let ctx = tool_ctx(bus, std::env::current_dir().unwrap(), manager);

    let err = spawn_tool()
        .execute(json!({}), &ctx)
        .await
        .expect_err("empty input rejected");
    assert!(err.to_string().contains("team_name"), "err: {err}");

    let err = spawn_tool()
        .execute(json!({"team_name": "t"}), &ctx)
        .await
        .expect_err("missing teammate_name rejected");
    assert!(err.to_string().contains("teammate_name"), "err: {err}");

    let err = spawn_tool()
        .execute(json!({"team_name": "t", "teammate_name": "bob"}), &ctx)
        .await
        .expect_err("missing prompt rejected");
    assert!(err.to_string().contains("prompt"), "err: {err}");
}

#[tokio::test]
async fn team_spawn_tool_returns_pending_without_manager() {
    let bus = Arc::new(EventBus::new(64));
    let ctx = ToolContext {
        session_id: "no-manager".to_string(),
        working_dir: std::env::current_dir().unwrap(),
        event_bus: bus,
        storage: None,
        agent_manager: None,
        active_model: None,
        provider_registry: None,
        team_context: None,
        team_manager: None,
        code_index: None,
        bg_service: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        allowed_roots: Vec::new(),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        tool_registry: ToolContext::default_tool_registry(),
    };

    let out = spawn_tool()
        .execute(
            json!({"team_name": "t", "teammate_name": "bob", "agent_type": "general", "prompt": "one item"}),
            &ctx,
        )
        .await
        .expect("pending_manager output");
    assert_eq!(
        out.metadata
            .as_ref()
            .and_then(|m| m.get("status"))
            .and_then(|v| v.as_str()),
        Some("pending_manager")
    );
}

#[tokio::test]
async fn team_spawn_tool_spawns_and_persists_memory_scope() {
    let (manager, _tmp, _proc) = live_team_manager("tool-spawn-team", "lead-1");
    let ws = workspace_of(&manager.team_dir);
    let ctx = tool_ctx(
        Arc::new(EventBus::new(64)),
        ws,
        Arc::clone(&manager) as Arc<dyn TeamManagerInterface>,
    );

    let out = spawn_tool()
        .execute(
            json!({
                "team_name": "tool-spawn-team",
                "teammate_name": "carol",
                "agent_type": "general",
                "prompt": "do exactly one thing",
                "memory": "project"
            }),
            &ctx,
        )
        .await
        .expect("spawn succeeds");

    let agent_id = out
        .metadata
        .as_ref()
        .and_then(|m| m.get("agent_id"))
        .and_then(|v| v.as_str())
        .expect("agent_id in metadata")
        .to_string();
    assert_eq!(agent_id, "tm-002");

    // The tool persists the requested memory scope on the member record.
    let config = load_config(&manager.team_dir);
    let member = config.member_by_name("carol").expect("carol recorded");
    assert_eq!(member.memory_scope, MemoryScope::Project);
    assert_eq!(member.status, MemberStatus::Working);
}
