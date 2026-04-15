//! Integration tests for Milestone 3 — Structured Memory Store.
//!
//! Tests the structured memory tools (memory_store, memory_recall, memory_forget)
//! and their interaction with the SQLite storage layer.

use std::path::PathBuf;
use std::sync::Arc;

use ragent_core::storage::Storage;
use ragent_core::tool::{Tool, ToolContext};

/// Helper: create a ToolContext with in-memory storage.
fn make_ctx() -> ToolContext {
    let storage = Arc::new(Storage::open_in_memory().unwrap());
    ToolContext {
        session_id: "test-session".to_string(),
        working_dir: PathBuf::from("/tmp/test-project"),
        event_bus: Arc::new(ragent_core::event::EventBus::new(100)),
        storage: Some(storage),
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
        code_index: None,
    }
}

// ── memory_store ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_memory_store_basic() {
    let ctx = make_ctx();
    let tool = ragent_core::tool::structured_memory::MemoryStoreTool;

    let input = serde_json::json!({
        "content": "Use Result<T, E> with thiserror for custom error types",
        "category": "pattern",
        "tags": ["rust", "error-handling"],
        "confidence": 0.9,
        "source": "manual"
    });

    let result = tool.execute(input, &ctx).await.unwrap();
    assert!(result.content.contains("Memory stored"));
    assert!(result.content.contains("pattern"));
    assert!(result.content.contains("0.90"));

    let meta = result.metadata.unwrap();
    assert!(meta["id"].as_i64().unwrap() > 0);
    assert_eq!(meta["category"], "pattern");
}

#[tokio::test]
async fn test_memory_store_invalid_category() {
    let ctx = make_ctx();
    let tool = ragent_core::tool::structured_memory::MemoryStoreTool;

    let input = serde_json::json!({
        "content": "test",
        "category": "invalid-category"
    });

    let result = tool.execute(input, &ctx).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid category"));
}

#[tokio::test]
async fn test_memory_store_invalid_confidence() {
    let ctx = make_ctx();
    let tool = ragent_core::tool::structured_memory::MemoryStoreTool;

    let input = serde_json::json!({
        "content": "test",
        "category": "fact",
        "confidence": 2.0
    });

    let result = tool.execute(input, &ctx).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Confidence"));
}

#[tokio::test]
async fn test_memory_store_no_storage() {
    let ctx = ToolContext {
        session_id: "test".to_string(),
        working_dir: PathBuf::from("/tmp"),
        event_bus: Arc::new(ragent_core::event::EventBus::new(100)),
        storage: None,
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
        code_index: None,
    };
    let tool = ragent_core::tool::structured_memory::MemoryStoreTool;

    let input = serde_json::json!({
        "content": "test",
        "category": "fact"
    });

    let result = tool.execute(input, &ctx).await;
    assert!(result.is_err());
}

// ── memory_recall ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_memory_recall_basic() {
    let ctx = make_ctx();

    // Store some memories first.
    let store_tool = ragent_core::tool::structured_memory::MemoryStoreTool;
    store_tool
        .execute(
            serde_json::json!({
                "content": "Use Result<T, E> for error handling in Rust",
                "category": "pattern",
                "tags": ["rust"],
                "confidence": 0.9
            }),
            &ctx,
        )
        .await
        .unwrap();

    store_tool
        .execute(
            serde_json::json!({
                "content": "Use try/except for error handling in Python",
                "category": "pattern",
                "tags": ["python"],
                "confidence": 0.8
            }),
            &ctx,
        )
        .await
        .unwrap();

    // Search for Rust patterns.
    let recall_tool = ragent_core::tool::structured_memory::MemoryRecallTool;
    let input = serde_json::json!({
        "query": "Rust error handling",
        "categories": ["pattern"],
        "tags": ["rust"]
    });

    let result = recall_tool.execute(input, &ctx).await.unwrap();
    assert!(result.content.contains("Use Result"));
    assert!(!result.content.contains("Python"));
}

#[tokio::test]
async fn test_memory_recall_with_min_confidence() {
    let ctx = make_ctx();

    let store_tool = ragent_core::tool::structured_memory::MemoryStoreTool;
    store_tool
        .execute(
            serde_json::json!({
                "content": "Low confidence observation",
                "category": "fact",
                "confidence": 0.3
            }),
            &ctx,
        )
        .await
        .unwrap();

    store_tool
        .execute(
            serde_json::json!({
                "content": "High confidence observation",
                "category": "fact",
                "confidence": 0.9
            }),
            &ctx,
        )
        .await
        .unwrap();

    let recall_tool = ragent_core::tool::structured_memory::MemoryRecallTool;
    let input = serde_json::json!({
        "query": "observation",
        "min_confidence": 0.8
    });

    let result = recall_tool.execute(input, &ctx).await.unwrap();
    assert!(result.content.contains("High confidence"));
    assert!(!result.content.contains("Low confidence"));
}

