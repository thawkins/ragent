#![allow(clippy::assert_is_empty)]
//! Integration tests for the `model_info` tool.

use ragent_agent::agent::ModelRef;
use ragent_agent::event::EventBus;
use ragent_agent::provider::create_default_registry;
use ragent_agent::tool::{ToolContext, create_default_registry as create_tool_registry};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

fn base_ctx(active_model: Option<ModelRef>) -> ToolContext {
    ToolContext {
        session_id: "session-1".to_string(),
        working_dir: PathBuf::from("/tmp"),
        event_bus: Arc::new(EventBus::new(16)),
        storage: None,
        agent_manager: None,
        active_model,
        provider_registry: Some(Arc::new(create_default_registry())),
        team_context: None,
        team_manager: None,
        code_index: None,
        bg_service: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
    }
}

#[tokio::test]
async fn test_model_info_reports_router_metadata() {
    let registry = create_tool_registry();
    let tool = registry.get("model_info").expect("model_info registered");
    let ctx = base_ctx(Some(ModelRef {
        provider_id: "router".to_string(),
        model_id: "router".to_string(),
    }));

    let output = tool.execute(json!({}), &ctx).await.expect("execute ok");
    assert!(
        output.content.contains("Model Router"),
        "expected router provider name in output: {}",
        output.content
    );
    assert!(
        output
            .content
            .contains("router selects a concrete downstream"),
        "expected router note: {}",
        output.content
    );
    let meta = output.metadata.expect("metadata present");
    assert_eq!(meta["provider_id"], "router");
    assert_eq!(meta["model_id"], "router");
    assert!(meta["router"].is_object());
}

#[tokio::test]
async fn test_model_info_json_format_omits_markdown() {
    let registry = create_tool_registry();
    let tool = registry.get("model_info").expect("model_info registered");
    let ctx = base_ctx(Some(ModelRef {
        provider_id: "router".to_string(),
        model_id: "router".to_string(),
    }));

    let output = tool
        .execute(json!({"format": "json"}), &ctx)
        .await
        .expect("execute ok");
    assert!(
        !output.content.contains("## "),
        "json format should not contain markdown headings: {}",
        output.content
    );
    let parsed: serde_json::Value = serde_json::from_str(&output.content).expect("valid json");
    assert_eq!(parsed["provider_id"], "router");
}

#[tokio::test]
async fn test_model_info_errors_without_active_model() {
    let registry = create_tool_registry();
    let tool = registry.get("model_info").expect("model_info registered");
    let ctx = base_ctx(None);

    let err = tool
        .execute(json!({}), &ctx)
        .await
        .expect_err("should fail without active model");
    assert!(err.to_string().contains("No active model"));
}

#[tokio::test]
async fn test_model_info_unknown_provider_falls_back_gracefully() {
    let registry = create_tool_registry();
    let tool = registry.get("model_info").expect("model_info registered");
    let ctx = base_ctx(Some(ModelRef {
        provider_id: "nonexistent".to_string(),
        model_id: "unknown-model".to_string(),
    }));

    let output = tool.execute(json!({}), &ctx).await.expect("execute ok");
    assert!(output.content.contains("nonexistent"));
    assert!(output.content.contains("unknown-model"));
    let meta = output.metadata.expect("metadata present");
    assert_eq!(meta["provider_id"], "nonexistent");
    assert_eq!(meta["model_id"], "unknown-model");
}
