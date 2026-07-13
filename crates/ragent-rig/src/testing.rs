//! Rig-backed mock-model and VCR test utilities (T-014 / FR-025 / NFR-005).
//!
//! This module is compiled only when the `mock` feature is enabled (which
//! pulls in `rig-core`).
//!
//! # Purpose
//!
//! Provides deterministic mock implementations of Rig's `CompletionModel` and
//! `EmbeddingModel` traits so that:
//!
//! * Unit tests can exercise the `ragent-rig` adapter layer
//!   (`RigCompletionBackend` → `RigLlmClient` → `StreamEvent`) without any
//!   network calls.
//! * Integration tests can verify tool-call routing and streaming behavior
//!   through the same `LlmClient::chat` entry point the ragent agent loop
//!   uses (NFR-005 / AC-4).
//!
//! The mocks are **programmatic**: a test pre-loads the response(s) the mock
//! should return, so the same harness covers both "echo text" and
//! "emit a tool call" scenarios.
//!
//! # Mock completion model
//!
//! [`MockCompletionModel`] implements [`rig::completion::CompletionModel`]. It
//! holds a list of canned [`AssistantContent`] responses and returns them
//! one per `completion()` call (round-robin if multiple are configured). This
//! lets a test simulate a multi-turn conversation where the first turn emits
//! text and the second emits a tool call.
//!
//! # Mock embedding model
//!
//! [`MockEmbeddingModel`] implements
//! [`rig::embeddings::EmbeddingModel`]. It produces deterministic
//! `Vec<f64>` vectors of a fixed dimension so embedding-backed tests are
//! reproducible.
//!
//! # Wiring helpers
//!
//! [`build_mock_llm_client`] wires a [`MockCompletionModel`] into a
//! [`crate::provider::RigLlmClient`] so a test can call `client.chat(req).await`
//! and collect the resulting [`StreamEvent`]s — exactly the path the ragent
//! agent loop uses. See the integration test
//! `tests/mock_model_test.rs` for the end-to-end example.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use rig::completion::message::{
    Text as RigText, ToolCall as RigToolCall, ToolFunction as RigToolFunction,
};
use rig::completion::{
    AssistantContent, CompletionModel as RigCompletionModel, CompletionRequest, CompletionResponse,
};
use rig::embeddings::{Embedding, EmbeddingError, EmbeddingModel as RigEmbeddingModel};
use rig::one_or_many::OneOrMany;

use crate::provider::{RigCompletionBackend, RigLlmClient};

// Re-export the marker so existing callers of `MockSupport` still compile.
pub use crate::testing_marker::MockSupport;

// Imports for the mock provider helper used by integration tests (T-016).
use ragent_config::{Capabilities, Cost};
use ragent_llm::provider::{ModelInfo, Provider};

/// A canned response for the mock completion model.
///
/// Built with [`MockResponse::text`] or [`MockResponse::tool_call`]; the mock
/// returns it verbatim (wrapped in a `CompletionResponse`) when its
/// `completion()` method is called.
#[derive(Clone, Debug)]
pub struct MockResponse {
    content: Vec<AssistantContent>,
}

impl MockResponse {
    /// Build a text-only response.
    ///
    /// The mock will return a single `AssistantContent::Text` containing the
    /// given string.
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![AssistantContent::Text(RigText { text: text.into() })],
        }
    }

    /// Build a tool-call response.
    ///
    /// The mock will return a single `AssistantContent::ToolCall` with the
    /// given tool name, id, and JSON arguments. This exercises the adapter's
    /// tool-call lifecycle mapping (start → delta → end → finish with
    /// `ToolUse` reason) — see AC-4.
    #[must_use]
    pub fn tool_call(
        name: impl Into<String>,
        id: impl Into<String>,
        args: serde_json::Value,
    ) -> Self {
        Self {
            content: vec![AssistantContent::ToolCall(RigToolCall {
                id: id.into(),
                function: RigToolFunction {
                    name: name.into(),
                    arguments: args,
                },
            })],
        }
    }

    /// Build a response from raw [`AssistantContent`] parts.
    ///
    /// Use this for multi-part responses (e.g. text + tool call in the same
    /// turn).
    #[must_use]
    pub fn parts(content: Vec<AssistantContent>) -> Self {
        Self { content }
    }

    /// Returns the inner content slice.
    #[must_use]
    pub fn content(&self) -> &[AssistantContent] {
        &self.content
    }
}

