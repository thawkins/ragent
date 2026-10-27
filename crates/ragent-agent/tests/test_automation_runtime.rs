//! Tests-only coverage for the automation runtime (`ragent_agent::automation`
//! `AutomationService`): run enqueue/history accessors, the async
//! `execute_run`/`run_turn`/`write_output`/`finalise` pipeline, and the
//! background `scheduler_loop` / `AutomationSchedulerHandle` (spec `openhands`
//! FR-013, FR-014, FR-018, FR-033).
//!
//! The run pipeline is driven hermetically: a failing run uses a processor with
//! no provider (so the turn errors and the record is finalised as `Error`), and
//! a successful run uses a scripted provider that streams a single text reply
//! (so the output file is written). The scheduler is driven with a one-shot
//! schedule whose due time is already in the past, so it fires immediately.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use anyhow::Result;
use futures::stream;
use ragent_agent::automation::AutomationService;
use ragent_agent::event::EventBus;
use ragent_agent::llm::{ChatRequest, LlmClient, LlmFinishReason, StreamEvent};
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::{ModelInfo, Provider, ProviderRegistry};
use ragent_agent::session::SessionManager;
use ragent_agent::session::processor::{CachedConfig, SessionProcessor};
use ragent_agent::storage::Storage;
use ragent_agent::tool;
use ragent_config::{
    AutomationConfig, AutomationDefinition, AutomationTriggerKind, Capabilities, Config, Cost,
};
use ragent_types::{AutomationRun, AutomationTrigger, RunOutcome};

// ---------------------------------------------------------------------------
// Processors
// ---------------------------------------------------------------------------

/// A `SessionProcessor` over in-memory storage with no provider: any agent turn
/// fails, so the run is finalised as `Error`.
fn bare_processor() -> Arc<SessionProcessor> {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let event_bus = Arc::new(EventBus::new(64));
    let session_manager = Arc::new(SessionManager::new(storage, event_bus.clone()));
    Arc::new(SessionProcessor {
        session_manager,
        provider_registry: Arc::new(ProviderRegistry::new()),
        tool_registry: Arc::new(tool::create_default_registry()),
        permission_checker: Arc::new(parking_lot::RwLock::new(PermissionChecker::new(vec![]))),
        event_bus,
        agent_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        connector_session: tokio::sync::RwLock::new(None),
        connector_statuses: tokio::sync::RwLock::new(None),
        code_index: std::sync::OnceLock::new(),
        active_spec: tokio::sync::RwLock::new(None),
        spec_manager: std::sync::OnceLock::new(),
        cached_tool_definitions: parking_lot::RwLock::new(None),
        cached_tool_names: parking_lot::RwLock::new(None),
        cached_tool_definition_bytes: parking_lot::RwLock::new(None),
        llm_client_cache: parking_lot::RwLock::new(HashMap::new()),
        cached_config: parking_lot::Mutex::new(None),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: Default::default(),
        auto_approve: false,
        team_context_cache: Arc::new(parking_lot::RwLock::new(HashMap::new())),
        tool_repeat_guard: Arc::new(parking_lot::Mutex::new(HashMap::new())),
        system_prompt_cache: parking_lot::RwLock::new(None),
        skill_body_cache: Arc::new(std::sync::RwLock::new(HashMap::new())),
        read_timestamps: Arc::new(std::sync::RwLock::new(HashMap::new())),
        telemetry: Arc::new(ragent_agent::telemetry::TelemetrySubsystem::disabled()),
        bg_service: std::sync::OnceLock::new(),
        activity_log: std::sync::OnceLock::new(),
        activity_log_tx: tokio::sync::Mutex::new(None),
        skill_registry_cache: parking_lot::Mutex::new(None),
        active_loops: tokio::sync::RwLock::new(HashMap::new()),
        active_loop_specs: tokio::sync::RwLock::new(HashMap::new()),
        loop_telemetry_recorded: std::sync::atomic::AtomicBool::new(false),
        active_loop_interrupts: parking_lot::RwLock::new(HashMap::new()),
        active_loop_captures: tokio::sync::RwLock::new(HashMap::new()),
        last_message_end_reason: std::sync::RwLock::new(HashMap::new()),
        pending_verdicts: ragent_agent::session::pending::PendingVerdicts::new(),
    })
}