#[tokio::test]
async fn test_memory_recall_no_results() {
    let ctx = make_ctx();

    let recall_tool = ragent_core::tool::structured_memory::MemoryRecallTool;
    let input = serde_json::json!({
        "query": "nonexistent_xyzzy"
    });

    let result = recall_tool.execute(input, &ctx).await.unwrap();
    assert!(result.content.contains("No memories found"));
}

// ── memory_forget ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_memory_forget_by_id() {
    let ctx = make_ctx();

    let store_tool = ragent_core::tool::structured_memory::MemoryStoreTool;
    let result = store_tool
        .execute(
            serde_json::json!({
                "content": "Temporary fact",
                "category": "fact"
            }),
            &ctx,
        )
        .await
        .unwrap();

    let mem_id = result.metadata.unwrap()["id"].as_i64().unwrap();

    let forget_tool = ragent_core::tool::structured_memory::MemoryForgetTool;
    let input = serde_json::json!({
        "id": mem_id
    });

    let result = forget_tool.execute(input, &ctx).await.unwrap();
    assert!(result.content.contains("Deleted memory"));
}

#[tokio::test]
async fn test_memory_forget_by_filter_category() {
    let ctx = make_ctx();

    let store_tool = ragent_core::tool::structured_memory::MemoryStoreTool;
    store_tool
        .execute(
            serde_json::json!({
                "content": "Error entry 1",
                "category": "error"
            }),
            &ctx,
        )
        .await
        .unwrap();

    store_tool
        .execute(
            serde_json::json!({
                "content": "Important fact",
                "category": "fact"
            }),
            &ctx,
        )
        .await
        .unwrap();

    let forget_tool = ragent_core::tool::structured_memory::MemoryForgetTool;
    let input = serde_json::json!({
        "category": "error"
    });

    let result = forget_tool.execute(input, &ctx).await.unwrap();
    assert!(result.content.contains("Deleted"));

    // The fact should still exist.
    let recall_tool = ragent_core::tool::structured_memory::MemoryRecallTool;
    let recall_result = recall_tool
        .execute(
            serde_json::json!({
                "query": "Important fact"
            }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(recall_result.content.contains("Important fact"));
}

#[tokio::test]
async fn test_memory_forget_no_criteria() {
    let ctx = make_ctx();

    let forget_tool = ragent_core::tool::structured_memory::MemoryForgetTool;
    let input = serde_json::json!({});

    let result = forget_tool.execute(input, &ctx).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("criterion"));
}

#[tokio::test]
async fn test_memory_forget_nonexistent_id() {
    let ctx = make_ctx();

    let forget_tool = ragent_core::tool::structured_memory::MemoryForgetTool;
    let input = serde_json::json!({
        "id": 999999
    });

    let result = forget_tool.execute(input, &ctx).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

// ── Storage layer tests ──────────────────────────────────────────────────────

#[test]
fn test_storage_create_and_get_memory() {
    let storage = Storage::open_in_memory().unwrap();

    let id = storage
        .create_memory(
            "Test content",
            "fact",
            "manual",
            0.8,
            "my-project",
            "sess-1",
            &["test".to_string()],
        )
        .unwrap();

    assert!(id > 0);

    let mem = storage.get_memory(id).unwrap().unwrap();
    assert_eq!(mem.content, "Test content");
    assert_eq!(mem.category, "fact");
    assert_eq!(mem.confidence, 0.8);
    assert_eq!(mem.project, "my-project");
}

#[test]
fn test_storage_get_memory_tags() {
    let storage = Storage::open_in_memory().unwrap();

    let id = storage
        .create_memory(
            "Tagged memory",
            "pattern",
            "auto",
            0.7,
            "project",
            "sess-1",
            &["rust".to_string(), "error-handling".to_string()],
        )
        .unwrap();

    let tags = storage.get_memory_tags(id).unwrap();
    assert_eq!(tags, vec!["error-handling", "rust"]);
}

#[test]
fn test_storage_search_memories() {
    let storage = Storage::open_in_memory().unwrap();

    storage
        .create_memory(
            "Use Result<T, E> for Rust error handling",
            "pattern",
            "manual",
            0.9,
            "project",
            "sess-1",
            &["rust".to_string()],
        )
        .unwrap();

    storage
        .create_memory(
            "Use try/except for Python error handling",
            "pattern",
            "manual",
            0.8,
            "project",
            "sess-1",
            &["python".to_string()],
        )
        .unwrap();

    let results = storage
        .search_memories("Rust error", None, None, 10, 0.0)
        .unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].content.contains("Rust"));
}

