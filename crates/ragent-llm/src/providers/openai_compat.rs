//! Shared request-body builder for OpenAI-compatible chat-completions providers.
//!
//! Audit T-401: the message-packing loop and the body scaffolding were
//! re-implemented in `openai`, `openrouter`, `copilot`, `ollama`, and
//! `ollama_cloud` (and inherited by `generic_openai` and `xai` through
//! [`super::openai::OpenAiClient`]). This module holds the single
//! implementation; providers pick their dialect through [`OpenAiCompatSpec`]
//! and their wire-specific tail through [`OpenAiCompat::finish`].
//!
//! The message loop is byte-for-byte the former `OpenAiClient::build_request_body`
//! loop, so the OpenAI request shape is unchanged; every other provider keeps
//! its documented divergence as an explicit [`OpenAiCompatSpec`] field rather
//! than a copied block.

use serde_json::{Value, json};

use super::tool_cache::{ToolFormat, cached_tools};
use crate::llm::{ChatContent, ChatRequest, ContentPart, ToolDefinition};

/// Dialect switches for [`build_openai_messages`] and [`OpenAiCompat::base_body`].
///
/// The defaults describe the canonical OpenAI Chat Completions shape; each
/// provider overrides only the fields where it genuinely differs.
#[derive(Debug, Clone, Copy)]
pub struct OpenAiCompatSpec {
    /// Wrap each message in the OpenAI `{"messages": [...]}` envelope and
    /// stamp `"stream": true`, and emit the shared sampling fields.
    ///
    /// Local Ollama and Ollama Cloud use the native `/api/chat` shape, which
    /// also carries `messages` and `stream` but a different tool/result wire
    /// format, so they build the body themselves via
    /// [`build_openai_messages`] and set `native_body = false`.
    pub openai_envelope: bool,
    /// Emit `"stream_options": {"include_usage": true}` in the base body.
    pub stream_options_in_base: bool,
    /// Collapse a single text part to a bare string only when that part is
    /// actually a text part (Copilot). The canonical providers collapse any
    /// single-element parts array unconditionally.
    pub strict_single_text_collapse: bool,
    /// Add `"tool_name"` to tool-result messages (local Ollama) so both the
    /// OpenAI-compatible and the native Ollama wire shapes are satisfied.
    pub tool_result_name: bool,
}

impl OpenAiCompatSpec {
    /// The canonical OpenAI Chat Completions dialect.
    #[must_use]
    pub const fn openai() -> Self {
        Self {
            openai_envelope: true,
            stream_options_in_base: true,
            strict_single_text_collapse: false,
            tool_result_name: false,
        }
    }

    /// GitHub Copilot: OpenAI shape without `stream_options` and with the
    /// stricter single-text collapse.
    #[must_use]
    pub const fn copilot() -> Self {
        Self {
            stream_options_in_base: false,
            strict_single_text_collapse: true,
            ..Self::openai()
        }
    }

    /// Local Ollama over the OpenAI-compatible `/v1/chat/completions` endpoint:
    /// adds `tool_name` to tool results and `stream_options` outside the base.
    #[must_use]
    pub const fn ollama() -> Self {
        Self {
            stream_options_in_base: false,
            tool_result_name: true,
            ..Self::openai()
        }
    }

    /// Ollama Cloud over the native `/api/chat` endpoint: a bare
    /// `model`/`messages`/`stream` envelope carrying none of the OpenAI-only
    /// fields (`stream_options` is never sent).
    #[must_use]
    pub const fn ollama_cloud() -> Self {
        Self {
            openai_envelope: false,
            stream_options_in_base: false,
            strict_single_text_collapse: false,
            tool_result_name: false,
        }
    }
}

