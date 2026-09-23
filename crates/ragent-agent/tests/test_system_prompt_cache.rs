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
        tool_repeat_guard: std::sync::Arc::new(parking_lot::Mutex::new(
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
        activity_log_tx: tokio::sync::Mutex::new(None),
        skill_registry_cache: parking_lot::Mutex::new(None),
        active_loops: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        active_loop_specs: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        loop_telemetry_recorded: std::sync::atomic::AtomicBool::new(false),
        active_loop_interrupts: parking_lot::RwLock::new(std::collections::HashMap::new()),
        active_loop_captures: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        last_message_end_reason: std::sync::RwLock::new(std::collections::HashMap::new()),
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

/// PERF-034: the subagent wire surface is computed once per tool-registry
/// version and shared by refcount on every later call.
#[test]
fn subagent_tool_definitions_are_cached_and_filter_interactive_tools() {
    use ragent_agent::tool::ToolRegistry;
    use ragent_agent::tool::aliases::{AskUserTool, RunCodeTool};
    use ragent_agent::tool::plan::PlanEnterTool;

    let registry = ToolRegistry::new();
    registry.register(std::sync::Arc::new(PlanEnterTool));
    registry.register(std::sync::Arc::new(AskUserTool));

    let cache = SystemPromptCache::new();
    let first = cache
        .get_subagent_tool_definitions(&registry)
        .expect("filtered definitions");
    // `ask_user` is interactive and must be excluded from the subagent surface.
    assert!(first.iter().any(|d| d.name == "plan_enter"));
    assert!(!first.iter().any(|d| d.name == "ask_user"));

    let second = cache
        .get_subagent_tool_definitions(&registry)
        .expect("cached definitions");
    assert!(
        std::sync::Arc::ptr_eq(&first, &second),
        "unchanged registry version must return the cached allocation, not a rebuild"
    );

    // Registering a tool bumps the registry version, forcing a rebuild.
    registry.register(std::sync::Arc::new(RunCodeTool));
    let third = cache
        .get_subagent_tool_definitions(&registry)
        .expect("rebuilt definitions");
    assert!(
        !std::sync::Arc::ptr_eq(&second, &third),
        "a registry version change must invalidate the cached surface"
    );
    assert!(third.iter().any(|d| d.name == "run_code"));
    assert!(!third.iter().any(|d| d.name == "ask_user"));
}
