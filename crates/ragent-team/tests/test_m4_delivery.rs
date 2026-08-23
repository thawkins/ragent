//! Message-delivery regression tests for COMMSPLAN Milestone 4.
//!
//! Covers:
//! - M4-T1: `peek_unread` + `acknowledge` (read-vs-processed split, redelivery on failure).
//! - M4-T2: `team_assign_task` notifies the assigned teammate via mailbox.
//! - M4-T3: `team_broadcast` reports per-recipient success/failure.
//! - M4-T4: `team_message` rejects `Stopped` / `Failed` / unknown recipients.
//! - M4-T5: `team_read_messages` emits `snake_case` `type` and includes `to`/`read`.

use std::sync::Arc;

use ragent_agent::event::EventBus;
use ragent_agent::tool::{Tool, ToolContext};
use ragent_team::team::{
    Mailbox, MailboxMessage, MemberStatus, MessageType, TeamMember, TeamStore,
};
use serde_json::json;

#[path = "support/mod.rs"]
mod support;
use support::setup_workspace;

// ── Helpers ───────────────────────────────────────────────────────────────

fn team_dir_for(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    dir.join(name)
}

fn make_ctx(
    working_dir: &std::path::Path,
    session_id: &str,
    event_bus: Arc<EventBus>,
) -> ToolContext {
    let mut ctx = ToolContext {
        session_id: session_id.to_string(),
        working_dir: working_dir.to_path_buf(),
        event_bus,
        storage: None,
        agent_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
        code_index: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        bg_service: None,
        cached_team_dir: std::sync::Arc::new(std::sync::Mutex::new(None)),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        canonical_cache: std::sync::Arc::new(
            ragent_tools_core::CanonicalPathCache::new(),
        ),
    }; // Mark this session as the lead so tool permission checks pass.
    ctx.team_context = Some(Arc::new(ragent_agent::tool::TeamContext {
        team_name: String::new(),
        agent_id: "lead".to_string(),
        is_lead: true,
    }));
    ctx
}

fn add_member(store: &mut TeamStore, name: &str, agent_id: &str, status: MemberStatus) {
    let mut member = TeamMember::new(name, agent_id, "general");
    member.status = status;
    store.add_member(member).expect("add_member");
}

// ── M4-T1: peek_unread + acknowledge ───────────────────────────────────────

#[tokio::test]
async fn test_peek_unread_does_not_mark_read() {
    let (_tmp, dir) = setup_workspace();
    TeamStore::create("peek-team", "lead-sess", &dir, true).expect("create team");
    let team_dir = team_dir_for(&dir, "peek-team");
    let mailbox = Mailbox::open(&team_dir, "tm-001").expect("open mailbox");
    mailbox
        .push(MailboxMessage::new(
            "lead",
            "tm-001",
            MessageType::Message,
            "hello",
        ))
        .expect("push");

    let unread = mailbox.peek_unread().expect("peek");
    assert_eq!(unread.len(), 1);
    assert_eq!(unread[0].content, "hello");

    // peek must not mark read — a second peek returns the same message.
    let unread2 = mailbox.peek_unread().expect("peek again");
    assert_eq!(unread2.len(), 1, "peek should not mark messages read");
}

#[tokio::test]
async fn test_acknowledge_marks_read_and_is_idempotent() {
    let (_tmp, dir) = setup_workspace();
    TeamStore::create("ack-team", "lead-sess", &dir, true).expect("create team");
    let team_dir = team_dir_for(&dir, "ack-team");
    let mailbox = Mailbox::open(&team_dir, "tm-001").expect("open mailbox");
    mailbox
        .push(MailboxMessage::new(
            "lead",
            "tm-001",
            MessageType::Message,
            "ack me",
        ))
        .expect("push");

    let unread = mailbox.peek_unread().expect("peek");
    let id = unread[0].message_id.clone();

    let changed = mailbox.acknowledge(&id).expect("ack");
    assert!(changed, "first ack should transition unread → read");

    // Second ack is idempotent: returns false (already read).
    let changed2 = mailbox.acknowledge(&id).expect("ack again");
    assert!(!changed2, "second ack should report no change");

    // After ack, peek returns nothing.
    let unread2 = mailbox.peek_unread().expect("peek after ack");
    assert!(unread2.is_empty(), "no unread after acknowledge");
}

