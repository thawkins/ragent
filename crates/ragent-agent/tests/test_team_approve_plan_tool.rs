//! The `team_approve_plan` tool (spec `teamssetup`).
//!
//! `team_approve_plan` had **zero** references in `crates/ragent-agent/tests/`,
//! and neither did the branch that the description calls out most loudly:
//! *what happens when the lead rejects without feedback*. These tests pin the
//! full verdict contract:
//!
//! 1. A missing required parameter is rejected by name.
//! 2. A verdict on a teammate that is not `plan_pending` is refused, and the
//!    refusal names the member's real state - it does not silently write a
//!    `plan_status` on a member that never submitted a plan.
//! 3. **Approval** moves the member to `Working`, records `PlanStatus::Approved`
//!    and delivers a `PlanApproved` mailbox message carrying the lead's
//!    authenticated session id and the plan's correlation id.
//! 4. **Rejection with feedback** keeps the member `plan_pending` (so the UI
//!    shows a resubmission is needed), records `PlanStatus::Rejected`, and the
//!    feedback is delivered verbatim.
//! 5. **Rejection without feedback** still produces a `PlanRejected` message:
//!    the tool substitutes its documented default rather than failing or
//!    sending an empty body.
//! 6. The member's `plan_request_id` is cleared once the verdict is sent, so a
//!    later approval cannot be correlated with a stale plan.
//!
//! Every fixture lives in its own `tempfile::tempdir()`, so nothing is written
//! into the repository.

use ragent_agent::team::{Mailbox, MemberStatus, PlanStatus, TeamMember, TeamStore};
use ragent_agent::tool::team_approve_plan::TeamApprovePlanTool;
use ragent_agent::tool::{TeamContext, Tool, ToolContext};
use serde_json::json;

const LEAD: &str = "lead";
const TEAMMATE_NAME: &str = "reviewer";
const TEAMMATE_ID: &str = "tm-001";

/// A workspace holding a project-local team with one member parked in
/// `plan_pending` with an outstanding `plan_request_id`.
///
/// Returns the tempdir (keep it alive), the workspace path, and the team
/// directory.
fn workspace_with_plan_pending_teammate()
-> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let workspace = tmp.path().to_path_buf();
    std::fs::create_dir_all(workspace.join(".ragent")).expect("create .ragent");
    let mut store = TeamStore::create("alpha", LEAD, &workspace, true).expect("create team");

    let mut member = TeamMember::new(TEAMMATE_NAME, TEAMMATE_ID, "general");
    member.status = MemberStatus::PlanPending;
    member.plan_status = PlanStatus::Pending;
    member.plan_request_id = Some("plan-req-7".to_string());
    store.config.members.push(member);
    store.save().expect("save team config");

    let dir = store.dir.clone();
    (tmp, workspace, dir)
}

/// A minimal `ToolContext` acting as the lead of `alpha`, in session `lead-session`.
fn tool_context(workspace: &std::path::Path) -> ToolContext {
    ToolContext {
        session_id: "lead-session".to_string(),
        working_dir: workspace.to_path_buf(),
        event_bus: std::sync::Arc::new(ragent_agent::event::EventBus::new(16)),
        storage: None,
        agent_manager: None,
        active_model: None,
        provider_registry: None,
        team_context: Some(std::sync::Arc::new(TeamContext {
            team_name: "alpha".to_string(),
            agent_id: LEAD.to_string(),
            is_lead: true,
        })),
        team_manager: None,
        code_index: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        bg_service: None,
        cached_team_dir: std::sync::Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        canonical_cache: std::sync::Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        allowed_roots: Vec::new(),
        tool_registry: ToolContext::default_tool_registry(),
    }
}

/// Read the single unread message in the teammate's mailbox.
fn only_message(team_dir: &std::path::Path) -> ragent_agent::team::MailboxMessage {
    let mailbox = Mailbox::open(team_dir, TEAMMATE_ID).expect("open mailbox");
    let messages = mailbox.drain_unread().expect("read mailbox");
    assert_eq!(
        messages.len(),
        1,
        "exactly one verdict message must be delivered"
    );
    messages.into_iter().next().expect("one message")
}

#[tokio::test]
async fn test_missing_required_parameter_is_named() {
    let (_tmp, workspace, _dir) = workspace_with_plan_pending_teammate();
    let ctx = tool_context(&workspace);

    let err = TeamApprovePlanTool
        .execute(json!({ "team_name": "alpha", "approved": true }), &ctx)
        .await
        .expect_err("a call with no teammate must be refused");
    assert!(
        err.to_string().contains("teammate"),
        "the refusal must name the missing parameter, got: {err}"
    );

    let err = TeamApprovePlanTool
        .execute(
            json!({ "team_name": "alpha", "teammate": TEAMMATE_NAME }),
            &ctx,
        )
        .await
        .expect_err("a call with no approved flag must be refused");
    assert!(
        err.to_string().contains("approved"),
        "the refusal must name the missing parameter, got: {err}"
    );
}

