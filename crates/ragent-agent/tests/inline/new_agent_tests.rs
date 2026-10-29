//! Inline tests for `new_agent.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use crate::event::EventBus;
use crate::tool::TeamContext;
use std::path::PathBuf;
use std::sync::Arc;

fn base_ctx() -> ToolContext {
    ToolContext {
        session_id: "session-1".to_string(),
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
#[tokio::test]
async fn test_new_task_without_team_context_tries_to_spawn() {
    let tool = NewAgentTool;
    let ctx = base_ctx();
    let err = tool
        .execute(
            json!({
                "agent": "explore",
                "task": "inspect the repository"
            }),
            &ctx,
        )
        .await
        .expect_err("missing task manager should be the first failure");

    assert!(
        err.to_string()
            .contains("AgentManager has not been initialised"),
        "unexpected error: {err:#}"
    );
}

/// A precondition failure inside an active team is an ERROR, not a successful
/// result whose content is prose (FR-009, FR-045). The refusal must name the
/// team, this session's role, and the supported alternative team tools, and the
/// guidance text must survive the shape change verbatim.
#[tokio::test]
async fn test_new_task_blocks_for_active_team_sessions() {
    let tool = NewAgentTool;
    let mut ctx = base_ctx();
    ctx.team_context = Some(Arc::new(TeamContext {
        team_name: "alpha".to_string(),
        agent_id: "lead".to_string(),
        is_lead: true,
    }));

    let err = tool
        .execute(
            json!({
                "agent": "explore",
                "task": "inspect the repository"
            }),
            &ctx,
        )
        .await
        .expect_err("team-context guard must be a tool error, not a successful result");

    let message = err.to_string();
    assert!(
        message.contains("team_context_active"),
        "refusal must carry the machine-readable reason: {message}"
    );
    assert!(
        message.contains("active team 'alpha'") && message.contains("is the lead"),
        "refusal must name the team and this session's role: {message}"
    );
    for tool_name in [
        "`team_spawn`",
        "`team_task_create`",
        "`team_assign_task`",
        "`team_message`",
    ] {
        assert!(
            message.contains(tool_name),
            "lead refusal must name {tool_name}: {message}"
        );
    }
}

/// The teammate arm names the teammate-side workflow tools instead (FR-045).
#[tokio::test]
async fn test_new_task_blocks_for_teammate_sessions() {
    let tool = NewAgentTool;
    let mut ctx = base_ctx();
    ctx.team_context = Some(Arc::new(TeamContext {
        team_name: "alpha".to_string(),
        agent_id: "tm-001".to_string(),
        is_lead: false,
    }));

    let err = tool
        .execute(
            json!({
                "agent": "explore",
                "task": "inspect the repository"
            }),
            &ctx,
        )
        .await
        .expect_err("team-context guard must refuse a teammate session too");

    let message = err.to_string();
    assert!(
        message.contains("teammate 'tm-001' in active team 'alpha'"),
        "refusal must name the teammate and the team: {message}"
    );
    for tool_name in [
        "`team_read_messages`",
        "`team_task_claim`",
        "`team_task_complete`",
        "`team_idle`",
    ] {
        assert!(
            message.contains(tool_name),
            "teammate refusal must name {tool_name}: {message}"
        );
    }
}

/// `detached: true` without `background: true` selects no contract that exists,
/// so the call is refused rather than degraded to a blocking foreground task
/// (FR-003). The refusal must name both parameters and the legal combinations,
/// and must happen before the `AgentManager` availability check so the caller
/// learns about the bad call shape, not the environment.
#[tokio::test]
async fn test_new_agent_refuses_detached_without_background() {
    let tool = NewAgentTool;
    let ctx = base_ctx();

    let err = tool
        .execute(
            json!({
                "agent": "explore",
                "task": "inspect the repository",
                "detached": true
            }),
            &ctx,
        )
        .await
        .expect_err("detached without background must be refused");

    let message = err.to_string();
    assert!(
        message.contains("`detached: true` requires `background: true`"),
        "refusal must name both parameters: {message}"
    );
    assert!(
        message.contains("background: true, detached: false")
            && message.contains("background: true, detached: true"),
        "refusal must name the two legal combinations: {message}"
    );
    assert!(
        message.contains("omit both parameters"),
        "refusal must name the foreground shape: {message}"
    );
}

/// The three legal combinations derive the three named contracts (FR-001).
#[test]
fn test_new_agent_execution_contract_derivation() {
    assert_eq!(
        Execution::derive(false, false).unwrap(),
        Execution::Foreground
    );
    assert_eq!(
        Execution::derive(true, false).unwrap(),
        Execution::Background
    );
    assert_eq!(Execution::derive(true, true).unwrap(), Execution::Detached);

    assert!(!Execution::Foreground.is_async());
    assert!(Execution::Background.is_async() && !Execution::Background.is_detached());
    assert!(Execution::Detached.is_async() && Execution::Detached.is_detached());
}

/// Every branchable refusal `new_agent` can produce carries a stable,
/// machine-readable reason code that a caller can match instead of the English
/// prose (FR-010). Each code is checked through the shared extractor, so the
/// test fails if a refusal reverts to an uncoded `anyhow!` message.
#[tokio::test]
async fn test_new_agent_refusals_carry_reason_codes() {
    use crate::tool::param_schema::reason_in;

    let tool = NewAgentTool;

    // Contract refusal: `detached` without `background`.
    let err = tool
        .execute(
            json!({ "agent": "explore", "task": "t", "detached": true }),
            &base_ctx(),
        )
        .await
        .expect_err("detached without background must be refused");
    assert_eq!(reason_in(&err.to_string()), Some(REASON_CONTRACT_REFUSED));

    // Per-call bound refusal: zero is not a positive integer.
    let err = tool
        .execute(
            json!({ "agent": "explore", "task": "t", "timeout_secs": 0 }),
            &base_ctx(),
        )
        .await
        .expect_err("a zero time bound must be refused");
    assert_eq!(
        reason_in(&err.to_string()),
        Some(REASON_INVALID_TIMEOUT_SECS)
    );

    // Missing manager: the tool cannot run here at all.
    let err = tool
        .execute(
            json!({ "agent": "explore", "task": "t", "background": true }),
            &base_ctx(),
        )
        .await
        .expect_err("a missing AgentManager must be reported");
    assert_eq!(
        reason_in(&err.to_string()),
        Some(REASON_MANAGER_UNAVAILABLE)
    );

    // Team context: the wrong tool for this session.
    let mut ctx = base_ctx();
    ctx.team_context = Some(Arc::new(TeamContext {
        team_name: "alpha".to_string(),
        agent_id: "lead".to_string(),
        is_lead: true,
    }));
    let err = tool
        .execute(json!({ "agent": "explore", "task": "t" }), &ctx)
        .await
        .expect_err("team context must be refused");
    assert_eq!(
        reason_in(&err.to_string()),
        Some(REASON_TEAM_CONTEXT_ACTIVE)
    );
}

/// A registry refusal reaches the caller through `new_agent` with the same
/// reason code, and the human half of the message survives the lift (FR-010).
#[test]
fn test_registry_refusal_keeps_its_reason_code() {
    use crate::tool::param_schema::reason_in;

    let registry = anyhow::anyhow!(
        "{}",
        crate::tool::param_schema::prefixed("limit_reached", "2 background tasks are running")
    );
    let lifted = lift_registry_refusal(registry);
    assert_eq!(reason_in(&lifted.to_string()), Some(REASON_LIMIT_REACHED));
    assert!(
        lifted
            .to_string()
            .contains("2 background tasks are running"),
        "the explanation must survive: {lifted}"
    );

    // An uncoded error is passed through untouched, so prose never looks like a
    // code.
    let plain = lift_registry_refusal(anyhow::anyhow!("provider exploded"));
    assert_eq!(reason_in(&plain.to_string()), None);
    assert_eq!(plain.to_string(), "provider exploded");
}
