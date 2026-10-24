//! End-to-end coverage for the inbound alias resolution in the dispatch
//! pipeline (spec `uniformtools` FR-015).
//!
//! The unit tests in `test_tool_alias_table.rs` prove the table and the
//! resolver in isolation. These cases prove the pipeline actually *applies* the
//! resolver - that a model naming a provider spelling reaches the registered
//! tool, that history records the canonical name, and that switching aliases off
//! restores the unknown-tool refusal with its did-you-mean hint.
//!
//! Every case uses an explicit `tools.aliases` entry onto the offline `think`
//! tool rather than a built-in row, because the built-in targets (`webfetch`,
//! `websearch`) perform network I/O and these tests must not touch a network.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

use anyhow::Result;
use futures::stream;
use ragent_agent::agent::{AgentInfo, ModelRef};
use ragent_agent::event::EventBus;
use ragent_agent::llm::{ChatRequest, LlmClient, LlmFinishReason, StreamEvent};
use ragent_agent::message::{Message, MessagePart, ToolCallStatus};
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::{ModelInfo, Provider, ProviderRegistry};
use ragent_agent::session::SessionManager;
use ragent_agent::session::processor::{CachedConfig, SessionProcessor};
use ragent_agent::storage::Storage;
use ragent_agent::tool;
use ragent_config::permission::PermissionRule;
use ragent_config::{Capabilities, Config as RagentConfig, Cost, StreamConfig};

const PROVIDER_ID: &str = "ollama";
const MODEL_ID: &str = "qwen3:latest";

// ---------------------------------------------------------------------------
// Scripted provider
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum ScriptedReply {
    /// One named tool call followed by `Finish { ToolUse }`.
    ToolCall { name: String, args: String },
    /// Text-only response ending with `Finish { Stop }`.
    Text(String),
}

#[derive(Clone)]
struct ScriptedProvider {
    replies: Arc<Mutex<Vec<ScriptedReply>>>,
    captured: Arc<Mutex<Vec<ChatRequest>>>,
}

struct ScriptedClient {
    replies: Arc<Mutex<Vec<ScriptedReply>>>,
    captured: Arc<Mutex<Vec<ChatRequest>>>,
}

#[async_trait::async_trait]
impl LlmClient for ScriptedClient {
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        self.captured
            .lock()
            .expect("captured requests lock")
            .push(request);
        let reply = {
            let mut replies = self.replies.lock().expect("replies lock");
            if replies.len() > 1 {
                replies.remove(0)
            } else {
                replies
                    .last()
                    .cloned()
                    .unwrap_or(ScriptedReply::Text("done".to_string()))
            }
        };
        let events: Vec<StreamEvent> = match reply {
            ScriptedReply::ToolCall { name, args } => vec![
                StreamEvent::ToolCallStart {
                    id: "call_1".to_string(),
                    name,
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
            ScriptedReply::Text(text) => vec![
                StreamEvent::TextDelta { text },
                StreamEvent::Usage {
                    input_tokens: 10,
                    output_tokens: 5,
                },
                StreamEvent::Finish {
                    reason: LlmFinishReason::Stop,
                },
            ],
        };
        Ok(Box::pin(stream::iter(events)))
    }
}

#[async_trait::async_trait]
impl Provider for ScriptedProvider {
    fn id(&self) -> &'static str {
        PROVIDER_ID
    }

    fn name(&self) -> &'static str {
        "Scripted Mock Ollama"
    }

    fn default_models(&self) -> Vec<ModelInfo> {
        vec![ModelInfo {
            id: MODEL_ID.to_string(),
            provider_id: PROVIDER_ID.to_string(),
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
            replies: Arc::clone(&self.replies),
            captured: Arc::clone(&self.captured),
        }))
    }
}

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

struct Harness {
    processor: SessionProcessor,
    working_dir: std::path::PathBuf,
    captured: Arc<Mutex<Vec<ChatRequest>>>,
    _dir: tempfile::TempDir,
}

