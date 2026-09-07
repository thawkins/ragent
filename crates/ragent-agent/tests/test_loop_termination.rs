//! Integration tests for the goal-driven loop lifecycle in
//! `SessionProcessor` (spec `agentloop`, tasks T-003/T-004): loop
//! registration, stop condition 1 (no-tool-call response terminates with
//! `GoalAchieved`), `Event::LoopTerminated` publication, and tracker
//! removal after termination (FR-010, FR-017, FR-025).

use std::collections::HashMap;
use std::sync::Arc;

use ragent_agent::event::{Event, EventBus};
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::ProviderRegistry;
use ragent_agent::session::SessionManager;
use ragent_agent::session::loop_state::{LoopSpec, StopCondition};
use ragent_agent::session::processor::SessionProcessor;
use ragent_agent::storage::Storage;
use ragent_agent::tool;

/// Build a minimal `SessionProcessor` for loop-lifecycle tests.
fn make_processor(event_bus: Arc<EventBus>) -> SessionProcessor {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let session_manager = Arc::new(SessionManager::new(storage, event_bus.clone()));
    SessionProcessor {
        session_manager,
        provider_registry: Arc::new(ProviderRegistry::new()),
        tool_registry: Arc::new(tool::create_default_registry()),
        permission_checker: Arc::new(parking_lot::RwLock::new(PermissionChecker::new(vec![]))),
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
        llm_client_cache: parking_lot::RwLock::new(HashMap::new()),
        cached_config: parking_lot::Mutex::new(None),
        team_context_cache: Arc::new(parking_lot::RwLock::new(HashMap::new())),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: ragent_agent::StreamConfig::default(),
        auto_approve: false,
        system_prompt_cache: parking_lot::RwLock::new(None),
        skill_body_cache: Arc::new(std::sync::RwLock::new(HashMap::new())),
        read_timestamps: Arc::new(std::sync::RwLock::new(HashMap::new())),
        telemetry: Arc::new(ragent_agent::telemetry::TelemetrySubsystem::disabled()),
        bg_service: std::sync::OnceLock::new(),
        activity_log: std::sync::OnceLock::new(),
        skill_registry_cache: parking_lot::Mutex::new(None),
        active_loops: tokio::sync::RwLock::new(HashMap::new()),
        active_loop_specs: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        loop_telemetry_recorded: std::sync::atomic::AtomicBool::new(false),
        active_loop_interrupts: parking_lot::RwLock::new(std::collections::HashMap::new()),
        active_loop_captures: tokio::sync::RwLock::new(std::collections::HashMap::new()),
    }
}

#[tokio::test]
async fn test_loop_start_makes_loop_active_for_session() {
    let event_bus = Arc::new(EventBus::new(64));
    let processor = make_processor(event_bus);

    assert!(!processor.loop_active("s1").await);
    processor
        .start_loop("s1", LoopSpec::new("coder", "all tests pass"))
        .await;
    assert!(processor.loop_active("s1").await);
    // A second session is unaffected by the first session's loop.
    assert!(!processor.loop_active("s2").await);
}

#[tokio::test]
async fn test_start_loop_replaces_existing_tracker() {
    let event_bus = Arc::new(EventBus::new(64));
    let processor = make_processor(event_bus);

    let mut first = LoopSpec::new("coder", "goal one");
    first.max_steps = Some(5);
    processor.start_loop("s1", first).await;

    // Replacing the spec resets the tracker (new budget, fresh steps).
    let second = LoopSpec::new("coder", "goal two");
    processor.start_loop("s1", second).await;
    assert!(processor.loop_active("s1").await);
    // The replaced tracker terminates cleanly.
    let stopped = processor
        .terminate_loop("s1", StopCondition::GoalAchieved, None, None)
        .await;
    assert_eq!(stopped, Some(StopCondition::GoalAchieved));
    assert!(!processor.loop_active("s1").await);
}

