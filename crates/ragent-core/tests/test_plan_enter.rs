//! Tests for the plan_enter tool.
//!
//! Unit tests cover schema validation, trait conformance, error handling,
//! and event publishing behaviour.

use std::sync::Arc;

use ragent_core::event::{Event, EventBus};
use ragent_core::tool::{Tool, ToolContext, create_default_registry};
use serde_json::json;

fn test_ctx() -> (ToolContext, Arc<EventBus>) {
    let bus = Arc::new(EventBus::new(16));
    let ctx = ToolContext {
        session_id: "test-session".to_string(),
        working_dir: std::env::temp_dir(),
        event_bus: bus.clone(),
        storage: None,
    };
    (ctx, bus)
}

// ── Trait & schema tests ─────────────────────────────────────────

#[test]
fn test_plan_enter_name() {
    let registry = create_default_registry();
    let tool = registry.get("plan_enter").unwrap();
    assert_eq!(tool.name(), "plan_enter");
}

#[test]
fn test_plan_enter_description() {
    let registry = create_default_registry();
    let tool = registry.get("plan_enter").unwrap();
    assert!(!tool.description().is_empty());
    assert!(tool.description().to_lowercase().contains("plan"));
}

#[test]
fn test_plan_enter_permission_category() {
    let registry = create_default_registry();
    let tool = registry.get("plan_enter").unwrap();
    assert_eq!(tool.permission_category(), "plan");
}

#[test]
fn test_plan_enter_schema_has_task() {
    let registry = create_default_registry();
    let tool = registry.get("plan_enter").unwrap();
    let schema = tool.parameters_schema();
    let props = schema["properties"].as_object().unwrap();
    assert!(props.contains_key("task"));
    let required = schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(required.contains(&"task"));
}

#[test]
fn test_plan_enter_schema_has_context() {
    let registry = create_default_registry();
    let tool = registry.get("plan_enter").unwrap();
    let schema = tool.parameters_schema();
    let props = schema["properties"].as_object().unwrap();
    assert!(props.contains_key("context"));
    // context is optional — not in required
    let required = schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(!required.contains(&"context"));
}

// ── Error condition tests ────────────────────────────────────────

#[tokio::test]
async fn test_plan_enter_missing_task() {
    let (ctx, _bus) = test_ctx();
    let registry = create_default_registry();
    let tool = registry.get("plan_enter").unwrap();
    let result = tool.execute(json!({}), &ctx).await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("task"), "Expected 'task' error, got: {msg}");
}

#[tokio::test]
async fn test_plan_enter_empty_task() {
    let (ctx, _bus) = test_ctx();
    let registry = create_default_registry();
    let tool = registry.get("plan_enter").unwrap();
    let result = tool.execute(json!({"task": "   "}), &ctx).await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("empty"), "Expected 'empty' error, got: {msg}");
}

// ── Success tests ────────────────────────────────────────────────

#[tokio::test]
async fn test_plan_enter_success() {
    let (ctx, bus) = test_ctx();
    let mut rx = bus.subscribe();

    let registry = create_default_registry();
    let tool = registry.get("plan_enter").unwrap();
    let result = tool
        .execute(json!({"task": "Analyze the auth module"}), &ctx)
        .await;

    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.content.contains("Delegating"));
    assert!(output.content.contains("auth module"));

    // Check metadata
    let meta = output.metadata.unwrap();
    assert_eq!(meta["agent_switch"], "plan");
    assert_eq!(meta["task"], "Analyze the auth module");

    // Check that AgentSwitchRequested event was published
    let event = rx.try_recv().unwrap();
    match event {
        Event::AgentSwitchRequested {
            session_id,
            to,
            task,
            context,
        } => {
            assert_eq!(session_id, "test-session");
            assert_eq!(to, "plan");
            assert_eq!(task, "Analyze the auth module");
            assert!(context.is_empty());
        }
        other => panic!("Expected AgentSwitchRequested, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_plan_enter_with_context() {
    let (ctx, bus) = test_ctx();
    let mut rx = bus.subscribe();

    let registry = create_default_registry();
    let tool = registry.get("plan_enter").unwrap();
    let result = tool
        .execute(
            json!({
                "task": "Plan refactoring",
                "context": "The user wants to split the monolith"
            }),
            &ctx,
        )
        .await;

    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.content.contains("Context:"));

    let event = rx.try_recv().unwrap();
    match event {
        Event::AgentSwitchRequested {
            context, task, ..
        } => {
            assert_eq!(task, "Plan refactoring");
            assert_eq!(context, "The user wants to split the monolith");
        }
        other => panic!("Expected AgentSwitchRequested, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_plan_enter_without_context() {
    let (ctx, _bus) = test_ctx();
    let registry = create_default_registry();
    let tool = registry.get("plan_enter").unwrap();
    let result = tool
        .execute(json!({"task": "Simple analysis"}), &ctx)
        .await;

    let output = result.unwrap();
    assert!(!output.content.contains("Context:"));
}

#[test]
fn test_plan_enter_registered() {
    let registry = create_default_registry();
    assert!(registry.get("plan_enter").is_some());
    assert_eq!(registry.list().len(), 21);
}