/// Builds a processor with `tools` applied to the per-turn config, so
/// `tools.aliases` reaches the dispatch pipeline exactly as a user's
/// `ragent.json` would.
fn make_processor(
    event_bus: Arc<EventBus>,
    replies: Vec<ScriptedReply>,
    tools: &str,
) -> Result<Harness> {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let mut provider_registry = ProviderRegistry::new();
    provider_registry.register(Box::new(ScriptedProvider {
        replies: Arc::new(Mutex::new(replies)),
        captured: Arc::clone(&captured),
    }));

    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let session_manager = Arc::new(SessionManager::new(storage, event_bus.clone()));
    // `think` declares permission category `none`; an empty ruleset still asks,
    // so grant it here rather than driving the interactive prompt.
    let checker = PermissionChecker::new(vec![PermissionRule {
        permission: ragent_config::permission::Permission::Custom("none".to_string()),
        pattern: Some("**".to_string()),
        action: ragent_config::permission::PermissionAction::Allow,
    }]);
    let config: RagentConfig =
        serde_json::from_str(&format!(r#"{{"tools": {tools}}}"#)).expect("valid config JSON");

    let processor = SessionProcessor {
        session_manager,
        provider_registry: Arc::new(provider_registry),
        tool_registry: Arc::new(tool::create_default_registry()),
        permission_checker: Arc::new(parking_lot::RwLock::new(checker)),
        event_bus,
        agent_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        team_context_cache: Arc::new(parking_lot::RwLock::new(HashMap::new())),
        tool_repeat_guard: Arc::new(parking_lot::Mutex::new(HashMap::new())),
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
        stream_config: StreamConfig {
            max_retries: 0,
            retry_backoff_secs: 0,
            ..Default::default()
        },
        auto_approve: true,
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
    };
    let tmp = tempfile::tempdir().expect("tempdir");
    Ok(Harness {
        processor,
        working_dir: tmp.path().to_path_buf(),
        captured,
        _dir: tmp,
    })
}

fn make_agent() -> AgentInfo {
    let mut agent = AgentInfo::new("general", "General");
    agent.model = Some(ModelRef {
        provider_id: PROVIDER_ID.to_string(),
        model_id: MODEL_ID.to_string(),
    });
    agent
}

/// Returns the first recorded `ToolCall` part, whatever name it carries - used
/// by the refusals, where the point is which spelling the record holds.
fn first_tool_call_part(reply: &Message) -> Option<&MessagePart> {
    reply
        .parts
        .iter()
        .find(|part| matches!(part, MessagePart::ToolCall { .. }))
}

/// Returns the recorded `ToolCall` part for `tool`, if the turn produced one.
fn tool_call_part<'a>(reply: &'a Message, tool: &str) -> Option<&'a MessagePart> {
    reply.parts.iter().find(|part| match part {
        MessagePart::ToolCall { tool: name, .. } => name == tool,
        _ => false,
    })
}

// ---------------------------------------------------------------------------
// A provider spelling reaches the registered tool (FR-015, on)
// ---------------------------------------------------------------------------

/// A call naming an alias spelling executes the registered tool, and history
/// records the name the registry holds rather than the spelling the model sent.
#[tokio::test]
async fn test_alias_spelling_dispatches_the_registered_tool() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let harness = make_processor(
        event_bus,
        vec![
            ScriptedReply::ToolCall {
                name: "Frobnicate".to_string(),
                args: r#"{"thought": "resolved through the table"}"#.to_string(),
            },
            ScriptedReply::Text("done".to_string()),
        ],
        r#"{"aliases": {"Frobnicate": "think"}}"#,
    )?;
    let session = harness
        .processor
        .session_manager
        .create_session(harness.working_dir.clone())
        .expect("session created");

    let reply = harness
        .processor
        .process_message(
            &session.id,
            "note it",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("an aliased call must not fail the turn");

    assert_eq!(reply.text_content(), "done");
    assert!(
        tool_call_part(&reply, "think").is_some(),
        "the canonical name is what history records"
    );
    assert!(
        tool_call_part(&reply, "Frobnicate").is_none(),
        "the provider spelling must not leak into the persisted record (FR-001)"
    );
    let Some(MessagePart::ToolCall { state, .. }) = tool_call_part(&reply, "think") else {
        panic!("the resolved call part is missing");
    };
    assert_eq!(
        state.status,
        ToolCallStatus::Completed,
        "the resolved tool ran instead of failing as unknown"
    );
    assert_eq!(
        harness.captured.lock().expect("captured lock").len(),
        2,
        "one step to call, one step for the final answer"
    );
    Ok(())
}

