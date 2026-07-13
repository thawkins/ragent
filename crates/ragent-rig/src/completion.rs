//! Internal adapter traits for the completion path.
//!
//! This module defines the stable, ragent-facing contract that the
//! Rig-backed completion backend (implemented in T-004 / T-005) satisfies.
//! The traits are intentionally decoupled from both `rig-core` specifics and
//! from ragent's concrete `LlmClient` trait so that:
//!
//! * Future Rig API changes only require updating the concrete impl, not the
//!   trait.
//! * The streaming-chunk mapping logic (T-005) has a single, well-typed input
//!   (`RigStreamChunk`) to translate into ragent's [`StreamEvent`].
//! * Tests can substitute a deterministic mock implementation without pulling
//!   in `rig-core` (see the `mock` feature and T-014).
//!
//! The flow is:
//!
//! ```text
//! ragent ChatRequest
//!     │
//!     ▼
//! CompletionBackend::complete          ← this trait (FR-004)
//!     │  (maps ChatRequest → Rig request, calls Rig, yields chunks)
//!     ▼
//! RigStreamChunk                        ← intermediate, Rig-agnostic chunk
//!     │
//!     ▼
//! chunk_to_stream_event()               ← pure mapping fn (FR-005 / FR-013)
//!     │
//!     ▼
//! StreamEvent                           ← ragent's internal streaming type
//! ```
//!
//! The existing [`crate::provider::ProviderAdapter`] remains the
//! configuration/metadata handle; this module adds the *behavioural* contract.

use std::pin::Pin;

use futures::Stream;
use ragent_types::event::FinishReason;
use ragent_types::llm::{ChatRequest, StreamEvent};

use crate::error::Result;

