//! ACP agent turn dispatch (spec `openhands` FR-015, FR-022, FR-036).
//!
//! When the active agent maps onto a registered external ACP agent, the turn is
//! relayed to that agent's subprocess over JSON-RPC on stdio instead of being
//! sent to a local ragent LLM provider (FR-022). This module owns the detection
//! and the bridge from [`crate::acp::AcpUpdate`]s onto the local event bus; the
//! transport itself lives in [`crate::acp`].

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use serde_json::Value;

use ragent_config::{AcpAgentConfig, Config};

use crate::acp::{self, AcpError, AcpUpdate};
use crate::agent::AgentInfo;
use crate::event::{Event, EventBus, FinishReason};
use crate::message::{Message, MessagePart, Role};
use crate::session::processor::SessionProcessor;

/// Resolve the registered ACP agent an [`AgentInfo`] maps onto, if any.
///
/// An agent is bound to an ACP agent when its `options.acp` names one, or when
/// its own name is a registered ACP agent id. Only **enabled** entries are
/// considered. This deliberately does **not** fall back to a configured
/// `default_agent`: a normal agent must never be silently rerouted to an
/// external process, so the binding has to be explicit.
#[must_use]
pub fn acp_agent_for<'a>(config: &'a Config, agent: &AgentInfo) -> Option<&'a AcpAgentConfig> {
    let acp = config.acp.as_ref()?;
    let by_option = agent.options.get("acp").and_then(Value::as_str);
    let id = by_option.unwrap_or(agent.name.as_str());
    acp.agents.get(id).filter(|entry| entry.enabled)
}

/// Translate one streamed ACP update into a local event-bus publication.
fn publish_update(bus: &EventBus, session_id: &str, update: AcpUpdate) {
    match update {
        AcpUpdate::AgentMessageChunk { text } => {
            bus.publish(Event::TextDelta {
                session_id: session_id.to_string(),
                text,
            });
        }
        AcpUpdate::AgentThoughtChunk { text } => {
            bus.publish(Event::ReasoningDelta {
                session_id: session_id.to_string(),
                text,
            });
        }
        AcpUpdate::ToolCall {
            tool_call_id,
            title,
            status,
        } => {
            bus.publish(Event::ToolCallStart {
                session_id: session_id.to_string(),
                call_id: tool_call_id,
                tool: title,
            });
            if status == "completed" || status == "failed" {
                bus.publish(Event::ToolCallEnd {
                    session_id: session_id.to_string(),
                    call_id: String::new(),
                    tool: String::new(),
                    error: (status == "failed").then(|| "tool call failed".to_string()),
                    duration_ms: 0,
                });
            }
        }
        AcpUpdate::ToolCallUpdate {
            tool_call_id,
            title,
            status,
        } => {
            if let Some(status) = status {
                bus.publish(Event::ToolCallEnd {
                    session_id: session_id.to_string(),
                    call_id: tool_call_id,
                    tool: title.unwrap_or_default(),
                    error: (status == "failed").then(|| "tool call failed".to_string()),
                    duration_ms: 0,
                });
            }
        }
        AcpUpdate::Plan { entries } => {
            bus.publish(Event::AgentNotice {
                session_id: session_id.to_string(),
                message: format!("ACP agent proposed a plan with {entries} step(s)"),
            });
        }
        AcpUpdate::Other { kind } => {
            bus.publish(Event::AgentNotice {
                session_id: session_id.to_string(),
                message: format!("ACP agent update: {kind}"),
            });
        }
    }
}

impl SessionProcessor {
    /// Whether the active agent should be driven as an external ACP agent.
    #[must_use]
    pub(crate) fn acp_agent_for_active(&self, agent: &AgentInfo) -> Option<AcpAgentConfig> {
        acp_agent_for(&self.load_config_cached(), agent).cloned()
    }

    /// Relay one turn to an external ACP agent and render its streamed updates.
    ///
    /// The user message has already been persisted and announced by the caller.
    /// This stores an assistant placeholder, relays the prompt, then persists the
    /// final assistant text and publishes `MessageEnd`. A relay failure (non-zero
    /// exit, malformed frame, JSON-RPC error, or timeout) is surfaced as an
    /// `AgentError` and returned as an `Err`, failing the turn with no local
    /// re-execution (FR-036).
    ///
    /// # Errors
    ///
    /// Returns the underlying [`AcpError`] wrapped in an [`anyhow::Error`] when
    /// the relay fails, or a storage error when the turn cannot be persisted.
    pub(crate) async fn process_acp_turn(
        &self,
        session_id: &str,
        acp_agent: AcpAgentConfig,
        user_msg: &Message,
        cancel_flag: Arc<AtomicBool>,
    ) -> anyhow::Result<Message> {
        // 1. Persist an assistant placeholder so the turn is visible/resumable
        //    while the external agent streams.
        let placeholder = Message::new(session_id, Role::Assistant, Vec::new());
        let assistant_msg_id = placeholder.id.clone();
        {
            let stored = placeholder.clone();
            self.storage_op(move |s| s.create_message(&stored)).await?;
        }

        let agent_id = acp_agent.id.clone();
        let display_name = acp_agent.display_name().to_string();
        self.event_bus.publish(Event::AgentNotice {
            session_id: session_id.to_string(),
            message: format!("relaying turn to ACP agent '{display_name}'"),
        });

        // 2. Resolve the session working directory for the subprocess.
        let working_dir = self
            .session_manager
            .get_session(session_id)?
            .map_or_else(|| PathBuf::from("."), |session| session.directory);

        // 3. Relay the turn, rendering each streamed update through the bus.
        let prompt = user_msg.text_content();
        let bus = self.event_bus.clone();
        let sid = session_id.to_string();
        let mut on_update = move |update: AcpUpdate| publish_update(&bus, &sid, update);

        let outcome = acp::relay_turn(
            &acp_agent,
            &working_dir,
            &prompt,
            &cancel_flag,
            &mut on_update,
        )
        .await;

        // 4. Persist the final assistant message (the accumulated agent text).
        let (text, failure): (String, Option<AcpError>) = match outcome {
            Ok(outcome) => (outcome.text, None),
            Err(error) => (String::new(), Some(error)),
        };
        let mut assistant_msg = Message::new(
            session_id,
            Role::Assistant,
            vec![MessagePart::Text { text }],
        );
        assistant_msg.id = assistant_msg_id.clone();
        self.storage_op(move |s| s.update_message(&assistant_msg))
            .await?;

        // 5. Publish the terminal event and, on failure, the error.
        match failure {
            None => {
                self.event_bus.publish(Event::MessageEnd {
                    session_id: session_id.to_string(),
                    message_id: assistant_msg_id,
                    reason: FinishReason::Stop,
                });
                Ok(Message::new(session_id, Role::Assistant, Vec::new()))
            }
            Some(error) => {
                let detail = describe(&error, &agent_id);
                self.event_bus.publish(Event::AgentError {
                    session_id: session_id.to_string(),
                    error: detail,
                });
                self.event_bus.publish(Event::MessageEnd {
                    session_id: session_id.to_string(),
                    message_id: assistant_msg_id,
                    reason: FinishReason::Stop,
                });
                Err(error.into())
            }
        }
    }
}

/// Render an [`AcpError`] with the ACP agent id, for user-facing surfaces.
fn describe(error: &AcpError, agent_id: &str) -> String {
    if error.agent() == agent_id {
        error.to_string()
    } else {
        format!("ACP agent '{agent_id}': {error}")
    }
}
