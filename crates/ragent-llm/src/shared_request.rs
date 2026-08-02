//! Shared handle to a chat request's body that is cheap to clone and
//! safe to share with the cancellation guard.
//!
//! (AgentPerf T-009 / FR-014).  The agent action loop used to allocate
//! a fresh `Vec<ChatMessage>` for every step of the tool-call loop.
//! Sharing a single `Arc<Vec<ChatMessage>>` between the `ChatRequest`
//! and the cancellation guard means the message body is allocated once
//! per `process_user_message` and re-used on every step.

use std::sync::Arc;

use crate::llm::{ChatMessage, ToolDefinition};

/// A shared, cheaply-cloneable handle to a chat request's body.
///
/// `ChatRequest` already holds its `messages` and `tools` fields as
/// `Arc<Vec<...>>` (see `ragent_llm::llm::ChatRequest`), so the
/// cancellation guard can hold a clone of the same `Arc` and inspect
/// the message body without ever triggering a deep `clone()`.
#[derive(Debug, Clone)]
pub struct SharedChatRequest {
    /// The chat message history.  Shared with the in-flight `ChatRequest`.
    pub messages: Arc<Vec<ChatMessage>>,
    /// The tool definitions.  Shared with the in-flight `ChatRequest`.
    pub tools: Arc<Vec<ToolDefinition>>,
}

impl SharedChatRequest {
    /// Construct a new shared request body from a message list and tool
    /// list.  Both arguments are moved into freshly-allocated `Arc`s;
    /// callers that already own an `Arc` should use [`from_arc`].
    pub fn new(messages: Vec<ChatMessage>, tools: Vec<ToolDefinition>) -> Self {
        Self {
            messages: Arc::new(messages),
            tools: Arc::new(tools),
        }
    }

    /// Construct a new shared request body from existing `Arc`s.
    pub fn from_arc(messages: Arc<Vec<ChatMessage>>, tools: Arc<Vec<ToolDefinition>>) -> Self {
        Self { messages, tools }
    }

    /// Number of messages in the body.
    #[must_use]
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Whether the body is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}
