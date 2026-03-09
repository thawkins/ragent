//! Message processing pipeline for agent sessions.
//!
//! [`SessionProcessor`] orchestrates the agentic loop: it accepts a user message,
//! streams an LLM response, executes any requested tool calls, and iterates
//! until the model signals completion or the step limit is reached.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Result, bail};
use futures::StreamExt;
use serde_json::{Value, json};
use tracing::{debug, warn};

use crate::agent::{AgentInfo, build_system_prompt};
use crate::event::{Event, EventBus, FinishReason};
use crate::llm::{ChatContent, ChatMessage, ChatRequest, ContentPart, StreamEvent};
use crate::message::{Message, MessagePart, Role, ToolCallState, ToolCallStatus};
use crate::permission::PermissionChecker;
use crate::provider::ProviderRegistry;
use crate::sanitize::redact_secrets;
use crate::session::SessionManager;
use crate::tool::{ToolContext, ToolRegistry};

/// Drives the agentic conversation loop for a single session.
///
/// Holds shared references to the session manager, LLM provider registry,
/// tool registry, permission checker, and event bus.
pub struct SessionProcessor {
    pub session_manager: Arc<SessionManager>,
    pub provider_registry: Arc<ProviderRegistry>,
    pub tool_registry: Arc<ToolRegistry>,
    pub permission_checker: Arc<tokio::sync::RwLock<PermissionChecker>>,
    pub event_bus: Arc<EventBus>,
}

