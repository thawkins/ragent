//! FR-020 coverage: a message enqueued for a teammate that terminates before
//! its first `team_read_messages` is **kept** (`read: false` on disk) and
//! **reported** by `/team show` and `team_status`, rather than dropped (F-16).
//!
//! The report is the only place such a message can surface: the mailbox poll
//! loop that would drain it belongs to the recipient, so once the recipient is
//! gone nothing reads that inbox again. These tests pin the three halves of
//! that contract - the message survives, the report finds it, and the report
//! says who it was for, who sent it, and what it said.

use std::sync::Arc;

use ragent_agent::event::EventBus;
use ragent_agent::team::{
    MemberStatus, MessageType, TeamMember, TeamStore, render_undelivered, undelivered_messages,
};
use ragent_agent::tool::{Tool, ToolContext};
use serde_json::json;

#[path = "team_support/mod.rs"]
mod team_support;
use team_support::setup_workspace;

type Workspace = (tempfile::TempDir, std::path::PathBuf);

/// Build a `ToolContext` rooted at the workspace's `.ragent/teams` directory.
fn make_ctx(working_dir: &std::path::Path) -> ToolContext {
    ToolContext {
        session_id: "lead-sess".to_string(),
        working_dir: working_dir.to_path_buf(),
        event_bus: Arc::new(EventBus::new(64)),
        storage: None,
        agent_manager: None,
        active_model: None,
        provider_registry: None,
        team_context: None,
        team_manager: None,
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
    }
}

/// Create a team whose single member `tm-001` carries `status`.
fn seed_team(dir: &std::path::Path, team: &str, status: MemberStatus) -> TeamStore {
    let mut store = TeamStore::create(team, "lead-sess", dir, true).expect("create team");
    let mut member = TeamMember::new("worker", "tm-001", "general");
    member.status = status;
    store.add_member(member).expect("add_member");
    store
}

/// Enqueue one message for `tm-001`, from `from`.
fn enqueue(store: &TeamStore, from: &str, content: &str) {
    let message =
        ragent_agent::team::MailboxMessage::new(from, "tm-001", MessageType::Message, content);
    // A body that claims to come from the lead must carry the authenticated
    // sender session id (SEC-ragent-team-003), so attach it here.
    let message = if from == "lead" {
        message.with_sender_session("lead-sess")
    } else {
        message
    };
    store
        .mailbox("tm-001")
        .expect("open mailbox")
        .push(message)
        .expect("push message");
}

// -- the message survives ---------------------------------------------------

/// FR-020 requires the message to be **left unread**, not deleted: a stopped
/// recipient must not take the content with it.
#[tokio::test]
async fn a_stopped_recipient_leaves_the_message_unread_on_disk() {
    let (_tmp, dir): Workspace = setup_workspace();
    let store = seed_team(&dir, "undelivered-team", MemberStatus::Stopped);
    enqueue(&store, "lead", "please review the patch");

    let all = store
        .mailbox("tm-001")
        .expect("reopen mailbox")
        .read_all()
        .expect("read_all");
    assert_eq!(all.len(), 1, "the message must not be dropped");
    assert!(!all[0].read, "the message must stay unread");
    assert_eq!(all[0].content, "please review the patch");
}

// -- the report finds it ----------------------------------------------------

/// The terminated recipient's unread message is reported, with the recipient's
/// agent id and its terminal status.
#[tokio::test]
async fn a_terminated_recipient_s_message_is_reported() {
    let (_tmp, dir): Workspace = setup_workspace();
    let store = seed_team(&dir, "undelivered-team", MemberStatus::Stopped);
    enqueue(&store, "lead", "please review the patch");

    let found = undelivered_messages(&store);
    assert_eq!(found.len(), 1, "one stranded message expected: {found:?}");
    assert_eq!(found[0].agent_id, "tm-001");
    assert_eq!(found[0].recipient, "worker");
    assert_eq!(found[0].recipient_status, MemberStatus::Stopped);
    assert_eq!(found[0].content, "please review the patch");
    assert_eq!(found[0].from, "lead");
    assert_eq!(found[0].message_type, "message");
}

/// Both terminal statuses report; `Failed` is not a special case.
#[tokio::test]
async fn both_terminal_statuses_report_their_stranded_messages() {
    let (_tmp, dir): Workspace = setup_workspace();
    for status in [MemberStatus::Stopped, MemberStatus::Failed] {
        // A team name is lowercase alphanumerics and hyphens only, so the
        // status token's underscores become hyphens.
        let team = format!("undelivered-{}", status.as_str().replace('_', "-"));
        let store = seed_team(&dir, &team, status);
        enqueue(&store, "lead", "never read");
        let found = undelivered_messages(&store);
        assert_eq!(found.len(), 1, "{status:?} must report its message");
        assert_eq!(found[0].recipient_status, status);
    }
}

