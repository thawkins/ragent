//! Extra coverage for the OpenAI-compatible client: request-body construction,
//! model discovery, and the streaming `chat()` path driven against a local
//! one-shot mock server (no real network egress).

use std::collections::HashMap;
use std::sync::Arc;

use futures::StreamExt;
use ragent_llm::llm::{ChatContent, ChatMessage, ChatRequest, ContentPart, LlmClient, StreamEvent};
use ragent_llm::provider::openai::{OpenAiClient, discover_openai_models};
use ragent_types::event::FinishReason;
use serde_json::json;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

fn request(model: &str) -> ChatRequest {
    ChatRequest {
        model: model.to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("hi".to_string()),
        }]),
        tools: Arc::new(vec![]),
        temperature: None,
        top_p: None,
        max_tokens: Some(64),
        system: Some(Arc::from("system")),
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: Some(5),
        thinking: None,
    }
}

#[test]
fn test_openai_client_accessors_and_request_body() {
    let client = OpenAiClient::new("key-123", "https://api.openai.com/");
    assert_eq!(client.api_key(), "key-123");
    assert_eq!(client.base_url(), "https://api.openai.com");

    let mut req = request("gpt-4o");
    req.temperature = Some(0.4);
    req.top_p = Some(0.9);
    req.max_tokens = Some(200);
    let body = client.build_request_body(&req);
    assert_eq!(body["model"], "gpt-4o");
    assert_eq!(body["stream"], true);
    assert_eq!(body["stream_options"], json!({"include_usage": true}));
    assert_eq!(body["max_tokens"], 200);
    assert!(body["temperature"].as_f64().expect("t") - 0.4 < 1e-6);
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][0]["content"], "system");
    assert_eq!(body["messages"][1]["content"], "hi");
}

#[test]
fn test_openai_request_body_tool_use_and_result_packing() {
    let client = OpenAiClient::new("k", "https://api.openai.com");
    let mut req = request("gpt-4o");
    req.system = None;
    req.messages = Arc::new(vec![
        ChatMessage {
            role: "assistant".to_string(),
            content: ChatContent::Parts(vec![ContentPart::ToolUse {
                id: "call_1".to_string(),
                name: "get_weather".to_string(),
                input: json!({"city": "Paris"}),
            }]),
        },
        ChatMessage {
            role: "tool".to_string(),
            content: ChatContent::Parts(vec![ContentPart::ToolResult {
                tool_use_id: "call_1".to_string(),
                content: Arc::from("sunny"),
            }]),
        },
    ]);
    let body = client.build_request_body(&req);
    assert_eq!(body["messages"][0]["tool_calls"][0]["id"], "call_1");
    assert_eq!(
        body["messages"][0]["tool_calls"][0]["function"]["name"],
        "get_weather"
    );
    // arguments are serialised as a JSON string in the OpenAI dialect
    let args = body["messages"][0]["tool_calls"][0]["function"]["arguments"]
        .as_str()
        .expect("args string");
    assert!(args.contains("\"city\""));
    assert_eq!(body["messages"][1]["role"], "tool");
    assert_eq!(body["messages"][1]["tool_call_id"], "call_1");
}

#[test]
fn test_openai_request_body_reasoning_effort_tail() {
    let client = OpenAiClient::new("k", "https://api.openai.com");
    let mut req = request("o3-mini");
    req.options
        .insert("reasoning_effort".to_string(), json!("high"));
    let body = client.build_request_body(&req);
    assert_eq!(body["reasoning_effort"], "high");
}

#[test]
fn test_openai_collapses_single_text_part() {
    let client = OpenAiClient::new("k", "https://api.openai.com");
    let mut req = request("gpt-4o");
    req.system = None;
    req.messages = Arc::new(vec![ChatMessage {
        role: "user".to_string(),
        content: ChatContent::Parts(vec![ContentPart::Text {
            text: "solo".to_string(),
        }]),
    }]);
    let body = client.build_request_body(&req);
    // A single text part collapses to a bare string.
    assert_eq!(body["messages"][0]["content"], "solo");
}

async fn spawn_json(body: String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
    });
    format!("http://{addr}")
}