impl SessionProcessor {
    /// Processes a user message within an agent session.
    ///
    /// Persists the user message, then enters an agentic loop that streams
    /// LLM responses, executes tool calls, and feeds results back to the model
    /// until completion or the agent's max-step limit is reached.
    ///
    /// # Errors
    ///
    /// Returns an error if the configured model or provider is missing, if the
    /// API key cannot be resolved, or if an LLM call fails.
    pub async fn process_message(
        &self,
        session_id: &str,
        user_text: &str,
        agent: &AgentInfo,
    ) -> Result<Message> {
        // 1. Store user message
        let user_msg = Message::user_text(session_id, user_text);
        self.session_manager.storage().create_message(&user_msg)?;

        self.event_bus.publish(Event::MessageStart {
            session_id: session_id.to_string(),
            message_id: user_msg.id.clone(),
        });

        // Helper: publish error + message-end so the TUI always resets.
        let publish_error = |bus: &EventBus, sid: &str, msg_id: &str, err: &str| {
            bus.publish(Event::AgentError {
                session_id: sid.to_string(),
                error: err.to_string(),
            });
            bus.publish(Event::MessageEnd {
                session_id: sid.to_string(),
                message_id: msg_id.to_string(),
                reason: FinishReason::Stop,
            });
        };

        // 2. Resolve model and create LLM client
        let model_ref = match agent.model.as_ref() {
            Some(m) => m,
            None => {
                let err = format!("Agent '{}' has no model configured", agent.name);
                publish_error(&self.event_bus, session_id, &user_msg.id, &err);
                bail!("{}", err);
            }
        };

        let provider = match self.provider_registry.get(&model_ref.provider_id) {
            Some(p) => p,
            None => {
                let err = format!("Provider '{}' not found", model_ref.provider_id);
                publish_error(&self.event_bus, session_id, &user_msg.id, &err);
                bail!("{}", err);
            }
        };

        // Try to get API key from environment or storage
        let api_key = match self.resolve_api_key(&model_ref.provider_id) {
            Ok(k) => k,
            Err(e) => {
                let err = e.to_string();
                publish_error(&self.event_bus, session_id, &user_msg.id, &err);
                return Err(e);
            }
        };

        // For Copilot, pass the stored plan-specific API base URL
        let base_url = if model_ref.provider_id == "copilot" {
            self.session_manager
                .storage()
                .get_setting("copilot_api_base")
                .ok()
                .flatten()
        } else {
            None
        };

        tracing::info!(
            provider = %model_ref.provider_id,
            model = %model_ref.model_id,
            api_base = ?base_url,
            "creating LLM client"
        );

        let client = match provider
            .create_client(&api_key, base_url.as_deref(), &HashMap::new())
            .await
        {
            Ok(c) => c,
            Err(e) => {
                let err = e.to_string();
                publish_error(&self.event_bus, session_id, &user_msg.id, &err);
                return Err(e);
            }
        };

        // 3. Build system prompt
        let working_dir = self
            .session_manager
            .get_session(session_id)?
            .map(|s| s.directory.clone())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

        let file_tree = build_file_tree(&working_dir, 2);
        let system_prompt = build_system_prompt(agent, &working_dir, &file_tree);

        // 4. Build chat messages from history
        let history = self.session_manager.get_messages(session_id)?;
        let mut chat_messages = history_to_chat_messages(&history);

        // 5. Agent loop
        let max_steps = agent.max_steps.unwrap_or(50) as usize;
        // Single-step agents (e.g. "chat") don't use tools — omit definitions
        // so providers aren't confused by unused tool schemas.
        let tool_definitions = if max_steps <= 1 {
            Vec::new()
        } else {
            self.tool_registry.definitions()
        };
        let mut assistant_parts: Vec<MessagePart> = Vec::new();
        let mut step = 0;

        loop {
            step += 1;
            if step > max_steps {
                warn!("Reached max steps ({}), stopping agent loop", max_steps);
                self.event_bus.publish(Event::AgentError {
                    session_id: session_id.to_string(),
                    error: format!("Reached maximum steps ({})", max_steps),
                });
                break;
            }

            debug!("Agent loop step {}/{}", step, max_steps);

            let request = ChatRequest {
                model: model_ref.model_id.clone(),
                messages: chat_messages.clone(),
                tools: tool_definitions.clone(),
                temperature: agent.temperature,
                top_p: agent.top_p,
                max_tokens: None,
                system: Some(system_prompt.clone()),
                options: agent.options.clone(),
            };

            // Log which tools are being sent with this request
            if !tool_definitions.is_empty() {
                let tool_names: Vec<String> =
                    tool_definitions.iter().map(|t| t.name.clone()).collect();
                self.event_bus.publish(Event::ToolsSent {
                    session_id: session_id.to_string(),
                    tools: tool_names,
                });
            }

            // Call LLM
            let mut stream = match client.chat(request).await {
                Ok(s) => s,
                Err(e) => {
                    // Full details logged at debug level; the AgentError event
                    // carries the message to the TUI log panel.
                    debug!("LLM call failed: {}", redact_secrets(&e.to_string()));
                    self.event_bus.publish(Event::AgentError {
                        session_id: session_id.to_string(),
                        error: e.to_string(),
                    });
                    bail!("LLM call failed: {}", e);
                }
            };

            // Process stream events
            let mut text_buffer = String::new();
            let mut reasoning_buffer = String::new();
            let mut tool_calls: Vec<PendingToolCall> = Vec::new();
            let mut finish_reason = FinishReason::Stop;

            while let Some(event) = stream.next().await {
                match event {
                    StreamEvent::TextDelta { text } => {
                        self.event_bus.publish(Event::TextDelta {
                            session_id: session_id.to_string(),
                            text: text.clone(),
                        });
                        text_buffer.push_str(&text);
                    }
                    StreamEvent::ReasoningStart => {}
                    StreamEvent::ReasoningDelta { text } => {
                        self.event_bus.publish(Event::ReasoningDelta {
                            session_id: session_id.to_string(),
                            text: text.clone(),
                        });
                        reasoning_buffer.push_str(&text);
                    }
                    StreamEvent::ReasoningEnd => {}
                    StreamEvent::ToolCallStart { id, name } => {
                        self.event_bus.publish(Event::ToolCallStart {
                            session_id: session_id.to_string(),
                            call_id: id.clone(),
                            tool: name.clone(),
                        });
                        tool_calls.push(PendingToolCall {
                            id,
                            name,
                            args_json: String::new(),
                        });
                    }
                    StreamEvent::ToolCallDelta { id, args_json } => {
                        if let Some(tc) = tool_calls.iter_mut().find(|t| t.id == id) {
                            tc.args_json.push_str(&args_json);
                        }
                    }
                    StreamEvent::ToolCallEnd { id: _ } => {
                        // Will be processed after stream ends
                    }
                    StreamEvent::Usage {
                        input_tokens,
                        output_tokens,
                    } => {
                        self.event_bus.publish(Event::TokenUsage {
                            session_id: session_id.to_string(),
                            input_tokens,
                            output_tokens,
                        });
                    }
                    StreamEvent::Error { message } => {
                        debug!("Stream error: {}", redact_secrets(&message));
                        self.event_bus.publish(Event::AgentError {
                            session_id: session_id.to_string(),
                            error: message,
                        });
                    }
                    StreamEvent::Finish { reason } => {
                        finish_reason = reason;
                    }
                }
            }

            // Collect parts from this turn
            if !reasoning_buffer.is_empty() {
                assistant_parts.push(MessagePart::Reasoning {
                    text: reasoning_buffer.clone(),
                });
            }
            if !text_buffer.is_empty() {
                // Log the model response text
                let response_preview = if text_buffer.len() > 200 {
                    format!("{}…", &text_buffer[..200])
                } else {
                    text_buffer.clone()
                };
                self.event_bus.publish(Event::ModelResponse {
                    session_id: session_id.to_string(),
                    text: response_preview,
                });
                assistant_parts.push(MessagePart::Text {
                    text: text_buffer.clone(),
                });
            }

            // If no tool calls, we're done
            if tool_calls.is_empty() || finish_reason != FinishReason::ToolUse {
                break;
            }

            // Build assistant message content for history
            let mut assistant_content_parts: Vec<ContentPart> = Vec::new();
            if !text_buffer.is_empty() {
                assistant_content_parts.push(ContentPart::Text {
                    text: text_buffer.clone(),
                });
            }

            // Execute tool calls
            let mut tool_result_parts: Vec<ContentPart> = Vec::new();
            for tc in &tool_calls {
                let input: Value = serde_json::from_str(&tc.args_json).unwrap_or_else(|e| {
                    warn!(error = %e, args = %tc.args_json, "Failed to parse tool call arguments");
                    json!({})
                });

                // Log tool call with arguments
                let args_preview = if tc.args_json.len() > 200 {
                    format!("{}…", &tc.args_json[..200])
                } else {
                    tc.args_json.clone()
                };
                self.event_bus.publish(Event::ToolCallArgs {
                    session_id: session_id.to_string(),
                    call_id: tc.id.clone(),
                    tool: tc.name.clone(),
                    args: args_preview,
                });

                assistant_content_parts.push(ContentPart::ToolUse {
                    id: tc.id.clone(),
                    name: tc.name.clone(),
                    input: input.clone(),
                });

                let start = Instant::now();
                let tool_ctx = ToolContext {
                    session_id: session_id.to_string(),
                    working_dir: working_dir.clone(),
                    event_bus: self.event_bus.clone(),
                };

                let result = self
                    .tool_registry
                    .get(&tc.name)
                    .ok_or_else(|| anyhow::anyhow!("Unknown tool: {}", tc.name));
                let result = match result {
                    Ok(tool) => tool.execute(input.clone(), &tool_ctx).await,
                    Err(e) => Err(e),
                };
                let duration_ms = start.elapsed().as_millis() as u64;

                let (output_value, error) = match &result {
                    Ok(output) => (Some(json!(output.content)), None),
                    Err(e) => (None, Some(e.to_string())),
                };

                let status = if result.is_ok() {
                    ToolCallStatus::Completed
                } else {
                    ToolCallStatus::Error
                };
                let success = status == ToolCallStatus::Completed;

                self.event_bus.publish(Event::ToolCallEnd {
                    session_id: session_id.to_string(),
                    call_id: tc.id.clone(),
                    tool: tc.name.clone(),
                    error: error.clone(),
                    duration_ms,
                });

                assistant_parts.push(MessagePart::ToolCall {
                    tool: tc.name.clone(),
                    call_id: tc.id.clone(),
                    state: ToolCallState {
                        status,
                        input,
                        output: output_value,
                        error,
                        duration_ms: Some(duration_ms),
                    },
                });

                let result_content = match result {
                    Ok(output) => output.content,
                    Err(e) => format!("Error: {}", e),
                };

                // Log the tool result
                let result_preview = if result_content.len() > 200 {
                    format!("{}…", &result_content[..200])
                } else {
                    result_content.clone()
                };
                self.event_bus.publish(Event::ToolResult {
                    session_id: session_id.to_string(),
                    call_id: tc.id.clone(),
                    tool: tc.name.clone(),
                    content: result_preview,
                    success,
                });

                tool_result_parts.push(ContentPart::ToolResult {
                    tool_use_id: tc.id.clone(),
                    content: result_content,
                });
            }

            // Add assistant message with tool uses to chat history
            chat_messages.push(ChatMessage {
                role: "assistant".to_string(),
                content: ChatContent::Parts(assistant_content_parts),
            });

            // Add tool results to chat history
            chat_messages.push(ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Parts(tool_result_parts),
            });
        }