/// The raw response type the mock reports back via `CompletionResponse::raw`.
///
/// It carries no real provider data; the mock fills it with `()` so the
/// `CompletionResponse<()>` is cheap to construct.
type MockRawResponse = ();

/// A deterministic mock implementation of Rig's [`CompletionModel`] trait.
///
/// The mock holds a list of [`MockResponse`]s and returns them in order, one
/// per `completion()` call. If the test calls `completion()` more times than
/// there are configured responses, the last response is reused
/// (round-robin), so a single-response mock works for unlimited turns.
///
/// # Examples
///
/// ```
/// use ragent_rig::testing::{MockCompletionModel, MockResponse};
///
/// let model = MockCompletionModel::new()
///     .with_response(MockResponse::text("hello"))
///     .with_response(MockResponse::tool_call("read", "c1", serde_json::json!({"path":"x"})));
/// // model.completion(req).await returns "hello", then the tool call.
/// ```
#[derive(Clone)]
pub struct MockCompletionModel {
    responses: Arc<Vec<MockResponse>>,
    call_index: Arc<AtomicUsize>,
}

impl MockCompletionModel {
    /// Create an empty mock model.
    ///
    /// An empty model returns an empty `AssistantContent` (a blank text
    /// message) on every call, which is rarely useful — prefer
    /// [`MockCompletionModel::with_response`] to configure at least one
    /// response.
    #[must_use]
    pub fn new() -> Self {
        Self {
            responses: Arc::new(Vec::new()),
            call_index: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Add a canned response.
    ///
    /// Responses are returned in the order they are added. Call this multiple
    /// times to script a multi-turn conversation.
    #[must_use]
    pub fn with_response(mut self, response: MockResponse) -> Self {
        // Re-construct the Arc with the new response appended. `Arc::make_mut`
        // would require `Vec: Clone` (it is) but we want a fresh allocation per
        // builder call so clones of the model share the same script.
        let mut responses = (*self.responses).clone();
        responses.push(response);
        self.responses = Arc::new(responses);
        self
    }

    /// Returns the number of configured responses.
    #[must_use]
    pub fn response_count(&self) -> usize {
        self.responses.len()
    }

    /// Returns how many times `completion()` has been called so far.
    #[must_use]
    pub fn call_count(&self) -> usize {
        self.call_index.load(Ordering::Relaxed)
    }

    /// Pick the response to return for this call (round-robin on overflow).
    fn next_response(&self) -> &[AssistantContent] {
        let idx = self.call_index.fetch_add(1, Ordering::Relaxed);
        if self.responses.is_empty() {
            // No responses configured: return a reference to an empty slice.
            // The caller wraps it in a blank text message.
            return EMPTY_CONTENT;
        }
        let idx = idx % self.responses.len();
        &self.responses[idx].content
    }
}

impl Default for MockCompletionModel {
    fn default() -> Self {
        Self::new()
    }
}

/// A static empty content slice used when no responses are configured.
static EMPTY_CONTENT: &[AssistantContent] = &[];

impl RigCompletionModel for MockCompletionModel {
    type Response = MockRawResponse;

    async fn completion(
        &self,
        _request: CompletionRequest,
    ) -> Result<CompletionResponse<Self::Response>, rig::completion::CompletionError> {
        let content = self.next_response();
        let choice: OneOrMany<AssistantContent> = if content.is_empty() {
            // No responses configured: emit a blank text so the response is
            // still a valid OneOrMany.
            OneOrMany::one(AssistantContent::Text(RigText {
                text: String::new(),
            }))
        } else {
            OneOrMany::many(content.to_vec()).map_err(|e| {
                rig::completion::CompletionError::ProviderError(format!(
                    "mock content build failed: {e}"
                ))
            })?
        };
        Ok(CompletionResponse {
            choice,
            raw_response: (),
        })
    }
}

/// A deterministic mock implementation of Rig's [`EmbeddingModel`] trait.
///
/// Each embedding vector is `[text.len() as f64; ndims]` so tests can verify
/// the text flowed through and the f64→f32 conversion happened. The dimension
/// is fixed at construction time.
///
/// # Examples
///
/// ```
/// use ragent_rig::testing::MockEmbeddingModel;
/// use rig::embeddings::EmbeddingModel;
///
/// # async fn run() {
/// let model = MockEmbeddingModel::new(4);
/// assert_eq!(model.ndims(), 4);
/// let emb = model.embed_text("hello").await.unwrap();
/// assert_eq!(emb.vec, vec![5.0_f64; 4]); // "hello".len() == 5
/// # }
/// ```
#[derive(Clone)]
pub struct MockEmbeddingModel {
    ndims: usize,
}

impl MockEmbeddingModel {
    /// Create a mock embedding model that produces vectors of `ndims`
    /// dimensions.
    #[must_use]
    pub fn new(ndims: usize) -> Self {
        Self { ndims }
    }
}

impl RigEmbeddingModel for MockEmbeddingModel {
    const MAX_DOCUMENTS: usize = 64;

    fn ndims(&self) -> usize {
        self.ndims
    }

    fn embed_texts(
        &self,
        texts: impl IntoIterator<Item = String> + Send,
    ) -> impl std::future::Future<Output = Result<Vec<Embedding>, EmbeddingError>> + Send {
        let ndims = self.ndims;
        let texts: Vec<String> = texts.into_iter().collect();
        async move {
            Ok(texts
                .into_iter()
                .map(|t| Embedding {
                    document: t.clone(),
                    vec: vec![t.len() as f64; ndims],
                })
                .collect())
        }
    }
}

// ── Wiring helpers ───���───────────────────────────────────────────────────────

/// Build a [`RigLlmClient`] backed by a [`MockCompletionModel`].
///
/// The resulting client implements ragent's [`LlmClient`] trait, so a test can
/// call `client.chat(request).await` and collect the [`StreamEvent`]s —
/// exactly the entry point the ragent agent loop uses (NFR-005 / AC-4).
///
/// `alias` is reported by the client via [`RigLlmClient::alias`] and
/// [`RigLlmClient::supports_streaming`].
///
/// # Non-streaming path
///
/// The mock uses the non-streaming `CompletionModel::completion()` path (the
/// mock does not implement `StreamingCompletionModel`). The adapter
/// synthesises a chunk stream from the buffered response via
/// `completion_response_to_chunks`, so the resulting `StreamEvent` sequence
/// is identical to what a real non-streaming provider produces — text deltas
/// (or a tool-call lifecycle triple) followed by a `Finish` event.
///
/// # Examples
///
/// ```
/// use ragent_rig::testing::{build_mock_llm_client, MockResponse};
/// use ragent_llm::llm::LlmClient;
/// # use ragent_types::llm::{ChatRequest, ChatMessage, ChatContent};
/// # use std::sync::Arc;
/// # async fn run() {
/// let client = build_mock_llm_client("rig-mock", MockResponse::text("hello"));
/// assert_eq!(client.alias(), "rig-mock");
/// # }
/// ```
#[must_use]
pub fn build_mock_llm_client(alias: impl Into<String>, response: MockResponse) -> RigLlmClient {
    build_mock_llm_client_multi(alias, vec![response])
}

/// Build a [`RigLlmClient`] backed by a [`MockCompletionModel`] with multiple
/// scripted responses.
///
/// Responses are returned in order across successive `chat()` calls. Use this
/// to script a multi-turn conversation where turn 1 emits text and turn 2
/// emits a tool call.
#[must_use]
pub fn build_mock_llm_client_multi(
    alias: impl Into<String>,
    responses: Vec<MockResponse>,
) -> RigLlmClient {
    let mut model = MockCompletionModel::new();
    for r in responses {
        model = model.with_response(r);
    }
    build_mock_llm_client_from_model(alias, model)
}

/// Build a [`RigLlmClient`] from an already-constructed
/// [`MockCompletionModel`].
///
/// Use this when the test needs to inspect the model afterwards (e.g. to
/// assert on [`MockCompletionModel::call_count`]).
#[must_use]
pub fn build_mock_llm_client_from_model(
    alias: impl Into<String>,
    model: MockCompletionModel,
) -> RigLlmClient {
    // Mirror the non-streaming provider builders: capture the model into a
    // closure that calls `.completion()` and synthesises chunks. We reuse the
    // same `completion_response_to_chunks` mapping the real non-streaming
    // backends use, so the StreamEvent sequence is faithful.
    use crate::provider::{StreamFn, completion_response_to_chunks};
    let stream_fn: StreamFn = Box::new(move |req: ragent_llm::llm::ChatRequest| {
        let model = model.clone();
        Box::pin(async_stream::stream! {
            let (preamble, history, prompt, tools, temp, max_tokens, params) =
                crate::provider::chat_request_to_rig(&req);
            let rig_req = rig::completion::CompletionRequest {
                prompt,
                preamble,
                chat_history: history,
                documents: Vec::new(),
                tools,
                temperature: temp,
                max_tokens,
                additional_params: params,
            };
            match <MockCompletionModel as RigCompletionModel>::completion(&model, rig_req).await {
                Ok(resp) => {
                    for chunk in completion_response_to_chunks(resp.choice) {
                        yield Ok(chunk);
                    }
                }
                Err(e) => {
                    yield Err(crate::error::RigError::BackendError(e.to_string()));
                }
            }
        })
    });
    let backend = RigCompletionBackend::new_non_streaming(alias.into(), stream_fn);
    RigLlmClient::new(Box::new(backend))
}

/// A test-only [`Provider`] implementation that returns a [`RigLlmClient`]
/// backed by a mock Rig completion model.
///
/// Use this to exercise the full provider-registry loop
/// (`ProviderRegistry::register` → `Provider::create_client` →
/// `LlmClient::chat`) without network calls (NFR-005 / T-016).
#[derive(Clone)]
pub struct MockRigProvider {
    alias: String,
    model_id: String,
    response: MockResponse,
}

impl MockRigProvider {
    /// Build a mock Rig provider that serves the given canned response.
    #[must_use]
    pub fn new(
        alias: impl Into<String>,
        model_id: impl Into<String>,
        response: MockResponse,
    ) -> Self {
        Self {
            alias: alias.into(),
            model_id: model_id.into(),
            response,
        }
    }
}

#[async_trait::async_trait]
impl Provider for MockRigProvider {
    fn id(&self) -> &str {
        &self.alias
    }

    fn name(&self) -> &str {
        &self.alias
    }

    fn default_models(&self) -> Vec<ModelInfo> {
        vec![ModelInfo {
            id: self.model_id.clone(),
            provider_id: self.alias.clone(),
            name: format!("{} {}", self.alias, self.model_id),
            cost: Cost::default(),
            capabilities: Capabilities {
                streaming: false,
                tool_use: true,
                ..Capabilities::default()
            },
            context_window: 128_000,
            max_output: None,
            request_multiplier: None,
            thinking_config: None,
        }]
    }

    async fn create_client(
        &self,
        _api_key: &str,
        _base_url: Option<&str>,
        _options: &std::collections::HashMap<String, serde_json::Value>,
    ) -> anyhow::Result<Box<dyn LlmClient>> {
        Ok(Box::new(build_mock_llm_client(
            self.alias.clone(),
            self.response.clone(),
        )))
    }
}

// Re-export common ragent types used by tests so a test file can write
// `use ragent_rig::testing::*` and get everything it needs.
pub use ragent_llm::llm::{ChatRequest, LlmClient, StreamEvent};
pub use ragent_types::event::FinishReason;
pub use ragent_types::llm::{ChatContent, ChatMessage, ContentPart, ToolDefinition};

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    fn make_request(text: &str) -> ChatRequest {
        ChatRequest {
            model: "rig-mock".to_owned(),
            messages: Arc::new(vec![ChatMessage {
                role: "user".to_owned(),
                content: ChatContent::Text(text.to_owned()),
            }]),
            tools: Arc::new(Vec::new()),
            temperature: None,
            top_p: None,
            max_tokens: None,
            system: None,
            options: std::collections::HashMap::new(),
            session_id: None,
            request_id: None,
            stream_timeout_secs: None,
            thinking: None,
        }
    }

    // ── MockCompletionModel unit tests ─────────────────────────���────────────

    #[tokio::test]
    async fn mock_model_returns_text_response() {
        let model = MockCompletionModel::new().with_response(MockResponse::text("hello"));
        let req = rig::completion::CompletionRequest {
            prompt: rig::completion::Message::user("hi"),
            preamble: None,
            chat_history: Vec::new(),
            documents: Vec::new(),
            tools: Vec::new(),
            temperature: None,
            max_tokens: None,
            additional_params: None,
        };
        let resp = model.completion(req).await.expect("completion");
        let items: Vec<&AssistantContent> = resp.choice.iter().collect();
        assert_eq!(items.len(), 1);
        match &items[0] {
            AssistantContent::Text(t) => assert_eq!(t.text, "hello"),
            other => panic!("expected Text, got {other:?}"),
        }
        assert_eq!(model.call_count(), 1);
    }

    #[tokio::test]
    async fn mock_model_returns_tool_call_response() {
        let model = MockCompletionModel::new().with_response(MockResponse::tool_call(
            "read",
            "c1",
            serde_json::json!({"path": "x"}),
        ));
        let req = rig::completion::CompletionRequest {
            prompt: rig::completion::Message::user("hi"),
            preamble: None,
            chat_history: Vec::new(),
            documents: Vec::new(),
            tools: Vec::new(),
            temperature: None,
            max_tokens: None,
            additional_params: None,
        };
        let resp = model.completion(req).await.expect("completion");
        let items: Vec<&AssistantContent> = resp.choice.iter().collect();
        assert_eq!(items.len(), 1);
        match &items[0] {
            AssistantContent::ToolCall(tc) => {
                assert_eq!(tc.id, "c1");
                assert_eq!(tc.function.name, "read");
            }
            other => panic!("expected ToolCall, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn mock_model_cycles_responses_round_robin() {
        let model = MockCompletionModel::new()
            .with_response(MockResponse::text("first"))
            .with_response(MockResponse::text("second"));
        let req = || rig::completion::CompletionRequest {
            prompt: rig::completion::Message::user("hi"),
            preamble: None,
            chat_history: Vec::new(),
            documents: Vec::new(),
            tools: Vec::new(),
            temperature: None,
            max_tokens: None,
            additional_params: None,
        };
        let r1 = model.completion(req()).await.unwrap();
        let r2 = model.completion(req()).await.unwrap();
        let r3 = model.completion(req()).await.unwrap(); // cycles back to first
        let text_of = |r: CompletionResponse<MockRawResponse>| match &r.choice.first() {
            AssistantContent::Text(t) => t.text.clone(),
            _ => String::new(),
        };
        assert_eq!(text_of(r1), "first");
        assert_eq!(text_of(r2), "second");
        assert_eq!(text_of(r3), "first");
        assert_eq!(model.call_count(), 3);
    }

    // ── MockEmbeddingModel unit tests ───────────────────────────────────────

    #[tokio::test]
    async fn mock_embedding_model_produces_deterministic_vectors() {
        let model = MockEmbeddingModel::new(4);
        assert_eq!(model.ndims(), 4);
        let emb = model.embed_text("hello").await.expect("embed");
        assert_eq!(emb.vec, vec![5.0_f64; 4]); // "hello".len() == 5
        let embs = model
            .embed_texts(vec!["a".to_string(), "bb".to_string()])
            .await
            .expect("batch");
        assert_eq!(embs.len(), 2);
        assert_eq!(embs[0].vec, vec![1.0_f64; 4]);
        assert_eq!(embs[1].vec, vec![2.0_f64; 4]);
    }

    // ── build_mock_llm_client: end-to-end through RigLlmClient ───────────────
    //
    // These are the NFR-005 / AC-4 tests: they drive the mock through the
    // same `LlmClient::chat` entry point the ragent agent loop uses, and
    // verify the resulting `StreamEvent` sequence (text + finish, or
    // tool-call lifecycle + finish).

    #[tokio::test]
    async fn mock_llm_client_emits_text_then_finish() {
        let client = build_mock_llm_client("rig-mock", MockResponse::text("hello world"));
        assert_eq!(client.alias(), "rig-mock");
        assert!(!client.supports_streaming()); // non-streaming path
        let mut stream = client.chat(make_request("hi")).await.expect("chat");
        let mut events = Vec::new();
        while let Some(ev) = stream.next().await {
            events.push(ev);
        }
        // TextDelta("hello world") + Finish(Stop)
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], StreamEvent::TextDelta { ref text } if text == "hello world"));
        assert!(matches!(
            events[1],
            StreamEvent::Finish {
                reason: FinishReason::Stop
            }
        ));
    }