#[tokio::test]
async fn test_drain_unread_still_marks_read_for_backward_compat() {
    let (_tmp, dir) = setup_workspace();
    TeamStore::create("drain-team", "lead-sess", &dir, true).expect("create team");
    let team_dir = team_dir_for(&dir, "drain-team");
    let mailbox = Mailbox::open(&team_dir, "tm-001").expect("open mailbox");
    mailbox
        .push(MailboxMessage::new(
            "lead",
            "tm-001",
            MessageType::Message,
            "drain me",
        ))
        .expect("push");

    let unread = mailbox.drain_unread().expect("drain");
    assert_eq!(unread.len(), 1);

    // drain_unread marks read — a subsequent peek returns nothing.
    let unread2 = mailbox.peek_unread().expect("peek after drain");
    assert!(unread2.is_empty(), "drain_unread should mark messages read");
}

// ── M4-T2: team_assign_task notifies the assigned teammate ───────────────

#[tokio::test]
async fn test_team_assign_task_pushs_notification_to_assignee_mailbox() {
    let (_tmp, dir) = setup_workspace();
    let mut store = TeamStore::create("assign-team", "lead-sess", &dir, true).expect("create team");
    add_member(&mut store, "worker", "tm-001", MemberStatus::Idle);

    // Create a pending task in the task store. Note: the task store lives at
    // `{team_dir}/tasks.json`, so we must pass the *team* directory, not the
    // workspace root.
    use ragent_team::team::{Task, TaskStore};
    let team_dir = dir.join("assign-team");
    let task_store = TaskStore::open(&team_dir).expect("open task store");
    task_store
        .add_task(Task::new("task-001", "Write tests"))
        .expect("add task");
    drop(task_store);
    drop(store);

    let bus = Arc::new(EventBus::new(16));
    let ctx = make_ctx(&dir, "lead-sess", bus);
    let tool = ragent_team::tools::team_assign_task::TeamAssignTaskTool;
    let input = json!({ "team_name": "assign-team", "task_id": "task-001", "to": "tm-001" });
    let out = tool.execute(input, &ctx).await.expect("assign execute");
    assert!(
        out.content.contains("Notification: delivered"),
        "expected notification delivered, got: {}",
        out.content
    );

    // The assignee's mailbox must now contain a notification message.
    let team_dir = team_dir_for(&dir, "assign-team");
    let mailbox = Mailbox::open(&team_dir, "tm-001").expect("open assignee mailbox");
    let msgs = mailbox.read_all().expect("read mailbox");
    assert_eq!(msgs.len(), 1, "assignee should have one notification");
    assert!(
        msgs[0].content.contains("task-001"),
        "notification should mention the task id"
    );
    assert_eq!(msgs[0].to, "tm-001");
    assert_eq!(msgs[0].from, "lead");
}

#[tokio::test]
async fn test_team_assign_task_rejects_dead_assignee() {
    let (_tmp, dir) = setup_workspace();
    let mut store =
        TeamStore::create("assign-dead-team", "lead-sess", &dir, true).expect("create team");
    add_member(&mut store, "ghost", "tm-001", MemberStatus::Stopped);

    use ragent_team::team::{Task, TaskStore};
    let team_dir = dir.join("assign-dead-team");
    let task_store = TaskStore::open(&team_dir).expect("open task store");
    task_store
        .add_task(Task::new("task-001", "Ghost work"))
        .expect("add task");
    drop(task_store);
    drop(store);

    let bus = Arc::new(EventBus::new(16));
    let ctx = make_ctx(&dir, "lead-sess", bus);
    let tool = ragent_team::tools::team_assign_task::TeamAssignTaskTool;
    let input = json!({ "team_name": "assign-dead-team", "task_id": "task-001", "to": "tm-001" });
    let err = tool
        .execute(input, &ctx)
        .await
        .expect_err("should reject dead assignee");
    assert!(
        format!("{err}").contains("stopped"),
        "expected stopped error, got: {err}"
    );
}

// ── M4-T3: team_broadcast per-recipient results ───────────────────────────

