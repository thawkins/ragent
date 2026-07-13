//! Integration tests for the Rig provider loop (T-016 / NFR-005).
//!
//! These tests exercise the full provider-registry path that the ragent agent
//! loop uses at runtime:
//!
//! ```text
//! ProviderRegistry::register
//!     -> Provider::create_client
//!         -> LlmClient::chat
//!             -> StreamEvent
//! ```
//!
//! A deterministic [`MockRigProvider`] stands in for a real Rig-backed provider
//! so the tests run without network access and verify that the Rig adapter is
//! indistinguishable from a native provider at the registry boundary (FR-012).

use futures::StreamExt;
use ragent_llm::llm::{ChatRequest, StreamEvent};
use ragent_llm::provider::{Provider, ProviderRegistry};
use ragent_rig::testing::{MockResponse, MockRigProvider};
use ragent_types::event::FinishReason;
use ragent_types::llm::{ChatContent, ChatMessage};
use std::collections::HashMap;
use std::sync::Arc;

fn make_request(text: &str) -> ChatRequest {
    ChatRequest {
        model: "mock-model".to_owned(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_owned(),
            content: ChatContent::Text(text.to_owned()),
        }]),
        tools: Arc::new(Vec::new()),
        temperature: None,
        top_p: None,
        max_tokens: None,
        system: None,
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    }
}

/// NFR-005 / T-016: a mock Rig provider registered in `ProviderRegistry` can
/// create an `LlmClient` and produce a text response + `Finish(Stop)` stream.
#[tokio::test]
async fn mock_rig_provider_text_response_through_registry() {
    let mut registry = ProviderRegistry::new();
    registry.register(Box::new(MockRigProvider::new(
        "rig-mock",
        "mock-model",
        MockResponse::text("hello from provider loop"),
    )));

    let provider: &dyn Provider = registry
        .get("rig-mock")
        .expect("rig-mock provider should be registered");
    assert_eq!(provider.id(), "rig-mock");
    let models = provider.default_models();
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].id, "mock-model");

    let client = provider
        .create_client("", None, &HashMap::new())
        .await
        .expect("create_client");

    let mut stream = client.chat(make_request("hi")).await.expect("chat");
    let mut events = Vec::new();
    while let Some(ev) = stream.next().await {
        events.push(ev);
    }

    assert_eq!(events.len(), 2, "expected TextDelta + Finish");
    assert!(
        matches!(&events[0], StreamEvent::TextDelta { text } if text == "hello from provider loop"),
        "first event should be TextDelta, got {:?}",
        events[0]
    );
    assert!(
        matches!(
            events[1],
            StreamEvent::Finish {
                reason: FinishReason::Stop
            }
        ),
        "second event should be Finish(Stop), got {:?}",
        events[1]
    );
}

/// NFR-005 / T-016: a mock Rig provider registered in `ProviderRegistry` can
/// route a tool-call response through the full provider loop, emitting the
/// tool-call lifecycle triple + `Finish(ToolUse)`.
#[tokio::test]
async fn mock_rig_provider_tool_call_through_registry() {
    let mut registry = ProviderRegistry::new();
    registry.register(Box::new(MockRigProvider::new(
        "rig-mock",
        "mock-model",
        MockResponse::tool_call("read", "c1", serde_json::json!({"path": "x"})),
    )));

    let provider: &dyn Provider = registry
        .get("rig-mock")
        .expect("rig-mock provider should be registered");
    let client = provider
        .create_client("", None, &HashMap::new())
        .await
        .expect("create_client");

    let mut stream = client.chat(make_request("read x")).await.expect("chat");
    let mut events = Vec::new();
    while let Some(ev) = stream.next().await {
        events.push(ev);
    }

    assert_eq!(
        events.len(),
        4,
        "expected ToolCallStart + ToolCallDelta + ToolCallEnd + Finish"
    );
    assert!(
        matches!(&events[0], StreamEvent::ToolCallStart { name, .. } if name == "read"),
        "expected ToolCallStart(read), got {:?}",
        events[0]
    );
    assert!(
        matches!(&events[1], StreamEvent::ToolCallDelta { .. }),
        "expected ToolCallDelta, got {:?}",
        events[1]
    );
    assert!(
        matches!(&events[2], StreamEvent::ToolCallEnd { .. }),
        "expected ToolCallEnd, got {:?}",
        events[2]
    );
    assert!(
        matches!(
            events[3],
            StreamEvent::Finish {
                reason: FinishReason::ToolUse
            }
        ),
        "expected Finish(ToolUse), got {:?}",
        events[3]
    );
}
