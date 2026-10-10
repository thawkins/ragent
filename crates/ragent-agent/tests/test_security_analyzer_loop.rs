//! Loop-level integration tests for the LLM security analyzer (spec `openhands`
//! T-015; FR-006, FR-016, FR-017).
//!
//! Drives a real `SessionProcessor` whose main provider is scripted and whose
//! analyzer client (a second provider registration) returns a fixed verdict, so
//! the analyzer layer is exercised end to end:
//!
//! - when the analyzer returns `allow` and no policy rule matches, the action
//!   runs and a `SecurityVerdict { verdict: "allow" }` is published *before* the
//!   tool result (FR-006, FR-017);
//! - a `deny` verdict blocks the action, the file survives, and the denial is
//!   reported to the model as a tool result (FR-006);
//! - a `deny` never overrides the analyzer's fail-safe on a broken analyzer, and
//!   the normal interactive flow still runs;
//! - the analyzer's `allow` never overrides an explicit policy `Deny`.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::Result;
use futures::stream;
use ragent_agent::agent::{AgentInfo, ModelRef};
use ragent_agent::event::{Event, EventBus};
use ragent_agent::llm::{ChatRequest, LlmClient, LlmFinishReason, StreamEvent};
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::{ModelInfo, Provider, ProviderRegistry};
use ragent_agent::session::SessionManager;
use ragent_agent::session::processor::{CachedConfig, SessionProcessor};
use ragent_agent::storage::Storage;
use ragent_agent::tool;
use ragent_config::permission::{Permission, PermissionAction, PermissionRule};
use ragent_config::{Capabilities, Config as RagentConfig, Cost};

/// A scripted main-loop reply.
#[derive(Clone)]
enum Reply {
    /// A named tool call followed by `Finish { ToolUse }`.
    ToolCall { name: &'static str, args: String },
    /// Text-only response ending with `Finish { Stop }`.
    TextOnly(&'static str),
}

struct MainClient {
    replies: Arc<Mutex<Vec<Reply>>>,
    calls: Arc<AtomicU32>,
}

#[async_trait::async_trait]
impl LlmClient for MainClient {
    async fn chat(
        &self,
        _request: ChatRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        let idx = self.calls.fetch_add(1, Ordering::SeqCst) as usize;
        let reply = self
            .replies
            .lock()
            .expect("replies lock")
            .get(idx)
            .cloned()
            .unwrap_or(Reply::TextOnly("done"));
        let events = match reply {
            Reply::TextOnly(text) => vec![
                StreamEvent::TextDelta {
                    text: text.to_string(),
                },
                StreamEvent::Finish {
                    reason: LlmFinishReason::Stop,
                },
            ],
            Reply::ToolCall { name, args } => vec![
                StreamEvent::ToolCallStart {
                    id: "call_1".to_string(),
                    name: name.to_string(),
                },
                StreamEvent::ToolCallDelta {
                    id: "call_1".to_string(),
                    args_json: args,
                },
                StreamEvent::ToolCallEnd {
                    id: "call_1".to_string(),
                },
                StreamEvent::Usage {
                    input_tokens: 10,
                    output_tokens: 5,
                },
                StreamEvent::Finish {
                    reason: LlmFinishReason::ToolUse,
                },
            ],
        };
        Ok(Box::pin(stream::iter(events)))
    }
}

struct MainProvider {
    replies: Arc<Mutex<Vec<Reply>>>,
    calls: Arc<AtomicU32>,
}

#[async_trait::async_trait]
impl Provider for MainProvider {
    fn id(&self) -> &'static str {
        "ollama"
    }
    fn name(&self) -> &'static str {
        "Scripted Main"
    }
    fn default_models(&self) -> Vec<ModelInfo> {
        vec![model_info()]
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
        Ok(Box::new(MainClient {
            replies: Arc::clone(&self.replies),
            calls: Arc::clone(&self.calls),
        }))
    }
}