/// An intermediate, provider-agnostic chunk emitted by a Rig-backed
/// completion backend.
///
/// Each variant corresponds to one logical piece of a model's streaming
/// response. The mapping to ragent's [`StreamEvent`] is performed by
/// [`chunk_to_stream_event`], keeping the translation logic in one place so
/// that T-005 can extend it without touching every backend.
///
/// This type is `RigStreamChunk` (not `StreamEvent`) so that backends can
/// represent Rig-specific signals (e.g. a bare usage block) that do not have a
/// 1:1 `StreamEvent` counterpart without loss of information.
#[derive(Debug, Clone)]
pub enum RigStreamChunk {
    /// Incremental assistant text.
    TextDelta {
        /// The text fragment.
        text: String,
    },
    /// Incremental reasoning / chain-of-thought text.
    ReasoningDelta {
        /// The reasoning text fragment.
        text: String,
    },
    /// The model started a reasoning block.
    ReasoningStart,
    /// The model ended a reasoning block.
    ReasoningEnd,
    /// The model began a tool invocation.
    ToolCallStart {
        /// Provider-assigned tool-call identifier.
        id: String,
        /// Name of the tool being invoked.
        name: String,
    },
    /// Incremental JSON fragment for an in-progress tool call.
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
    /// Token-usage statistics for the request so far.
    Usage {
        /// Number of input/prompt tokens.
        input_tokens: u64,
        /// Number of output/completion tokens.
        output_tokens: u64,
    },
    /// Rate-limit / quota information, when the backend surfaces it.
    RateLimit {
        /// Percentage of request quota consumed, if known.
        requests_used_pct: Option<f32>,
        /// Percentage of token quota consumed, if known.
        tokens_used_pct: Option<f32>,
    },
    /// A backend-reported error.
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

/// The internal contract for a Rig-backed completion backend.
///
/// A concrete implementation (T-004) wraps a `rig::client::CompletionModel`,
/// converts the incoming [`ChatRequest`] into Rig's request representation,
/// invokes the Rig model, and yields [`RigStreamChunk`]s as the response
/// streams.
///
/// The resulting chunk stream is then mapped onto ragent's [`StreamEvent`] by
/// [`chunk_to_stream_event`], so that the TUI and server consume Rig-backed
/// responses identically to native providers (FR-005 / FR-013).
///
/// # Object safety
///
/// The trait is object-safe (`Send + Sync`, returns a pinned boxed stream of
/// `Send` items) so it can be stored as `Box<dyn CompletionBackend>` inside the
/// `LlmClient` adapter implemented in T-004.
pub trait CompletionBackend: Send + Sync {
    /// Execute a completion request and return a stream of intermediate
    /// [`RigStreamChunk`]s.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::RigError`] if the backend cannot service the
    /// request (provider not enabled, network failure, invalid request, …).
    fn complete(
        &self,
        request: &ChatRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<RigStreamChunk>> + Send>>;

    /// Returns the provider alias this backend was constructed with.
    fn alias(&self) -> &str;

    /// Returns whether the backend can stream responses.
    ///
    /// Defaults to `true`; backends that cannot stream override this to
    /// `false` so the `LlmClient` wrapper can reject streaming requests early
    /// with [`crate::error::RigError::StreamingNotSupported`].
    fn supports_streaming(&self) -> bool {
        true
    }
}

/// Maps a single [`RigStreamChunk`] onto the corresponding ragent
/// [`StreamEvent`].
///
/// This is the single translation point required by FR-005 and FR-013: every
/// Rig-backed chunk flows through this function before being emitted on the
/// agent's event bus, so the TUI and server see the same `StreamEvent`
/// variants that native providers produce.
///
/// The mapping is currently 1:1 (each `RigStreamChunk` variant maps to a
/// `StreamEvent` variant of the same shape). Keeping it as a standalone,
/// pure function lets T-005 extend the mapping (e.g. coalescing multiple
/// deltas, normalising tool-call ids) without modifying each backend.
#[must_use]
pub fn chunk_to_stream_event(chunk: RigStreamChunk) -> StreamEvent {
    match chunk {
        RigStreamChunk::TextDelta { text } => StreamEvent::TextDelta { text },
        RigStreamChunk::ReasoningDelta { text } => StreamEvent::ReasoningDelta { text },
        RigStreamChunk::ReasoningStart => StreamEvent::ReasoningStart,
        RigStreamChunk::ReasoningEnd => StreamEvent::ReasoningEnd,
        RigStreamChunk::ToolCallStart { id, name } => StreamEvent::ToolCallStart { id, name },
        RigStreamChunk::ToolCallDelta { id, args_json } => {
            StreamEvent::ToolCallDelta { id, args_json }
        }
        RigStreamChunk::ToolCallEnd { id } => StreamEvent::ToolCallEnd { id },
        RigStreamChunk::Usage {
            input_tokens,
            output_tokens,
        } => StreamEvent::Usage {
            input_tokens,
            output_tokens,
        },
        RigStreamChunk::RateLimit {
            requests_used_pct,
            tokens_used_pct,
        } => StreamEvent::RateLimit {
            requests_used_pct,
            tokens_used_pct,
        },
        RigStreamChunk::Error { message } => StreamEvent::Error { message },
        RigStreamChunk::Finish { reason } => StreamEvent::Finish { reason },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_delta_maps_to_stream_event() {
        let event = chunk_to_stream_event(RigStreamChunk::TextDelta {
            text: "hi".to_owned(),
        });
        match event {
            StreamEvent::TextDelta { text } => assert_eq!(text, "hi"),
            other => panic!("expected TextDelta, got {other:?}"),
        }
    }

    #[test]
    fn tool_call_chunks_map_in_order() {
        let start = chunk_to_stream_event(RigStreamChunk::ToolCallStart {
            id: "c1".to_owned(),
            name: "read".to_owned(),
        });
        let delta = chunk_to_stream_event(RigStreamChunk::ToolCallDelta {
            id: "c1".to_owned(),
            args_json: "{\"path\":".to_owned(),
        });
        let end = chunk_to_stream_event(RigStreamChunk::ToolCallEnd {
            id: "c1".to_owned(),
        });
        assert!(matches!(start, StreamEvent::ToolCallStart { .. }));
        assert!(matches!(delta, StreamEvent::ToolCallDelta { .. }));
        assert!(matches!(end, StreamEvent::ToolCallEnd { .. }));
    }

    #[test]
    fn usage_and_finish_map_correctly() {
        let usage = chunk_to_stream_event(RigStreamChunk::Usage {
            input_tokens: 10,
            output_tokens: 5,
        });
        assert!(matches!(usage, StreamEvent::Usage { .. }));

        let finish = chunk_to_stream_event(RigStreamChunk::Finish {
            reason: FinishReason::Stop,
        });
        assert!(matches!(finish, StreamEvent::Finish { .. }));
    }

    #[test]
    fn reasoning_chunks_map_correctly() {
        assert!(matches!(
            chunk_to_stream_event(RigStreamChunk::ReasoningStart),
            StreamEvent::ReasoningStart
        ));
        assert!(matches!(
            chunk_to_stream_event(RigStreamChunk::ReasoningEnd),
            StreamEvent::ReasoningEnd
        ));
        let d = chunk_to_stream_event(RigStreamChunk::ReasoningDelta {
            text: "thinking".to_owned(),
        });
        match d {
            StreamEvent::ReasoningDelta { text } => assert_eq!(text, "thinking"),
            other => panic!("expected ReasoningDelta, got {other:?}"),
        }
    }

    #[test]
    fn error_and_rate_limit_map_correctly() {
        let err = chunk_to_stream_event(RigStreamChunk::Error {
            message: "boom".to_owned(),
        });
        match err {
            StreamEvent::Error { message } => assert_eq!(message, "boom"),
            other => panic!("expected Error, got {other:?}"),
        }
        let rl = chunk_to_stream_event(RigStreamChunk::RateLimit {
            requests_used_pct: Some(50.0),
            tokens_used_pct: None,
        });
        match rl {
            StreamEvent::RateLimit {
                requests_used_pct,
                tokens_used_pct,
            } => {
                assert_eq!(requests_used_pct, Some(50.0));
                assert!(tokens_used_pct.is_none());
            }
            other => panic!("expected RateLimit, got {other:?}"),
        }
    }

    /// A minimal mock backend used to verify the trait is object-safe and
    /// that a `Box<dyn CompletionBackend>` can be constructed and called.
    struct StubBackend {
        alias: String,
    }

    impl CompletionBackend for StubBackend {
        fn complete(
            &self,
            _request: &ChatRequest,
        ) -> Pin<Box<dyn Stream<Item = Result<RigStreamChunk>> + Send>> {
            use futures::stream;
            let chunks = vec![
                Ok(RigStreamChunk::TextDelta {
                    text: "hello".to_owned(),
                }),
                Ok(RigStreamChunk::Finish {
                    reason: FinishReason::Stop,
                }),
            ];
            Box::pin(stream::iter(chunks))
        }

        fn alias(&self) -> &str {
            &self.alias
        }
    }

    #[tokio::test]
    async fn boxed_backend_is_object_safe_and_yields_chunks() {
        use futures::StreamExt;

        let backend: Box<dyn CompletionBackend> = Box::new(StubBackend {
            alias: "stub".to_owned(),
        });
        assert_eq!(backend.alias(), "stub");
        assert!(backend.supports_streaming());

        let request = ChatRequest {
            model: "stub-model".to_owned(),
            messages: std::sync::Arc::new(Vec::new()),
            tools: std::sync::Arc::new(Vec::new()),
            temperature: None,
            top_p: None,
            max_tokens: None,
            system: None,
            options: std::collections::HashMap::new(),
            session_id: None,
            request_id: None,
            stream_timeout_secs: None,
            thinking: None,
        };

        let mut stream = backend.complete(&request);
        let mut events = Vec::new();
        while let Some(item) = stream.next().await {
            events.push(chunk_to_stream_event(item.expect("chunk")));
        }
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], StreamEvent::TextDelta { .. }));
        assert!(matches!(events[1], StreamEvent::Finish { .. }));
    }
}
