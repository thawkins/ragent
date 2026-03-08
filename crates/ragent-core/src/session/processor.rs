use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{bail, Result};
use futures::StreamExt;
use serde_json::{json, Value};
use tracing::{debug, error, warn};

use crate::agent::{build_system_prompt, AgentInfo};
use crate::event::{Event, EventBus, FinishReason};
use crate::llm::{
    ChatContent, ChatMessage, ChatRequest, ContentPart, StreamEvent,
};
use crate::message::{Message, MessagePart, Role, ToolCallState, ToolCallStatus};
use crate::permission::PermissionChecker;
use crate::provider::ProviderRegistry;
use crate::session::SessionManager;
use crate::tool::{ToolContext, ToolRegistry};

pub struct SessionProcessor {
    pub session_manager: Arc<SessionManager>,
    pub provider_registry: Arc<ProviderRegistry>,
    pub tool_registry: Arc<ToolRegistry>,
    pub permission_checker: Arc<std::sync::Mutex<PermissionChecker>>,
    pub event_bus: Arc<EventBus>,
}

impl SessionProcessor {
    pub async fn process_message(
        &self,
        session_id: &str,
        user_text: &str,
        agent: &AgentInfo,
    ) -> Result<Message> {
        // 1. Store user message
        let user_msg = Message::user_text(session_id, user_text);
        self.session_manager
            .get_messages(session_id)
            .ok(); // ensure session exists
        if let Some(storage) = self.get_storage() {
            storage.create_message(&user_msg)?;
        }

        self.event_bus.publish(Event::MessageStart {
            session_id: session_id.to_string(),
            message_id: user_msg.id.clone(),
        });

        // 2. Resolve model and create LLM client
        let model_ref = agent
            .model
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Agent '{}' has no model configured", agent.name))?;

        let provider = self
            .provider_registry
            .get(&model_ref.provider_id)
            .ok_or_else(|| {
                anyhow::anyhow!("Provider '{}' not found", model_ref.provider_id)
            })?;

        // Try to get API key from environment or storage
        let api_key = self.resolve_api_key(&model_ref.provider_id)?;
        let client = provider
            .create_client(&api_key, None, &HashMap::new())
            .await?;

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
        let tool_definitions = self.tool_registry.definitions();
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
            };

            // Call LLM
            let mut stream = match client.chat(request).await {
                Ok(s) => s,
                Err(e) => {
                    error!("LLM call failed: {}", e);
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
                        error!("Stream error: {}", message);
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
                let input: Value =
                    serde_json::from_str(&tc.args_json).unwrap_or(json!({}));

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

                let result = match self.tool_registry.get(&tc.name) {
                    Some(tool) => tool.execute(input.clone(), &tool_ctx).await,
                    None => Err(anyhow::anyhow!("Unknown tool: {}", tc.name)),
                };
                let duration_ms = start.elapsed().as_millis() as u64;

                let (output_value, error) = match &result {
                    Ok(output) => (
                        Some(json!(output.content)),
                        None,
                    ),
                    Err(e) => (None, Some(e.to_string())),
                };

                let status = if result.is_ok() {
                    ToolCallStatus::Completed
                } else {
                    ToolCallStatus::Error
                };

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
                        input: input.clone(),
                        output: output_value.clone(),
                        error: error.clone(),
                        duration_ms: Some(duration_ms),
                    },
                });

                let result_content = match result {
                    Ok(output) => output.content,
                    Err(e) => format!("Error: {}", e),
                };

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
        if let Some(storage) = self.get_storage() {
            storage.create_message(&assistant_msg)?;
        }

        self.event_bus.publish(Event::MessageEnd {
            session_id: session_id.to_string(),
            message_id: assistant_msg.id.clone(),
            reason: FinishReason::Stop,
        });

        Ok(assistant_msg)
    }

    fn resolve_api_key(&self, provider_id: &str) -> Result<String> {
        // Check common environment variable names
        let env_vars = match provider_id {
            "anthropic" => vec!["ANTHROPIC_API_KEY"],
            "openai" => vec!["OPENAI_API_KEY"],
            _ => vec![],
        };

        for var in env_vars {
            if let Ok(key) = std::env::var(var)
                && !key.is_empty()
            {
                return Ok(key);
            }
        }

        bail!(
            "No API key found for provider '{}'. Set the appropriate environment variable.",
            provider_id
        )
    }

    fn get_storage(&self) -> Option<Arc<crate::storage::Storage>> {
        // Access storage through session manager's storage
        // The session manager holds storage, we use it via the manager's methods
        None // Storage is accessed through session_manager
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
            let new_prefix = format!(
                "{}{}",
                prefix,
                if is_last { "    " } else { "│   " }
            );
            build_tree_recursive(&path, &new_prefix, depth + 1, max_depth, lines);
        } else {
            lines.push(format!("{}{}{}", prefix, connector, name_str));
        }
    }
}