#[tokio::test]
async fn test_terminate_loop_publishes_completed_event_and_deactivates() {
    let event_bus = Arc::new(EventBus::new(64));
    let mut rx = event_bus.subscribe();
    let processor = make_processor(event_bus.clone());

    let mut spec = LoopSpec::new("coder", "all existing tests should pass");
    spec.max_steps = Some(10);
    processor.start_loop("session-a", spec).await;

    let stopped = processor
        .terminate_loop("session-a", StopCondition::GoalAchieved, None, None)
        .await;
    assert_eq!(stopped, Some(StopCondition::GoalAchieved));
    assert!(!processor.loop_active("session-a").await);

    let event = rx.try_recv().expect("LoopTerminated published");
    let Event::LoopTerminated {
        session_id,
        status,
        iterations,
        verification,
        reason,
    } = event
    else {
        panic!("expected LoopTerminated, got {event:?}");
    };
    assert_eq!(session_id, "session-a");
    assert_eq!(status, "completed");
    assert_eq!(verification, None);
    assert_eq!(reason, None);
    // No steps were recorded on the tracker (the loop engine's budget-gate
    // wiring in T-003 increments it), so the published count is the
    // tracker's step total.
    assert_eq!(iterations, 0);
}

#[tokio::test]
async fn test_terminate_loop_without_active_loop_is_noop() {
    let event_bus = Arc::new(EventBus::new(64));
    let mut rx = event_bus.subscribe();
    let processor = make_processor(event_bus);

    let stopped = processor
        .terminate_loop("missing", StopCondition::GoalAchieved, None, None)
        .await;
    assert_eq!(stopped, None);
    assert!(
        rx.try_recv().is_err(),
        "no event should be published without an active loop"
    );
}

#[tokio::test]
async fn test_terminate_loop_is_idempotent_tracker_removed() {
    let event_bus = Arc::new(EventBus::new(64));
    let processor = make_processor(event_bus);

    processor
        .start_loop("s1", LoopSpec::new("coder", "goal"))
        .await;
    let first = processor
        .terminate_loop("s1", StopCondition::GoalAchieved, None, None)
        .await;
    let second = processor
        .terminate_loop("s1", StopCondition::UnrecoverableError, None, None)
        .await;
    assert_eq!(first, Some(StopCondition::GoalAchieved));
    // FR-017: no stage runs after termination — the tracker is gone, so a
    // second termination has nothing to act on.
    assert_eq!(second, None);
    assert!(!processor.loop_active("s1").await);
}

#[tokio::test]
async fn test_clear_loop_removes_tracker_without_terminating() {
    let event_bus = Arc::new(EventBus::new(64));
    let processor = make_processor(event_bus);

    processor
        .start_loop("s1", LoopSpec::new("coder", "goal"))
        .await;
    processor.clear_loop("s1").await;
    assert!(!processor.loop_active("s1").await);
    // After clearing, termination is a no-op.
    let stopped = processor
        .terminate_loop("s1", StopCondition::BudgetExhausted, None, None)
        .await;
    assert_eq!(stopped, None);
}

#[tokio::test]
async fn test_terminate_loop_carries_verification_and_reason() {
    let event_bus = Arc::new(EventBus::new(64));
    let mut rx = event_bus.subscribe();
    let processor = make_processor(event_bus);

    processor
        .start_loop("s1", LoopSpec::new("coder", "goal"))
        .await;
    let stopped = processor
        .terminate_loop(
            "s1",
            StopCondition::BudgetExhausted,
            Some("cargo test: 3 failed".to_string()),
            Some("step budget exhausted".to_string()),
        )
        .await;
    assert_eq!(stopped, Some(StopCondition::BudgetExhausted));

    let event = rx.try_recv().expect("LoopTerminated published");
    let Event::LoopTerminated {
        status,
        verification,
        reason,
        ..
    } = event
    else {
        panic!("expected LoopTerminated, got {event:?}");
    };
    assert_eq!(status, "budget_exhausted");
    assert_eq!(verification.as_deref(), Some("cargo test: 3 failed"));
    assert_eq!(reason.as_deref(), Some("step budget exhausted"));
}