/// A minimal provider whose client streams one text reply then `Stop`.
struct ScriptedProvider {
    calls: Arc<AtomicU32>,
}

struct ScriptedClient {
    calls: Arc<AtomicU32>,
}

#[async_trait::async_trait]
impl LlmClient for ScriptedClient {
    async fn chat(
        &self,
        _request: ChatRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let events: [StreamEvent; 3] = [
            StreamEvent::TextDelta {
                text: "automation done".to_string(),
            },
            StreamEvent::Usage {
                input_tokens: 3,
                output_tokens: 2,
            },
            StreamEvent::Finish {
                reason: LlmFinishReason::Stop,
            },
        ];
        Ok(Box::pin(stream::iter(events)))
    }
}

#[async_trait::async_trait]
impl Provider for ScriptedProvider {
    fn id(&self) -> &'static str {
        "ollama"
    }

    fn name(&self) -> &'static str {
        "Scripted Mock Ollama"
    }

    fn default_models(&self) -> Vec<ModelInfo> {
        vec![ModelInfo {
            id: "qwen3:latest".to_string(),
            provider_id: "ollama".to_string(),
            name: "Qwen3".to_string(),
            cost: Cost {
                input: 0.0,
                output: 0.0,
            },
            capabilities: Capabilities {
                vision: false,
                tool_use: true,
                thinking_levels: vec![],
                reasoning: false,
                streaming: true,
            },
            context_window: 128_000,
            max_output: Some(8_192),
            request_multiplier: None,
            thinking_config: None,
        }]
    }

    fn as_any_static(&self) -> &(dyn std::any::Any + 'static) {
        self
    }

    async fn create_client(
        &self,
        _api_key: &str,
        _base_url: Option<&str>,
        _options: &HashMap<String, serde_json::Value>,
    ) -> Result<Box<dyn LlmClient>> {
        Ok(Box::new(ScriptedClient {
            calls: Arc::clone(&self.calls),
        }))
    }
}