        // 6. Store assistant message
        let assistant_msg = Message::new(session_id, Role::Assistant, assistant_parts);
        self.session_manager
            .storage()
            .create_message(&assistant_msg)?;

        self.event_bus.publish(Event::MessageEnd {
            session_id: session_id.to_string(),
            message_id: assistant_msg.id.clone(),
            reason: FinishReason::Stop,
        });

        Ok(assistant_msg)
    }

    fn resolve_api_key(&self, provider_id: &str) -> Result<String> {
        // Ollama does not require an API key for local servers
        if provider_id == "ollama" {
            return Ok(std::env::var("OLLAMA_API_KEY").unwrap_or_default());
        }

        // Copilot: prefer DB-stored device flow token (works for token
        // exchange), then fall back to env var → IDE → gh CLI discovery.
        if provider_id == "copilot" {
            // DB first — device flow tokens stored here work for copilot_internal/v2/token
            if let Ok(Some(key)) = self.session_manager.storage().get_provider_auth("copilot") {
                if !key.is_empty() {
                    return Ok(key);
                }
            }
            let db_lookup = || -> Option<String> { None }; // already checked above
            if let Some(token) =
                crate::provider::copilot::resolve_copilot_github_token(Some(&db_lookup))
            {
                return Ok(token);
            }
            bail!(
                "No GitHub token found for Copilot. Use /provider to configure, \
                 or authenticate with `gh auth login` then `gh auth refresh -s copilot`."
            );
        }

        // Check common environment variable names
        let env_vars = match provider_id {
            "anthropic" => vec!["ANTHROPIC_API_KEY"],
            "openai" => vec!["OPENAI_API_KEY"],
            _ => vec![],
        };

        for var in &env_vars {
            if let Ok(key) = std::env::var(var)
                && !key.is_empty()
            {
                return Ok(key);
            }
        }

        // Check the database for a stored API key
        if let Ok(Some(key)) = self
            .session_manager
            .storage()
            .get_provider_auth(provider_id)
        {
            if !key.is_empty() {
                return Ok(key);
            }
        }

        bail!(
            "No API key found for provider '{}'. Set the appropriate environment variable \
             or run `ragent auth {} <key>` to store one.",
            provider_id,
            provider_id
        )
    }
}

