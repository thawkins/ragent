//! LLM request/response primitive types.
//!
//! This module is the canonical home for the provider-agnostic LLM
//! conversation types: [`ChatRequest`], [`ChatMessage`], [`ChatContent`],
//! [`ContentPart`], [`StreamEvent`], and [`ToolDefinition`].
//!
//! The streaming client trait (`LlmClient`) and provider implementations
//! live in `ragent-llm`; `ragent-llm::llm` re-exports the types defined here
//! so existing `use ragent_llm::llm::{...}` sites continue to resolve (see
//! `REMPLAN.md` M1 / T1.3 for the consolidation history).
//!
//! `ragent-types` owns these primitives because they are referenced by the
//! tool crates (`ragent-tools-core`, `ragent-tools-extended`,
//! `ragent-tools-vcs`) which depend on `ragent-types` but not on
//! `ragent-llm`. Keeping `ToolDefinition` here avoids a reverse dependency
//! from the tool crates to the provider crate.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

use crate::event::FinishReason;

// Re-export FinishReason so LLM consumers can use it directly.
pub use crate::event::FinishReason as LlmFinishReason;

/// Events emitted by an LLM streaming response.
///
/// Each variant represents a discrete piece of the model's output as it is
/// generated, enabling incremental rendering and tool-call handling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamEvent {
    /// The model began a reasoning/thinking block.
    ReasoningStart,
    /// An incremental chunk of reasoning text.
    ReasoningDelta {
        /// The reasoning text fragment.
        text: String,
    },
    /// The model finished the reasoning block.
    ReasoningEnd,
    /// The provider's signature over the reasoning block it just streamed.
    ///
    /// Anthropic (and the Bedrock Anthropic shape) send the signature as a
    /// `signature_delta` *inside* the thinking block, after the thinking text,
    /// and require it to be echoed back with the block on the turn that answers
    /// a tool call. It is its own event rather than a field on
    /// [`Self::ReasoningEnd`] because the two arrive in separate frames under
    /// different `type` values, and it is not a [`Self::ReasoningDelta`] because
    /// a signature is not reasoning text: an agent that appended it to the
    /// reasoning buffer would show it to the user and lose it as a signature
    /// (FR-011).
    ReasoningSignature {
        /// The provider's opaque signature over the block.
        signature: String,
    },
    /// An incremental chunk of response text.
    TextDelta {
        /// The text fragment.
        text: String,
    },
    /// The model started a tool invocation.
    ToolCallStart {
        /// Unique identifier for this tool call.
        id: String,
        /// Name of the tool being invoked.
        name: String,
    },
    /// An incremental chunk of tool-call argument JSON.
    ToolCallDelta {
        /// Identifier of the tool call this delta belongs to.
        id: String,
        /// Partial JSON fragment of the tool arguments.
        args_json: String,
    },
    /// The model finished building a tool call.
    ToolCallEnd {
        /// Identifier of the completed tool call.
        id: String,
    },
    /// Token usage statistics for the request.
    Usage {
        /// Number of tokens in the prompt/input.
        input_tokens: u64,
        /// Number of tokens in the completion/output.
        output_tokens: u64,
    },
    /// Rate-limit / quota information from response headers.
    RateLimit {
        /// Percentage of request quota consumed (0.0-100.0), if known.
        requests_used_pct: Option<f32>,
        /// Percentage of token quota consumed (0.0-100.0), if known.
        tokens_used_pct: Option<f32>,
    },
    /// An error reported by the provider.
    Error {
        /// Human-readable error description.
        message: String,
    },
    /// The stream has ended.
    Finish {
        /// Why the model stopped generating.
        reason: FinishReason,
    },
}

/// A request to an LLM chat-completion endpoint.
///
/// Groups the model identifier, conversation history, available tools, and
/// sampling parameters into a single payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    /// Model identifier (e.g. `"claude-sonnet-4-20250514"`).
    pub model: String,
    /// Ordered conversation history.
    pub messages: Arc<Vec<ChatMessage>>,
    /// Tools the model is allowed to invoke.
    #[serde(default)]
    pub tools: Arc<Vec<ToolDefinition>>,
    /// Sampling temperature (higher = more random).
    pub temperature: Option<f32>,
    /// Nucleus-sampling probability mass cutoff.
    pub top_p: Option<f32>,
    /// Maximum number of tokens to generate.
    pub max_tokens: Option<u32>,
    /// Optional system prompt prepended to the conversation.
    ///
    /// Held as `Arc<str>` (PERF-006) so the system prompt - which can run
    /// 5,000-20,000 characters - is built once per `process_user_message`
    /// and cheaply `Arc::clone`d on every step and every retry attempt
    /// instead of being deep-cloned as a `String`.  The on-wire JSON
    /// format is unchanged (still a plain string) thanks to
    /// [`arc_str_serde`].
    #[serde(default, with = "optional_arc_str_serde")]
    pub system: Option<Arc<str>>,
    /// Arbitrary key-value options forwarded to the provider (e.g. thinking control).
    #[serde(default)]
    pub options: HashMap<String, Value>,
    /// Session ID for request tracing (used by providers like Copilot to avoid re-billing).
    #[serde(skip)]
    pub session_id: Option<String>,
    /// Unique request ID for this specific API call (for provider-side request tracking).
    #[serde(skip)]
    pub request_id: Option<String>,
    /// Stream timeout in seconds - how long to wait for data before considering
    /// the stream stalled.  Set from `StreamConfig.timeout_secs`.
    #[serde(skip)]
    pub stream_timeout_secs: Option<u64>,
    /// Thinking/reasoning configuration for this request.
    /// Provider adapters use this to set provider-specific parameters
    /// (e.g. Anthropic `thinking.*`, `OpenAI` `reasoning_effort`).
    /// Falls back to legacy `options["thinking"]` if not set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking: Option<crate::ThinkingConfig>,
}