/// A processor whose provider streams a successful single-text reply, so the
/// run's turn succeeds and its output file is written.
fn scripted_processor() -> Arc<SessionProcessor> {
    let mut registry = ProviderRegistry::new();
    registry.register(Box::new(ScriptedProvider {
        calls: Arc::new(AtomicU32::new(0)),
    }));

    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let event_bus = Arc::new(EventBus::new(64));
    let session_manager = Arc::new(SessionManager::new(storage, event_bus.clone()));
    let config: Config = serde_json::from_str("{}").expect("default config JSON");
    Arc::new(SessionProcessor {
        session_manager,
        provider_registry: Arc::new(registry),
        tool_registry: Arc::new(tool::create_default_registry()),
        permission_checker: Arc::new(parking_lot::RwLock::new(PermissionChecker::new(vec![]))),
        event_bus,
        agent_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        connector_session: tokio::sync::RwLock::new(None),
        connector_statuses: tokio::sync::RwLock::new(None),
        code_index: std::sync::OnceLock::new(),
        active_spec: tokio::sync::RwLock::new(None),
        spec_manager: std::sync::OnceLock::new(),
        cached_tool_definitions: parking_lot::RwLock::new(None),
        cached_tool_names: parking_lot::RwLock::new(None),
        cached_tool_definition_bytes: parking_lot::RwLock::new(None),
        llm_client_cache: parking_lot::RwLock::new(HashMap::new()),
        cached_config: parking_lot::Mutex::new(Some(CachedConfig {
            config: Arc::new(config),
            file_mtimes: Vec::new(),
            env_overrides_present: false,
        })),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: Default::default(),
        auto_approve: false,
        team_context_cache: Arc::new(parking_lot::RwLock::new(HashMap::new())),
        tool_repeat_guard: Arc::new(parking_lot::Mutex::new(HashMap::new())),
        system_prompt_cache: parking_lot::RwLock::new(None),
        skill_body_cache: Arc::new(std::sync::RwLock::new(HashMap::new())),
        read_timestamps: Arc::new(std::sync::RwLock::new(HashMap::new())),
        telemetry: Arc::new(ragent_agent::telemetry::TelemetrySubsystem::disabled()),
        bg_service: std::sync::OnceLock::new(),
        activity_log: std::sync::OnceLock::new(),
        activity_log_tx: tokio::sync::Mutex::new(None),
        skill_registry_cache: parking_lot::Mutex::new(None),
        active_loops: tokio::sync::RwLock::new(HashMap::new()),
        active_loop_specs: tokio::sync::RwLock::new(HashMap::new()),
        loop_telemetry_recorded: std::sync::atomic::AtomicBool::new(false),
        active_loop_interrupts: parking_lot::RwLock::new(HashMap::new()),
        active_loop_captures: tokio::sync::RwLock::new(HashMap::new()),
        last_message_end_reason: std::sync::RwLock::new(HashMap::new()),
        pending_verdicts: ragent_agent::session::pending::PendingVerdicts::new(),
    })
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn definition(id: &str, trigger: AutomationTriggerKind, enabled: bool) -> AutomationDefinition {
    AutomationDefinition {
        id: id.to_string(),
        name: None,
        agent: None,
        prompt: "run {{payload}}".to_string(),
        trigger,
        backend: None,
        dispatch: Vec::new(),
        enabled,
    }
}

fn config_with(defs: Vec<AutomationDefinition>) -> AutomationConfig {
    AutomationConfig {
        enabled: true,
        run_history_cap: 3,
        scheduler_tick_secs: 1,
        automations: defs,
        output_dir: None,
    }
}

/// Poll `get_run` until the run reaches a terminal state, or panic on timeout.
async fn wait_terminal(service: &Arc<AutomationService>, run_id: &str) -> AutomationRun {
    for _ in 0..200 {
        if let Ok(Some(run)) = service.get_run(run_id).await
            && run.outcome.is_some()
        {
            return run;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("run {run_id} never reached a terminal state");
}

// ---------------------------------------------------------------------------
// Accessors
// ---------------------------------------------------------------------------

#[test]
fn config_and_definition_accessors() {
    let service = AutomationService::new(
        bare_processor(),
        config_with(vec![
            definition("on-issue", AutomationTriggerKind::Webhook, true),
            definition("off", AutomationTriggerKind::Webhook, false),
        ]),
        std::path::PathBuf::from("."),
    );

    assert!(service.is_enabled());
    assert_eq!(service.definitions().len(), 2);
    assert_eq!(service.config().run_history_cap, 3);
    assert_eq!(
        service.definition("on-issue").map(|d| d.id.as_str()),
        Some("on-issue")
    );
    // A disabled definition is filtered out by `definition`.
    assert!(service.definition("off").is_none());
    assert!(service.definition("nope").is_none());
    // A fresh service has no in-flight run.
    assert!(!service.is_running("on-issue"));
}

#[tokio::test]
async fn run_now_returns_a_run_and_rejects_unknown() {
    let service = Arc::new(AutomationService::new(
        bare_processor(),
        config_with(vec![definition(
            "on-issue",
            AutomationTriggerKind::Webhook,
            true,
        )]),
        std::path::PathBuf::from("."),
    ));

    let run = service.run_now("on-issue").await.expect("run_now ok");
    assert_eq!(run.automation_id, "on-issue");
    assert_eq!(run.trigger, AutomationTrigger::Manual);

    let err = service.run_now("nope").await.expect_err("unknown id");
    assert!(err.to_string().contains("nope"), "err: {err}");
}

#[tokio::test]
async fn list_runs_and_get_run_hit_and_miss() {
    let service = Arc::new(AutomationService::new(
        bare_processor(),
        config_with(vec![definition(
            "on-issue",
            AutomationTriggerKind::Webhook,
            true,
        )]),
        std::path::PathBuf::from("."),
    ));

    let run = service.run_now("on-issue").await.expect("run_now ok");

    let listed = service.list_runs("on-issue", 10).await.expect("list_runs");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, run.id);

    let fetched = service
        .get_run(&run.id)
        .await
        .expect("get_run")
        .expect("present");
    assert_eq!(fetched.automation_id, "on-issue");

    assert!(
        service
            .get_run("no-such-run")
            .await
            .expect("get_run")
            .is_none()
    );
}

// ---------------------------------------------------------------------------
// Run pipeline
// ---------------------------------------------------------------------------

#[tokio::test]
async fn successful_run_writes_output_file() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let service = Arc::new(AutomationService::new(
        scripted_processor(),
        config_with(vec![definition(
            "writer",
            AutomationTriggerKind::Webhook,
            true,
        )]),
        tmp.path().to_path_buf(),
    ));

    let run = service
        .enqueue("writer", AutomationTrigger::Webhook, "go")
        .await
        .expect("enqueue");

    let terminal = wait_terminal(&service, &run.id).await;
    assert_eq!(terminal.outcome, Some(RunOutcome::Success));

    let out = terminal.output_ref.expect("output_ref written");
    let path = std::path::Path::new(&out);
    assert!(path.exists(), "output file must exist at {out}");
    assert_eq!(
        std::fs::read_to_string(path).expect("read output"),
        "automation done"
    );
    assert!(
        out.contains("log/automation"),
        "output defaults under log/automation: {out}"
    );

    // The in-flight marker is cleared once the run finishes.
    assert!(!service.is_running("writer"));
}

#[tokio::test]
async fn failing_run_records_error_outcome() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let service = Arc::new(AutomationService::new(
        bare_processor(),
        config_with(vec![definition(
            "broken",
            AutomationTriggerKind::Webhook,
            true,
        )]),
        tmp.path().to_path_buf(),
    ));

    let run = service.run_now("broken").await.expect("run_now ok");
    let terminal = wait_terminal(&service, &run.id).await;

    assert_eq!(terminal.outcome, Some(RunOutcome::Error));
    // The error path records the child session id as the output reference.
    assert!(terminal.output_ref.is_some());
    assert!(!service.is_running("broken"));
}

