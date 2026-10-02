//! Integration tests for the interim assistant-save gate (M-008, amended).
//!
//! The interim save must persist completed tool-call parts mid-run so the
//! child session's SQLite row stays in step with the run. The TUI output-view
//! overlay renders that row for a running sub-agent, and its generation key
//! (message count + `edit_seq`, neither of which changes mid-run) relies on
//! the row being updated as steps complete — a tool-only step must therefore
//! trigger an interim save.
//!
//! The mock provider scripts a `think` tool call (which needs no permission
//! prompt) followed by a text-only reply. The test asserts that the tool-call
//! part is already visible in SQLite BEFORE the loop publishes `MessageEnd`
//! (the final save), i.e. it landed via the interim save.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::Result;
use futures::stream;
use ragent_agent::agent::{AgentInfo, ModelRef};
use ragent_agent::event::Event;
use ragent_agent::event::EventBus;
use ragent_agent::llm::{ChatRequest, LlmClient, LlmFinishReason, StreamEvent};
use ragent_agent::provider::{ModelInfo, Provider};
use ragent_agent::session::SessionManager;
use ragent_agent::tool;
use ragent_config::{Capabilities, Config as RagentConfig, Cost};
use ragent_types::message::{Message, MessagePart};

#[derive(Clone, Debug)]
enum ScriptedReply {
    /// A named tool call followed by `Finish { ToolUse }`.
    ToolCallNamed { name: &'static str, args: String },
    /// Text-only response ending with `Finish { Stop }`.
    TextOnly(&'static str),
}

#[derive(Clone)]
struct ScriptedProvider {
    replies: Arc<std::sync::Mutex<Vec<ScriptedReply>>>,
    call_count: Arc<AtomicU32>,
}

struct ScriptedClient {
    replies: Arc<std::sync::Mutex<Vec<ScriptedReply>>>,
    call_count: Arc<AtomicU32>,
}

#[async_trait::async_trait]
impl LlmClient for ScriptedClient {
    async fn chat(
        &self,
        _request: ChatRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        let call = self.call_count.fetch_add(1, Ordering::SeqCst);
        let reply = {
            let mut replies = self.replies.lock().expect("replies lock");
            if replies.is_empty() {
                ScriptedReply::TextOnly("done")
            } else {
                let index = usize::try_from(call).unwrap_or(usize::MAX);
                if index < replies.len() - 1 {
                    replies.remove(0)
                } else {
                    replies
                        .last()
                        .cloned()
                        .unwrap_or(ScriptedReply::TextOnly("done"))
                }
            }
        };
        let events: Vec<StreamEvent> = match reply {
            ScriptedReply::TextOnly(text) => vec![
                StreamEvent::TextDelta {
                    text: text.to_string(),
                },
                StreamEvent::Usage {
                    input_tokens: 10,
                    output_tokens: 5,
                },
                StreamEvent::Finish {
                    reason: LlmFinishReason::Stop,
                },
            ],
            ScriptedReply::ToolCallNamed { name, args } => vec![
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
                reasoning: false,
                streaming: true,
                vision: false,
                tool_use: true,
                thinking_levels: vec![],
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
            call_count: Arc::clone(&self.call_count),
        }))
    }
}

/// Build a processor whose provider replays `replies`, mirroring the
/// `test_loop_telemetry` harness (config injected through the serde
/// representation, no disk config or env overrides).
fn make_processor(
    event_bus: Arc<EventBus>,
    replies: Vec<ScriptedReply>,
) -> Result<(
    ragent_agent::session::processor::SessionProcessor,
    tempfile::TempDir,
)> {
    let mut provider_registry = ragent_agent::provider::ProviderRegistry::new();
    provider_registry.register(Box::new(ScriptedProvider {
        replies: Arc::new(std::sync::Mutex::new(replies)),
        call_count: Arc::new(AtomicU32::new(0)),
    }));

    let storage = Arc::new(ragent_agent::storage::Storage::open_in_memory()?);
    let session_manager = Arc::new(SessionManager::new(storage, event_bus.clone()));
    let config: RagentConfig =
        serde_json::from_str(r#"{"loop":{"max_steps":25}}"#).expect("valid config JSON");

    let processor = ragent_agent::session::processor::SessionProcessor {
        session_manager,
        provider_registry: Arc::new(provider_registry),
        tool_registry: Arc::new(tool::create_default_registry()),
        permission_checker: Arc::new(parking_lot::RwLock::new(
            ragent_agent::permission::PermissionChecker::new(vec![]),
        )),
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
        cached_config: parking_lot::Mutex::new(Some(
            ragent_agent::session::processor::CachedConfig {
                config: Arc::new(config),
                file_mtimes: Vec::new(),
                env_overrides_present: false,
            },
        )),
        team_context_cache: Arc::new(parking_lot::RwLock::new(HashMap::new())),
        tool_repeat_guard: std::sync::Arc::new(parking_lot::Mutex::new(HashMap::new())),
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
    };
    Ok((processor, tempfile::tempdir()?))
}

fn make_agent() -> AgentInfo {
    let mut agent = AgentInfo::new("general", "General");
    agent.model = Some(ModelRef {
        provider_id: "ollama".to_string(),
        model_id: "qwen3:latest".to_string(),
    });
    agent
}

/// The `think` tool needs no permission prompt, so a scripted call executes
/// cleanly against the default registry.
fn think_call() -> ScriptedReply {
    ScriptedReply::ToolCallNamed {
        name: "think",
        args: r#"{"thought": "considering the next step"}"#.to_string(),
    }
}

/// A tool-only step (no text parts) must trigger an interim save that
/// persists the completed tool-call part BEFORE the run ends, so the TUI
/// output-view overlay can render live steps. If the interim gate skipped
/// tool-only appends again, the part would only reach SQLite on the final
/// save (after `MessageEnd` was published) and this test would fail.
#[tokio::test]
async fn test_interim_save_persists_tool_call_parts_mid_run() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let mut rx = event_bus.subscribe();
    let (processor, dir_guard) = make_processor(
        event_bus.clone(),
        vec![think_call(), ScriptedReply::TextOnly("finished")],
    )?;
    let _dir_guard = dir_guard;
    let storage = processor.session_manager.storage().clone();
    let session = processor
        .session_manager
        .create_session(tempfile::tempdir()?.path().to_path_buf())
        .expect("session created");
    let child_session_id = session.id.clone();
    let agent = Arc::new(make_agent());
    let run_agent = Arc::clone(&agent);
    let run_processor = Arc::new(processor);
    let run = tokio::spawn({
        let processor = Arc::clone(&run_processor);
        let session_id = session.id.clone();
        async move {
            processor
                .process_message(
                    &session_id,
                    "go",
                    &run_agent,
                    Arc::new(AtomicBool::new(false)),
                )
                .await
        }
    });

    // Consume bus events until the run completes, checking after each event
    // whether the tool-call part has already been persisted. The check must
    // succeed strictly before `MessageEnd` (the final save).
    let mut persisted_mid_run = false;
    let mut saw_message_end = false;
    // (storage / child_session_id captured before the run task was spawned)
    loop {
        let event = tokio::time::timeout(std::time::Duration::from_secs(30), rx.recv()).await;
        let Ok(Ok(event)) = event else {
            break;
        };
        if matches!(event, Event::MessageEnd { .. }) {
            saw_message_end = true;
            // The final save may have raced ahead of this poll; only the
            // mid-run observation matters for the assertion below.
            let msgs = storage.get_messages(&child_session_id)?;
            persisted_mid_run = msgs.iter().any(|m: &Message| {
                m.role == ragent_types::message::Role::Assistant
                    && m.parts
                        .iter()
                        .any(|p| matches!(p, MessagePart::ToolCall { .. }))
            });
            break;
        }
        // After any pre-`MessageEnd` event (e.g. the tool phase's
        // `ToolCallBatch`), the interim save for that step has either already
        // run or is about to run in the same task; poll briefly.
        if !persisted_mid_run {
            for _ in 0..50 {
                let msgs = storage.get_messages(&child_session_id)?;
                if msgs.iter().any(|m| {
                    m.role == ragent_types::message::Role::Assistant
                        && m.parts
                            .iter()
                            .any(|p| matches!(p, MessagePart::ToolCall { .. }))
                }) {
                    persisted_mid_run = true;
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        }
    }
    run.await.expect("run task")?;

    assert!(saw_message_end, "the scripted run must finish");
    assert!(
        persisted_mid_run,
        "tool-call parts must reach SQLite via the interim save before MessageEnd"
    );
    Ok(())
}

/// The final persisted assistant message carries the tool call with its
/// completed state (status, input, output) after the loop ends.
#[tokio::test]
async fn test_final_message_keeps_completed_tool_call_state() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let mut rx = event_bus.subscribe();
    let (processor, dir_guard) = make_processor(
        event_bus,
        vec![think_call(), ScriptedReply::TextOnly("finished")],
    )?;
    let session = processor
        .session_manager
        .create_session(dir_guard.path().to_path_buf())
        .expect("session created");
    let agent = make_agent();
    processor
        .process_message(&session.id, "go", &agent, Arc::new(AtomicBool::new(false)))
        .await?;

    // Drain the completion events so nothing is missed, then inspect storage.
    while let Ok(_event) = rx.try_recv() {}

    let msgs = processor
        .session_manager
        .storage()
        .get_messages(&session.id)?;
    let tool_calls: Vec<&MessagePart> = msgs
        .iter()
        .filter(|m| m.role == ragent_types::message::Role::Assistant)
        .flat_map(|m| m.parts.iter())
        .filter(|p| matches!(p, MessagePart::ToolCall { .. }))
        .collect();
    assert_eq!(tool_calls.len(), 1, "exactly one persisted tool call");
    let MessagePart::ToolCall { tool, state, .. } = tool_calls[0] else {
        unreachable!("filtered to ToolCall");
    };
    assert_eq!(tool, "think");
    assert!(
        state
            .output
            .as_ref()
            .is_some_and(|v| v.get("thought").is_some()),
        "the think output must be persisted: {:?}",
        state.output
    );
    Ok(())
}
