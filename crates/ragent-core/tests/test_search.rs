//! Tests for search tool (model-friendly alias for grep)

use ragent_core::event::EventBus;
use ragent_core::tool::search::SearchTool;
use ragent_core::tool::{Tool, ToolContext};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

fn ctx() -> ToolContext {
    ToolContext {
        session_id: "test".to_string(),
        working_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        event_bus: Arc::new(EventBus::new(16)),
        storage: None,
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
    }
}

#[tokio::test]
async fn test_search_basic() {
    let result = SearchTool
        .execute(json!({"query": "SearchTool"}), &ctx())
        .await
        .unwrap();
    assert!(
        result.content.contains("match"),
        "Expected matches but got: {}",
        result.content
    );
}

#[tokio::test]
async fn test_search_with_path() {
    let result = SearchTool
        .execute(json!({"query": "SearchTool", "path": "tests"}), &ctx())
        .await
        .unwrap();
    assert!(
        result.content.contains("match"),
        "Expected matches with path but got: {}",
        result.content
    );
}

/// Test that search tool returns standardized metadata fields.
#[tokio::test]
async fn test_search_metadata_standardized() {
    let result = SearchTool
        .execute(json!({"query": "SearchTool", "path": "tests"}), &ctx())
        .await
        .unwrap();

    let metadata = result.metadata.expect("should have metadata");

    // Check standardized field names
    assert!(
        metadata.get("count").is_some(),
        "should have 'count' field, got: {:?}",
        metadata
    );
    assert!(
        metadata.get("truncated").is_some(),
        "should have 'truncated' field, got: {:?}",
        metadata
    );
}
