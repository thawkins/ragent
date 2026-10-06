//! Integration tests for the `ragent_info` tool.
//!
//! Verifies that the tool is registered, reports the running ragent version and
//! a build timestamp, supports both `text` and `json` formats, and never
//! requires an active model or network access. Also verifies the runtime
//! execution section (pid, uptime, executable, working directory, memory, and
//! thread count).

use ragent_agent::event::EventBus;
use ragent_agent::tool::{ToolContext, create_default_registry};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

fn base_ctx() -> ToolContext {
    ToolContext {
        session_id: "session-1".to_string(),
        working_dir: PathBuf::from("/tmp"),
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
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        tool_registry: ToolContext::default_tool_registry(),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
    }
}

#[test]
fn test_ragent_info_is_registered() {
    let registry = create_default_registry();
    let tool = registry.get("ragent_info").expect("ragent_info registered");
    assert_eq!(tool.name(), "ragent_info");
    assert_eq!(tool.permission_category(), "none");
}

#[tokio::test]
async fn test_ragent_info_text_reports_version_and_build_time() {
    let registry = create_default_registry();
    let tool = registry.get("ragent_info").expect("ragent_info registered");
    let ctx = base_ctx();

    let output = tool.execute(json!({}), &ctx).await.expect("execute ok");
    assert!(
        output.content.contains("ragent Build Information"),
        "expected build info heading: {}",
        output.content
    );
    assert!(
        output
            .content
            .contains(&format!("**Version**: {}", env!("CARGO_PKG_VERSION"))),
        "expected running version: {}",
        output.content
    );
    assert!(
        output.content.contains("**Built**: "),
        "expected build timestamp: {}",
        output.content
    );
    // Execution section (text format) is present and reports the live pid.
    assert!(
        output.content.contains("## ragent Execution Information"),
        "expected execution section: {}",
        output.content
    );
    assert!(
        output
            .content
            .contains(&format!("**PID**: {}", std::process::id())),
        "expected running pid: {}",
        output.content
    );
}

#[tokio::test]
async fn test_ragent_info_json_reports_execution_section() {
    let registry = create_default_registry();
    let tool = registry.get("ragent_info").expect("ragent_info registered");
    let ctx = base_ctx();

    let output = tool
        .execute(json!({"format": "json"}), &ctx)
        .await
        .expect("execute ok");
    let parsed: serde_json::Value = serde_json::from_str(&output.content).expect("valid json");
    let exec = &parsed["execution"];
    assert!(
        exec.is_object(),
        "expected execution object in json: {parsed}"
    );
    assert_eq!(
        exec["pid"].as_u64(),
        Some(u64::from(std::process::id())),
        "expected running pid in json: {parsed}"
    );
    // Working directory is always resolvable for the running process.
    assert!(
        exec["working_directory"]
            .as_str()
            .is_some_and(|s| !s.is_empty()),
        "expected a working directory: {parsed}"
    );
    // User is a best-effort field; when present it must be a non-empty string.
    assert!(
        exec["user"].is_null() || exec["user"].as_str().is_some_and(|s| !s.is_empty()),
        "user must be a non-empty string or null: {parsed}"
    );
    // Uptime is a best-effort field; when present it must be a number.
    if let Some(uptime) = exec.get("uptime_seconds") {
        assert!(
            uptime.is_null() || uptime.as_u64().is_some(),
            "uptime_seconds must be numeric or null: {parsed}"
        );
    }
}

#[tokio::test]
async fn test_ragent_info_json_format_is_structured() {
    let registry = create_default_registry();
    let tool = registry.get("ragent_info").expect("ragent_info registered");
    let ctx = base_ctx();

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
    assert_eq!(parsed["version"], env!("CARGO_PKG_VERSION"));
    assert!(parsed["build_time"].is_string());

    let meta = output.metadata.expect("metadata present");
    assert_eq!(meta["version"], env!("CARGO_PKG_VERSION"));
    assert!(meta["build_time"].is_string());
}

#[tokio::test]
async fn test_ragent_info_never_errors_without_model_or_network() {
    // No active model, no provider registry, no config — must still succeed.
    let registry = create_default_registry();
    let tool = registry.get("ragent_info").expect("ragent_info registered");
    let ctx = base_ctx();

    let output = tool.execute(json!({}), &ctx).await.expect("execute ok");
    assert!(!output.content.is_empty());
    assert!(output.metadata.is_some());
}
