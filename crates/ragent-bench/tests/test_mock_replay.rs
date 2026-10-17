//! Replay tests for the hermetic benchmark mock LLM client (`mock.rs`).
//!
//! `MockLlmScript` / `MockLlmClient` were entirely untested; these cover the
//! script constructors, `events()` inspection, both client builders, the
//! `Arc<dyn LlmClient>` convenience factory, and one full `chat()` replay.
//!
//! `StreamEvent` does not implement `PartialEq`, so assertions match variants
//! and compare their fields rather than whole values.

use futures::StreamExt;
use ragent_bench::{MockLlmClient, MockLlmScript, mock_llm_client};
use ragent_llm::llm::{ChatContent, ChatMessage, ChatRequest, LlmClient};
use ragent_types::StreamEvent;
use ragent_types::event::FinishReason;
use std::collections::HashMap;
use std::sync::Arc;

/// Build a minimal `ChatRequest` to drive `chat()`.
fn demo_request() -> ChatRequest {
    ChatRequest {
        model: "demo".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("hello".to_string()),
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

/// Replay a client's script to completion via the public `chat()` path.
fn replay(client: &MockLlmClient) -> Vec<StreamEvent> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        let mut stream = client.chat(demo_request()).await.expect("chat");
        let mut collected = Vec::new();
        while let Some(event) = stream.next().await {
            collected.push(event);
        }
        collected
    })
}

#[test]
fn test_mock_script_new_preserves_event_sequence_length() {
    let events = vec![
        StreamEvent::TextDelta {
            text: "a".to_string(),
        },
        StreamEvent::Finish {
            reason: FinishReason::Stop,
        },
    ];
    let script = MockLlmScript::new(events.clone());
    assert_eq!(script.events().len(), 2);
    assert!(matches!(
        &script.events()[0],
        StreamEvent::TextDelta { text } if text == "a"
    ));
    assert!(matches!(
        &script.events()[1],
        StreamEvent::Finish {
            reason: FinishReason::Stop
        }
    ));
}

#[test]
fn test_mock_script_text_only_emits_delta_usage_and_stop() {
    let script = MockLlmScript::text_only("hello world");
    let events = script.events();
    assert_eq!(events.len(), 3);
    assert!(matches!(
        &events[0],
        StreamEvent::TextDelta { text } if text == "hello world"
    ));
    assert!(matches!(
        &events[1],
        StreamEvent::Usage {
            input_tokens: 16,
            output_tokens: 8
        }
    ));
    assert!(matches!(
        &events[2],
        StreamEvent::Finish {
            reason: FinishReason::Stop
        }
    ));
}

#[test]
fn test_mock_script_single_tool_call_emits_start_delta_end() {
    let script = MockLlmScript::single_tool_call("echo", "{\"k\":1}");
    let events = script.events();
    assert_eq!(events.len(), 5);
    assert!(matches!(
        &events[0],
        StreamEvent::ToolCallStart { id, name } if id == "call-1" && name == "echo"
    ));
    assert!(matches!(
        &events[1],
        StreamEvent::ToolCallDelta { id, args_json }
            if id == "call-1" && args_json == "{\"k\":1}"
    ));
    assert!(matches!(
        &events[2],
        StreamEvent::ToolCallEnd { id } if id == "call-1"
    ));
    assert!(matches!(
        &events[4],
        StreamEvent::Finish {
            reason: FinishReason::ToolUse
        }
    ));
}

#[test]
fn test_mock_client_constructors_delegate_to_script() {
    let from_new = replay(&MockLlmClient::new(MockLlmScript::text_only("x")));
    let from_text = replay(&MockLlmClient::text_only("x"));
    let from_tool = replay(&MockLlmClient::single_tool_call("echo", "{}"));

    for events in [&from_new, &from_text] {
        assert_eq!(events.len(), 3);
        assert!(matches!(
            &events[0],
            StreamEvent::TextDelta { text } if text == "x"
        ));
    }
    assert_eq!(from_tool.len(), 5);
    assert!(matches!(
        &from_tool[0],
        StreamEvent::ToolCallStart { name, .. } if name == "echo"
    ));
}

#[test]
fn test_mock_llm_client_factory_returns_replayable_arc() {
    let client = mock_llm_client(MockLlmScript::text_only("factory"));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let events = runtime.block_on(async {
        let mut stream = client.chat(demo_request()).await.expect("chat");
        let mut collected = Vec::new();
        while let Some(event) = stream.next().await {
            collected.push(event);
        }
        collected
    });
    assert_eq!(events.len(), 3);
    assert!(matches!(
        &events[0],
        StreamEvent::TextDelta { text } if text == "factory"
    ));
}

#[test]
fn test_mock_client_chat_replays_full_script_verbatim() {
    let script = MockLlmScript::single_tool_call("grep", "{\"pattern\":\"x\"}");
    let client = MockLlmClient::new(script);
    let events = replay(&client);
    assert_eq!(events.len(), 5);
    assert!(matches!(
        &events[1],
        StreamEvent::ToolCallDelta { args_json, .. } if args_json == "{\"pattern\":\"x\"}"
    ));
    // A second `chat()` on the same client must replay the same script.
    let second = replay(&client);
    assert_eq!(second.len(), 5);
    assert!(matches!(
        &second[0],
        StreamEvent::ToolCallStart { name, .. } if name == "grep"
    ));
}