    #[tokio::test]
    async fn mock_llm_client_emits_tool_call_lifecycle_then_finish() {
        // AC-4: "verifies tool-call routing and streaming behavior".
        let client = build_mock_llm_client(
            "rig-mock",
            MockResponse::tool_call("read", "c1", serde_json::json!({"path": "x"})),
        );
        let mut stream = client.chat(make_request("use read")).await.expect("chat");
        let mut events = Vec::new();
        while let Some(ev) = stream.next().await {
            events.push(ev);
        }
        // ToolCallStart + ToolCallDelta + ToolCallEnd + Finish(ToolUse)
        assert_eq!(events.len(), 4);
        assert!(matches!(events[0], StreamEvent::ToolCallStart { ref name, .. } if name == "read"));
        assert!(matches!(events[1], StreamEvent::ToolCallDelta { .. }));
        assert!(matches!(events[2], StreamEvent::ToolCallEnd { .. }));
        assert!(matches!(
            events[3],
            StreamEvent::Finish {
                reason: FinishReason::ToolUse
            }
        ));
    }

    #[tokio::test]
    async fn mock_llm_client_multi_response_scripted_turns() {
        let client = build_mock_llm_client_multi(
            "rig-mock",
            vec![
                MockResponse::text("ack"),
                MockResponse::tool_call("read", "c1", serde_json::json!({"path": "x"})),
            ],
        );
        // Turn 1: text
        let mut s1 = client.chat(make_request("hi")).await.expect("chat");
        let mut e1 = Vec::new();
        while let Some(ev) = s1.next().await {
            e1.push(ev);
        }
        assert!(
            e1.iter()
                .any(|e| matches!(e, StreamEvent::TextDelta { text } if text == "ack"))
        );
        // Turn 2: tool call
        let mut s2 = client.chat(make_request("read x")).await.expect("chat");
        let mut e2 = Vec::new();
        while let Some(ev) = s2.next().await {
            e2.push(ev);
        }
        assert!(
            e2.iter()
                .any(|e| matches!(e, StreamEvent::ToolCallStart { name, .. } if name == "read"))
        );
    }
}