struct PendingToolCall {
    id: String,
    name: String,
    args_json: String,
}

fn history_to_chat_messages(messages: &[Message]) -> Vec<ChatMessage> {
    messages
        .iter()
        .map(|msg| {
            let role = match msg.role {
                Role::User => "user",
                Role::Assistant => "assistant",
            };

            let content = if msg.parts.len() == 1 {
                match &msg.parts[0] {
                    MessagePart::Text { text } => ChatContent::Text(text.clone()),
                    _ => parts_to_chat_content(&msg.parts),
                }
            } else {
                parts_to_chat_content(&msg.parts)
            };

            ChatMessage {
                role: role.to_string(),
                content,
            }
        })
        .collect()
}

fn parts_to_chat_content(parts: &[MessagePart]) -> ChatContent {
    let content_parts: Vec<ContentPart> = parts
        .iter()
        .filter_map(|part| match part {
            MessagePart::Text { text } => Some(ContentPart::Text { text: text.clone() }),
            MessagePart::ToolCall {
                tool,
                call_id,
                state,
            } => Some(ContentPart::ToolUse {
                id: call_id.clone(),
                name: tool.clone(),
                input: state.input.clone(),
            }),
            MessagePart::Reasoning { .. } => None,
        })
        .collect();
    ChatContent::Parts(content_parts)
}

fn build_file_tree(dir: &std::path::Path, max_depth: usize) -> String {
    let mut lines = Vec::new();
    build_tree_recursive(dir, "", 0, max_depth, &mut lines);
    lines.join("\n")
}

fn build_tree_recursive(
    dir: &std::path::Path,
    prefix: &str,
    depth: usize,
    max_depth: usize,
    lines: &mut Vec<String>,
) {
    if depth >= max_depth {
        return;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());

    // Filter hidden and common non-source dirs
    entries.retain(|e| {
        let name = e.file_name();
        let name_str = name.to_string_lossy();
        !name_str.starts_with('.')
            && !matches!(
                name_str.as_ref(),
                "node_modules" | "target" | "__pycache__" | "dist" | "build" | ".git"
            )
    });

    let count = entries.len();
    for (i, entry) in entries.iter().enumerate() {
        let is_last = i == count - 1;
        let connector = if is_last { "└── " } else { "├── " };
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let path = entry.path();

        if path.is_dir() {
            lines.push(format!("{}{}{}/", prefix, connector, name_str));
            let new_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
            build_tree_recursive(&path, &new_prefix, depth + 1, max_depth, lines);
        } else {
            lines.push(format!("{}{}{}", prefix, connector, name_str));
        }
    }
}
