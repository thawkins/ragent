//! LLM client abstraction layer.
//!
//! This module re-exports the provider-agnostic request/response types
//! ([`ChatRequest`], [`ChatMessage`], [`StreamEvent`], [`ChatContent`],
//! [`ContentPart`], [`ToolDefinition`]) from their canonical home in
//! [`ragent_types::llm`] (see `REMPLAN.md` M1 / T1.3) and defines the
//! [`LlmClient`] streaming-client trait, which lives here because it
//! requires `futures` and `anyhow` — dependencies that `ragent-types`
//! intentionally does not pull in.
//!
//! Provider implementations in `crate::providers` import these types via
//! `use crate::llm::{…}`; that path now resolves to the re-exports below,
//! so no provider source changes were needed for the consolidation.

use std::pin::Pin;

// Re-export the canonical primitive types so existing
// `use crate::llm::{…}` sites inside `ragent-llm` and downstream crates
// keep resolving after the M1 / T1.3 consolidation.  `LlmFinishReason` is
// re-exported by `ragent_types::llm` (as an alias for
// `ragent_types::event::FinishReason`), so no separate re-export is needed
// here.
pub use ragent_types::llm::{
    ChatContent, ChatMessage, ChatRequest, ContentPart, LlmFinishReason, StreamEvent,
    ToolDefinition,
};

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