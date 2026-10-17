//! Inline tests for `anthropic.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn test_anthropic_model_to_info_uses_live_thinking_capability() {
    let defaults = AnthropicProvider
        .default_models()
        .into_iter()
        .map(|model| (model.id.clone(), model))
        .collect::<HashMap<_, _>>();
    let entry = json!({
        "id": "claude-sonnet-4-20250514",
        "display_name": "Claude Sonnet 4",
        "context_window": 250_000,
        "max_output_tokens": 32000,
        "capabilities": {
            "thinking": {
                "supported": true,
                "types": ["adaptive", "enabled"]
            }
        }
    });

    let model = anthropic_model_to_info(&entry, &defaults).expect("model info");
    assert!(model.capabilities.reasoning);
    assert_eq!(model.capabilities.thinking_levels, full_reasoning_levels());
    assert_eq!(model.context_window, 250_000);
    assert_eq!(model.max_output, Some(32_000));
}

// ---------------------------------------------------------------------------
// chat() / SSE parsing via a local mock server
// ---------------------------------------------------------------------------

use crate::llm::{ChatRequest, LlmClient, StreamEvent};
use ragent_types::event::FinishReason;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

fn anthropic_request() -> ChatRequest {
    ChatRequest {
        model: "claude-sonnet-4".to_string(),
        messages: Arc::new(vec![crate::llm::ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("hi".to_string()),
        }]),
        tools: Arc::new(vec![]),
        temperature: None,
        top_p: None,
        max_tokens: Some(64),
        system: None,
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: Some(5),
        thinking: None,
    }
}

async fn spawn_anthropic_sse(body: String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n{body}"
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
    });
    format!("http://{addr}")
}

fn anthropic_client(base_url: String) -> AnthropicClient {
    AnthropicClient {
        api_key: "key".to_string(),
        base_url,
        http: crate::provider::http_client::create_streaming_http_client(),
    }
}

#[tokio::test]
async fn test_anthropic_chat_text_thinking_and_usage() {
    let sse = concat!(
        "event: message_start\n",
        "data: {\"message\":{\"usage\":{\"input_tokens\":12}}}\n\n",
        "event: content_block_start\n",
        "data: {\"index\":0,\"content_block\":{\"type\":\"thinking\"}}\n\n",
        "event: content_block_delta\n",
        "data: {\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"hmm\"}}\n\n",
        "event: content_block_delta\n",
        "data: {\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\n",
        "event: message_delta\n",
        "data: {\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":5}}\n\n",
    )
    .to_string();
    let url = spawn_anthropic_sse(sse).await;
    let events: Vec<StreamEvent> = anthropic_client(url)
        .chat(anthropic_request())
        .await
        .expect("chat")
        .collect()
        .await;
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::ReasoningStart))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::ReasoningDelta { text } if text == "hmm"))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::TextDelta { text } if text == "Hello"))
    );
    assert!(events.iter().any(|e| matches!(
        e,
        StreamEvent::Usage {
            input_tokens: 12,
            output_tokens: 0
        }
    )));
    assert!(events.iter().any(|e| matches!(
        e,
        StreamEvent::Usage {
            input_tokens: 0,
            output_tokens: 5
        }
    )));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::Finish { reason } if *reason == FinishReason::Stop))
    );
}

#[tokio::test]
async fn test_anthropic_chat_tool_use_indexed_deltas() {
    let sse = concat!(
        "event: content_block_start\n",
        "data: {\"index\":2,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_1\",\"name\":\"get_weather\"}}\n\n",
        "event: content_block_delta\n",
        "data: {\"index\":2,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"city\\\":\"}}\n\n",
        "event: content_block_delta\n",
        "data: {\"index\":2,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"\\\"Paris\\\"}\"}}\n\n",
        "event: content_block_stop\n",
        "data: {\"index\":2}\n\n",
        "event: message_delta\n",
        "data: {\"delta\":{\"stop_reason\":\"tool_use\"}}\n\n",
    )
    .to_string();
    let url = spawn_anthropic_sse(sse).await;
    let events: Vec<StreamEvent> = anthropic_client(url)
        .chat(anthropic_request())
        .await
        .expect("chat")
        .collect()
        .await;
    assert!(events.iter().any(|e| matches!(
        e,
        StreamEvent::ToolCallStart { id, name } if id == "toolu_1" && name == "get_weather"
    )));
    let args: String = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::ToolCallDelta { args_json, .. } => Some(args_json.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(args, "{\"city\":\"Paris\"}");
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::ToolCallEnd { id } if id == "toolu_1"))
    );
    assert!(
        events.iter().any(
            |e| matches!(e, StreamEvent::Finish { reason } if *reason == FinishReason::ToolUse)
        )
    );
}

