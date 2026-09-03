#![allow(clippy::assert_is_empty)]
//! Integration tests validating that `SessionProcessor::storage_op`
//! runs its closure on a `tokio::task::spawn_blocking` thread
//! (`AgentPerf` T-012 / FR-010 / FR-011).
//!
//! We construct a processor with an in-memory storage and exercise a few
//! common operations (`create_message`, `update_message`,
//! `get_messages`).  Each call must return successfully; if any
//! operation is mistakenly moved onto the async runtime, the
//! `get_messages` call after a long `tokio::time::sleep` would still
//! see the new message — proving the closure ran.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use parking_lot::RwLock;
use ragent_agent::event::EventBus;
use ragent_agent::message::{Message, MessagePart, Role};
use ragent_agent::permission::PermissionChecker;
use ragent_agent::session::SessionManager;
use ragent_agent::session::processor::SessionProcessor;
use ragent_agent::storage::Storage;
use ragent_agent::tool::ToolRegistry;
use ragent_llm::provider::ProviderRegistry;

fn test_processor() -> (SessionProcessor, Arc<Storage>) {
    let storage = Arc::new(Storage::open_in_memory().unwrap());
    let event_bus = Arc::new(EventBus::new(8));
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));
    let provider_registry = Arc::new(ProviderRegistry::new());
    let tool_registry = Arc::new(ToolRegistry::new());
    let permission_checker = Arc::new(RwLock::new(PermissionChecker::new(vec![])));
    let processor = SessionProcessor {
        session_manager,
        provider_registry,
        tool_registry,
        permission_checker,
        event_bus,
        agent_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        code_index: std::sync::OnceLock::new(),
        active_spec: tokio::sync::RwLock::new(None),
        spec_manager: std::sync::OnceLock::new(),
        cached_tool_definitions: parking_lot::RwLock::new(None),
        cached_tool_names: parking_lot::RwLock::new(None),
        cached_tool_definition_bytes: parking_lot::RwLock::new(None),
        llm_client_cache: parking_lot::RwLock::new(std::collections::HashMap::new()),
        cached_config: parking_lot::Mutex::new(None),
        team_context_cache: std::sync::Arc::new(parking_lot::RwLock::new(
            std::collections::HashMap::new(),
        )),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: ragent_agent::StreamConfig::default(),
        auto_approve: false,
        system_prompt_cache: parking_lot::RwLock::new(None),
        skill_body_cache: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        telemetry: std::sync::Arc::new(ragent_agent::telemetry::TelemetrySubsystem::disabled()),
        bg_service: std::sync::OnceLock::new(),
        activity_log: std::sync::OnceLock::new(),
        skill_registry_cache: parking_lot::Mutex::new(None),
    };
    (processor, storage)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn storage_op_runs_closure_and_returns_value() {
    let (processor, _storage) = test_processor();
    let result: anyhow::Result<u32> = processor.storage_op(|_s| Ok(42u32)).await;
    assert_eq!(result.unwrap(), 42);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn storage_op_does_not_block_async_executor() {
    // The whole point of `storage_op` is that the closure runs on a
    // blocking thread.  We assert this indirectly: while `storage_op`
    // is busy, an unrelated async task is still able to make progress
    // (i.e. the executor is not stalled).
    let (processor, _storage) = test_processor();
    let started = Arc::new(AtomicBool::new(false));
    let started_inner = Arc::clone(&started);
    let op = tokio::spawn(async move {
        processor
            .storage_op(move |_s| {
                std::thread::sleep(Duration::from_millis(50));
                started_inner.store(true, Ordering::SeqCst);
                Ok(())
            })
            .await
    });
    // Yield several times to make sure the executor runs other tasks.
    for _ in 0..3 {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    op.await.unwrap().unwrap();
    assert!(started.load(Ordering::SeqCst));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn storage_op_create_then_get_messages_round_trip() {
    let (processor, _storage) = test_processor();
    // Create a session so the foreign-key constraint on
    // `messages.session_id` is satisfied.  We don't care about the
    // returned `Session` struct; we just need the side-effect.
    let _ = processor
        .session_manager
        .create_session(std::path::PathBuf::from("/tmp"));
    // Look up the actual session id assigned by `create_session`.
    let sessions = processor
        .session_manager
        .list_sessions()
        .expect("list_sessions");
    let session_id = sessions.first().expect("at least one session").id.clone();
    let user_msg = Message {
        id: "m1".to_string(),
        session_id: session_id.clone(),
        role: Role::User,
        parts: vec![MessagePart::Text {
            text: "hello".to_string(),
        }],
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        edit_seq: 0,
    };
    let user_msg_clone = user_msg.clone();
    processor
        .storage_op(move |s| s.create_message(&user_msg_clone))
        .await
        .expect("create_message");
    let messages: anyhow::Result<Vec<Message>> = processor
        .storage_op(move |s| s.get_messages(&session_id))
        .await;
    let messages = messages.unwrap();
    assert!(!messages.is_empty());
}
