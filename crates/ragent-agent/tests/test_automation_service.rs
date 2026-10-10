//! Integration tests for the automation service: backend confinement and
//! third-party dispatch (spec `openhands` T-016; FR-013, FR-014, FR-018,
//! FR-033).
//!
//! The dispatch payloads and the execution-backend confinement are exercised
//! hermetically; the enqueue path is exercised against in-memory storage so the
//! durable run record is asserted without a live model.

use std::sync::Arc;

use ragent_agent::automation::{
    AutomationService, apply_session_backend_override, backend_descriptor, clear_session_backend,
    payload_for, register_session_backend, resolve_token, session_backend_override,
};
use ragent_agent::event::EventBus;
use ragent_agent::permission::PermissionChecker;
use ragent_agent::session::SessionManager;
use ragent_agent::session::processor::SessionProcessor;
use ragent_agent::storage::Storage;
use ragent_agent::tool::ToolRegistry;
use ragent_config::{
    AutomationConfig, AutomationDefinition, AutomationTriggerKind, Config, DispatchTarget,
};
use ragent_types::{AutomationRun, AutomationTrigger, RunOutcome};

/// A `SessionProcessor` over in-memory storage with no provider.
fn test_processor() -> (Arc<SessionProcessor>, Arc<Storage>) {
    let storage = Arc::new(Storage::open_in_memory().unwrap());
    let event_bus = Arc::new(EventBus::new(8));
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));
    let processor = SessionProcessor {
        session_manager,
        provider_registry: Arc::new(ragent_agent::provider::ProviderRegistry::new()),
        tool_registry: Arc::new(ToolRegistry::new()),
        permission_checker: Arc::new(parking_lot::RwLock::new(PermissionChecker::new(vec![]))),
        event_bus: event_bus.clone(),
        agent_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        connector_session: tokio::sync::RwLock::new(None),
        connector_statuses: tokio::sync::RwLock::new(None),
        code_index: std::sync::OnceLock::new(),
        stream_config: Default::default(),
        extraction_engine: std::sync::OnceLock::new(),
        auto_approve: false,
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
        system_prompt_cache: parking_lot::RwLock::new(None),
        skill_body_cache: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        telemetry: std::sync::Arc::new(ragent_agent::telemetry::TelemetrySubsystem::disabled()),
        activity_log: std::sync::OnceLock::new(),
        activity_log_tx: tokio::sync::Mutex::new(None),
        skill_registry_cache: parking_lot::Mutex::new(None),
        active_loops: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        active_loop_specs: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        loop_telemetry_recorded: std::sync::atomic::AtomicBool::new(false),
        active_loop_interrupts: parking_lot::RwLock::new(std::collections::HashMap::new()),
        active_loop_captures: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        last_message_end_reason: std::sync::RwLock::new(std::collections::HashMap::new()),
        bg_service: std::sync::OnceLock::new(),
    };
    (Arc::new(processor), storage)
}

fn definition(
    id: &str,
    trigger: AutomationTriggerKind,
    backend: Option<&str>,
) -> AutomationDefinition {
    AutomationDefinition {
        id: id.to_string(),
        name: None,
        agent: None,
        prompt: "run {{payload}}".to_string(),
        trigger,
        backend: backend.map(str::to_string),
        dispatch: Vec::new(),
        enabled: true,
    }
}

// ---------------------------------------------------------------------------
// Backend confinement (FR-033)
// ---------------------------------------------------------------------------

#[test]
fn session_backend_override_is_applied_then_cleared() {
    // FR-033: an automation installs its configured backend for its run's
    // session only.
    let config = Config::default();
    assert_eq!(config.effective_execution_backend().as_str(), "local");

    let descriptor = backend_descriptor(&config, "podman");
    register_session_backend("auto-child-1", descriptor);
    assert!(session_backend_override("auto-child-1").is_some());

    let overridden = apply_session_backend_override(&Arc::new(config.clone()), "auto-child-1");
    assert_eq!(overridden.effective_execution_backend().as_str(), "podman");

    clear_session_backend("auto-child-1");
    assert!(session_backend_override("auto-child-1").is_none());
    // Another session with no override is untouched.
    let untouched = apply_session_backend_override(&Arc::new(config), "other");
    assert_eq!(untouched.effective_execution_backend().as_str(), "local");
}

#[test]
fn backend_descriptor_reuses_a_registered_entry() {
    // FR-033: a label that matches a registered entry reuses its connection.
    let cfg: Config = serde_json::from_str(
        r#"{
            "backends": [
                { "id": "studio", "kind": "remote", "url": "http://127.0.0.1:9100", "api_key": "k" }
            ]
        }"#,
    )
    .expect("parse");
    let descriptor = backend_descriptor(&cfg, "remote");
    assert_eq!(descriptor.id, "studio");
    assert_eq!(descriptor.url.as_deref(), Some("http://127.0.0.1:9100"));
    // An unregistered label synthesises a kind-only descriptor.
    let synthetic = backend_descriptor(&Config::default(), "docker");
    assert_eq!(synthetic.kind, "docker");
    assert!(synthetic.url.is_none());
}

