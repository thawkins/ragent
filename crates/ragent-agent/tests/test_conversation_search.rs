#![allow(clippy::assert_is_empty)]
//! Tests for the `conversation_search` tool.

use ragent_agent::event::EventBus;
use ragent_agent::storage::Storage;
use ragent_agent::tool::{Tool, ToolContext, conversation_search::ConversationSearchTool};
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
        provider_registry: None,
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
async fn test_conversation_search_keyword_finds_match() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    storage.create_session("sess-1", "/tmp").unwrap();
    storage
        .create_message(&Message::user_text(
            "sess-1",
            "let's discuss database migration",
        ))
        .unwrap();
    storage
        .create_message(&Message::assistant_text(
            "sess-1",
            "the migration is scheduled",
        ))
        .unwrap();

    let tool = ConversationSearchTool;
    let ctx = ctx_with_storage(storage, "sess-1");
    let out = tool
        .execute(json!({"query": "migration", "limit": 5}), &ctx)
        .await
        .expect("execute");

    assert!(
        out.content.contains("migration"),
        "output should contain the matched term: {}",
        out.content
    );
    let meta = out.metadata.expect("metadata");
    assert_eq!(meta["mode"], "keyword");
    assert_eq!(meta["result_count"], 2);
}

#[tokio::test]
async fn test_conversation_search_keyword_no_match() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    storage.create_session("sess-1", "/tmp").unwrap();
    storage
        .create_message(&Message::user_text("sess-1", "hello"))
        .unwrap();

    let tool = ConversationSearchTool;
    let ctx = ctx_with_storage(storage, "sess-1");
    let out = tool
        .execute(json!({"query": "kubernetes"}), &ctx)
        .await
        .expect("execute");

    assert!(out.content.contains("No messages"));
    assert_eq!(out.metadata.expect("metadata")["result_count"], 0);
}

#[tokio::test]
async fn test_conversation_search_stats_mode() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    storage.create_session("sess-1", "/tmp").unwrap();
    storage
        .create_message(&Message::user_text("sess-1", "hello"))
        .unwrap();
    storage
        .create_message(&Message::assistant_text("sess-1", "hi"))
        .unwrap();

    let tool = ConversationSearchTool;
    let ctx = ctx_with_storage(storage, "sess-1");
    let out = tool
        .execute(json!({"mode": "stats"}), &ctx)
        .await
        .expect("execute");

    assert!(out.content.contains("Total messages: 2"));
    assert!(out.content.contains("User messages: 1"));
    assert!(out.content.contains("Assistant messages: 1"));
    let meta = out.metadata.expect("metadata");
    assert_eq!(meta["total"], 2);
    assert_eq!(meta["user_count"], 1);
    assert_eq!(meta["assistant_count"], 1);
}

#[tokio::test]
async fn test_conversation_search_turn_range() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    storage.create_session("sess-1", "/tmp").unwrap();
    storage
        .create_message(&Message::user_text("sess-1", "first"))
        .unwrap();
    storage
        .create_message(&Message::assistant_text("sess-1", "second"))
        .unwrap();
    storage
        .create_message(&Message::user_text("sess-1", "third"))
        .unwrap();

    let tool = ConversationSearchTool;
    let ctx = ctx_with_storage(storage, "sess-1");
    let out = tool
        .execute(
            json!({"mode": "turn_range", "start_turn": 2, "end_turn": 3}),
            &ctx,
        )
        .await
        .expect("execute");

    assert!(out.content.contains("Turns 2–3"));
    assert!(out.content.contains("second"));
    assert!(out.content.contains("third"));
    assert!(!out.content.contains("first"));
}
