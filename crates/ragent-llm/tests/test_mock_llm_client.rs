#![allow(clippy::assert_is_empty)]
//! External tests for `tests` from `crates/ragent-llm/src/providers/mock_llm_client.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use futures::StreamExt;
use ragent_llm::llm::{ChatRequest, LlmClient, StreamEvent};
use ragent_llm::providers::mock_llm_client::{MockLlmClient, MockScenario};
use ragent_types::event::FinishReason;
use std::sync::Arc;

#[test]
fn scenario_as_str_is_stable() {
    assert_eq!(MockScenario::SimpleTextReply.as_str(), "simple_text_reply");
    assert_eq!(MockScenario::SingleToolCall.as_str(), "single_tool_call");
    assert_eq!(MockScenario::MultiStepLoop.as_str(), "multi_step_loop");
    assert_eq!(MockScenario::Empty.as_str(), "empty");
}

#[test]
fn default_scenario_is_simple_text_reply() {
    assert_eq!(MockScenario::default(), MockScenario::SimpleTextReply);
    let client = MockLlmClient::new();
    assert_eq!(client.scenario(), MockScenario::SimpleTextReply);
}

#[tokio::test]
async fn simple_text_reply_emits_five_deltas_and_finish() {
    let client = MockLlmClient::with_scenario(MockScenario::SimpleTextReply);
    let events = collect_events(client).await;
    // 5 text deltas + usage + finish = 7
    assert_eq!(events.len(), 7);
    let text: String = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::TextDelta { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(text, "Hello, world from the mock LLM client. Bye!\n");
    assert!(matches!(
        events.last().unwrap(),
        StreamEvent::Finish {
            reason: FinishReason::Stop
        }
    ));
}

#[tokio::test]
async fn single_tool_call_emits_start_delta_end_and_tool_use_finish() {
    let client = MockLlmClient::with_scenario(MockScenario::SingleToolCall);
    let events = collect_events(client).await;
    assert_eq!(events.len(), 5);
    assert!(matches!(events[0], StreamEvent::ToolCallStart { .. }));
    assert!(matches!(events[1], StreamEvent::ToolCallDelta { .. }));
    assert!(matches!(events[2], StreamEvent::ToolCallEnd { .. }));
    assert!(matches!(
        events.last().unwrap(),
        StreamEvent::Finish {
            reason: FinishReason::ToolUse
        }
    ));
}

#[tokio::test]
async fn multi_step_loop_emits_two_tool_calls() {
    let client = MockLlmClient::with_scenario(MockScenario::MultiStepLoop);
    let events = collect_events(client).await;
    assert_eq!(events.len(), 7);
    let start_count = events
        .iter()
        .filter(|e| matches!(e, StreamEvent::ToolCallStart { .. }))
        .count();
    assert_eq!(start_count, 2);
}

#[tokio::test]
async fn empty_scenario_emits_only_finish() {
    let client = MockLlmClient::with_scenario(MockScenario::Empty);
    let events = collect_events(client).await;
    assert_eq!(events.len(), 1);
    assert!(matches!(
        events[0],
        StreamEvent::Finish {
            reason: FinishReason::Stop
        }
    ));
}

#[tokio::test]
async fn request_count_increments_per_chat_call() {
    let client = MockLlmClient::with_scenario(MockScenario::Empty);
    assert_eq!(client.request_count(), 0);
    let _ = collect_events(client.clone()).await;
    assert_eq!(client.request_count(), 1);
    let _ = collect_events(client.clone()).await;
    assert_eq!(client.request_count(), 2);
}

#[tokio::test]
async fn events_are_deterministic_across_runs() {
    // Two independent clients with the same scenario must produce
    // byte-identical event sequences.
    let a = collect_events(MockLlmClient::with_scenario(MockScenario::SimpleTextReply)).await;
    let b = collect_events(MockLlmClient::with_scenario(MockScenario::SimpleTextReply)).await;
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(b.iter()) {
        assert_eq!(format!("{x:?}"), format!("{y:?}"));
    }
}

/// Drive a `MockLlmClient` to exhaustion and return its event sequence.
async fn collect_events(client: MockLlmClient) -> Vec<StreamEvent> {
    let request = ChatRequest {
        model: "mock-model".to_string(),
        messages: Arc::new(vec![]),
        tools: Arc::new(vec![]),
        temperature: None,
        top_p: None,
        max_tokens: None,
        system: None,
        options: std::collections::HashMap::new(),
        thinking: None,
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
    };
    let mut stream = client.chat(request).await.expect("chat should start");
    let mut out = Vec::new();
    while let Some(event) = stream.next().await {
        out.push(event);
    }
    out
}