/// A single message in a chat conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// The speaker role (e.g. `"user"`, `"assistant"`, `"tool"`).
    pub role: String,
    /// The message body.
    pub content: ChatContent,
}

/// The content payload of a [`ChatMessage`].
///
/// Can be either a plain text string or a sequence of structured
/// [`ContentPart`]s (e.g. text interleaved with tool calls/results).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChatContent {
    /// Plain text content.
    Text(String),
    /// A sequence of structured content parts.
    Parts(Vec<ContentPart>),
}

/// A typed content block within a [`ChatMessage`].
///
/// Variants cover plain text, tool invocations, tool results, and images.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    /// A plain-text content block.
    Text {
        /// The text content.
        text: String,
    },
    /// A tool invocation issued by the model.
    ToolUse {
        /// Unique identifier for this tool call.
        id: String,
        /// Name of the tool to invoke.
        name: String,
        /// JSON input arguments for the tool.
        ///
        /// Deliberately `Value`, not a typed struct - see
        /// `specs/uniformtools/T-025-TYPED-ARGS-DECISION.md`. This part is
        /// replayed to the provider on every subsequent step (FR-006, FR-018),
        /// so the payload must round-trip unchanged; a struct with a fixed
        /// field set would drop any key the provider sent that the struct does
        /// not model, and the model's next turn would see a different call than
        /// the one it made.
        input: Value,
    },
    /// The result returned from a tool execution.
    ToolResult {
        /// Identifier of the tool call this result answers.
        tool_use_id: String,
        /// The tool's output as a string.  Held as `Arc<str>` (FR-013)
        /// to avoid the per-step `String::clone()` previously paid on
        /// every tool call result.
        #[serde(with = "arc_str_serde")]
        content: Arc<str>,
        /// Whether the call failed (FR-014).
        ///
        /// Derived from the record's `ToolCallStatus` at the one history
        /// construction site, never guessed from the content. A profile that
        /// declares `supports_is_error` sends it natively; every other profile's
        /// encoder prefixes the content with `ERROR: ` instead.
        ///
        /// The field defaults to `false` on deserialisation so a persisted
        /// session or a fixture written before the flag existed keeps its
        /// prior meaning (a result with no recorded error).
        #[serde(default)]
        is_error: bool,
    },
    /// An image specified as a URL or `data:` URI.
    ImageUrl {
        /// The image URL or `data:image/png;base64,...` data URI.
        url: String,
    },
    /// Model reasoning (extended thinking) recorded for replay (FR-011).
    ///
    /// Anthropic's extended thinking requires the signed thinking block that
    /// accompanied a tool call to be echoed back on the turn that answers it,
    /// so history records the block and a profile whose `echo_reasoning` is
    /// `true` (T-018) re-emits it. Every other profile's encoder skips the
    /// part, matching the history converter's longstanding behaviour of not
    /// forwarding reasoning.
    Reasoning {
        /// The reasoning text.
        text: String,
        /// The provider's signature over the block, when it supplies one
        /// (Anthropic's `signature`); echoed with the replayed block so the
        /// provider can verify it originated the reasoning.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },
}

/// Schema describing a tool that the LLM may invoke.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// The tool's unique name.
    pub name: String,
    /// Human-readable description of what the tool does.
    pub description: String,
    /// JSON Schema describing the tool's accepted parameters.
    ///
    /// Deliberately `Value`, not a typed JSON Schema struct - see
    /// `specs/uniformtools/T-025-TYPED-ARGS-DECISION.md`. The field is
    /// forwarded, not interpreted: in-tree tools hand-write schema literals
    /// (155 uses of `additionalProperties`, 44 of `enum`, and further
    /// `items`/`format`/`default`/`pattern`/`minimum`/`maximum`), and MCP
    /// servers and plugins supply arbitrary JSON Schema ragent cannot enumerate
    /// (`$ref`, `oneOf`/`anyOf`, `if`/`then`/`else`, `patternProperties`,
    /// vendor extensions). A typed subset would either drop keywords the model
    /// is told about, or need a schema crate - which NFR-002 forbids. FR-003
    /// additionally needs the encoded bytes to be identical on a cache hit and
    /// a miss, which only a faithful pass-through can guarantee.
    pub parameters: Value,
}

/// Serde adapter for `Arc<str>` so the JSON wire format is unchanged
/// (still a plain string) but the in-memory representation is
/// reference-counted and cheaply cloneable.
mod arc_str_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::sync::Arc;

    pub(super) fn serialize<S: Serializer>(value: &Arc<str>, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(value)
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<Arc<str>, D::Error> {
        let s = String::deserialize(de)?;
        Ok(Arc::from(s))
    }
}

/// Serde adapter for `Option<Arc<str>>` (PERF-006). On the wire this is a
/// plain nullable string - `null` or a JSON string - identical to the
/// legacy `Option<String>` representation. In memory the `Some` variant
/// carries an `Arc<str>` so the system prompt can be shared across
/// retry attempts without deep cloning.
mod optional_arc_str_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::sync::Arc;

    pub(super) fn serialize<S: Serializer>(
        value: &Option<Arc<str>>,
        ser: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(arc) => ser.serialize_some(&**arc),
            None => ser.serialize_none(),
        }
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        de: D,
    ) -> Result<Option<Arc<str>>, D::Error> {
        let opt = Option::<String>::deserialize(de)?;
        Ok(opt.map(Arc::from))
    }
}