/// A provider whose client always returns a fixed analyzer verdict.
struct AnalyzerClient {
    raw: String,
}

#[async_trait::async_trait]
impl LlmClient for AnalyzerClient {
    async fn chat(
        &self,
        _request: ChatRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        Ok(Box::pin(stream::iter(vec![
            StreamEvent::TextDelta {
                text: self.raw.clone(),
            },
            StreamEvent::Finish {
                reason: LlmFinishReason::Stop,
            },
        ])))
    }
}

struct AnalyzerProvider {
    raw: String,
}

#[async_trait::async_trait]
impl Provider for AnalyzerProvider {
    fn id(&self) -> &'static str {
        "securitymock"
    }
    fn name(&self) -> &'static str {
        "Scripted Analyzer"
    }
    fn default_models(&self) -> Vec<ModelInfo> {
        let mut m = model_info();
        m.provider_id = "securitymock".to_string();
        m.id = "analyzer".to_string();
        vec![m]
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
        Ok(Box::new(AnalyzerClient {
            raw: self.raw.clone(),
        }))
    }
}

fn model_info() -> ModelInfo {
    ModelInfo {
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
    }
}

type Harness = (Arc<SessionProcessor>, std::path::PathBuf, tempfile::TempDir);

/// Build a processor with a scripted main provider, a scripted analyzer
/// provider, and the given permission rules / analyzer config JSON.
fn make_processor(
    event_bus: Arc<EventBus>,
    replies: Vec<Reply>,
    analyzer_raw: &str,
    rules: Vec<PermissionRule>,
    security_json: &str,
) -> Result<Harness> {
    let mut registry = ProviderRegistry::new();
    registry.register(Box::new(MainProvider {
        replies: Arc::new(Mutex::new(replies)),
        calls: Arc::new(AtomicU32::new(0)),
    }));
    registry.register(Box::new(AnalyzerProvider {
        raw: analyzer_raw.to_string(),
    }));

    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let session_manager = Arc::new(SessionManager::new(storage, event_bus.clone()));
    let checker = PermissionChecker::new(rules);
    let config: RagentConfig = serde_json::from_str(&format!(
        r#"{{"loop":{{"max_steps":10,"checkpoint_timeout_secs":100}},"security_analyzer":{security_json}}}"#
    ))
    .expect("valid config JSON");

    let processor = Arc::new(SessionProcessor {
        session_manager,
        provider_registry: Arc::new(registry),
        tool_registry: Arc::new(tool::create_default_registry()),
        permission_checker: Arc::new(parking_lot::RwLock::new(checker)),
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
        team_context_cache: Arc::new(parking_lot::RwLock::new(HashMap::new())),
        tool_repeat_guard: Arc::new(parking_lot::Mutex::new(HashMap::new())),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: ragent_agent::StreamConfig::default(),
        auto_approve: false,
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
    });
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().to_path_buf();
    Ok((processor, path, tmp))
}

fn make_agent() -> AgentInfo {
    let mut agent = AgentInfo::new("general", "General agent");
    agent.model = Some(ModelRef {
        provider_id: "ollama".to_string(),
        model_id: "qwen3:latest".to_string(),
    });
    agent
}

fn write_call() -> Reply {
    Reply::ToolCall {
        name: "write",
        args: r#"{"path": "out.txt", "content": "hello"}"#.to_string(),
    }
}

fn drain_all(rx: &mut tokio::sync::broadcast::Receiver<Event>) -> Vec<Event> {
    let mut found = Vec::new();
    while let Ok(event) = rx.try_recv() {
        found.push(event);
    }
    found
}

