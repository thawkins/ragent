//! LLM client abstraction layer.
//!
//! Provides the [`LlmClient`] trait for streaming chat completions, along with
//! request/response types ([`ChatRequest`], [`ChatMessage`], [`StreamEvent`])
//! that are provider-agnostic.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;

use crate::event::FinishReason;

// Re-export FinishReason so LLM consumers can use it directly
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
        /// Percentage of request quota consumed (0.0–100.0), if known.
        requests_used_pct: Option<f32>,
        /// Percentage of token quota consumed (0.0–100.0), if known.
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
    pub messages: Vec<ChatMessage>,
    /// Tools the model is allowed to invoke.
    #[serde(default)]
    pub tools: Vec<ToolDefinition>,
    /// Sampling temperature (higher = more random).
    pub temperature: Option<f32>,
    /// Nucleus-sampling probability mass cutoff.
    pub top_p: Option<f32>,
    /// Maximum number of tokens to generate.
    pub max_tokens: Option<u32>,
    /// Optional system prompt prepended to the conversation.
    pub system: Option<String>,
    /// Arbitrary key-value options forwarded to the provider (e.g. thinking control).
    #[serde(default)]
    pub options: HashMap<String, Value>,
    /// Session ID for request tracing (used by providers like Copilot to avoid re-billing).
    #[serde(skip)]
    pub session_id: Option<String>,
    /// Unique request ID for this specific API call (for provider-side request tracking).
    #[serde(skip)]
    pub request_id: Option<String>,
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
        // TODO: Replace `Value` with a typed struct for tool-use input.
        /// JSON input arguments for the tool.
        input: Value,
    },
    /// The result returned from a tool execution.
    ToolResult {
        /// Identifier of the tool call this result answers.
        tool_use_id: String,
        /// The tool's output as a string.
        content: String,
    },
    /// An image specified as a URL or `data:` URI.
    ImageUrl {
        /// The image URL or `data:image/png;base64,...` data URI.
        url: String,
    },
}

/// Schema describing a tool that the LLM may invoke.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// The tool's unique name.
    pub name: String,
    /// Human-readable description of what the tool does.
    pub description: String,
    // TODO: Replace `Value` with a typed JSON Schema struct.
    /// JSON Schema describing the tool's accepted parameters.
    pub parameters: Value,
}

/// Trait implemented by LLM provider backends (e.g. Anthropic, OpenAI).
///
/// Implementors convert a [`ChatRequest`] into a stream of [`StreamEvent`]s.
#[async_trait::async_trait]
pub trait LlmClient: Send + Sync {
    /// Send a chat-completion request and receive a streaming response.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying provider request fails (network,
    /// authentication, rate-limiting, etc.).
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> anyhow::Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>>;
}
