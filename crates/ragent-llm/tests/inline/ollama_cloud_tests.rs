//! Inline tests for `ollama_cloud.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use serde_json::json;
use std::sync::Arc;

#[test]
fn test_provider_defaults() {
    let provider = OllamaCloudProvider::new();
    assert_eq!(provider.id(), "ollama_cloud");
    assert_eq!(provider.name(), "Ollama Cloud");
    assert!(provider.default_models().is_empty());
}

#[test]
fn test_with_custom_url() {
    let provider = OllamaCloudProvider::with_url("https://example.com/");
    assert_eq!(provider.base_url, "https://example.com");
}

#[test]
fn test_context_length_parses_top_level_string_fields() {
    let response: OllamaShowResponse = serde_json::from_value(json!({
        "context_length": "1048576",
        "capabilities": []
    }))
    .expect("show response should parse");

    assert_eq!(response.context_length(), Some(1_048_576));
}

#[test]
fn test_context_length_parses_alternate_model_info_keys() {
    let response: OllamaShowResponse = serde_json::from_value(json!({
        "model_info": {
            "llama.context_window": 1_048_576
        },
        "capabilities": []
    }))
    .expect("show response should parse");

    assert_eq!(response.context_length(), Some(1_048_576));
}

#[test]
fn test_has_thinking_from_capabilities() {
    let response: OllamaShowResponse = serde_json::from_value(json!({
        "capabilities": ["vision", "thinking"]
    }))
    .expect("show response should parse");

    assert!(response.has_thinking());
}

#[test]
fn test_has_thinking_from_template_markers() {
    // Template with <!-- think --> marker
    let response: OllamaShowResponse = serde_json::from_value(json!({
        "template": "Some text <!-- think --> thinking block {{ .Content }}",
        "capabilities": []
    }))
    .expect("show response should parse");

    assert!(response.has_thinking());
}

#[test]
fn test_has_thinking_from_template_go_template() {
    // Template with Go template {{--think}} marker
    let response: OllamaShowResponse = serde_json::from_value(json!({
        "template": "{{--think}}\n{{ .Content }}",
        "capabilities": []
    }))
    .expect("show response should parse");

    assert!(response.has_thinking());
}

#[test]
fn test_has_thinking_false_when_no_indicators() {
    let response: OllamaShowResponse = serde_json::from_value(json!({
        "template": "{{ .Content }}",
        "capabilities": ["vision"]
    }))
    .expect("show response should parse");

    assert!(!response.has_thinking());
}

// ---------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------

#[test]
fn test_parse_usize_value_numeric_and_string() {
    assert_eq!(parse_usize_value(&json!(4096)), Some(4096));
    assert_eq!(parse_usize_value(&json!("8192")), Some(8192));
    assert_eq!(parse_usize_value(&json!("  2048  ")), Some(2048));
    assert_eq!(parse_usize_value(&json!("")), None);
    assert_eq!(parse_usize_value(&json!("not-a-number")), None);
    assert_eq!(parse_usize_value(&json!(true)), None);
}

#[test]
fn test_infer_parameter_size_from_tag() {
    assert_eq!(
        infer_parameter_size("llama3.3:70b"),
        Some("70B".to_string())
    );
    // The scanner stops at the first non-digit once a digit has been seen, so
    // only the integer prefix of a decimal tag is captured.
    assert_eq!(infer_parameter_size("qwen2.5:0.5b"), Some("0".to_string()));
    assert_eq!(
        infer_parameter_size("gemma2:9b-instruct"),
        Some("9B".to_string())
    );
    assert_eq!(infer_parameter_size("mistral:7B"), Some("7B".to_string()));
    // No tag
    assert_eq!(infer_parameter_size("llama3.3"), None);
    // Tag with no leading digits
    assert_eq!(infer_parameter_size("model:latest"), None);
}

#[test]
fn test_format_model_name_uses_details_or_inferred_size() {
    let with_details = OllamaModelDetails {
        parameter_size: "13B".to_string(),
        family: "llama".to_string(),
    };
    assert_eq!(
        format_model_name("llama2:13b", &with_details),
        "llama2:13b (13B)"
    );

    let empty = OllamaModelDetails::default();
    assert_eq!(format_model_name("qwen2.5:7b", &empty), "qwen2.5:7b (7B)");
    assert_eq!(
        format_model_name("mystery:latest", &empty),
        "mystery:latest"
    );
}

#[test]
fn test_has_vision_flag() {
    let yes: OllamaShowResponse = serde_json::from_value(json!({
        "capabilities": ["vision", "thinking"]
    }))
    .expect("parse");
    assert!(yes.has_vision());

    let no: OllamaShowResponse =
        serde_json::from_value(json!({"capabilities": ["thinking"]})).expect("parse");
    assert!(!no.has_vision());
}

