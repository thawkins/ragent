//! Machine-readable reason codes on the `cancel_agent` and `wait_agents`
//! refusals (spec `agentssetup` FR-010, NFR-005).
//!
//! A caller cannot branch on English prose, so every refusal these tools can
//! produce names its condition with a stable code. Errors carry it in the
//! message (`reason_code: <code>: <explanation>`), and the failures that are
//! reported as *results* carry it in `metadata.reason`.

use std::path::PathBuf;
use std::sync::Arc;

use ragent_agent::tool::cancel_agent::CancelAgentTool;
use ragent_agent::tool::param_schema::reason_in;
use ragent_agent::tool::wait_agents::WaitAgentsTool;
use ragent_agent::tool::{Tool, ToolContext};
use serde_json::json;

#[path = "task_support/mod.rs"]
mod task_support;
use task_support::manager;

/// A `ToolContext` with no `AgentManager` wired in.
fn base_ctx() -> ToolContext {
    use ragent_agent::event::EventBus;
    ToolContext {
        session_id: "parent".to_string(),
        working_dir: PathBuf::from("target/temp"),
        event_bus: Arc::new(EventBus::new(16)),
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
        tool_registry: ToolContext::default_tool_registry(),
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
    }
}

/// `cancel_agent`'s refusal to act without a registry carries its own code.
#[tokio::test]
async fn test_cancel_agent_names_a_missing_manager() {
    let err = CancelAgentTool
        .execute(json!({ "task_id": "explore-1" }), &base_ctx())
        .await
        .expect_err("no manager must error");
    assert_eq!(reason_in(&err.to_string()), Some("manager_unavailable"));
    assert!(
        err.to_string()
            .contains("AgentManager has not been initialised"),
        "the refusal must name the condition: {err}"
    );
}

/// An id the registry does not hold is reported as a *result* carrying a
/// reason, never as a successful cancellation (FR-010).
#[tokio::test]
async fn test_cancel_agent_reports_an_unknown_task() {
    let mut ctx = base_ctx();
    ctx.agent_manager = Some(manager(8));

    let out = CancelAgentTool
        .execute(json!({ "task_id": "explore-absent" }), &ctx)
        .await
        .expect("an unknown task is a result, not an escalation");
    assert_eq!(
        out.metadata.as_ref().and_then(|m| m.get("reason")),
        Some(&json!("task_not_found")),
        "metadata: {:?}",
        out.metadata
    );
    assert_eq!(
        out.metadata
            .as_ref()
            .and_then(|m| m.get("cancel_requested")),
        Some(&json!(false)),
        "an unknown task must not claim a cancellation: {:?}",
        out.metadata
    );
    assert!(
        out.content.to_lowercase().contains("list_agents"),
        "the content must name a supported next action: {}",
        out.content
    );
}

/// `wait_agents` names a malformed `task_ids` value, its own missing-manager
/// case, and the outcome of a completed wait.
#[tokio::test]
async fn test_wait_agents_reason_codes() {
    let err = WaitAgentsTool
        .execute(json!({ "task_ids": "explore-1" }), &base_ctx())
        .await
        .expect_err("task_ids must be an array");
    assert_eq!(reason_in(&err.to_string()), Some("invalid_task_ids"));
    assert!(
        err.to_string().to_lowercase().contains("omit `task_ids`"),
        "the refusal must name a supported next action: {err}"
    );

    let err = WaitAgentsTool
        .execute(json!({ "task_ids": [7] }), &base_ctx())
        .await
        .expect_err("a non-string element must be refused");
    assert_eq!(reason_in(&err.to_string()), Some("invalid_task_ids"));

    let err = WaitAgentsTool
        .execute(json!({}), &base_ctx())
        .await
        .expect_err("no manager must error");
    assert_eq!(reason_in(&err.to_string()), Some("manager_unavailable"));

    // Nothing to await: the result still names its outcome.
    let mut ctx = base_ctx();
    ctx.agent_manager = Some(manager(8));
    let out = WaitAgentsTool
        .execute(json!({}), &ctx)
        .await
        .expect("an empty wait is a result");
    assert_eq!(
        out.metadata.as_ref().and_then(|m| m.get("reason")),
        Some(&json!("wait_settled"))
    );
}
