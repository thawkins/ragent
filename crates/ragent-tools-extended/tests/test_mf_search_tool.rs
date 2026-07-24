//! Integration tests for the `mf_search` tool surface.
//!
//! These tests exercise the tool's `Tool` trait implementation (name,
//! description, schema, permission category, and input validation) without
//! making any network calls.

use ragent_config::Config;
use ragent_tools_extended::masterfetch::tools::search_tool::MfSearchTool;
use ragent_tools_extended::{Tool, ToolContext};
use serde_json::json;

/// Build a minimal `ToolContext` for tests that do not touch the filesystem.
fn ctx() -> ToolContext {
    ToolContext {
        session_id: "test".to_string(),
        working_dir: std::env::temp_dir(),
        event_bus: std::sync::Arc::new(ragent_types::event::EventBus::new(64)),
        storage: None,
        code_index: None,
        config: None,
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
    }
}

/// Build a `ToolContext` with the given LangSearch API key in its config.
fn ctx_with_langsearch_key(key: &str) -> ToolContext {
    let mut config = Config::default();
    config.langsearch_api_key = Some(key.to_string());
    ToolContext {
        session_id: "test".to_string(),
        working_dir: std::env::temp_dir(),
        event_bus: std::sync::Arc::new(ragent_types::event::EventBus::new(64)),
        storage: None,
        code_index: None,
        config: Some(std::sync::Arc::new(config)),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
    }
}

#[test]
fn test_tool_name_is_mf_search() {
    let tool = MfSearchTool;
    assert_eq!(tool.name(), "mf_search");
}

#[test]
fn test_permission_category_is_web() {
    let tool = MfSearchTool;
    assert_eq!(tool.permission_category(), "web");
}

#[test]
fn test_description_mentions_key_features() {
    let tool = MfSearchTool;
    let desc = tool.description();
    assert!(
        desc.contains("DuckDuckGo"),
        "description should mention DuckDuckGo"
    );
    assert!(desc.contains("Brave"), "description should mention Brave");
    assert!(
        desc.contains("LangSearch"),
        "description should mention LangSearch"
    );
    assert!(
        desc.contains("langsearch_api_key"),
        "description should mention the optional API key"
    );
    assert!(
        desc.contains("relevance_score"),
        "description should mention relevance_score"
    );
    assert!(
        desc.contains("engines_consensus"),
        "description should mention engines_consensus"
    );
}

#[test]
fn test_orchestrator_without_key_uses_two_engines() {
    let orchestrator = MfSearchTool::build_orchestrator(&ctx());
    assert_eq!(orchestrator.engine_count(), 2);
    let names = orchestrator.engine_names();
    assert!(names.contains(&"duckduckgo"));
    assert!(names.contains(&"brave"));
}

#[test]
fn test_orchestrator_with_key_adds_langsearch_engine() {
    let orchestrator = MfSearchTool::build_orchestrator(&ctx_with_langsearch_key("ls-test-key"));
    assert_eq!(orchestrator.engine_count(), 3);
    let names = orchestrator.engine_names();
    assert!(names.contains(&"langsearch"));
}

#[test]
fn test_orchestrator_with_empty_key_omits_langsearch_engine() {
    let orchestrator = MfSearchTool::build_orchestrator(&ctx_with_langsearch_key(""));
    assert_eq!(orchestrator.engine_count(), 2);
    let names = orchestrator.engine_names();
    assert!(!names.contains(&"langsearch"));
}

#[test]
fn test_parameters_schema_has_required_query() {
    let tool = MfSearchTool;
    let schema = tool.parameters_schema();
    let required = schema["required"]
        .as_array()
        .expect("required should be an array");
    assert!(
        required.iter().any(|v| v == "query"),
        "query should be required"
    );
    assert_eq!(schema["properties"]["query"]["type"], "string");
    assert_eq!(schema["properties"]["site"]["type"], "string");
    assert_eq!(schema["properties"]["exclude_sites"]["type"], "array");
    assert!(
        schema["properties"]["freshness"]["enum"]
            .as_array()
            .unwrap()
            .contains(&json!("day"))
    );
    assert_eq!(schema["properties"]["max_results"]["type"], "integer");
    assert_eq!(schema["properties"]["page"]["type"], "integer");
}

#[tokio::test]
async fn test_execute_rejects_empty_query() {
    let tool = MfSearchTool;
    let result = tool.execute(json!({"query": ""}), &ctx()).await;
    assert!(result.is_err(), "empty query should error");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("empty"),
        "error should mention empty query: {err}"
    );
}

#[tokio::test]
async fn test_execute_rejects_missing_query() {
    let tool = MfSearchTool;
    let result = tool.execute(json!({}), &ctx()).await;
    assert!(result.is_err(), "missing query should error");
}
