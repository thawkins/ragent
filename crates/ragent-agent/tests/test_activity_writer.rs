//! PERF-040: tests for the batched activity-log writer.
//!
//! The writer task ([`SessionProcessor::start_activity_writer`]) is the single
//! consumer of the activity-log queue; `record_activity_event` routes every
//! append through it instead of spawning a per-event blocking task.

use std::sync::Arc;

use parking_lot::RwLock;
use ragent_agent::session::processor::SessionProcessor;
use ragent_agent::tool::ToolRegistry;
use ragent_storage::ActivityLog;
use ragent_types::EventBus;

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
    }
}

/// PERF-040: with the writer running, `record_activity_event` appends through
/// the queue and the writer drains every event; without it, appends are dropped
/// rather than spawning a blocking task.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn writer_drains_queued_activity_events() {
    ragent_config::activity_log::set_enabled(true);
    let processor = test_processor();
    let log = Arc::new(ActivityLog::open_in_memory().expect("open activity log"));
    processor.set_activity_log(Arc::clone(&log));
    processor.start_activity_writer().await;

    let run = ragent_types::id::RunId::from("perf040-run");
    for i in 0..20 {
        let run = run.clone();
        processor
            .record_activity_event(move |log| {
                log.record_model_message(&run, "user", format!("msg-{i}"), None)
                    .map(Some)
            })
            .await;
    }

    // Wait for the writer to drain the queue (bounded poll).
    let mut count = 0u64;
    for _ in 0..100 {
        count = log.count(&run).unwrap_or(0);
        if count == 20 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_eq!(count, 20, "writer must persist every queued event");
}

/// PERF-040: when no writer has been started, `record_activity_event` is a
/// no-op — no panic, no task spawned, no events persisted.
#[tokio::test]
async fn no_writer_drops_events_without_panicking() {
    ragent_config::activity_log::set_enabled(true);
    let processor = test_processor();
    let log = Arc::new(ActivityLog::open_in_memory().expect("open activity log"));
    processor.set_activity_log(Arc::clone(&log));
    // Deliberately do NOT start the writer.

    let run = ragent_types::id::RunId::from("perf040-nostart");
    processor
        .record_activity_event(move |log| {
            log.record_model_message(&run, "user", "dropped", None)
                .map(Some)
        })
        .await;

    let run = ragent_types::id::RunId::from("perf040-nostart");
    assert_eq!(log.count(&run).unwrap_or(0), 0);
}
