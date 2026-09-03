#![allow(clippy::assert_is_empty)]
//! Integration tests for `mf_version` tool — version info (T-025, FR-017,
//! FR-022, FR-026, NFR-003).
//!
//! Covers: tool name, permission category, version content, structured
//! metadata, tool set listing, no network calls.

use std::sync::Arc;

use ragent_tools_extended::masterfetch::tools::version::MfVersionTool;
use ragent_tools_extended::{Tool, ToolContext};

/// Build a minimal `ToolContext` for testing.
fn ctx() -> ToolContext {
    ToolContext {
        session_id: "test".to_string(),
        working_dir: std::path::PathBuf::from("."),
        event_bus: Arc::new(ragent_types::event::EventBus::new(64)),
        storage: None,
        code_index: None,
        config: None,
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
    }
}

// ---------------------------------------------------------------------------
// Tool identity
// ---------------------------------------------------------------------------

#[test]
fn test_tool_name() {
    let tool = MfVersionTool;
    assert_eq!(tool.name(), "mf_version");
}

#[test]
fn test_permission_category_is_system() {
    // FR-022: mf_version returns "system" since it does not make network calls.
    let tool = MfVersionTool;
    assert_eq!(tool.permission_category(), "system");
}

#[test]
fn test_description_mentions_version_and_tools() {
    let tool = MfVersionTool;
    let desc = tool.description();
    assert!(desc.contains("version"));
    assert!(desc.contains("tool set"));
}

#[test]
fn test_parameters_schema_is_empty_object() {
    let tool = MfVersionTool;
    let schema = tool.parameters_schema();
    assert_eq!(schema["type"], "object");
    // No required parameters.
    assert!(schema.get("required").is_none() || schema["required"].is_null());
}

// ---------------------------------------------------------------------------
// Execute — version content
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_execute_returns_masterfetch_version() {
    let tool = MfVersionTool;
    let output = tool.execute(serde_json::json!({}), &ctx()).await.unwrap();

    // Content should contain the masterfetch version.
    assert!(output.content.contains("MasterFetch Integration v"));
    assert!(output.content.contains("0.1.0"));
}

#[tokio::test]
async fn test_execute_returns_ragent_version() {
    let tool = MfVersionTool;
    let output = tool.execute(serde_json::json!({}), &ctx()).await.unwrap();

    // Content should contain the ragent version.
    assert!(output.content.contains("ragent v"));
    // The ragent version is the crate version at compile time.
    let ragent_ver = env!("CARGO_PKG_VERSION");
    assert!(output.content.contains(ragent_ver));
}

#[tokio::test]
async fn test_execute_lists_all_six_tools() {
    let tool = MfVersionTool;
    let output = tool.execute(serde_json::json!({}), &ctx()).await.unwrap();

    // All six tools should be mentioned in the content.
    assert!(output.content.contains("mf_fetch"));
    assert!(output.content.contains("mf_crawl"));
    assert!(output.content.contains("mf_search"));
    assert!(output.content.contains("mf_screenshot"));
    assert!(output.content.contains("mf_cache_clear"));
    assert!(output.content.contains("mf_version"));
}

#[tokio::test]
async fn test_execute_content_is_text_not_raw_json() {
    // FR-026: content is a human-readable text string, not raw JSON.
    let tool = MfVersionTool;
    let output = tool.execute(serde_json::json!({}), &ctx()).await.unwrap();

    // Content should not start with { or [ (raw JSON).
    let trimmed = output.content.trim_start();
    assert!(
        !trimmed.starts_with('{') && !trimmed.starts_with('['),
        "content should be human-readable text, not raw JSON"
    );
}

#[tokio::test]
async fn test_execute_mentions_http_only_mode() {
    let tool = MfVersionTool;
    let output = tool.execute(serde_json::json!({}), &ctx()).await.unwrap();

    assert!(output.content.contains("HTTP-only mode"));
    assert!(output.content.contains("graceful degradation"));
}

// ---------------------------------------------------------------------------
// Execute — structured metadata
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_execute_metadata_has_masterfetch_version() {
    let tool = MfVersionTool;
    let output = tool.execute(serde_json::json!({}), &ctx()).await.unwrap();

    let meta = output.metadata.expect("metadata should be present");
    assert_eq!(meta["masterfetch_version"], "0.1.0");
}

#[tokio::test]
async fn test_execute_metadata_has_ragent_version() {
    let tool = MfVersionTool;
    let output = tool.execute(serde_json::json!({}), &ctx()).await.unwrap();

    let meta = output.metadata.expect("metadata should be present");
    let ragent_ver = env!("CARGO_PKG_VERSION");
    assert_eq!(meta["ragent_version"], ragent_ver);
}

#[tokio::test]
async fn test_execute_metadata_has_tool_list() {
    let tool = MfVersionTool;
    let output = tool.execute(serde_json::json!({}), &ctx()).await.unwrap();

    let meta = output.metadata.expect("metadata should be present");
    let tools = meta["tools"].as_array().expect("tools should be an array");
    assert_eq!(tools.len(), 6);
    assert!(tools.iter().any(|t| t == "mf_fetch"));
    assert!(tools.iter().any(|t| t == "mf_crawl"));
    assert!(tools.iter().any(|t| t == "mf_search"));
    assert!(tools.iter().any(|t| t == "mf_screenshot"));
    assert!(tools.iter().any(|t| t == "mf_cache_clear"));
    assert!(tools.iter().any(|t| t == "mf_version"));
}

#[tokio::test]
async fn test_execute_metadata_has_tool_count() {
    let tool = MfVersionTool;
    let output = tool.execute(serde_json::json!({}), &ctx()).await.unwrap();

    let meta = output.metadata.expect("metadata should be present");
    assert_eq!(meta["tool_count"], 6);
}

// ---------------------------------------------------------------------------
// Execute — robustness
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_execute_succeeds_with_arbitrary_input() {
    // The tool ignores input — it should succeed with any JSON.
    let tool = MfVersionTool;
    let output = tool
        .execute(serde_json::json!({"foo": "bar"}), &ctx())
        .await
        .unwrap();
    assert!(!output.content.is_empty());
}

#[tokio::test]
async fn test_execute_never_errors() {
    // FR-024: no panics. The tool always succeeds.
    let tool = MfVersionTool;
    let result = tool.execute(serde_json::json!({}), &ctx()).await;
    assert!(result.is_ok());
}