#[test]
fn test_context_length_precedence_top_level_over_model_info() {
    let response: OllamaShowResponse = serde_json::from_value(json!({
        "context_length": 4096,
        "model_info": { "llama.context_length": 9999 },
        "capabilities": []
    }))
    .expect("parse");
    assert_eq!(response.context_length(), Some(4096));
}

#[test]
fn test_context_length_unknown_returns_none() {
    let response: OllamaShowResponse =
        serde_json::from_value(json!({"capabilities": []})).expect("parse");
    assert_eq!(response.context_length(), None);
}

// ---------------------------------------------------------------------------
// build_request_body
// ---------------------------------------------------------------------------

fn cloud_client() -> OllamaCloudClient {
    OllamaCloudClient {
        api_key: "k".to_string(),
        base_url: "https://ollama.com".to_string(),
        http: crate::provider::http_client::create_http_client(),
    }
}

fn cloud_request(
    messages: Vec<crate::llm::ChatMessage>,
    tools: Vec<ToolDefinition>,
) -> ChatRequest {
    ChatRequest {
        model: "qwen3:8b".to_string(),
        messages: Arc::new(messages),
        tools: Arc::new(tools),
        temperature: Some(0.5),
        top_p: Some(0.9),
        max_tokens: Some(256),
        system: Some(Arc::from("be helpful")),
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    }
}

#[test]
fn test_build_request_body_system_text_and_sampling() {
    let messages = vec![crate::llm::ChatMessage {
        role: "user".to_string(),
        content: ChatContent::Text("hello".to_string()),
    }];
    let body = cloud_client().build_request_body(&cloud_request(messages, vec![]), &[]);
    assert_eq!(body["model"], "qwen3:8b");
    assert_eq!(body["stream"], true);
    assert_eq!(body["temperature"], 0.5);
    assert!(
        (body["top_p"].as_f64().expect("top_p") - 0.9).abs() < 1e-6,
        "top_p should round-trip to ~0.9"
    );
    assert_eq!(body["max_tokens"], 256);
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][0]["content"], "be helpful");
    assert_eq!(body["messages"][1]["role"], "user");
    assert_eq!(body["messages"][1]["content"], "hello");
}

#[test]
fn test_build_request_body_tool_use_and_result_pairing() {
    use crate::llm::ContentPart;
    let messages = vec![
        crate::llm::ChatMessage {
            role: "assistant".to_string(),
            content: ChatContent::Parts(vec![ContentPart::ToolUse {
                id: "call_9".to_string(),
                name: "get_weather".to_string(),
                input: json!({"loc": "London"}),
            }]),
        },
        crate::llm::ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Parts(vec![ContentPart::ToolResult {
                tool_use_id: "call_9".to_string(),
                content: Arc::from("sunny"),
                is_error: false,
            }]),
        },
    ];
    let body = cloud_client().build_request_body(&cloud_request(messages, vec![]), &[]);
    let assistant = &body["messages"][1];
    assert_eq!(assistant["role"], "assistant");
    assert_eq!(
        assistant["tool_calls"][0]["function"]["name"],
        "get_weather"
    );
    assert_eq!(
        assistant["tool_calls"][0]["function"]["arguments"]["loc"],
        "London"
    );
    let tool = &body["messages"][2];
    assert_eq!(tool["role"], "tool");
    assert_eq!(tool["name"], "get_weather");
    assert_eq!(tool["content"], "sunny");
}

#[test]
fn test_build_request_body_only_last_user_carries_images() {
    use crate::llm::ContentPart;
    let mk = |url: &str| {
        ChatContent::Parts(vec![ContentPart::ImageUrl {
            url: url.to_string(),
        }])
    };
    let messages = vec![
        crate::llm::ChatMessage {
            role: "user".to_string(),
            content: mk("data:image/png;base64,OLDB64"),
        },
        crate::llm::ChatMessage {
            role: "user".to_string(),
            content: mk("data:image/png;base64,NEWB64"),
        },
    ];
    let body = cloud_client().build_request_body(&cloud_request(messages, vec![]), &[]);
    // First (historical) user message must NOT carry images.
    assert!(body["messages"][1].get("images").is_none());
    // Last user message strips the data-URL prefix.
    assert_eq!(body["messages"][2]["images"][0], "NEWB64");
}

#[test]
fn test_build_request_body_think_flag_when_thinking_enabled() {
    let mut req = cloud_request(
        vec![crate::llm::ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("hi".to_string()),
        }],
        vec![],
    );
    req.thinking = Some(ThinkingConfig::new(ragent_types::ThinkingLevel::High));
    let body = cloud_client().build_request_body(&req, &[]);
    assert_eq!(body["think"], true);
}