// ---------------------------------------------------------------------------
// Third-party dispatch (FR-013)
// ---------------------------------------------------------------------------

fn finished_run() -> AutomationRun {
    let mut run = AutomationRun::start(
        "run-1".to_string(),
        "on-issue".to_string(),
        AutomationTrigger::Webhook,
        "local".to_string(),
    );
    run.outcome = Some(RunOutcome::Success);
    run.ended_at = Some(run.started_at);
    run.output_ref = Some("log/automation/run-1.md".to_string());
    run
}

#[test]
fn dispatch_payloads_are_provider_shaped() {
    let run = finished_run();

    let slack = payload_for(
        &DispatchTarget {
            kind: "slack".into(),
            url: None,
            token_env: None,
            target: Some("alerts".into()),
        },
        "on-issue",
        &run,
    );
    assert_eq!(slack["channel"], "alerts");
    assert!(slack["text"].as_str().unwrap().contains("on-issue"));
    assert!(slack["text"].as_str().unwrap().contains("success"));

    let github = payload_for(
        &DispatchTarget {
            kind: "github".into(),
            url: None,
            token_env: None,
            target: None,
        },
        "on-issue",
        &run,
    );
    assert!(github["title"].as_str().unwrap().contains("on-issue"));
    assert!(github["body"].as_str().unwrap().contains("run-1"));

    let generic = payload_for(
        &DispatchTarget {
            kind: "webhook".into(),
            url: None,
            token_env: None,
            target: None,
        },
        "on-issue",
        &run,
    );
    assert_eq!(generic["automation_id"], "on-issue");
    assert_eq!(generic["run_id"], "run-1");
    assert_eq!(generic["outcome"], "success");
    assert_eq!(generic["backend"], "local");
}

#[test]
fn resolve_token_reads_only_the_named_env_var() {
    // FR-035: the token is read from the environment at dispatch time.
    let target = DispatchTarget {
        kind: "slack".into(),
        url: None,
        token_env: Some("RAGENT_AUTOMATION_TEST_TOKEN_XYZ".into()),
        target: None,
    };
    assert!(resolve_token(&target).is_none());
    // A target naming no var resolves to nothing.
    let no_env = DispatchTarget {
        kind: "slack".into(),
        url: None,
        token_env: None,
        target: None,
    };
    assert!(resolve_token(&no_env).is_none());
}

// ---------------------------------------------------------------------------
// Enqueue writes a durable run record (FR-013, FR-018)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn enqueue_writes_a_running_run_record() {
    // FR-013, FR-018: a webhook enqueue persists a running record with the
    // automation id, trigger, and selected backend before the agent turn runs.
    let (processor, storage) = test_processor();
    let config = AutomationConfig {
        enabled: true,
        automations: vec![definition(
            "on-issue",
            AutomationTriggerKind::Webhook,
            Some("local"),
        )],
        ..AutomationConfig::default()
    };
    let service = Arc::new(AutomationService::new(
        processor,
        config,
        std::path::PathBuf::from("."),
    ));

    let run = service
        .enqueue("on-issue", AutomationTrigger::Webhook, r#"{"issue":"42"}"#)
        .await
        .expect("enqueue");

    assert_eq!(run.automation_id, "on-issue");
    assert_eq!(run.trigger, AutomationTrigger::Webhook);
    assert_eq!(run.backend, "local");
    assert!(service.is_running("on-issue"));

    let stored = storage
        .get_automation_run(&run.id)
        .expect("read")
        .expect("present");
    assert_eq!(stored.automation_id, "on-issue");
    assert_eq!(stored.trigger, AutomationTrigger::Webhook);
    assert_eq!(stored.backend, "local");
}

#[tokio::test]
async fn enqueue_rejects_an_unknown_automation() {
    let (processor, _storage) = test_processor();
    let service = Arc::new(AutomationService::new(
        processor,
        AutomationConfig::default(),
        std::path::PathBuf::from("."),
    ));
    let result = service
        .enqueue("nope", AutomationTrigger::Webhook, "")
        .await;
    assert!(result.is_err());
}

#[test]
fn disabled_service_reports_itself_disabled() {
    let (processor, _storage) = test_processor();
    let service = AutomationService::new(
        processor,
        AutomationConfig {
            enabled: false,
            ..AutomationConfig::default()
        },
        std::path::PathBuf::from("."),
    );
    assert!(!service.is_enabled());
}
