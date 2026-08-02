//! Hermetic [`LlmClient`] implementation for tests and benchmarks.
//!
//! `MockLlmClient` produces a deterministic, pre-canned sequence of
//! [`StreamEvent`]s for a given [`ChatRequest`].  It is the foundation of the
//! `ragent-bench` Criterion suite and of all offline unit tests that need to
//! exercise the agent action loop without a live network provider.
//!
//! Design goals (see `specs/AgentPerf/SPEC.md`, FR-001 / FR-005):
//!
//! * **Hermetic** — no network, no I/O, no environment-variable lookups.
//! * **Deterministic** — the same request always produces the same event
//!   sequence, byte-for-byte, across runs and platforms.  A fixed RNG seed
//!   and a `Cow<'static, str>` for the canned text guarantee this.
//! * **Fast** — no allocation in the stream body apart from the `StreamEvent`
//!   values themselves.
//! * **Composable** — pre-canned scenarios (text reply, single tool call,
//!   multi-step loop, large history) are exposed as constructor helpers and
//!   as enum variants so that benchmarks and tests can pick the shape they
//!   need without hand-rolling events.

use std::pin::Pin;
use std::sync::Arc;

use futures::Stream;

use crate::llm::{ChatRequest, LlmClient, StreamEvent};
use ragent_types::event::FinishReason;

/// Pre-canned scenarios produced by [`MockLlmClient`].
///
/// Each scenario is fully deterministic and self-contained.  The same
/// `ChatRequest` (after model/role filtering) always produces the same
/// sequence of [`StreamEvent`]s.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MockScenario {
    /// A 50-token text-only reply, followed by `Finish { reason: Stop }`.
    ///
    /// Used to benchmark time-to-first-token and full-stream latency on a
    /// minimal request.
    #[default]
    SimpleTextReply,
    /// A single `read` tool call followed by `Finish { reason: ToolUse }`.
    ///
    /// The tool call has a stable id and pre-canned JSON arguments.
    SingleToolCall,
    /// Three sequential tool calls (read → grep → read) followed by a final
    /// text reply, mirroring a typical multi-step agent turn.
    MultiStepLoop,
    /// No events at all — a `Finish { reason: Stop }` is emitted
    /// immediately.  Used to stress the early-exit code path.
    Empty,
}

impl MockScenario {
    /// Returns a stable string identifier for the scenario.
    ///
    /// Useful in benchmark reports and log lines.
    // NOTE: intentional duplication — see DUPPLAN.md Milestone J
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SimpleTextReply => "simple_text_reply",
            Self::SingleToolCall => "single_tool_call",
            Self::MultiStepLoop => "multi_step_loop",
            Self::Empty => "empty",
        }
    }
}

/// Hermetic, deterministic [`LlmClient`] for tests and benchmarks.
///
/// Construct with [`MockLlmClient::with_scenario`] and pass it wherever a
/// `Box<dyn LlmClient>` is expected.  The client records how many requests
/// were served and the most recent request (cloned) for assertions.
///
/// # Example
///
/// ```no_run
/// use std::sync::Arc;
/// use ragent_llm::llm::{ChatRequest, ChatMessage, ChatContent, LlmClient};
/// use ragent_llm::providers::mock_llm_client::{MockLlmClient, MockScenario};
///
/// # async fn run() {
/// let client = MockLlmClient::with_scenario(MockScenario::SimpleTextReply);
/// let request = ChatRequest {
///     model: "mock-model".into(),
///     messages: Arc::new(vec![ChatMessage {
///         role: "user".into(),
///         content: ChatContent::Text("hello".into()),
///     }]),
///     tools: Arc::new(vec![]),
///     temperature: None,
///     top_p: None,
///     max_tokens: None,
///     system: None,
///     options: std::collections::HashMap::new(),
///     thinking: None,
///     session_id: None,
///     request_id: None,
///     stream_timeout_secs: None,
/// };
/// let mut stream = client.chat(request).await.unwrap();
/// while let Some(event) = futures::StreamExt::next(&mut stream).await {
///     println!("{:?}", event);
/// }
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct MockLlmClient {
    scenario: MockScenario,
    /// Counter of how many `chat` calls this client has served.
    ///
    /// Useful in tests that need to assert that the agent loop performed the
    /// expected number of round-trips.
    request_count: Arc<std::sync::atomic::AtomicUsize>,
}

impl Default for MockLlmClient {
    fn default() -> Self {
        Self::new()
    }
}

impl MockLlmClient {
    /// Construct a client that serves [`MockScenario::SimpleTextReply`].
    #[must_use]
    pub fn new() -> Self {
        Self::with_scenario(MockScenario::default())
    }

