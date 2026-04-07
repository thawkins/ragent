//! Tests for list tool

use ragent_core::event::EventBus;
use ragent_core::tool::list::ListTool;
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
async fn test_list_basic() {
    let result = ListTool.execute(json!({}), &ctx()).await.unwrap();
    assert!(
        !result.content.is_empty(),
        "Expected non-empty listing but got: {}",
        result.content
    );
}

#[tokio::test]
async fn test_list_with_path() {
    let result = ListTool
        .execute(json!({"path": "tests"}), &ctx())
        .await
        .unwrap();
    assert!(
        result.content.contains("tests"),
        "Expected listing containing 'tests' but got: {}",
        result.content
    );
}

/// Test that list tool returns standardized metadata fields.
#[tokio::test]
async fn test_list_metadata_standardized() {
    let result = ListTool
        .execute(json!({"path": "tests"}), &ctx())
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
        metadata.get("path").is_some(),
        "should have 'path' field, got: {:?}",
        metadata
    );

    // Old field name should NOT exist
    assert!(
        metadata.get("entries").is_none(),
        "should NOT have old 'entries' field"
    );
}

#[tokio::test]
async fn test_list_with_depth() {
    let result = ListTool
        .execute(json!({"path": "tests", "depth": 1}), &ctx())
        .await
        .unwrap();
    assert!(
        !result.content.is_empty(),
        "Expected non-empty listing with depth but got: {}",
        result.content
    );
}