#[tokio::test]
async fn test_verdict_on_a_member_without_a_plan_is_refused() {
    let (tmp, workspace, team_dir) = workspace_with_plan_pending_teammate();
    let _ = tmp;
    // Move the member out of plan_pending: it is working, not awaiting a verdict.
    {
        let mut store = TeamStore::load(&team_dir).expect("load team");
        let member = store
            .config
            .member_by_id_mut(TEAMMATE_ID)
            .expect("member present");
        member.status = MemberStatus::Working;
        member.plan_status = PlanStatus::None;
        store.save().expect("save team");
    }

    let ctx = tool_context(&workspace);
    let err = TeamApprovePlanTool
        .execute(
            json!({ "team_name": "alpha", "teammate": TEAMMATE_NAME, "approved": true }),
            &ctx,
        )
        .await
        .expect_err("a verdict on a working member must be refused");

    let message = err.to_string();
    assert!(
        message.contains("plan_pending") || message.contains("plan awaiting a verdict"),
        "the refusal must name the state a plan verdict needs, got: {message}"
    );

    // Nothing was written: the member is still Working and its plan_status untouched.
    let store = TeamStore::load(&team_dir).expect("reload team");
    let member = store
        .config
        .member_by_id(TEAMMATE_ID)
        .expect("member present");
    assert_eq!(member.status, MemberStatus::Working);
    assert_eq!(member.plan_status, PlanStatus::None);
}

#[tokio::test]
async fn test_approval_moves_the_member_to_working_and_delivers_the_verdict() {
    let (_tmp, workspace, team_dir) = workspace_with_plan_pending_teammate();
    let ctx = tool_context(&workspace);

    let out = TeamApprovePlanTool
        .execute(
            json!({ "team_name": "alpha", "teammate": TEAMMATE_NAME, "approved": true }),
            &ctx,
        )
        .await
        .expect("approving a parked plan must succeed");

    assert!(out.content.contains("approved"), "content: {}", out.content);

    let store = TeamStore::load(&team_dir).expect("reload team");
    let member = store.config.member_by_id(TEAMMATE_ID).expect("member");
    assert_eq!(
        member.status,
        MemberStatus::Working,
        "approval must release the teammate to work"
    );
    assert_eq!(member.plan_status, PlanStatus::Approved);
    assert_eq!(
        member.plan_request_id, None,
        "the correlation id must be cleared once the verdict is sent"
    );

    let message = only_message(&team_dir);
    assert_eq!(
        message.message_type,
        ragent_agent::team::MessageType::PlanApproved
    );
    assert_eq!(
        message.correlation_id.as_deref(),
        Some("plan-req-7"),
        "the verdict must carry the plan's correlation id"
    );
    assert_eq!(
        message.sender_session_id.as_deref(),
        Some("lead-session"),
        "the verdict must carry the lead's authenticated session id"
    );
}

#[tokio::test]
async fn test_rejection_with_feedback_keeps_the_member_waiting() {
    let (_tmp, workspace, team_dir) = workspace_with_plan_pending_teammate();
    let ctx = tool_context(&workspace);

    TeamApprovePlanTool
        .execute(
            json!({
                "team_name": "alpha",
                "teammate": TEAMMATE_NAME,
                "approved": false,
                "feedback": "Split the migration into two steps"
            }),
            &ctx,
        )
        .await
        .expect("rejecting with feedback must succeed");

    let store = TeamStore::load(&team_dir).expect("reload team");
    let member = store.config.member_by_id(TEAMMATE_ID).expect("member");
    assert_eq!(
        member.status,
        MemberStatus::PlanPending,
        "a rejected plan keeps the teammate parked so it can resubmit"
    );
    assert_eq!(member.plan_status, PlanStatus::Rejected);

    let message = only_message(&team_dir);
    assert_eq!(
        message.message_type,
        ragent_agent::team::MessageType::PlanRejected
    );
    assert!(
        message
            .content
            .contains("Split the migration into two steps"),
        "the lead's feedback must be delivered verbatim, got: {}",
        message.content
    );
}

#[tokio::test]
async fn test_rejection_without_feedback_uses_the_documented_default() {
    // The branch the tool description calls out: feedback is "required when
    // rejecting", but omitting it must not fail the call or deliver an empty
    // message - the tool substitutes its own default text.
    let (_tmp, workspace, team_dir) = workspace_with_plan_pending_teammate();
    let ctx = tool_context(&workspace);

    let out = TeamApprovePlanTool
        .execute(
            json!({ "team_name": "alpha", "teammate": TEAMMATE_NAME, "approved": false }),
            &ctx,
        )
        .await
        .expect("rejecting without feedback must still record a verdict");

    let meta = out.metadata.expect("metadata");
    assert_eq!(meta["approved"], false);
    assert_eq!(
        meta["feedback"], "Plan rejected.",
        "the default rejection text must be recorded in the metadata"
    );

    let message = only_message(&team_dir);
    assert_eq!(
        message.message_type,
        ragent_agent::team::MessageType::PlanRejected
    );
    assert!(
        !message.content.trim().is_empty(),
        "a rejection must never deliver an empty message"
    );
    assert!(
        message.content.contains("rejected"),
        "the delivered text must say the plan was rejected, got: {}",
        message.content
    );
}
