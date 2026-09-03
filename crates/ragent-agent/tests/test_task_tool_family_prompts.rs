#![allow(clippy::assert_is_empty)]
//! Tests for the task tool family guidance, schemas, and descriptions.
//!
//! These tests guard against regressions in the guidance that prevents models
//! from confusing `agent_complete` (autonomous loop signal) with
//! `team_task_complete` (team workflow) — see AGENTS.md for context.
//!
//! The original confusion was:
//! - Model called `new_agent(agent: "explore")` without supplying `task` — the
//!   schema rejected it with "Missing required parameter: task".
//! - Model called `agent_complete(task_id: "...", result: "...")` confusing it
//!   with `team_task_complete` — the schema rejected it with "Missing required
//!   'summary' parameter".
//!
//! These tests ensure the descriptions and schemas clearly distinguish the two
//! tools, and that the system prompt always carries the anti-confusion guidance.

use ragent_agent::agent::TASK_TOOL_FAMILY_GUIDANCE;
use ragent_agent::tool::create_default_registry;
use serde_json::Value;

fn def_for(tool_name: &str) -> ragent_llm::llm::ToolDefinition {
    let registry = create_default_registry();
    registry
        .definitions()
        .iter()
        .find(|d| d.name == tool_name)
        .cloned()
        .unwrap_or_else(|| panic!("tool {tool_name} should be registered"))
}

fn schema_for(tool_name: &str) -> Value {
    def_for(tool_name).parameters
}

fn description_for(tool_name: &str) -> String {
    def_for(tool_name).description
}

#[test]
fn test_agent_complete_schema_requires_summary_and_only_summary() {
    let schema = schema_for("agent_complete");
    let required = schema
        .get("required")
        .and_then(|v| v.as_array())
        .expect("agent_complete schema must declare a 'required' array");
    let required: Vec<&str> = required.iter().filter_map(|v| v.as_str()).collect();
    assert_eq!(
        required,
        vec!["summary"],
        "agent_complete must require ONLY `summary`"
    );
    assert!(
        schema
            .get("properties")
            .and_then(|p| p.get("summary"))
            .is_some(),
        "agent_complete schema must define a `summary` property"
    );
    // It should NOT advertise `task_id`, `team_name`, `result`, or `output`.
    let props: Vec<String> = schema
        .get("properties")
        .and_then(|p| p.as_object())
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default();
    for forbidden in ["task_id", "team_name", "result", "output"] {
        assert!(
            !props.iter().any(|p| p == forbidden),
            "agent_complete must not advertise a `{forbidden}` property (got {props:?})"
        );
    }
    // additionalProperties: false so unknown keys are rejected, not silently ignored.
    assert_eq!(
        schema.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "agent_complete must set additionalProperties: false"
    );
}

#[test]
fn test_agent_complete_description_warns_about_team_task_complete_confusion() {
    let desc = description_for("agent_complete");
    assert!(
        desc.contains("team_task_complete"),
        "agent_complete description must reference team_task_complete to prevent confusion, got: {desc}"
    );
    assert!(
        desc.contains("summary"),
        "agent_complete description must mention the `summary` parameter, got: {desc}"
    );
    assert!(
        desc.to_lowercase().contains("terminal")
            || desc.to_lowercase().contains("ends the loop")
            || desc.to_lowercase().contains("ends the agent loop"),
        "agent_complete description must warn that it ends the session loop, got: {desc}"
    );
    // It must NOT list `task_id` as a *valid* key.  The description
    // IS allowed to mention `task_id` as part of the
    // "DO NOT confuse with `team_task_complete`" anti-confusion warning,
    // but it must not advertise it as a parameter this tool accepts.
    let lower = desc.to_lowercase();
    assert!(
        !lower.contains("`task_id` (string)")
            && !lower.contains("`task_id` is required")
            && !lower.contains("required: `task_id`"),
        "agent_complete description must not advertise `task_id` as a required parameter, got: {desc}"
    );
}

#[test]
fn test_team_task_complete_schema_requires_team_name_and_task_id() {
    let schema = schema_for("team_task_complete");
    let required = schema
        .get("required")
        .and_then(|v| v.as_array())
        .expect("team_task_complete schema must declare a 'required' array");
    let required: Vec<&str> = required.iter().filter_map(|v| v.as_str()).collect();
    assert_eq!(
        required,
        vec!["team_name", "task_id"],
        "team_task_complete must require BOTH `team_name` and `task_id`"
    );
    // It should NOT advertise `summary` as the only field — the description
    // should explicitly say it does NOT take `summary`.
    let desc = description_for("team_task_complete");
    assert!(
        desc.contains("team_task_complete")
            && desc.contains("agent_complete")
            && desc.contains("NOT `summary`"),
        "team_task_complete description must warn that it does NOT take `summary` and \
         reference agent_complete to prevent confusion, got: {desc}"
    );
    assert_eq!(
        schema.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "team_task_complete must set additionalProperties: false"
    );
}