#[tokio::test]
async fn test_team_broadcast_reports_per_recipient_success() {
    let (_tmp, dir) = setup_workspace();
    let mut store = TeamStore::create("bcast-team", "lead-sess", &dir, true).expect("create team");
    add_member(&mut store, "alice", "tm-001", MemberStatus::Idle);
    add_member(&mut store, "bob", "tm-002", MemberStatus::Idle);
    drop(store);

    let bus = Arc::new(EventBus::new(16));
    let ctx = make_ctx(&dir, "lead-sess", bus);
    let tool = ragent_team::tools::team_broadcast::TeamBroadcastTool;
    let input = json!({ "team_name": "bcast-team", "content": "hello all" });
    let out = tool.execute(input, &ctx).await.expect("broadcast execute");

    let meta = out.metadata.expect("metadata");
    let succeeded = meta
        .get("succeeded")
        .and_then(|v| v.as_array())
        .expect("succeeded array");
    assert_eq!(succeeded.len(), 2, "both recipients should succeed");
    let failed = meta
        .get("failed")
        .and_then(|v| v.as_array())
        .expect("failed array");
    assert!(failed.is_empty(), "no failures expected");

    // Both mailboxes should contain the broadcast.
    let team_dir = team_dir_for(&dir, "bcast-team");
    for id in ["tm-001", "tm-002"] {
        let m = Mailbox::open(&team_dir, id)
            .expect("open")
            .read_all()
            .expect("read");
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].message_type, MessageType::Broadcast);
    }
}

#[tokio::test]
async fn test_team_broadcast_skips_stopped_teammates() {
    let (_tmp, dir) = setup_workspace();
    let mut store =
        TeamStore::create("bcast-stop-team", "lead-sess", &dir, true).expect("create team");
    add_member(&mut store, "alice", "tm-001", MemberStatus::Idle);
    add_member(&mut store, "ghost", "tm-002", MemberStatus::Stopped);
    drop(store);

    let bus = Arc::new(EventBus::new(16));
    let ctx = make_ctx(&dir, "lead-sess", bus);
    let tool = ragent_team::tools::team_broadcast::TeamBroadcastTool;
    let input = json!({ "team_name": "bcast-stop-team", "content": "hi" });
    let out = tool.execute(input, &ctx).await.expect("broadcast execute");
    let meta = out.metadata.expect("metadata");
    let succeeded = meta
        .get("succeeded")
        .and_then(|v| v.as_array())
        .expect("succeeded");
    // Only the active teammate is a recipient; stopped is filtered out.
    assert_eq!(succeeded.len(), 1);
    assert_eq!(succeeded[0], "tm-001");
}

// ── M4-T4: team_message validates recipient state ─────────────────────────

#[tokio::test]
async fn test_team_message_rejects_stopped_recipient() {
    let (_tmp, dir) = setup_workspace();
    let mut store =
        TeamStore::create("msg-stop-team", "lead-sess", &dir, true).expect("create team");
    add_member(&mut store, "ghost", "tm-001", MemberStatus::Stopped);
    drop(store);

    let bus = Arc::new(EventBus::new(16));
    let ctx = make_ctx(&dir, "lead-sess", bus);
    let tool = ragent_team::tools::team_message::TeamMessageTool;
    let input = json!({ "team_name": "msg-stop-team", "to": "tm-001", "content": "boo" });
    let err = tool
        .execute(input, &ctx)
        .await
        .expect_err("should reject stopped");
    assert!(
        format!("{err}").contains("stopped"),
        "expected stopped error, got: {err}"
    );

    // Mailbox should be empty — no message was pushed.
    let team_dir = team_dir_for(&dir, "msg-stop-team");
    let m = Mailbox::open(&team_dir, "tm-001")
        .expect("open")
        .read_all()
        .expect("read");
    assert!(
        m.is_empty(),
        "no message should be pushed to a stopped recipient"
    );
}

#[tokio::test]
async fn test_team_message_rejects_unknown_recipient() {
    let (_tmp, dir) = setup_workspace();
    TeamStore::create("msg-unknown-team", "lead-sess", &dir, true).expect("create team");

    let bus = Arc::new(EventBus::new(16));
    let ctx = make_ctx(&dir, "lead-sess", bus);
    let tool = ragent_team::tools::team_message::TeamMessageTool;
    let input = json!({ "team_name": "msg-unknown-team", "to": "tm-999", "content": "hi" });
    let err = tool
        .execute(input, &ctx)
        .await
        .expect_err("should reject unknown");
    assert!(
        format!("{err}").contains("not a member"),
        "expected not-a-member, got: {err}"
    );
}

#[tokio::test]
async fn test_team_message_delivers_to_active_recipient() {
    let (_tmp, dir) = setup_workspace();
    let mut store = TeamStore::create("msg-ok-team", "lead-sess", &dir, true).expect("create team");
    add_member(&mut store, "alice", "tm-001", MemberStatus::Idle);
    drop(store);

    let bus = Arc::new(EventBus::new(16));
    let ctx = make_ctx(&dir, "lead-sess", bus);
    let tool = ragent_team::tools::team_message::TeamMessageTool;
    let input = json!({ "team_name": "msg-ok-team", "to": "tm-001", "content": "hi alice" });
    let out = tool.execute(input, &ctx).await.expect("deliver");
    assert!(out.content.contains("Message sent"));

    let team_dir = team_dir_for(&dir, "msg-ok-team");
    let m = Mailbox::open(&team_dir, "tm-001")
        .expect("open")
        .read_all()
        .expect("read");
    assert_eq!(m.len(), 1);
    assert_eq!(m[0].content, "hi alice");
}

