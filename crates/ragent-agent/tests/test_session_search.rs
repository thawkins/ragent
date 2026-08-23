//! Tests for the `session_search` tool.

use ragent_agent::event::EventBus;
use ragent_agent::storage::Storage;
use ragent_agent::tool::{Tool, ToolContext, session_search::SessionSearchTool};
use ragent_types::message::Message;
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
        canonical_cache: std::sync::Arc::new(ragent_tools_core::CanonicalPathCache::new()),
    }
}

#[tokio::test]
async fn test_session_search_finds_messages_across_sessions() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    storage.create_session("sess-a", "/tmp/project-a").unwrap();
    storage.create_session("sess-b", "/tmp/project-b").unwrap();
    storage
        .create_message(&Message::user_text("sess-a", "database migration plan"))
        .unwrap();
    storage
        .create_message(&Message::assistant_text(
            "sess-b",
            "the migration is complete",
        ))
        .unwrap();

    let tool = SessionSearchTool;
    let ctx = ctx_with_storage(storage, "current");
    let out = tool
        .execute(json!({"query": "migration", "limit": 10}), &ctx)
        .await
        .expect("execute");

    assert!(
        out.content.contains("sess-a"),
        "output should mention sess-a: {}",
        out.content
    );
    assert!(
        out.content.contains("sess-b"),
        "output should mention sess-b: {}",
        out.content
    );
    let meta = out.metadata.expect("metadata");
    assert_eq!(meta["result_count"], 2);
}

#[tokio::test]
async fn test_session_search_filters_by_session_id() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    storage.create_session("sess-a", "/tmp/project-a").unwrap();
    storage.create_session("sess-b", "/tmp/project-b").unwrap();
    storage
        .create_message(&Message::user_text("sess-a", "database migration"))
        .unwrap();
    storage
        .create_message(&Message::assistant_text("sess-b", "migration done"))
        .unwrap();

    let tool = SessionSearchTool;
    let ctx = ctx_with_storage(storage, "current");
    let out = tool
        .execute(json!({"query": "migration", "session_id": "sess-a"}), &ctx)
        .await
        .expect("execute");

    assert!(out.content.contains("sess-a"));
    assert!(!out.content.contains("sess-b"));
}

#[tokio::test]
async fn test_session_search_filters_by_role() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    storage.create_session("sess-a", "/tmp").unwrap();
    storage
        .create_message(&Message::user_text("sess-a", "migration question"))
        .unwrap();
    storage
        .create_message(&Message::assistant_text("sess-a", "migration answer"))
        .unwrap();

    let tool = SessionSearchTool;
    let ctx = ctx_with_storage(storage, "current");
    let out = tool
        .execute(json!({"query": "migration", "roles": ["user"]}), &ctx)
        .await
        .expect("execute");

    assert!(out.content.contains("user"));
    assert!(!out.content.contains("assistant"));
}

#[tokio::test]
async fn test_session_search_max_per_session() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    storage.create_session("sess-a", "/tmp").unwrap();
    storage.create_session("sess-b", "/tmp").unwrap();
    storage
        .create_message(&Message::user_text("sess-a", "migration one"))
        .unwrap();
    storage
        .create_message(&Message::user_text("sess-a", "migration two"))
        .unwrap();
    storage
        .create_message(&Message::user_text("sess-b", "migration three"))
        .unwrap();

    let tool = SessionSearchTool;
    let ctx = ctx_with_storage(storage, "current");
    let out = tool
        .execute(
            json!({"query": "migration", "max_per_session": 1, "limit": 10}),
            &ctx,
        )
        .await
        .expect("execute");

    let meta = out.metadata.expect("metadata");
    assert_eq!(meta["result_count"], 2);
}

#[tokio::test]
async fn test_session_search_no_match() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    storage.create_session("sess-a", "/tmp").unwrap();
    storage
        .create_message(&Message::user_text("sess-a", "hello"))
        .unwrap();

    let tool = SessionSearchTool;
    let ctx = ctx_with_storage(storage, "current");
    let out = tool
        .execute(json!({"query": "kubernetes"}), &ctx)
        .await
        .expect("execute");

    assert!(out.content.contains("No messages"));
    assert_eq!(out.metadata.expect("metadata")["result_count"], 0);
}