/// The same call with alias resolution off is refused as an unknown tool, and
/// the refusal names the spelling the model actually sent.
#[tokio::test]
async fn test_unknown_spelling_is_refused_when_aliases_are_off() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let harness = make_processor(
        event_bus,
        vec![
            ScriptedReply::ToolCall {
                name: "Frobnicate".to_string(),
                args: r#"{"thought": "never runs"}"#.to_string(),
            },
            ScriptedReply::Text("recovered".to_string()),
        ],
        r#"{"aliases": false}"#,
    )?;
    let session = harness
        .processor
        .session_manager
        .create_session(harness.working_dir.clone())
        .expect("session created");

    let reply = harness
        .processor
        .process_message(
            &session.id,
            "note it",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("an unknown tool name must not fail the turn");

    assert_eq!(reply.text_content(), "recovered");
    let Some(MessagePart::ToolCall { tool, state, .. }) = first_tool_call_part(&reply) else {
        panic!("the failed call is still recorded so the model can correct course");
    };
    assert_eq!(tool, "Frobnicate");
    assert_eq!(state.status, ToolCallStatus::Error);
    let message = state.error.as_deref().unwrap_or_default();
    assert!(
        message.contains("Unknown tool: 'Frobnicate'"),
        "the refusal names the spelling the client sent: {message}"
    );
    assert!(
        tool_call_part(&reply, "think").is_none(),
        "the tool must not run when resolution is off"
    );
    Ok(())
}

/// An alias whose target this build does not register falls through to the
/// unknown-tool error rather than resolving onto a name that cannot dispatch.
#[tokio::test]
async fn test_alias_to_an_unregistered_target_still_refuses() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let harness = make_processor(
        event_bus,
        vec![
            ScriptedReply::ToolCall {
                name: "Frobnicate".to_string(),
                args: r#"{"thought": "never runs"}"#.to_string(),
            },
            ScriptedReply::Text("recovered".to_string()),
        ],
        r#"{"aliases": {"Frobnicate": "frobnicate"}}"#,
    )?;
    let session = harness
        .processor
        .session_manager
        .create_session(harness.working_dir.clone())
        .expect("session created");

    let reply = harness
        .processor
        .process_message(
            &session.id,
            "note it",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("turn does not fail");

    let Some(MessagePart::ToolCall { tool, state, .. }) = first_tool_call_part(&reply) else {
        panic!("the failed call is recorded");
    };
    assert_eq!(tool, "Frobnicate");
    assert_eq!(state.status, ToolCallStatus::Error);
    Ok(())
}

/// A registered name is never re-pointed by the table: a `think` call still runs
/// `think` even when an entry maps `think` elsewhere, and no second call appears.
#[tokio::test]
async fn test_registered_name_wins_over_a_table_entry() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let harness = make_processor(
        event_bus,
        vec![
            ScriptedReply::ToolCall {
                name: "think".to_string(),
                args: r#"{"thought": "direct"}"#.to_string(),
            },
            ScriptedReply::Text("done".to_string()),
        ],
        r#"{"aliases": {"think": "calculator"}}"#,
    )?;
    let session = harness
        .processor
        .session_manager
        .create_session(harness.working_dir.clone())
        .expect("session created");

    let reply = harness
        .processor
        .process_message(
            &session.id,
            "note it",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("turn succeeds");

    assert!(
        tool_call_part(&reply, "think").is_some(),
        "a registered name resolves by lookup, never through the table"
    );
    assert!(
        tool_call_part(&reply, "calculator").is_none(),
        "the entry must not re-point a tool that is already registered"
    );
    Ok(())
}
