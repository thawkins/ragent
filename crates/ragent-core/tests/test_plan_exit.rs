//! Tests for test_plan_exit.rs

//! Tests for the plan_exit tool.
//!
//! Unit tests cover schema validation, trait conformance, error handling,
//! and event publishing behaviour.

use std::sync::Arc;

use ragent_core::event::{Event, EventBus};
use ragent_core::tool::{ToolContext, create_default_registry};
use serde_json::json;

fn test_ctx() -> (ToolContext, Arc<EventBus>) {
    let bus = Arc::new(EventBus::new(16));
    let ctx = ToolContext {
        session_id: "test-session".to_string(),
        working_dir: std::env::temp_dir(),
        event_bus: bus.clone(),
        storage: None,
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
    };
    (ctx, bus)
}

// ── Trait & schema tests ─────────────────────────────────────────

#[test]
fn test_plan_exit_name() {
    let registry = create_default_registry();
    let tool = registry.get("plan_exit").unwrap();
    assert_eq!(tool.name(), "plan_exit");
}

#[test]
fn test_plan_exit_description() {
    let registry = create_default_registry();
    let tool = registry.get("plan_exit").unwrap();
    assert!(!tool.description().is_empty());
    assert!(tool.description().to_lowercase().contains("plan"));
}

#[test]
fn test_plan_exit_permission_category() {
    let registry = create_default_registry();
    let tool = registry.get("plan_exit").unwrap();
    assert_eq!(tool.permission_category(), "plan");
}

#[test]
fn test_plan_exit_schema_has_summary() {
    let registry = create_default_registry();
    let tool = registry.get("plan_exit").unwrap();
    let schema = tool.parameters_schema();
    let props = schema["properties"].as_object().unwrap();
    assert!(props.contains_key("summary"));
    let required = schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(required.contains(&"summary"));
}

// ── Error condition tests ────────────────────────────────────────

#[tokio::test]
async fn test_plan_exit_missing_summary() {
    let (ctx, _bus) = test_ctx();
    let registry = create_default_registry();
    let tool = registry.get("plan_exit").unwrap();
    let result = tool.execute(json!({}), &ctx).await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("summary"),
        "Expected 'summary' error, got: {msg}"
    );
}

#[tokio::test]
async fn test_plan_exit_empty_summary() {
    let (ctx, _bus) = test_ctx();
    let registry = create_default_registry();
    let tool = registry.get("plan_exit").unwrap();
    let result = tool.execute(json!({"summary": "   "}), &ctx).await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("empty"), "Expected 'empty' error, got: {msg}");
}

// ── Success tests ────────────────────────────────────────────────

#[tokio::test]
async fn test_plan_exit_success() {
    let (ctx, bus) = test_ctx();
    let mut rx = bus.subscribe();

    let registry = create_default_registry();
    let tool = registry.get("plan_exit").unwrap();
    let summary_text = "1. Refactor auth module\n2. Add tests\n3. Update docs";
    let result = tool.execute(json!({"summary": summary_text}), &ctx).await;

    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.content.contains("Returning to previous agent"));
    assert!(output.content.contains(summary_text));

    // Check metadata
    let meta = output.metadata.unwrap();
    assert_eq!(meta["agent_restore"], true);
    assert_eq!(meta["summary_length"], summary_text.len());

    // Check that AgentRestoreRequested event was published
    let event = rx.try_recv().unwrap();
    match event {
        Event::AgentRestoreRequested {
            session_id,
            summary,
        } => {
            assert_eq!(session_id, "test-session");
            assert_eq!(summary, summary_text);
        }
        other => panic!("Expected AgentRestoreRequested, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_plan_exit_metadata_has_restore_flag() {
    let (ctx, _bus) = test_ctx();
    let registry = create_default_registry();
    let tool = registry.get("plan_exit").unwrap();
    let result = tool
        .execute(json!({"summary": "Done planning"}), &ctx)
        .await
        .unwrap();
    let meta = result.metadata.unwrap();
    assert_eq!(meta["agent_restore"], true);
}

#[test]
fn test_plan_exit_registered() {
    let registry = create_default_registry();
    assert!(registry.get("plan_exit").is_some());
    assert!(
        registry.list().len() >= 31,
        "expected at least 31 tools, found {}",
        registry.list().len()
    );
}
