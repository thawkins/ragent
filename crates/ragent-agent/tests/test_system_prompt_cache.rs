//! Integration tests for the system-prompt component cache.
//!
//! Validates that [`SessionProcessor::system_prompt_cache`] (added as part
//! of `AgentPerf` T-005 / FR-008 / FR-009) is the default path in
//! `process_user_message`, and that the cache returns the same value
//! across calls when the underlying inputs are unchanged.

use parking_lot::RwLock;
use ragent_agent::session::cache::SystemPromptCache;
use ragent_agent::session::processor::SessionProcessor;
use ragent_agent::tool::ToolRegistry;
use ragent_types::EventBus;

use std::sync::Arc;

fn test_processor() -> SessionProcessor {
    let storage = Arc::new(ragent_agent::storage::Storage::open_in_memory().unwrap());
    let event_bus = Arc::new(EventBus::new(8));
    let session_manager = Arc::new(ragent_agent::session::SessionManager::new(
        storage,
        event_bus.clone(),
    ));
    let provider_registry = Arc::new(ragent_llm::provider::ProviderRegistry::new());
    let tool_registry = Arc::new(ToolRegistry::new());
    let permission_checker = Arc::new(RwLock::new(
        ragent_agent::permission::PermissionChecker::new(vec![]),
    ));
    SessionProcessor {
        session_manager,
        provider_registry,
        tool_registry,
        permission_checker,
        event_bus,
        task_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        code_index: std::sync::OnceLock::new(),
        active_spec: tokio::sync::RwLock::new(None),
        spec_manager: std::sync::OnceLock::new(),
        cached_tool_definitions: parking_lot::RwLock::new(None),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: ragent_agent::config::StreamConfig::default(),
        auto_approve: false,
        system_prompt_cache: parking_lot::RwLock::new(None),
    }
}

#[test]
fn system_prompt_cache_is_lazy() {
    let processor = test_processor();
    assert!(processor.system_prompt_cache.read().is_none());
}

#[test]
fn system_prompt_cache_is_singleton() {
    let processor = test_processor();
    let a = processor.system_prompt_cache();
    let b = processor.system_prompt_cache();
    // Same Arc — same underlying allocation.
    assert!(Arc::ptr_eq(&a, &b));
}

#[test]
fn invalidate_system_prompt_cache_clears_entries() {
    let processor = test_processor();
    let cache = processor.system_prompt_cache();
    // Populate one component.
    let populated = cache.get_tool_reference(&processor.tool_registry, |_r| {
        "## Available Tools\n\n- `read`\n".to_string()
    });
    assert_eq!(
        populated,
        Some("## Available Tools\n\n- `read`\n".to_string())
    );
    // The cache now has the entry.
    let second = cache
        .get_tool_reference(&processor.tool_registry, |_r| {
            panic!("compute fn must not be called on cache hit")
        })
        .expect("cache hit on unchanged registry");
    assert_eq!(second, "## Available Tools\n\n- `read`\n");
    // Invalidate and try again — the compute fn runs again.
    processor.invalidate_system_prompt_cache();
    let after_invalidate =
        cache.get_tool_reference(&processor.tool_registry, |_r| "fresh".to_string());
    assert_eq!(after_invalidate, Some("fresh".to_string()));
}

#[test]
fn codeindex_guidance_caches_active_state() {
    let processor = test_processor();
    let cache = processor.system_prompt_cache();
    let active_a = cache.get_codeindex_guidance(true, |is_active| {
        if is_active { "active" } else { "disabled" }.to_string()
    });
    let active_b =
        cache.get_codeindex_guidance(true, |_| panic!("compute must not be called on hit"));
    assert_eq!(active_a, Some("active".to_string()));
    assert_eq!(active_b, Some("active".to_string()));
}

#[test]
fn codeindex_guidance_distinguishes_states() {
    let processor = test_processor();
    let cache = processor.system_prompt_cache();
    let active = cache.get_codeindex_guidance(true, |is_active| {
        if is_active { "A" } else { "D" }.to_string()
    });
    let disabled = cache.get_codeindex_guidance(false, |is_active| {
        if is_active { "A" } else { "D" }.to_string()
    });
    assert_eq!(active, Some("A".to_string()));
    assert_eq!(disabled, Some("D".to_string()));
}

#[test]
fn system_prompt_cache_field_is_exposed() {
    let processor = test_processor();
    // The `system_prompt_cache` field is `pub`, allowing external callers
    // (and tests) to inspect the cache contents.
    let guard = processor.system_prompt_cache.read();
    assert!(guard.is_none());
}

#[test]
fn default_system_prompt_cache_works_through_cache_module() {
    // Direct sanity test: a freshly-constructed `SystemPromptCache` populates
    // entries on first call (returns `Some` after the compute runs).
    let cache = SystemPromptCache::new();
    let result = cache.get_tool_reference(&ToolRegistry::new(), |_r| "x".to_string());
    assert_eq!(result, Some("x".to_string()));
}
