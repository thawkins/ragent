//! LLM client abstraction layer.
//!
//! Provides the [`LlmClient`] trait for streaming chat completions, along with
//! request/response types ([`ChatRequest`], [`ChatMessage`], [`StreamEvent`])
//! that are provider-agnostic.

use serde::{Deserialize, Serialize};
use serde_json::Value;
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
    ReasoningStart,
    ReasoningDelta {
        text: String,
    },
    ReasoningEnd,
    TextDelta {
        text: String,
    },
    ToolCallStart {
        id: String,
        name: String,
    },
    ToolCallDelta {
        id: String,
        args_json: String,
    },
    ToolCallEnd {
        id: String,
    },
    Usage {
        input_tokens: u64,
        output_tokens: u64,
    },
    Error {
        message: String,
    },
    Finish {
        reason: FinishReason,
    },
}

/// A request to an LLM chat-completion endpoint.
///
/// Groups the model identifier, conversation history, available tools, and
/// sampling parameters into a single payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub tools: Vec<ToolDefinition>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_tokens: Option<u32>,
    pub system: Option<String>,
}

/// A single message in a chat conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: ChatContent,
}

/// The content payload of a [`ChatMessage`].
///
/// Can be either a plain text string or a sequence of structured
/// [`ContentPart`]s (e.g. text interleaved with tool calls/results).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChatContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

/// A typed content block within a [`ChatMessage`].
///
/// Variants cover plain text, tool invocations, and tool results.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        // TODO: Replace `Value` with a typed struct for tool-use input.
        input: Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

/// Schema describing a tool that the LLM may invoke.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    // TODO: Replace `Value` with a typed JSON Schema struct.
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
