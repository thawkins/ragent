//! Tests for the Agents-panel reconciliation after broadcast lag bursts.
//!
//! Root cause being covered: with many parallel sub-agents, per-`TextDelta`
//! streaming events can overflow the event-bus broadcast buffer; the TUI's
//! bridge task reports `Lagged` and the dropped window can silently swallow
//! a `SubagentStart` / `SubagentComplete`, leaving `active_tasks` (and the
//! Agents button count beside the chat input) stale.
//!
//! Fix: the bridge increments a shared lag counter; `poll_active_tasks_
//! reconcile` fetches an authoritative `AgentManager::tasks_snapshot` and
//! `apply_registry_reconciliation` merges it (re-add missing running agents,
//! remove ghosts of dropped completions, keep non-registry entries).

use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::Duration;

use chrono::Utc;
use ragent_agent::task::TaskEntry;
use ragent_tui::App;
use ragent_tui::app::AGENTS_RECONCILE_INTERVAL;

mod support;

use support::make_app;

/// Build a `TaskEntry` with an arbitrary status (registry-side simulation).
fn entry(id: &str, status: ragent_agent::task::TaskStatus) -> TaskEntry {
    TaskEntry {
        id: id.to_string(),
        parent_session_id: "s1".to_string(),
        child_session_id: format!("child-{id}"),
        agent_name: "explore".to_string(),
        task_prompt: "x".to_string(),
        background: true,
        status,
        result: None,
        error: None,
        created_at: Utc::now(),
        completed_at: None,
        reported: false,
        waiter_count: 0,
        output_file: None,
        report_status: ragent_agent::task::ReportStatus::default(),
    }
}

#[test]
fn test_reconcile_readds_dropped_start() {
    // SubagentStart for t2 was dropped in a lag burst: only t1 is tracked
    // even though the registry has both running.
    let mut app = make_app();
    app.active_tasks
        .push(entry("t1", ragent_agent::task::TaskStatus::Running));

    let snapshot = vec![
        entry("t1", ragent_agent::task::TaskStatus::Running),
        entry("t2", ragent_agent::task::TaskStatus::Running),
    ];
    app.apply_registry_reconciliation(snapshot);

    let ids: Vec<String> = app.active_tasks.iter().map(|t| t.id.clone()).collect();
    assert_eq!(ids, vec!["t1".to_string(), "t2".to_string()]);
    assert!(app.needs_redraw, "reconciliation must flag a redraw");
}

#[test]
fn test_reconcile_removes_ghost_of_dropped_completion() {
    // SubagentComplete for t1 was dropped: t1 lingers as a ghost while the
    // registry says Completed.
    let mut app = make_app();
    app.active_tasks
        .push(entry("t1", ragent_agent::task::TaskStatus::Running));

    let snapshot = vec![
        entry("t1", ragent_agent::task::TaskStatus::Completed),
        entry("t2", ragent_agent::task::TaskStatus::Running),
    ];
    app.apply_registry_reconciliation(snapshot);

    let ids: Vec<String> = app.active_tasks.iter().map(|t| t.id.clone()).collect();
    assert_eq!(ids, vec!["t2".to_string()]);
}

#[test]
fn test_reconcile_keeps_entries_not_tracked_by_registry() {
    // Bench tasks and other in-memory views are not in the agent registry;
    // a reconcile snapshot must never wipe them.
    let mut app = make_app();
    app.active_tasks
        .push(entry("bench-t1", ragent_agent::task::TaskStatus::Running));

    let snapshot = vec![entry("t2", ragent_agent::task::TaskStatus::Running)];
    app.apply_registry_reconciliation(snapshot);

    let ids: Vec<String> = app.active_tasks.iter().map(|t| t.id.clone()).collect();
    assert!(ids.contains(&"bench-t1".to_string()), "ids: {ids:?}");
    assert!(ids.contains(&"t2".to_string()), "ids: {ids:?}");
}

#[test]
fn test_reconcile_is_idempotent() {
    let mut app = make_app();
    app.active_tasks
        .push(entry("t1", ragent_agent::task::TaskStatus::Running));
    let snapshot = vec![
        entry("t1", ragent_agent::task::TaskStatus::Running),
        entry("t2", ragent_agent::task::TaskStatus::Running),
    ];
    app.apply_registry_reconciliation(snapshot.clone());
    let after_first: Vec<String> = app.active_tasks.iter().map(|t| t.id.clone()).collect();
    app.apply_registry_reconciliation(snapshot);
    let after_second: Vec<String> = app.active_tasks.iter().map(|t| t.id.clone()).collect();
    assert_eq!(after_first, after_second);
}

#[tokio::test]
async fn test_poll_triggers_on_lag_change_and_adopts_snapshot() {
    let mut app = make_app();
    // No AgentManager in the test processor: the trigger path records the
    // lag but cannot fetch; the adoption path is driven by depositing a
    // snapshot directly (mirroring the off-thread fetch landing).
    app.set_tui_event_lag_counter(Arc::new(AtomicU64::new(5)));
    app.poll_active_tasks_reconcile();
    assert_eq!(app.seen_event_lag, 5);
    assert!(
        !app.active_tasks_reconcile_inflight,
        "no manager means no in-flight fetch"
    );

    // Simulate the fetch landing.
    *app.active_tasks_reconcile_result.lock().unwrap() =
        Some(vec![entry("t9", ragent_agent::task::TaskStatus::Running)]);
    app.active_tasks_reconcile_inflight = true;
    app.poll_active_tasks_reconcile();

    let ids: Vec<String> = app.active_tasks.iter().map(|t| t.id.clone()).collect();
    assert_eq!(ids, vec!["t9".to_string()]);
    assert!(!app.active_tasks_reconcile_inflight);
}

