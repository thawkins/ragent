//! Tests for glob tool

use ragent_core::event::EventBus;
use ragent_core::tool::glob::GlobTool;
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
async fn test_glob_basic() {
    let result = GlobTool
        .execute(json!({"pattern": "**/*.rs"}), &ctx())
        .await
        .unwrap();
    assert!(
        result.content.contains("found"),
        "Expected files found but got: {}",
        result.content
    );
}

#[tokio::test]
async fn test_glob_with_path() {
    let result = GlobTool
        .execute(json!({"pattern": "*.rs", "path": "tests"}), &ctx())
        .await
        .unwrap();
    assert!(
        result.content.contains("found") || result.content.contains("No files"),
        "Expected result but got: {}",
        result.content
    );
}

/// Test that glob tool returns standardized metadata fields.
#[tokio::test]
async fn test_glob_metadata_standardized() {
    let result = GlobTool
        .execute(json!({"pattern": "*.rs", "path": "tests"}), &ctx())
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
        metadata.get("pattern").is_some(),
        "should have 'pattern' field, got: {:?}",
        metadata
    );
}

/// Test that glob tool handles empty results correctly.
#[tokio::test]
async fn test_glob_empty_results() {
    let result = GlobTool
        .execute(json!({"pattern": "*.nonexistent"}), &ctx())
        .await
        .unwrap();

    assert!(
        result.content.contains("No files"),
        "Expected 'No files' message but got: {}",
        result.content
    );

    let metadata = result.metadata.expect("should have metadata");
    let count = metadata.get("count").and_then(|v| v.as_u64()).unwrap_or(1);
    assert_eq!(count, 0, "count should be 0 for empty results");
}