#[tokio::test]
async fn test_anthropic_chat_malformed_frame_and_done() {
    let sse = concat!(
        "data: {not json}\n\n",
        "event: content_block_delta\n",
        "data: {\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"ok\"}}\n\n",
        "data: [DONE]\n\n",
    )
    .to_string();
    let url = spawn_anthropic_sse(sse).await;
    let events: Vec<StreamEvent> = anthropic_client(url)
        .chat(anthropic_request())
        .await
        .expect("chat")
        .collect()
        .await;
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::TextDelta { text } if text == "ok"))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::Finish { reason } if *reason == FinishReason::Stop))
    );
}

#[test]
fn test_parse_anthropic_rate_limit_headers() {
    use reqwest::header::{HeaderMap, HeaderValue};
    let mut headers = HeaderMap::new();
    headers.insert(
        "anthropic-ratelimit-requests-limit",
        HeaderValue::from_static("1000"),
    );
    headers.insert(
        "anthropic-ratelimit-requests-remaining",
        HeaderValue::from_static("750"),
    );
    headers.insert(
        "anthropic-ratelimit-tokens-limit",
        HeaderValue::from_static("0"),
    );
    headers.insert(
        "anthropic-ratelimit-tokens-remaining",
        HeaderValue::from_static("0"),
    );
    let event = parse_anthropic_rate_limit_headers(&headers).expect("event");
    match event {
        StreamEvent::RateLimit {
            requests_used_pct,
            tokens_used_pct,
        } => {
            assert!((requests_used_pct.expect("req") - 25.0).abs() < 0.01);
            assert!((tokens_used_pct.expect("tok") - 0.0).abs() < 0.01);
        }
        other => panic!("unexpected: {other:?}"),
    }
    assert!(parse_anthropic_rate_limit_headers(&HeaderMap::new()).is_none());
}

#[test]
fn test_anthropic_build_request_body_parts_and_images() {
    use crate::llm::ContentPart;
    let client = AnthropicClient {
        api_key: "k".to_string(),
        base_url: "http://x".to_string(),
        http: crate::provider::http_client::create_http_client(),
    };
    let req = ChatRequest {
        model: "claude".to_string(),
        messages: Arc::new(vec![crate::llm::ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Parts(vec![
                ContentPart::Text {
                    text: "look".to_string(),
                },
                ContentPart::ImageUrl {
                    url: "data:image/jpeg;base64,QUJD".to_string(),
                },
                ContentPart::ToolResult {
                    tool_use_id: "t1".to_string(),
                    content: Arc::from("done"),
                },
            ]),
        }]),
        tools: Arc::new(vec![]),
        temperature: Some(0.2),
        top_p: Some(0.8),
        max_tokens: Some(32),
        system: Some(Arc::from("sys")),
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    };
    let body = client.build_request_body(&req);
    assert_eq!(body["system"], "sys");
    assert!((body["temperature"].as_f64().expect("t") - 0.2).abs() < 1e-6);
    assert!((body["top_p"].as_f64().expect("p") - 0.8).abs() < 1e-6);
    assert_eq!(body["max_tokens"], 32);
    let parts = &body["messages"][0]["content"];
    assert_eq!(parts[0]["type"], "text");
    assert_eq!(parts[1]["type"], "image");
    assert_eq!(parts[1]["source"]["media_type"], "image/jpeg");
    assert_eq!(parts[1]["source"]["data"], "QUJD");
    assert_eq!(parts[2]["type"], "tool_result");
}
