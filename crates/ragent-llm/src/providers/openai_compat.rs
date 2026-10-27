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
use super::wire::encode::{MessageContext, encode_message, tool_names_by_id};
use super::wire::profile_table::OPENAI_CHAT;
use crate::llm::{ChatRequest, ToolDefinition};

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
    /// Add `"tool_name"` to tool-result messages (local Ollama) so both the
    /// OpenAI-compatible and the native Ollama wire shapes are satisfied.
    pub tool_result_name: bool,
}

impl OpenAiCompatSpec {
    /// The wire format this dialect's messages are encoded as (FR-002, T-017).
    ///
    /// Every provider that builds a body through this module speaks the OpenAI
    /// chat-completions *message* vocabulary; what differs between them is
    /// dialect switches on top (a mirrored `tool_name`, the envelope, the
    /// sampling fields), never a second message shape.
    const MESSAGE_FORMAT: ToolFormat = ToolFormat::OpenAi;

    /// The canonical OpenAI Chat Completions dialect.
    #[must_use]
    pub const fn openai() -> Self {
        Self {
            openai_envelope: true,
            stream_options_in_base: true,
            tool_result_name: false,
        }
    }

    /// GitHub Copilot: the OpenAI shape without `stream_options`.
    ///
    /// The stricter single-text collapse Copilot used to ask for is now the only
    /// rule the shared encoder applies (FR-006), so there is nothing to set.
    #[must_use]
    pub const fn copilot() -> Self {
        Self {
            stream_options_in_base: false,
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

    // Every per-message shape below is the one shared OpenAI-family encoder
    // (FR-006). This function keeps only what is a *dialect* rather than a wire
    // contract: the system prompt packing above, and whether a tool result also
    // names its tool (local Ollama asks for both the OpenAI `tool_call_id` and the
    // native `tool_name`).
    let tool_names = if spec.tool_result_name {
        Some(tool_names_by_id(&request.messages))
    } else {
        None
    };
    let ctx = MessageContext {
        tool_names: tool_names.as_ref(),
        mirror_tool_name: spec.tool_result_name,
        // FR-014: this dialect assembles its own `messages` array, so the shared
        // encoder never sees its tool results. The format states the marking rule
        // instead of the caller restating it: `ToolFormat::result_content` applies
        // the `ERROR: ` marker exactly where the result's profile declares
        // `supports_is_error == false`, which is the same rule the encoder's own
        // carriers use, so the two paths cannot disagree.
        error_marking: spec
            .tool_result_name
            .then_some(OpenAiCompatSpec::MESSAGE_FORMAT),
        // The one name rewrite a family applies inside its envelope, taken from
        // the format rather than restated here, so a request's tool names and the
        // names in the calls it is answered with cannot drift apart.
        name_prefix: OpenAiCompatSpec::MESSAGE_FORMAT.message_name_prefix(),
        ..MessageContext::default()
    };
    for msg in request.messages.iter() {
        messages.extend(encode_message(OPENAI_CHAT, msg, &ctx));
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
            body[ToolFormat::OpenAi.body_key()] = cached.openai_tools_array();
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
