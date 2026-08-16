//! Tests for the `memory_store` structured-memory tool.
//!
//! These tests verify that the tool correctly persists memories to SQLite,
//! including tags, and that the returned metadata clearly indicates success.

use ragent_agent::event::EventBus;
use ragent_agent::storage::Storage;
use ragent_agent::tool::structured_memory::MemoryStoreTool;
use ragent_agent::tool::{Tool, ToolContext};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

fn ctx_with_storage(storage: Arc<Storage>, session_id: &str) -> ToolContext {
    ToolContext {
        session_id: session_id.to_string(),
        working_dir: PathBuf::from("/tmp"),
        event_bus: Arc::new(EventBus::new(16)),
        storage: Some(storage),
        agent_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
        code_index: None,
        bg_service: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
    }
}

#[tokio::test]
async fn test_memory_store_persists_content_and_tags() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let tool = MemoryStoreTool;
    let ctx = ctx_with_storage(storage.clone(), "sess-1");

    let out = tool
        .execute(
            json!({
                "content": "Use anyhow for application error handling",
                "category": "pattern",
                "confidence": 0.95,
                "tags": ["rust", "error-handling"],
                "source": "manual"
            }),
            &ctx,
        )
        .await
        .expect("execute should succeed");

    assert!(
        out.content.contains("Memory stored"),
        "output should report success: {}",
        out.content
    );

    let meta = out.metadata.expect("metadata");
    assert_eq!(meta["stored"], true, "metadata should mark stored=true");
    let id = meta["id"].as_i64().expect("id");
    assert_eq!(meta["category"], "pattern");
    assert_eq!(meta["confidence"], 0.95);
    assert_eq!(meta["tags"], json!(["rust", "error-handling"]));

    let row = storage
        .get_memory(id)
        .expect("get_memory")
        .expect("memory should exist");
    assert_eq!(row.content, "Use anyhow for application error handling");
    assert_eq!(row.category, "pattern");
    assert!((row.confidence - 0.95).abs() < f64::EPSILON);

    let tags = storage.get_memory_tags(id).expect("tags");
    assert_eq!(tags, vec!["error-handling", "rust"]);

    // Regression guard (TUI memory panel uses the full working directory as
    // the project key). Storing only the directory basename would make the
    // memory invisible in the panel and `/memory show`.
    assert_eq!(
        row.project, "/tmp",
        "project should be the full working directory path"
    );
    let listed = storage.list_memories("/tmp", 10).expect("list");
    assert!(
        listed.iter().any(|r| r.id == id),
        "memory should be listable by full project path"
    );
}

#[tokio::test]
async fn test_memory_store_rejects_invalid_category() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let tool = MemoryStoreTool;
    let ctx = ctx_with_storage(storage, "sess-1");

    let err = tool
        .execute(
            json!({
                "content": "bad category",
                "category": "opinion"
            }),
            &ctx,
        )
        .await
        .expect_err("should fail");

    assert!(
        err.to_string().contains("Invalid category"),
        "error should mention invalid category: {err}"
    );
}

#[tokio::test]
async fn test_memory_store_rejects_invalid_confidence() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let tool = MemoryStoreTool;
    let ctx = ctx_with_storage(storage, "sess-1");

    let err = tool
        .execute(
            json!({
                "content": "bad confidence",
                "category": "fact",
                "confidence": 1.5
            }),
            &ctx,
        )
        .await
        .expect_err("should fail");

    assert!(
        err.to_string().contains("Confidence must be between"),
        "error should mention confidence range: {err}"
    );
}

#[tokio::test]
async fn test_memory_store_rejects_invalid_tags() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let tool = MemoryStoreTool;
    let ctx = ctx_with_storage(storage, "sess-1");

    let err = tool
        .execute(
            json!({
                "content": "bad tags",
                "category": "fact",
                "tags": ["UpperCase"]
            }),
            &ctx,
        )
        .await
        .expect_err("should fail");

    assert!(
        err.to_string().contains("Invalid tags"),
        "error should mention invalid tags: {err}"
    );
}
