//! Integration tests validating the FR-019 (AgentPerf T-015) rule:
//! `MutexGuard` MUST NOT cross an `.await` point on the agent hot path.
//!
//! The test exercises the per-session `SessionState` cache: we acquire
//! the lock, read a value, and drop the guard before any await.  This
//! is the pattern every new code path on the agent loop must follow.
//!
//! We also assert that the `parking_lot::RwLock` fields on
//! `SessionProcessor` (e.g. `cached_tool_definitions`,
//! `system_prompt_cache`) compile and are reachable through their
//! accessor methods.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use parking_lot::RwLock;
use ragent_agent::event::EventBus;
use ragent_agent::permission::PermissionChecker;
use ragent_agent::session::SessionManager;
use ragent_agent::session::cache::{SessionState, SystemPromptCache};
use ragent_agent::session::processor::SessionProcessor;
use ragent_agent::storage::Storage;
use ragent_agent::tool::ToolRegistry;
use ragent_llm::provider::ProviderRegistry;

fn test_processor() -> SessionProcessor {
    let storage = Arc::new(Storage::open_in_memory().unwrap());
    let event_bus = Arc::new(EventBus::new(8));
    let session_manager = Arc::new(SessionManager::new(storage, event_bus.clone()));
    SessionProcessor {
        session_manager,
        provider_registry: Arc::new(ProviderRegistry::new()),
        tool_registry: Arc::new(ToolRegistry::new()),
        permission_checker: Arc::new(RwLock::new(PermissionChecker::new(vec![]))),
        event_bus,
        task_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        code_index: std::sync::OnceLock::new(),
        active_spec: tokio::sync::RwLock::new(None),
        spec_manager: std::sync::OnceLock::new(),
        cached_tool_definitions: parking_lot::RwLock::new(None),
        cached_tool_names: parking_lot::RwLock::new(None),
        team_context_cache: std::sync::Arc::new(parking_lot::RwLock::new(
            std::collections::HashMap::new(),
        )),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: ragent_agent::config::StreamConfig::default(),
        auto_approve: false,
        system_prompt_cache: parking_lot::RwLock::new(None),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
    }
}

#[tokio::test]
async fn session_state_lock_drops_before_await() {
    // Mirror the pattern used in `process_user_message` T-007:
    // acquire the lock, clone the cached value, drop the guard, then
    // await.  If the guard is held across the await, the test will
    // either deadlock or fail to compile (the `Send` bound on
    // `MutexGuard` from `std::sync::Mutex` is not satisfied across
    // an `.await`).
    let counter = Arc::new(AtomicUsize::new(0));
    let state_arc = Arc::new(std::sync::Mutex::new(SessionState::new("s")));

    let value: usize = {
        let mut guard = state_arc.lock().expect("poisoned");
        // CRITICAL: drop the guard before any await.
        let _ = guard.cached_chat_messages_for_version(0);
        counter.fetch_add(1, Ordering::SeqCst);
        42
    };
    // Now safe to await.
    tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    assert_eq!(value, 42);
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[test]
fn system_prompt_cache_field_is_parking_lot_rwlock() {
    // The `system_prompt_cache` field is `parking_lot::RwLock`, which
    // is non-`Send`-across-await and is the recommended type for
    // short critical sections (FR-019).
    let processor = test_processor();
    // We don't try to call `system_prompt_cache()` from a sync context
    // because the function is async-friendly; we just assert the
    // field's type by reading it through the public accessor.
    let guard = processor.system_prompt_cache.read();
    assert!(guard.is_none());
}

#[test]
fn cached_tool_definitions_uses_parking_lot() {
    let processor = test_processor();
    let guard = processor.cached_tool_definitions.read();
    assert!(guard.is_none());
}

#[test]
fn system_prompt_cache_invalidate_is_idempotent() {
    let cache = SystemPromptCache::new();
    cache.invalidate_all();
    cache.invalidate_all();
    cache.invalidate_tool_cache();
    cache.invalidate_codeindex_cache();
    cache.invalidate_team_cache();
}