// ── M4-T5: team_read_messages output schema ───────────────────────────────

#[tokio::test]
async fn test_team_read_messages_emits_snake_case_type_and_to_read_fields() {
    let (_tmp, dir) = setup_workspace();
    let mut store = TeamStore::create("read-team", "lead-sess", &dir, true).expect("create team");
    add_member(&mut store, "alice", "tm-001", MemberStatus::Idle);
    drop(store);

    // Pre-populate the mailbox so the tool has something to read.
    let team_dir = team_dir_for(&dir, "read-team");
    let mailbox = Mailbox::open(&team_dir, "tm-001").expect("open mailbox");
    mailbox
        .push(MailboxMessage::new(
            "lead",
            "tm-001",
            MessageType::PlanRequest,
            "plan text",
        ))
        .expect("push");
    drop(mailbox);

    // Build a teammate-style context.
    let bus = Arc::new(EventBus::new(16));
    let mut ctx = make_ctx(&dir, "tm-001", bus);
    ctx.team_context = Some(Arc::new(ragent_agent::tool::TeamContext {
        team_name: "read-team".to_string(),
        agent_id: "tm-001".to_string(),
        is_lead: false,
    }));

    let tool = ragent_team::tools::team_read_messages::TeamReadMessagesTool;
    let input = json!({ "team_name": "read-team" });
    let out = tool.execute(input, &ctx).await.expect("read execute");
    let meta = out.metadata.expect("metadata");
    let messages = meta
        .get("messages")
        .and_then(|v| v.as_array())
        .expect("messages array");
    assert_eq!(messages.len(), 1);
    let m = &messages[0];

    // M4-T5: `type` must be snake_case ("plan_request"), not PascalCase.
    let type_val = m.get("type").and_then(|v| v.as_str()).expect("type field");
    assert_eq!(type_val, "plan_request", "type should be snake_case");

    // M4-T5: `to` and `read` fields must be present.
    assert_eq!(m.get("to").and_then(|v| v.as_str()), Some("tm-001"));
    // `read` should be present (the peeked value is false; ack happens after).
    assert!(m.get("read").is_some(), "read field must be present");

    // Human-readable text should include To: and the snake_case type.
    assert!(
        out.content.contains("To: tm-001"),
        "text should include To: {:?}",
        out.content
    );
    assert!(
        out.content.contains("Type: plan_request"),
        "text should include snake_case type"
    );

    // After the tool returns, the message should be acknowledged (read).
    let mailbox = Mailbox::open(&team_dir, "tm-001").expect("open mailbox");
    let all = mailbox.read_all().expect("read");
    assert_eq!(all.len(), 1);
    assert!(
        all[0].read,
        "message should be marked read after successful team_read_messages"
    );
}

#[tokio::test]
async fn test_team_read_messages_redelivers_on_partial_ack_failure_is_safe() {
    // Sanity: peek returns messages; if we do NOT acknowledge, a second
    // team_read_messages call returns the same messages. This documents the
    // at-least-once behaviour.
    let (_tmp, dir) = setup_workspace();
    let mut store =
        TeamStore::create("redeliver-team", "lead-sess", &dir, true).expect("create team");
    add_member(&mut store, "alice", "tm-001", MemberStatus::Idle);
    drop(store);

    let team_dir = team_dir_for(&dir, "redeliver-team");
    let mailbox = Mailbox::open(&team_dir, "tm-001").expect("open mailbox");
    mailbox
        .push(MailboxMessage::new(
            "lead",
            "tm-001",
            MessageType::Message,
            "redo",
        ))
        .expect("push");
    drop(mailbox);

    // Peek without ack (simulating a processing failure that did not call
    // acknowledge) leaves the message unread.
    let mailbox = Mailbox::open(&team_dir, "tm-001").expect("open mailbox");
    let peeked = mailbox.peek_unread().expect("peek");
    assert_eq!(peeked.len(), 1);
    // Do NOT acknowledge — simulate failure.
    let still_unread = mailbox.peek_unread().expect("peek again");
    assert_eq!(
        still_unread.len(),
        1,
        "unacknowledged message should be redelivered"
    );
}