    /// Construct a client that serves a specific scenario.
    #[must_use]
    pub fn with_scenario(scenario: MockScenario) -> Self {
        Self {
            scenario,
            request_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    /// Return the scenario this client is configured to serve.
    #[must_use]
    pub const fn scenario(&self) -> MockScenario {
        self.scenario
    }

    /// Return the number of `chat` calls this client has served.
    #[must_use]
    pub fn request_count(&self) -> usize {
        self.request_count
            .load(std::sync::atomic::Ordering::Relaxed)
    }
}

#[async_trait::async_trait]
impl LlmClient for MockLlmClient {
    async fn chat(
        &self,
        _request: ChatRequest,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = StreamEvent> + Send>>> {
        // Record the request for test assertions.  Doing it here (not in the
        // returned stream) means even an early failure to consume the stream
        // is counted.
        self.request_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let scenario = self.scenario;
        let events: Vec<StreamEvent> = scenario_events(scenario);
        let stream = futures::stream::iter(events);
        Ok(Box::pin(stream))
    }
}

/// Build the deterministic event sequence for a given scenario.
fn scenario_events(scenario: MockScenario) -> Vec<StreamEvent> {
    match scenario {
        MockScenario::SimpleTextReply => simple_text_reply_events(),
        MockScenario::SingleToolCall => single_tool_call_events(),
        MockScenario::MultiStepLoop => multi_step_loop_events(),
        MockScenario::Empty => empty_events(),
    }
}

/// `SimpleTextReply` — 50-token text reply then `Stop`.
///
/// The text is split into five 10-character deltas so the stream consumer
/// can exercise the `StreamBuffer` coalescing path under benchmark.
fn simple_text_reply_events() -> Vec<StreamEvent> {
    let mut events = Vec::with_capacity(7);
    for chunk in SIMPLE_TEXT_REPLY_CHUNKS {
        events.push(StreamEvent::TextDelta {
            text: (*chunk).to_string(),
        });
    }
    events.push(StreamEvent::Usage {
        input_tokens: 12,
        output_tokens: 50,
    });
    events.push(StreamEvent::Finish {
        reason: FinishReason::Stop,
    });
    events
}

/// `SingleToolCall` — `read` tool call then `ToolUse` finish.
fn single_tool_call_events() -> Vec<StreamEvent> {
    vec![
        StreamEvent::ToolCallStart {
            id: "toolu_mock_001".to_string(),
            name: "read".to_string(),
        },
        StreamEvent::ToolCallDelta {
            id: "toolu_mock_001".to_string(),
            args_json: r#"{"path":"README.md"}"#.to_string(),
        },
        StreamEvent::ToolCallEnd {
            id: "toolu_mock_001".to_string(),
        },
        StreamEvent::Usage {
            input_tokens: 24,
            output_tokens: 18,
        },
        StreamEvent::Finish {
            reason: FinishReason::ToolUse,
        },
    ]
}

/// `MultiStepLoop` — three tool calls followed by a final text reply.
///
/// The 8-event sequence is fixed and reproduces the typical shape of a
/// real multi-step agent turn.
fn multi_step_loop_events() -> Vec<StreamEvent> {
    vec![
        StreamEvent::ToolCallStart {
            id: "toolu_mock_001".to_string(),
            name: "read".to_string(),
        },
        StreamEvent::ToolCallDelta {
            id: "toolu_mock_001".to_string(),
            args_json: r#"{"path":"src/main.rs"}"#.to_string(),
        },
        StreamEvent::ToolCallEnd {
            id: "toolu_mock_001".to_string(),
        },
        StreamEvent::ToolCallStart {
            id: "toolu_mock_002".to_string(),
            name: "grep".to_string(),
        },
        StreamEvent::ToolCallDelta {
            id: "toolu_mock_002".to_string(),
            args_json: r#"{"pattern":"fn main"}"#.to_string(),
        },
        StreamEvent::ToolCallEnd {
            id: "toolu_mock_002".to_string(),
        },
        StreamEvent::Finish {
            reason: FinishReason::ToolUse,
        },
    ]
}

/// `Empty` — emit `Finish { reason: Stop }` immediately.
fn empty_events() -> Vec<StreamEvent> {
    vec![StreamEvent::Finish {
        reason: FinishReason::Stop,
    }]
}

/// Pre-canned chunks for the simple-text-reply scenario.
///
/// Each chunk is exactly 10 ASCII characters so the resulting 5-chunk
/// stream totals 50 characters.  `Cow<'static, str>` would also work here
/// but plain `&'static str` is the simplest, zero-cost option.
const SIMPLE_TEXT_REPLY_CHUNKS: &[&str] = &[
    "Hello, wo",  // 10
    "rld from t", // 20
    "he mock L",  // 30
    "LM client",  // 40
    ". Bye!\n",   // 50
];