/// FR-006, FR-017: an `allow` verdict lets a rule-less action run and is
/// published before the tool result.
#[tokio::test]
async fn test_analyzer_allow_permits_action_and_publishes_verdict() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, _dir) = make_processor(
        event_bus.clone(),
        vec![write_call(), Reply::TextOnly("done")],
        r#"{"verdict":"allow","rationale":"safe write"}"#,
        // No rule for `write`, so the static checker returns `Ask`.
        Vec::new(),
        // Pin the analyzer to the scripted analyzer provider; without the model
        // override it would resolve to the session model and never see `analyzer_raw`.
        r#"{"enabled":true,"model":{"provider_id":"securitymock","model_id":"analyzer"}}"#,
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session");

    processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(Default::default()),
        )
        .await
        .expect("turn completes");

    let events = drain_all(&mut rx);

    // The verdict was published with the analyzer's rationale (FR-017).
    let verdicts: Vec<&Event> = events
        .iter()
        .filter(|e| matches!(e, Event::SecurityVerdict { .. }))
        .collect();
    assert_eq!(verdicts.len(), 1, "exactly one verdict for the write call");
    if let Event::SecurityVerdict {
        verdict, rationale, ..
    } = verdicts[0]
    {
        assert_eq!(verdict, "allow");
        assert_eq!(rationale, "safe write");
    }

    // The verdict precedes the tool result (FR-017: published before the
    // action is permitted).
    let verdict_pos = events
        .iter()
        .position(|e| matches!(e, Event::SecurityVerdict { .. }))
        .expect("verdict present");
    let result_pos = events
        .iter()
        .position(|e| matches!(e, Event::ToolCallEnd { .. }))
        .expect("tool end present");
    assert!(
        verdict_pos < result_pos,
        "the verdict is published before the tool result"
    );

    // The action ran: the analyzer's `allow` satisfied the bare `Ask`.
    assert!(
        working_dir.join("out.txt").exists(),
        "the analyzer's allow let the write execute"
    );
    Ok(())
}

/// FR-006: a `deny` verdict blocks the action and reports the rationale to the
/// model as the tool result.
#[tokio::test]
async fn test_analyzer_deny_blocks_action_and_reports_rationale() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, _dir) = make_processor(
        event_bus.clone(),
        vec![write_call(), Reply::TextOnly("done")],
        r#"{"verdict":"deny","rationale":"writes outside the sandbox"}"#,
        Vec::new(),
        r#"{"enabled":true,"model":{"provider_id":"securitymock","model_id":"analyzer"}}"#,
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session");

    processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(Default::default()),
        )
        .await
        .expect("turn completes");

    let events = drain_all(&mut rx);

    // The file was never written.
    assert!(
        !working_dir.join("out.txt").exists(),
        "the analyzer's deny blocked the write"
    );

    // The tool result carries the analyzer's denial + rationale.
    let err = events
        .iter()
        .find_map(|e| match e {
            Event::ToolCallEnd {
                error: Some(err), ..
            } => Some(err.clone()),
            _ => None,
        })
        .expect("a tool-end error");
    assert!(
        err.contains("denied by security analyzer"),
        "the denial is reported to the model: {err}"
    );
    assert!(
        err.contains("writes outside the sandbox"),
        "the analyzer rationale is the observation: {err}"
    );
    Ok(())
}

/// FR-006: an analyzer `allow` never overrides an explicit policy `Deny`.
#[tokio::test]
async fn test_analyzer_allow_never_overrides_explicit_deny() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, _dir) = make_processor(
        event_bus.clone(),
        vec![write_call(), Reply::TextOnly("done")],
        r#"{"verdict":"allow","rationale":"looks fine"}"#,
        vec![PermissionRule {
            permission: Permission::Custom("file:write".to_string()),
            pattern: Some("**".to_string()),
            action: PermissionAction::Deny,
        }],
        r#"{"enabled":true,"model":{"provider_id":"securitymock","model_id":"analyzer"}}"#,
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session");

    processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(Default::default()),
        )
        .await
        .expect("turn completes");

    assert!(
        !working_dir.join("out.txt").exists(),
        "an explicit policy deny is never overridden by an analyzer allow"
    );
    Ok(())
}