#[tokio::test]
async fn test_poll_triggers_periodically_without_lag() {
    // The periodic safety net: even with NO lag-counter change, a poll after
    // AGENTS_RECONCILE_INTERVAL must start a registry fetch. This covers the
    // failure shape where SubagentStart is lost without the bridge observing
    // a broadcast Lagged (e.g. filtered by the session-lineage guard for
    // nested sub-agents) and the old lag-only trigger never fired.
    let mut app = make_app();
    app.set_tui_event_lag_counter(Arc::new(AtomicU64::new(0)));
    // First poll within the interval: nothing to do.
    app.poll_active_tasks_reconcile();
    assert!(
        !app.active_tasks_reconcile_inflight,
        "no fetch should start inside the interval"
    );

    // Wait for the interval to elapse, then poll again: the fetch must start
    // (no AgentManager in the test processor, so inflight resets immediately).
    tokio::time::sleep(AGENTS_RECONCILE_INTERVAL + Duration::from_millis(50)).await;
    app.poll_active_tasks_reconcile();
    assert!(
        !app.active_tasks_reconcile_inflight,
        "no manager: fetch skipped but interval timestamp refreshed"
    );
    assert_eq!(app.seen_event_lag, 0);
}

#[tokio::test]
async fn test_poll_end_to_end_with_registry_backed_manager() {
    // Full loop: bridge lag -> registry fetch -> merge into active_tasks.
    // Builds a real AgentManager (registry) seeded with a running task and
    // installs it into the app's session processor.
    let event_bus = Arc::new(ragent_agent::event::EventBus::new(16));
    let storage = Arc::new(ragent_agent::storage::Storage::open_in_memory().expect("storage"));
    let provider_registry = Arc::new(ragent_agent::provider::create_default_registry());
    let tool_registry = Arc::new(ragent_agent::tool::create_default_registry());
    let permission_checker = Arc::new(parking_lot::RwLock::new(
        ragent_agent::permission::PermissionChecker::new(vec![]),
    ));
    let session_manager = Arc::new(ragent_agent::session::SessionManager::new(
        storage.clone(),
        event_bus.clone(),
    ));
    let session_processor = Arc::new(ragent_agent::session::processor::SessionProcessor {
        session_manager,
        provider_registry,
        tool_registry,
        permission_checker,
        event_bus: event_bus.clone(),
        agent_manager: std::sync::OnceLock::new(),
        bg_service: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        code_index: std::sync::OnceLock::new(),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: ragent_agent::StreamConfig::default(),
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
        auto_approve: false,
        system_prompt_cache: parking_lot::RwLock::new(None),
        skill_body_cache: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        telemetry: std::sync::Arc::new(ragent_agent::telemetry::TelemetrySubsystem::disabled()),
        activity_log: std::sync::OnceLock::new(),
        skill_registry_cache: parking_lot::Mutex::new(None),
        active_loops: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        active_loop_specs: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        loop_telemetry_recorded: std::sync::atomic::AtomicBool::new(false),
        active_loop_interrupts: parking_lot::RwLock::new(std::collections::HashMap::new()),
        active_loop_captures: tokio::sync::RwLock::new(std::collections::HashMap::new()),
    });

    let manager = Arc::new(ragent_agent::task::AgentManager::new(
        event_bus.clone(),
        Arc::clone(&session_processor),
        4,
        300,
    ));
    manager
        .seed_completed_for_test(entry(
            "reconciled-1",
            ragent_agent::task::TaskStatus::Running,
        ))
        .await;
    session_processor
        .agent_manager
        .set(Arc::clone(&manager))
        .unwrap_or_else(|_| panic!("set agent manager once"));

    let agent_info = ragent_agent::agent::resolve_agent("general", &Default::default())
        .expect("resolve general agent");
    let mut app = App::new(
        event_bus,
        storage,
        Arc::new(ragent_agent::provider::create_default_registry()),
        Arc::clone(&session_processor),
        Arc::unwrap_or_clone(agent_info),
        false,
        std::path::PathBuf::new(),
    );

    // Simulate the bridge observing a lag burst.
    app.set_tui_event_lag_counter(Arc::new(AtomicU64::new(3)));
    app.poll_active_tasks_reconcile();
    assert!(app.active_tasks_reconcile_inflight, "fetch should start");

    // Pump the runtime until the spawned fetch deposits its snapshot.
    for _ in 0..200 {
        tokio::time::sleep(Duration::from_millis(10)).await;
        app.poll_active_tasks_reconcile();
        if !app.active_tasks_reconcile_inflight {
            break;
        }
    }
    assert!(
        !app.active_tasks_reconcile_inflight,
        "snapshot should have been adopted"
    );
    let ids: Vec<String> = app.active_tasks.iter().map(|t| t.id.clone()).collect();
    assert!(
        ids.contains(&"reconciled-1".to_string()),
        "registry running entry must be re-added, ids: {ids:?}"
    );
}