/// A message already read by the recipient before it terminated is *delivered*,
/// so it must not be reported as undelivered.
#[tokio::test]
async fn a_read_message_is_not_reported_as_undelivered() {
    let (_tmp, dir): Workspace = setup_workspace();
    let store = seed_team(&dir, "delivered-team", MemberStatus::Stopped);
    enqueue(&store, "lead", "already handled");

    let ids: Vec<String> = store
        .mailbox("tm-001")
        .expect("mailbox")
        .read_all()
        .expect("read_all")
        .into_iter()
        .map(|m| m.message_id)
        .collect();
    store
        .mailbox("tm-001")
        .expect("mailbox")
        .mark_all_read(&ids)
        .expect("mark read");

    assert!(
        undelivered_messages(&store).is_empty(),
        "a delivered message is not undelivered"
    );
}

// -- a live or parked recipient is not reported -----------------------------

/// A member that is still alive or parked will drain its own mailbox, so its
/// backlog is not stranded - reporting it would be noise on every status call.
#[tokio::test]
async fn a_live_or_parked_recipient_is_never_reported() {
    let (_tmp, dir): Workspace = setup_workspace();
    let statuses = [
        MemberStatus::Spawning,
        MemberStatus::Working,
        MemberStatus::Idle,
        MemberStatus::PlanPending,
        MemberStatus::Blocked,
        MemberStatus::Suspended,
        MemberStatus::ShuttingDown,
    ];
    for status in statuses {
        let team = format!("alive-{}", status.as_str().replace('_', "-"));
        let store = seed_team(&dir, &team, status);
        enqueue(&store, "lead", "will be read when it wakes");
        assert!(
            undelivered_messages(&store).is_empty(),
            "{status:?} can still read its mailbox and must not be reported"
        );
    }
}

// -- the rendered report ----------------------------------------------------

/// The rendered block names the count, every recipient with its status, the
/// sender, the type, and the message body - entity, state, and content.
#[tokio::test]
async fn the_rendered_block_names_recipient_status_sender_and_content() {
    let (_tmp, dir): Workspace = setup_workspace();
    let store = seed_team(&dir, "render-team", MemberStatus::Failed);
    enqueue(&store, "lead", "unblock task-002 before stopping");

    let found = undelivered_messages(&store);
    let block = render_undelivered("render-team", &found, 2);

    assert!(block.contains("## Undelivered messages (1)"), "{block}");
    assert!(block.contains("worker"), "{block}");
    assert!(block.contains("tm-001"), "{block}");
    assert!(block.contains("failed"), "{block}");
    assert!(block.contains("from lead"), "{block}");
    assert!(block.contains("type message"), "{block}");
    assert!(
        block.contains("unblock task-002 before stopping"),
        "{block}"
    );
    // The block states that the message was kept, and names both next actions.
    assert!(block.contains("not deleted"), "{block}");
    assert!(block.contains("team_read_messages"), "{block}");
}

/// A team with nothing stranded renders nothing - no empty heading.
#[tokio::test]
async fn nothing_stranded_renders_nothing() {
    assert_eq!(render_undelivered("quiet-team", &[], 2), String::new());
    assert_eq!(
        render_undelivered("quiet-team", &[], 1),
        String::new(),
        "an empty report must not emit a heading"
    );
}

/// The heading depth follows the caller so the block nests correctly in
/// `/team show` (h2) and sits at the top level of `team_status` (h1).
#[tokio::test]
async fn the_heading_depth_follows_the_caller() {
    let (_tmp, dir): Workspace = setup_workspace();
    let store = seed_team(&dir, "depth-team", MemberStatus::Stopped);
    enqueue(&store, "lead", "body");
    let found = undelivered_messages(&store);

    assert!(render_undelivered("depth-team", &found, 1).contains("# Undelivered messages (1)"));
    assert!(render_undelivered("depth-team", &found, 2).contains("## Undelivered messages (1)"));
}

