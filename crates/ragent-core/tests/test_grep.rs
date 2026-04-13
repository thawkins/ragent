//! Tests for grep tool (ripgrep-based)

use ragent_core::event::EventBus;
use ragent_core::tool::grep::GrepTool;
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
        code_index: None,
    }
}

#[tokio::test]
async fn test_grep_basic_no_glob() {
    let result = GrepTool
        .execute(json!({"pattern": "GrepTool"}), &ctx())
        .await
        .unwrap();
    assert!(
        result.content.contains("match"),
        "Expected matches but got: {}",
        result.content
    );
}

/// Regression: old implementation matched include globs against full absolute path,
/// so "*.rs" never matched files in subdirectories. The new implementation follows
/// gitignore semantics (filename-only matching for patterns without '/').
#[tokio::test]
async fn test_grep_include_single_star_matches_subdirs() {
    let result = GrepTool
        .execute(json!({"pattern": "GrepTool", "include": "*.rs"}), &ctx())
        .await
        .unwrap();
    assert!(
        result.content.contains("match"),
        "include '*.rs' should match files in subdirectories (gitignore semantics), got: {}",
        result.content
    );
}

#[tokio::test]
async fn test_grep_include_doublestar() {
    let result = GrepTool
        .execute(json!({"pattern": "GrepTool", "include": "**/*.rs"}), &ctx())
        .await
        .unwrap();
    assert!(
        result.content.contains("match"),
        "Expected matches with **/*.rs but got: {}",
        result.content
    );
}

/// .git/ internals should not appear in search results.
#[tokio::test]
async fn test_grep_skips_git_dir() {
    let result = GrepTool
        .execute(json!({"pattern": "GrepTool"}), &ctx())
        .await
        .unwrap();
    assert!(
        !result.content.contains(".git/"),
        "Should not search .git/ internals, got: {}",
        result.content
    );
}

/// Test that grep tool returns standardized metadata fields.
#[tokio::test]
async fn test_grep_metadata_standardized() {
    let result = GrepTool
        .execute(json!({"pattern": "GrepTool", "path": "tests"}), &ctx())
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
        metadata.get("file_count").is_some(),
        "should have 'file_count' field, got: {:?}",
        metadata
    );
    assert!(
        metadata.get("pattern").is_some(),
        "should have 'pattern' field, got: {:?}",
        metadata
    );

    // Old field names should NOT exist
    assert!(
        metadata.get("matches").is_none(),
        "should NOT have old 'matches' field"
    );
    assert!(
        metadata.get("files_searched").is_none(),
        "should NOT have old 'files_searched' field"
    );
}