#[tokio::test]
async fn test_discover_openai_models_filters_non_chat_and_maps_capabilities() {
    let payload = json!({
        "data": [
            {"id": "gpt-4o"},
            {"id": "gpt-4o-mini"},
            {"id": "o3-mini"},
            {"id": "text-embedding-3-large"},
            {"id": "whisper-1"},
            {"id": "dall-e-3"},
            {"id": "gpt-3.5-turbo-instruct"},
            {"id": "omni-moderation-latest"}
        ]
    })
    .to_string();
    let url = spawn_json(payload).await;
    let models = discover_openai_models("k", &url, "openai")
        .await
        .expect("models");
    let ids: Vec<String> = models.iter().map(|m| m.id.clone()).collect();
    assert!(ids.contains(&"gpt-4o".to_string()));
    assert!(ids.contains(&"gpt-4o-mini".to_string()));
    assert!(ids.contains(&"o3-mini".to_string()));
    // Non-chat models are filtered out.
    assert!(!ids.iter().any(|id| id.contains("embedding")));
    assert!(!ids.iter().any(|id| id.contains("whisper")));
    assert!(!ids.iter().any(|id| id.contains("dall")));
    assert!(!ids.iter().any(|id| id.contains("instruct")));
    assert!(!ids.iter().any(|id| id.contains("moderation")));

    let gpt4o = models.iter().find(|m| m.id == "gpt-4o").expect("gpt-4o");
    assert_eq!(gpt4o.provider_id, "openai");
    assert!(gpt4o.capabilities.vision, "gpt-4o should be vision-capable");
    assert!(gpt4o.capabilities.streaming);
    assert!(gpt4o.capabilities.tool_use);

    let o3 = models.iter().find(|m| m.id == "o3-mini").expect("o3-mini");
    assert!(o3.capabilities.reasoning, "o3-mini should be reasoning");
}

async fn spawn_sse(body: String, extra_headers: &str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let extra = extra_headers.to_string();
    tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n{extra}Connection: close\r\n\r\n{body}"
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
    });
    format!("http://{addr}")
}

fn openai_client(url: String) -> OpenAiClient {
    OpenAiClient::new_with_provider("k", &url, "openai")
}

#[tokio::test]
async fn test_openai_chat_stream_text_tool_usage_and_finish() {
    let sse = concat!(
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hel\"}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"c1\",\"function\":{\"name\":\"calc\"}}]}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"a\\\":1}\"}}]}}]}\n\n",
        "data: {\"usage\":{\"prompt_tokens\":4,\"completion_tokens\":2},\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n",
    )
    .to_string();
    let url = spawn_sse(sse, "").await;
    let events: Arc<Vec<StreamEvent>> = Arc::new(
        openai_client(url)
            .chat(request("gpt-4o"))
            .await
            .expect("chat")
            .collect()
            .await,
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::TextDelta { text } if text == "Hel"))
    );
    assert!(events.iter().any(|e| matches!(
        e,
        StreamEvent::ToolCallStart { id, name } if id == "c1" && name == "calc"
    )));
    assert!(events.iter().any(|e| matches!(
        e,
        StreamEvent::ToolCallDelta { args_json, .. } if args_json == "{\"a\":1}"
    )));
    assert!(
        events.iter().any(
            |e| matches!(e, StreamEvent::Finish { reason } if *reason == FinishReason::ToolUse)
        )
    );
    assert!(events.iter().any(|e| matches!(
        e,
        StreamEvent::Usage {
            input_tokens: 4,
            output_tokens: 2
        }
    )));
}

#[tokio::test]
async fn test_openai_chat_emits_rate_limit_from_headers() {
    let sse = "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"x\"},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n".to_string();
    let headers = concat!(
        "x-ratelimit-limit-requests: 100\r\n",
        "x-ratelimit-remaining-requests: 25\r\n",
        "x-ratelimit-limit-tokens: 1000\r\n",
        "x-ratelimit-remaining-tokens: 500\r\n",
    );
    let url = spawn_sse(sse, headers).await;
    let events: Vec<StreamEvent> = openai_client(url)
        .chat(request("gpt-4o"))
        .await
        .expect("chat")
        .collect()
        .await;
    let rl = events
        .iter()
        .find_map(|e| match e {
            StreamEvent::RateLimit {
                requests_used_pct,
                tokens_used_pct,
            } => Some((*requests_used_pct, *tokens_used_pct)),
            _ => None,
        })
        .expect("rate limit event");
    assert!((rl.0.expect("req") - 75.0).abs() < 0.5);
    assert!((rl.1.expect("tok") - 50.0).abs() < 0.5);
}

#[tokio::test]
async fn test_openai_chat_error_status_is_surfaced() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        let body = "{\"error\":{\"message\":\"quota exceeded\"}}";
        let response = format!(
            "HTTP/1.1 429 Too Many Requests\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
    });
    let err = match openai_client(format!("http://{addr}"))
        .chat(request("gpt-4o"))
        .await
    {
        Ok(_) => panic!("expected error"),
        Err(e) => e.to_string(),
    };
    assert!(err.contains("429"), "error: {err}");
    assert!(err.contains("quota exceeded"), "error: {err}");
}