#[test]
fn test_storage_search_with_category_filter() {
    let storage = Storage::open_in_memory().unwrap();

    storage
        .create_memory(
            "A fact about the project",
            "fact",
            "manual",
            0.9,
            "project",
            "sess-1",
            &[],
        )
        .unwrap();

    storage
        .create_memory(
            "A pattern for error handling",
            "pattern",
            "manual",
            0.8,
            "project",
            "sess-1",
            &[],
        )
        .unwrap();

    let results = storage
        .search_memories(
            "project pattern",
            Some(&["fact".to_string()]),
            None,
            10,
            0.0,
        )
        .unwrap();
    // Only "fact" category results should match the FTS query.
    // If "pattern" also matches FTS, the category filter will exclude it.
    for r in &results {
        assert_eq!(r.category, "fact");
    }
}

#[test]
fn test_storage_search_with_tag_filter() {
    let storage = Storage::open_in_memory().unwrap();

    storage
        .create_memory(
            "Rust error handling",
            "pattern",
            "manual",
            0.9,
            "project",
            "sess-1",
            &["rust".to_string(), "error".to_string()],
        )
        .unwrap();

    storage
        .create_memory(
            "Python error handling",
            "pattern",
            "manual",
            0.8,
            "project",
            "sess-1",
            &["python".to_string(), "error".to_string()],
        )
        .unwrap();

    let results = storage
        .search_memories("error handling", None, Some(&["rust".to_string()]), 10, 0.0)
        .unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].content.contains("Rust"));
}

#[test]
fn test_storage_list_memories() {
    let storage = Storage::open_in_memory().unwrap();

    for i in 1..=5 {
        storage
            .create_memory(
                &format!("Memory {i}"),
                "fact",
                "manual",
                0.5 + i as f64 * 0.1,
                "my-project",
                "sess-1",
                &[],
            )
            .unwrap();
    }

    let memories = storage.list_memories("my-project", 3).unwrap();
    assert_eq!(memories.len(), 3);
}

#[test]
fn test_storage_delete_memory() {
    let storage = Storage::open_in_memory().unwrap();

    let id = storage
        .create_memory(
            "To delete",
            "fact",
            "manual",
            0.5,
            "project",
            "sess-1",
            &["tag".to_string()],
        )
        .unwrap();

    let deleted = storage.delete_memory(id).unwrap();
    assert!(deleted);

    let mem = storage.get_memory(id).unwrap();
    assert!(mem.is_none());
}

#[test]
fn test_storage_delete_by_filter() {
    let storage = Storage::open_in_memory().unwrap();

    storage
        .create_memory("Error 1", "error", "manual", 0.3, "project", "s1", &[])
        .unwrap();

    storage
        .create_memory("Error 2", "error", "manual", 0.2, "project", "s1", &[])
        .unwrap();

    storage
        .create_memory(
            "Important fact",
            "fact",
            "manual",
            0.9,
            "project",
            "s1",
            &[],
        )
        .unwrap();

    let count = storage
        .delete_memories_by_filter(None, Some(0.5), Some("error"), None)
        .unwrap();
    assert_eq!(count, 2);

    // The fact should still exist.
    let remaining = storage.list_memories("project", 10).unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].category, "fact");
}

#[test]
fn test_storage_delete_by_filter_no_criteria() {
    let storage = Storage::open_in_memory().unwrap();
    let result = storage.delete_memories_by_filter(None, None, None, None);
    assert!(result.is_err());
}

#[test]
fn test_storage_update_confidence() {
    let storage = Storage::open_in_memory().unwrap();

    let id = storage
        .create_memory("Test", "fact", "manual", 0.5, "project", "s1", &[])
        .unwrap();

    let updated = storage.update_memory_confidence(id, 0.95).unwrap();
    assert!(updated);

    let mem = storage.get_memory(id).unwrap().unwrap();
    assert!((mem.confidence - 0.95).abs() < 0.001);
}

#[test]
fn test_storage_increment_access() {
    let storage = Storage::open_in_memory().unwrap();

    let id = storage
        .create_memory("Test", "fact", "manual", 0.5, "project", "s1", &[])
        .unwrap();

    let updated = storage.increment_memory_access(id).unwrap();
    assert!(updated);

    let mem = storage.get_memory(id).unwrap().unwrap();
    assert_eq!(mem.access_count, 1);
    assert!(mem.last_accessed.is_some());
}

#[test]
fn test_storage_count_memories() {
    let storage = Storage::open_in_memory().unwrap();

    assert_eq!(storage.count_memories().unwrap(), 0);

    storage
        .create_memory("A", "fact", "manual", 0.5, "project", "s1", &[])
        .unwrap();
    assert_eq!(storage.count_memories().unwrap(), 1);

    storage
        .create_memory("B", "pattern", "manual", 0.5, "project", "s1", &[])
        .unwrap();
    assert_eq!(storage.count_memories().unwrap(), 2);
}

