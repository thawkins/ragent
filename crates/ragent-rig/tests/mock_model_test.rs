//! Integration test: mock-model through the full ragent agent-loop entry point.
//!
//! **Spec:** `rig` — T-014 / FR-025 / NFR-005 / AC-4.
//!
//! This test exercises a [`MockCompletionModel`] through the same
//! [`LlmClient::chat`] entry point the ragent agent loop uses. It verifies
//! both text routing and tool-call routing + streaming behavior (AC-4),
//! using only the `mock` feature (no network calls).

use futures::StreamExt;
use ragent_llm::llm::{ChatRequest, LlmClient, StreamEvent};
use ragent_rig::testing::{MockResponse, build_mock_llm_client};
use ragent_types::event::FinishReason;
use ragent_types::llm::{ChatContent, ChatMessage};
use std::collections::HashMap;
use std::sync::Arc;

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
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    }
}

/// NFR-005 / AC-4: text response routes through the agent-loop entry point
/// and produces a `TextDelta` + `Finish(Stop)` event sequence.
#[tokio::test]
async fn mock_model_text_response_through_agent_entry_point() {
    let client = build_mock_llm_client("rig-mock", MockResponse::text("hello world"));
    assert_eq!(client.alias(), "rig-mock");

    let mut stream = client.chat(make_request("hi")).await.expect("chat");
    let mut events = Vec::new();
    while let Some(ev) = stream.next().await {
        events.push(ev);
    }

    assert_eq!(events.len(), 2, "expected TextDelta + Finish");
    assert!(
        matches!(&events[0], StreamEvent::TextDelta { text } if text == "hello world"),
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

/// NFR-005 / AC-4: tool-call response routes through the agent-loop entry
/// point and produces the tool-call lifecycle triple + `Finish(ToolUse)`.
#[tokio::test]
async fn mock_model_tool_call_routing_through_agent_entry_point() {
    let client = build_mock_llm_client(
        "rig-mock",
        MockResponse::tool_call("read", "c1", serde_json::json!({"path": "x"})),
    );

    let mut stream = client.chat(make_request("read x")).await.expect("chat");
    let mut events = Vec::new();
    while let Some(ev) = stream.next().await {
        events.push(ev);
    }

    // ToolCallStart + ToolCallDelta + ToolCallEnd + Finish(ToolUse)
    assert_eq!(events.len(), 4, "expected tool-call lifecycle + Finish");
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
            &events[3],
            StreamEvent::Finish {
                reason: FinishReason::ToolUse
            }
        ),
        "expected Finish(ToolUse), got {:?}",
        events[3]
    );
}
