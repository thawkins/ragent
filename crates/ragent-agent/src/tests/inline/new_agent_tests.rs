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

#[tokio::test]
async fn test_new_task_blocks_for_active_team_sessions() {
    let tool = NewAgentTool;
    let mut ctx = base_ctx();
    ctx.team_context = Some(Arc::new(TeamContext {
        team_name: "alpha".to_string(),
        agent_id: "lead".to_string(),
        is_lead: true,
    }));

    let output = tool
        .execute(
            json!({
                "agent": "explore",
                "task": "inspect the repository"
            }),
            &ctx,
        )
        .await
        .expect("team-context guard should return a blocked result");

    let metadata = output
        .metadata
        .expect("blocked result should include metadata");
    assert_eq!(metadata["blocked"], true);
    assert_eq!(metadata["reason"], "team_context_active");
}