/// Several messages for several terminated recipients all appear, one row each,
/// in roster order.
#[tokio::test]
async fn several_stranded_messages_all_appear() {
    let (_tmp, dir): Workspace = setup_workspace();
    let mut store = TeamStore::create("multi-team", "lead-sess", &dir, true).expect("create");
    for (name, agent_id) in [("alpha", "tm-001"), ("beta", "tm-002")] {
        let mut member = TeamMember::new(name, agent_id, "general");
        member.status = MemberStatus::Stopped;
        store.add_member(member).expect("add_member");
    }
    for (agent_id, content) in [("tm-001", "for alpha"), ("tm-002", "for beta")] {
        let message = ragent_agent::team::MailboxMessage::new(
            "lead",
            agent_id,
            MessageType::Message,
            content,
        )
        .with_sender_session("lead-sess");
        store
            .mailbox(agent_id)
            .expect("mailbox")
            .push(message)
            .expect("push");
    }

    let found = undelivered_messages(&store);
    assert_eq!(found.len(), 2, "both stranded messages expected: {found:?}");
    let block = render_undelivered("multi-team", &found, 2);
    assert!(block.contains("Undelivered messages (2)"), "{block}");
    assert!(block.contains("for alpha"), "{block}");
    assert!(block.contains("for beta"), "{block}");
}

// -- team_status surfaces it ------------------------------------------------

/// `team_status` renders the undelivered block and mirrors it in metadata, so
/// an agent reading the tool result learns what was stranded.
#[tokio::test]
async fn team_status_reports_undelivered_messages() {
    let (_tmp, dir): Workspace = setup_workspace();
    let store = seed_team(&dir, "status-team", MemberStatus::Stopped);
    enqueue(&store, "lead", "task-004 is blocked on the schema change");
    drop(store);

    let ctx = make_ctx(&dir);
    let out = ragent_agent::tool::team_status::TeamStatusTool
        .execute(json!({ "team_name": "status-team" }), &ctx)
        .await
        .expect("team_status execute");

    assert!(
        out.content.contains("Undelivered messages (1)"),
        "team_status must report the stranded message: {}",
        out.content
    );
    assert!(
        out.content
            .contains("task-004 is blocked on the schema change"),
        "the stranded content must be visible: {}",
        out.content
    );
    assert!(
        out.content.contains("tm-001"),
        "the stranded recipient must be named: {}",
        out.content
    );

    let meta = out.metadata.expect("metadata");
    assert_eq!(meta["undelivered_count"], json!(1), "{meta}");
    assert_eq!(
        meta["undelivered"][0]["agent_id"],
        json!("tm-001"),
        "{meta}"
    );
    assert_eq!(
        meta["undelivered"][0]["recipient_status"],
        json!("stopped"),
        "{meta}"
    );
    assert_eq!(
        meta["undelivered"][0]["content"],
        json!("task-004 is blocked on the schema change"),
        "{meta}"
    );
}

/// A healthy team's `team_status` carries no undelivered section, and reports a
/// zero count so a caller can assert on the shape rather than on absence.
#[tokio::test]
async fn team_status_has_no_undelivered_section_when_nothing_is_stranded() {
    let (_tmp, dir): Workspace = setup_workspace();
    let mut store = TeamStore::create("healthy-team", "lead-sess", &dir, true).expect("create");
    let mut member = TeamMember::new("worker", "tm-001", "general");
    member.status = MemberStatus::Working;
    store.add_member(member).expect("add_member");
    drop(store);

    let ctx = make_ctx(&dir);
    let out = ragent_agent::tool::team_status::TeamStatusTool
        .execute(json!({ "team_name": "healthy-team" }), &ctx)
        .await
        .expect("team_status execute");

    assert!(
        !out.content.contains("Undelivered"),
        "no section when nothing is stranded: {}",
        out.content
    );
    let meta = out.metadata.expect("metadata");
    assert_eq!(meta["undelivered_count"], json!(0), "{meta}");
    assert_eq!(meta["undelivered"], json!([]), "{meta}");
}

/// Reporting does not consume the message: two consecutive `team_status` calls
/// both see it, and the mailbox row is still unread afterwards.
#[tokio::test]
async fn reporting_is_idempotent_and_does_not_mark_the_message_read() {
    let (_tmp, dir): Workspace = setup_workspace();
    let store = seed_team(&dir, "idem-team", MemberStatus::Stopped);
    enqueue(&store, "lead", "still here");
    drop(store);

    let ctx = make_ctx(&dir);
    for call in 1..=2 {
        let out = ragent_agent::tool::team_status::TeamStatusTool
            .execute(json!({ "team_name": "idem-team" }), &ctx)
            .await
            .expect("team_status execute");
        assert!(
            out.content.contains("still here"),
            "call {call} must still report the message: {}",
            out.content
        );
    }

    let store = TeamStore::load_by_name("idem-team", &dir).expect("reload");
    let all = store
        .mailbox("tm-001")
        .expect("mailbox")
        .read_all()
        .expect("read_all");
    assert_eq!(all.len(), 1);
    assert!(
        !all[0].read,
        "reporting must not mark the stranded message read"
    );
}