/// Build the `messages` array for an OpenAI-compatible request.
#[must_use]
pub fn build_openai_messages(request: &ChatRequest, spec: &OpenAiCompatSpec) -> Vec<Value> {
    let mut messages = Vec::new();

    if let Some(system) = &request.system {
        messages.push(json!({
            "role": "system",
            "content": &**system
        }));
    }

    // Map tool_use_id -> tool_name so tool results can carry both the
    // OpenAI `tool_call_id` and the native Ollama `tool_name` when asked.
    let tool_id_to_name = if spec.tool_result_name {
        let mut map = std::collections::HashMap::new();
        for msg in request.messages.iter() {
            if let ChatContent::Parts(parts) = &msg.content {
                for part in parts {
                    if let ContentPart::ToolUse { id, name, .. } = part {
                        map.insert(id.clone(), name.clone());
                    }
                }
            }
        }
        Some(map)
    } else {
        None
    };

    for msg in request.messages.iter() {
        let content = match &msg.content {
            ChatContent::Text(text) => json!(text),
            ChatContent::Parts(parts) => {
                let content_parts: Vec<Value> = parts
                    .iter()
                    .filter_map(|part| match part {
                        ContentPart::Text { text } => Some(json!({
                            "type": "text",
                            "text": text
                        })),
                        ContentPart::ImageUrl { url } => Some(json!({
                            "type": "image_url",
                            "image_url": { "url": url }
                        })),
                        ContentPart::ToolResult { .. } | ContentPart::ToolUse { .. } => None,
                    })
                    .collect();
                let single_text = !spec.strict_single_text_collapse
                    || content_parts
                        .first()
                        .and_then(|part| part.get("type"))
                        .and_then(Value::as_str)
                        == Some("text");
                if content_parts.len() == 1 && single_text {
                    content_parts[0]["text"].clone()
                } else {
                    json!(content_parts)
                }
            }
        };

        match &msg.content {
            ChatContent::Parts(parts) => {
                let tool_results: Vec<&ContentPart> = parts
                    .iter()
                    .filter(|p| matches!(p, ContentPart::ToolResult { .. }))
                    .collect();
                let tool_uses: Vec<&ContentPart> = parts
                    .iter()
                    .filter(|p| matches!(p, ContentPart::ToolUse { .. }))
                    .collect();

                if !tool_uses.is_empty() {
                    let tool_calls: Vec<Value> = tool_uses
                        .iter()
                        .filter_map(|p| match p {
                            ContentPart::ToolUse { id, name, input } => Some(json!({
                                "id": id,
                                "type": "function",
                                "function": {
                                    "name": name,
                                    "arguments": input.to_string()
                                }
                            })),
                            _ => None,
                        })
                        .collect();
                    messages.push(json!({
                        "role": "assistant",
                        "tool_calls": tool_calls
                    }));
                } else if !tool_results.is_empty() {
                    for result in tool_results {
                        if let ContentPart::ToolResult {
                            tool_use_id,
                            content,
                        } = result
                        {
                            let mut tool_msg = json!({
                                "role": "tool",
                                "tool_call_id": tool_use_id,
                                "content": content
                            });
                            if let Some(name) = tool_id_to_name
                                .as_ref()
                                .and_then(|map| map.get(tool_use_id))
                            {
                                tool_msg["tool_name"] = json!(name);
                            }
                            messages.push(tool_msg);
                        }
                    }
                } else {
                    messages.push(json!({
                        "role": msg.role,
                        "content": content
                    }));
                }
            }
            _ => {
                messages.push(json!({
                    "role": msg.role,
                    "content": content
                }));
            }
        }
    }

    messages
}

/// The shared portion of an OpenAI-compatible request body.
///
/// Providers construct one, call [`build_openai_messages`] to fill `messages`,
/// then layer provider-specific fields on top via [`OpenAiCompat::finish`].
pub struct OpenAiCompat {
    body: Value,
}

impl OpenAiCompat {
    /// Create the base body for `request`, with `messages` set from
    /// `build_openai_messages` and the shared sampling fields applied.
    #[must_use]
    pub fn base_body(
        request: &ChatRequest,
        spec: OpenAiCompatSpec,
        tools: &[ToolDefinition],
    ) -> Self {
        Self::with_messages(request, spec, tools, build_openai_messages(request, &spec))
    }

    /// Create the base body around an already-built `messages` array.
    ///
    /// Ollama Cloud speaks the native `/api/chat` wire format, so its message
    /// packing cannot come from [`build_openai_messages`]; it builds the array
    /// itself and delegates the shared `model`/`stream`/sampling/tools scaffold
    /// here (audit T-401).
    #[must_use]
    pub fn with_messages(
        request: &ChatRequest,
        spec: OpenAiCompatSpec,
        tools: &[ToolDefinition],
        messages: Vec<Value>,
    ) -> Self {
        let mut body = json!({
            "model": request.model,
            "messages": messages,
            "stream": true
        });
        if spec.openai_envelope && spec.stream_options_in_base {
            body["stream_options"] = json!({ "include_usage": true });
        }

        if let Some(temp) = request.temperature {
            body["temperature"] = json!(temp);
        }
        if let Some(top_p) = request.top_p {
            body["top_p"] = json!(top_p);
        }
        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = json!(max_tokens);
        }
        if !tools.is_empty() {
            // H2: reuse the cached serialised tool list instead of building a
            // fresh `Vec<Value>` of `json!` tool objects on every call.
            let cached = cached_tools(ToolFormat::OpenAi, tools);
            body["tools"] = cached.openai_tools_array();
        }

        Self { body }
    }

    /// Mutable access to the underlying JSON body.
    pub fn body_mut(&mut self) -> &mut Value {
        &mut self.body
    }

    /// Consume the builder and return the finished JSON body.
    #[must_use]
    pub fn finish(self) -> Value {
        self.body
    }
}
