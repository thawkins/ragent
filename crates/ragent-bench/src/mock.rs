//! Mock LLM client for hermetic agent-loop benchmarks (PERFPLAN Milestone F-1).
//!
//! [`MockLlmClient`] implements [`crate::llm::LlmClient`] by replaying a
//! canned sequence of [`StreamEvent`]s on every `chat()` call. This lets the
//! agent-loop benchmark (Milestone F-2) exercise the full
//! [`SessionProcessor::process_user_message`] pipeline — including stream
//! buffering, tool-call assembly, and the per-step allocations targeted by
//! Milestones A–E — without hitting a real provider.
//!
//! The mock is deterministic: the same script produces the same event stream
//! on every call, so Criterion can measure step latency and tool-call
//! throughput with low variance.

use std::pin::Pin;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use futures::Stream;
use futures::stream;
use ragent_llm::llm::{ChatRequest, LlmClient};
use ragent_types::StreamEvent;
use ragent_types::event::FinishReason;

/// A canned sequence of [`StreamEvent`]s replayed by [`MockLlmClient`].
///
/// The script is cloned on every `chat()` call so the client is safe to share
/// across concurrent benchmark threads. Cheap to clone when the events are
/// small (the common benchmark case).
#[derive(Debug, Clone)]
pub struct MockLlmScript {
    events: Vec<StreamEvent>,
}

impl MockLlmScript {
    /// Build a new script from the supplied event sequence.
    #[must_use]
    pub const fn new(events: Vec<StreamEvent>) -> Self {
        Self { events }
    }

    /// Build a simple text-only script that emits a single `TextDelta` plus
    /// `Usage` + `Finish(Stop)`. Useful for measuring the no-tool step latency.
    #[must_use]
    pub fn text_only(text: &str) -> Self {
        Self::new(vec![
            StreamEvent::TextDelta {
                text: text.to_string(),
            },
            StreamEvent::Usage {
                input_tokens: 16,
                output_tokens: 8,
            },
            StreamEvent::Finish {
                reason: FinishReason::Stop,
            },
        ])
    }

    /// Build a script that emits a single tool call (start → delta → end),
    /// followed by `Usage` + `Finish(Stop)`. The tool-call id is `call-1` and
    /// the tool name is `echo`. Useful for measuring per-tool-call throughput.
    #[must_use]
    pub fn single_tool_call(tool_name: &str, args_json: &str) -> Self {
        Self::new(vec![
            StreamEvent::ToolCallStart {
                id: "call-1".to_string(),
                name: tool_name.to_string(),
            },
            StreamEvent::ToolCallDelta {
                id: "call-1".to_string(),
                args_json: args_json.to_string(),
            },
            StreamEvent::ToolCallEnd {
                id: "call-1".to_string(),
            },
            StreamEvent::Usage {
                input_tokens: 16,
                output_tokens: 8,
            },
            StreamEvent::Finish {
                reason: FinishReason::ToolUse,
            },
        ])
    }

    /// Return the event sequence for inspection / assertions.
    #[must_use]
    pub fn events(&self) -> &[StreamEvent] {
        &self.events
    }
}

/// Mock [`LlmClient`] that replays a [`MockLlmScript`] on every `chat()` call.
///
/// Designed for benchmarks: zero network I/O, deterministic output, and the
/// `Arc<LlmClient>` shape the session processor expects.
#[derive(Debug, Clone)]
pub struct MockLlmClient {
    script: MockLlmScript,
}

impl MockLlmClient {
    /// Build a new mock client backed by the supplied script.
    #[must_use]
    pub const fn new(script: MockLlmScript) -> Self {
        Self { script }
    }

    /// Build a mock client that emits a single text-only response.
    #[must_use]
    pub fn text_only(text: &str) -> Self {
        Self::new(MockLlmScript::text_only(text))
    }

    /// Build a mock client that emits a single tool-call response.
    #[must_use]
    pub fn single_tool_call(tool_name: &str, args_json: &str) -> Self {
        Self::new(MockLlmScript::single_tool_call(tool_name, args_json))
    }
}

#[async_trait]
impl LlmClient for MockLlmClient {
    async fn chat(
        &self,
        _request: ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = StreamEvent> + Send>>> {
        // Clone the canned events and stream them. The stream is `Send` because
        // `StreamEvent` is `'static` and the underlying iterator is thread-safe.
        let events = self.script.events.clone();
        let stream = stream::iter(events);
        Ok(Box::pin(stream))
    }
}

/// Convenience: build an `Arc<dyn LlmClient>` from a script for use in
/// `SessionProcessor` wiring.
#[must_use]
pub fn mock_llm_client(script: MockLlmScript) -> Arc<dyn LlmClient> {
    Arc::new(MockLlmClient::new(script))
}