// ---------------------------------------------------------------------------
// chat() NDJSON stream via a local mock server
// ---------------------------------------------------------------------------

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

async fn spawn_cloud_body(body: String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nConnection: close\r\n\r\n{body}"
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
    });
    format!("http://{addr}")
}

async fn spawn_cloud_status(status: u16, body: String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        let response = format!(
            "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
    });
    format!("http://{addr}")
}

fn cloud_client_at(url: String) -> OllamaCloudClient {
    OllamaCloudClient {
        api_key: "k".to_string(),
        base_url: url,
        http: crate::provider::http_client::create_streaming_http_client(),
    }
}

fn simple_cloud_request() -> ChatRequest {
    cloud_request(
        vec![crate::llm::ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("hi".to_string()),
        }],
        vec![],
    )
}

#[tokio::test]
async fn test_cloud_chat_response_text_and_done_usage() {
    use crate::llm::LlmClient;
    let ndjson = concat!(
        "{\"message\":{\"role\":\"assistant\",\"content\":\"Hel\"},\"done\":false}\n",
        "{\"message\":{\"role\":\"assistant\",\"content\":\"lo\"},\"done\":false}\n",
        "{\"done\":true,\"done_reason\":\"stop\",\"prompt_eval_count\":11,\"eval_count\":4}\n",
    )
    .to_string();
    let url = spawn_cloud_body(ndjson).await;
    let events: Vec<StreamEvent> = cloud_client_at(url)
        .chat(simple_cloud_request())
        .await
        .expect("chat")
        .collect()
        .await;
    let text: String = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::TextDelta { text } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(text, "Hello");
    assert!(events.iter().any(|e| matches!(
        e,
        StreamEvent::Usage {
            input_tokens: 11,
            output_tokens: 4
        }
    )));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::Finish { reason } if *reason == FinishReason::Stop))
    );
}

#[tokio::test]
async fn test_cloud_chat_thinking_and_tool_calls() {
    use crate::llm::LlmClient;
    let ndjson = concat!(
        "{\"message\":{\"thinking\":\"pondering\"},\"done\":false}\n",
        "{\"message\":{\"tool_calls\":[{\"id\":\"c1\",\"function\":{\"name\":\"read\",\"arguments\":{\"path\":\"a\"}}}]},\"done\":false}\n",
        "{\"done\":true,\"done_reason\":\"tool_calls\"}\n",
    )
    .to_string();
    let url = spawn_cloud_body(ndjson).await;
    let events: Vec<StreamEvent> = cloud_client_at(url)
        .chat(simple_cloud_request())
        .await
        .expect("chat")
        .collect()
        .await;
    assert!(matches!(events[0], StreamEvent::ReasoningStart));
    assert!(matches!(&events[1], StreamEvent::ReasoningDelta { text } if text == "pondering"));
    assert!(matches!(events[2], StreamEvent::ReasoningEnd));
    assert!(events.iter().any(
        |e| matches!(e, StreamEvent::ToolCallStart { id, name } if id == "c1" && name == "read")
    ));
    assert!(
        events.iter().any(
            |e| matches!(e, StreamEvent::Finish { reason } if *reason == FinishReason::ToolUse)
        )
    );
}

#[tokio::test]
async fn test_cloud_chat_http_error_surfaces_body() {
    use crate::llm::LlmClient;
    let url = spawn_cloud_status(500, r#"{"error":"exploded"}"#.to_string()).await;
    let err = match cloud_client_at(url).chat(simple_cloud_request()).await {
        Ok(_) => panic!("expected error"),
        Err(e) => e.to_string(),
    };
    assert!(err.contains("500"), "error: {err}");
    assert!(err.contains("exploded"), "error: {err}");
}

#[tokio::test]
async fn test_list_ollama_cloud_models_maps_show_details() {
    // Server responds to /api/tags then to /api/show per model.
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        // First connection: tags listing.
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        let mut buf = vec![0_u8; 4096];
        let _ = socket.read(&mut buf).await;
        let body = r#"{"models":[{"name":"qwen3:8b","model":"qwen3:8b","details":{"parameter_size":"8B","family":"qwen"}}]}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;

        // Second connection: /api/show.
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        let mut buf = vec![0_u8; 4096];
        let _ = socket.read(&mut buf).await;
        let body = r#"{"context_length":"131072","capabilities":["vision","thinking"]}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
    });
    let models = crate::providers::ollama_cloud::list_ollama_cloud_models(
        "k",
        Some(&format!("http://{addr}")),
    )
    .await
    .expect("models");
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].id, "qwen3:8b");
    assert_eq!(models[0].provider_id, "ollama_cloud");
    assert_eq!(models[0].context_window, 131_072);
    assert!(models[0].capabilities.vision);
    assert!(models[0].capabilities.reasoning);
}