#[test]
fn test_storage_search_increments_access_count() {
    let storage = Storage::open_in_memory().unwrap();

    let id = storage
        .create_memory(
            "Rust pattern for error handling",
            "pattern",
            "manual",
            0.8,
            "project",
            "s1",
            &["rust".to_string()],
        )
        .unwrap();

    // Access count should be 0 initially.
    let mem = storage.get_memory(id).unwrap().unwrap();
    assert_eq!(mem.access_count, 0);

    // Search should increment access count.
    let _ = storage
        .search_memories("Rust error", None, None, 10, 0.0)
        .unwrap();

    let mem = storage.get_memory(id).unwrap().unwrap();
    assert_eq!(mem.access_count, 1);
}

// ── Event emission tests ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_memory_store_emits_event() {
    let ctx = make_ctx();
    let mut rx = ctx.event_bus.subscribe();

    let tool = ragent_core::tool::structured_memory::MemoryStoreTool;
    let input = serde_json::json!({
        "content": "Test memory for event",
        "category": "fact"
    });

    tool.execute(input, &ctx).await.unwrap();

    let event = rx.try_recv().unwrap();
    match &event {
        ragent_core::event::Event::MemoryStored { category, .. } => {
            assert_eq!(category, "fact");
        }
        _ => panic!("Expected MemoryStored event, got {:?}", event),
    }
}

#[tokio::test]
async fn test_memory_recall_emits_event() {
    let ctx = make_ctx();

    // Store first.
    let store_tool = ragent_core::tool::structured_memory::MemoryStoreTool;
    store_tool
        .execute(
            serde_json::json!({
                "content": "Searchable content about Rust",
                "category": "fact"
            }),
            &ctx,
        )
        .await
        .unwrap();

    let mut rx = ctx.event_bus.subscribe();

    let recall_tool = ragent_core::tool::structured_memory::MemoryRecallTool;
    let input = serde_json::json!({
        "query": "Rust"
    });

    recall_tool.execute(input, &ctx).await.unwrap();

    let event = rx.try_recv().unwrap();
    match &event {
        ragent_core::event::Event::MemoryRecalled {
            query,
            result_count,
            ..
        } => {
            assert_eq!(query, "Rust");
            assert_eq!(*result_count, 1);
        }
        _ => panic!("Expected MemoryRecalled event, got {:?}", event),
    }
}

#[tokio::test]
async fn test_memory_forget_emits_event() {
    let ctx = make_ctx();

    let store_tool = ragent_core::tool::structured_memory::MemoryStoreTool;
    let result = store_tool
        .execute(
            serde_json::json!({
                "content": "To forget",
                "category": "error"
            }),
            &ctx,
        )
        .await
        .unwrap();

    // Skip the store event.
    let mut rx = ctx.event_bus.subscribe();

    let mem_id = result.metadata.unwrap()["id"].as_i64().unwrap();

    let forget_tool = ragent_core::tool::structured_memory::MemoryForgetTool;
    let input = serde_json::json!({
        "id": mem_id
    });

    forget_tool.execute(input, &ctx).await.unwrap();

    let event = rx.try_recv().unwrap();
    match &event {
        ragent_core::event::Event::MemoryForgotten { count, .. } => {
            assert_eq!(*count, 1);
        }
        _ => panic!("Expected MemoryForgotten event, got {:?}", event),
    }
}

// ── Config tests ──────────────────────────────────────────────────────────��──

#[test]
fn test_memory_config_defaults() {
    use ragent_core::config::MemoryConfig;

    let config = MemoryConfig::default();
    assert!(config.enabled);
    assert_eq!(config.tier, "core");
    assert!(config.structured.enabled);
    assert_eq!(config.retrieval.max_memories_per_prompt, 5);
    assert!((config.retrieval.recency_weight - 0.3).abs() < 0.001);
    assert!((config.retrieval.relevance_weight - 0.7).abs() < 0.001);
}

#[test]
fn test_memory_config_deserialization() {
    use ragent_core::config::MemoryConfig;

    let json = r#"{ "enabled": false, "tier": "structured" }"#;
    let config: MemoryConfig = serde_json::from_str(json).unwrap();
    assert!(!config.enabled);
    assert_eq!(config.tier, "structured");
}

#[test]
fn test_memory_config_missing_fields_use_defaults() {
    use ragent_core::config::MemoryConfig;

    let json = r#"{}"#;
    let config: MemoryConfig = serde_json::from_str(json).unwrap();
    assert!(config.enabled);
    assert_eq!(config.tier, "core");
    assert_eq!(config.retrieval.max_memories_per_prompt, 5);
}