#[test]
fn test_new_agent_schema_requires_both_agent_and_task() {
    let schema = schema_for("new_agent");
    let required = schema
        .get("required")
        .and_then(|v| v.as_array())
        .expect("new_agent schema must declare a 'required' array");
    let required: Vec<&str> = required.iter().filter_map(|v| v.as_str()).collect();
    assert!(
        required.contains(&"agent") && required.contains(&"task"),
        "new_agent must require BOTH `agent` and `task`, got required={required:?}"
    );
    let desc = description_for("new_agent");
    assert!(
        desc.contains("BOTH") || desc.to_lowercase().contains("both"),
        "new_agent description must explicitly state that BOTH agent and task are required, got: {desc}"
    );
    assert_eq!(
        schema.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "new_agent must set additionalProperties: false"
    );
}

#[test]
fn test_task_tool_family_guidance_constant_present() {
    assert!(!TASK_TOOL_FAMILY_GUIDANCE.is_empty());
    assert!(
        TASK_TOOL_FAMILY_GUIDANCE.contains("Task Tool Family"),
        "guidance must include the section header"
    );
    assert!(
        TASK_TOOL_FAMILY_GUIDANCE.contains("agent_complete")
            && TASK_TOOL_FAMILY_GUIDANCE.contains("team_task_complete"),
        "guidance must reference both agent_complete and team_task_complete"
    );
    // The guidance must call out the most common mistake explicitly.
    assert!(
        TASK_TOOL_FAMILY_GUIDANCE.contains("summary")
            && TASK_TOOL_FAMILY_GUIDANCE.contains("task_id")
            && TASK_TOOL_FAMILY_GUIDANCE.contains("team_name"),
        "guidance must list the required parameters of each tool"
    );
    assert!(
        TASK_TOOL_FAMILY_GUIDANCE.contains("TERMINAL")
            || TASK_TOOL_FAMILY_GUIDANCE.contains("ends the"),
        "guidance must warn that agent_complete is terminal / ends the loop"
    );
    assert!(
        TASK_TOOL_FAMILY_GUIDANCE.contains("new_agent"),
        "guidance must cover the new_agent requirement that both agent and task are required"
    );
}

#[tokio::test]
async fn test_agent_complete_rejects_task_id_and_result_inputs() {
    use ragent_agent::tool::ToolContext;
    use serde_json::json;
    use std::path::PathBuf;
    use std::sync::Arc;

    let registry = create_default_registry();
    let tool = registry
        .get("agent_complete")
        .expect("agent_complete tool should be registered");
    let event_bus = Arc::new(ragent_agent::event::EventBus::new(16));

    // The model should not pass `task_id` or `result`. The tool should error
    // because `summary` is missing.
    let input = json!({
        "task_id": "explore-1234",
        "result": "some result text",
    });
    let ctx = ToolContext {
        session_id: "test-session".to_string(),
        working_dir: PathBuf::from("/tmp"),
        event_bus: event_bus.clone(),
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
        cached_team_dir: std::sync::Arc::new(std::sync::Mutex::new(None)),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        canonical_cache: std::sync::Arc::new(ragent_tools_core::CanonicalPathCache::new()),
    };
    let result = tool.execute(input, &ctx).await;
    assert!(
        result.is_err(),
        "agent_complete must reject calls that omit `summary`"
    );
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("summary"),
        "error message must mention `summary`, got: {err}"
    );
}

#[tokio::test]
async fn test_agent_complete_accepts_summary_input() {
    use ragent_agent::tool::ToolContext;
    use serde_json::json;
    use std::path::PathBuf;
    use std::sync::Arc;

    let registry = create_default_registry();
    let tool = registry
        .get("agent_complete")
        .expect("agent_complete tool should be registered");
    let event_bus = Arc::new(ragent_agent::event::EventBus::new(16));

    let input = json!({
        "summary": "Implemented feature X, wrote 3 tests, updated docs",
    });
    let ctx = ToolContext {
        session_id: "test-session".to_string(),
        working_dir: PathBuf::from("/tmp"),
        event_bus: event_bus.clone(),
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
        cached_team_dir: std::sync::Arc::new(std::sync::Mutex::new(None)),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        canonical_cache: std::sync::Arc::new(ragent_tools_core::CanonicalPathCache::new()),
    };
    let result = tool.execute(input, &ctx).await;
    assert!(
        result.is_ok(),
        "agent_complete must accept calls with valid `summary`, got: {:?}",
        result.err()
    );
    let output = result.unwrap();
    let metadata = output
        .metadata
        .expect("agent_complete output should include metadata");
    assert_eq!(
        metadata.get("agent_complete").and_then(Value::as_bool),
        Some(true),
        "agent_complete metadata should include agent_complete: true"
    );
    assert_eq!(
        metadata.get("summary").and_then(Value::as_str),
        Some("Implemented feature X, wrote 3 tests, updated docs"),
    );
}