#[tokio::test]
async fn write_output_failure_leaves_no_output_ref() {
    // An `output_dir` that points at an existing file makes `create_dir_all`
    // fail, so `write_output` returns `None` while the turn still succeeds.
    let tmp = tempfile::tempdir().expect("tempdir");
    let not_a_dir = tmp.path().join("blocker");
    std::fs::write(&not_a_dir, "x").expect("seed blocker file");

    let mut config = config_with(vec![definition(
        "writer",
        AutomationTriggerKind::Webhook,
        true,
    )]);
    config.output_dir = Some(not_a_dir);
    let service = Arc::new(AutomationService::new(
        scripted_processor(),
        config,
        tmp.path().to_path_buf(),
    ));

    let run = service.run_now("writer").await.expect("run_now ok");
    let terminal = wait_terminal(&service, &run.id).await;

    assert_eq!(terminal.outcome, Some(RunOutcome::Success));
    assert!(
        terminal.output_ref.is_none(),
        "a failed write must not record an output reference"
    );
}

// ---------------------------------------------------------------------------
// Scheduler
// ---------------------------------------------------------------------------

#[tokio::test]
async fn scheduler_fires_a_due_schedule_and_stops() {
    let tmp = tempfile::tempdir().expect("tempdir");
    // A one-shot schedule whose due time is already in the past: the scheduler
    // enqueues on its first tick, then drops the schedule (advance == None).
    let service = Arc::new(AutomationService::new(
        bare_processor(),
        config_with(vec![definition(
            "sched",
            AutomationTriggerKind::Schedule {
                schedule: "at 2020-01-01T00:00:00Z".to_string(),
            },
            true,
        )]),
        tmp.path().to_path_buf(),
    ));

    let handle = service.spawn_scheduler();

    let mut fired = false;
    for _ in 0..200 {
        if let Ok(runs) = service.list_runs("sched", 10).await
            && !runs.is_empty()
        {
            assert_eq!(runs[0].trigger, AutomationTrigger::Schedule);
            fired = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(fired, "the scheduler must enqueue the due one-shot run");

    handle.stop();
    // Dropping the handle also signals stop (idempotent).
    drop(handle);
}

#[tokio::test]
async fn scheduler_loop_returns_immediately_when_disabled() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut config = config_with(vec![definition(
        "sched",
        AutomationTriggerKind::Schedule {
            schedule: "at 2020-01-01T00:00:00Z".to_string(),
        },
        true,
    )]);
    config.enabled = false;
    let service = Arc::new(AutomationService::new(
        bare_processor(),
        config,
        tmp.path().to_path_buf(),
    ));

    let handle = service.spawn_scheduler();
    tokio::time::sleep(Duration::from_millis(50)).await;

    // A disabled service's loop returns before seeding any schedule.
    assert_eq!(service.list_runs("sched", 10).await.expect("list").len(), 0);
    handle.stop();
}
