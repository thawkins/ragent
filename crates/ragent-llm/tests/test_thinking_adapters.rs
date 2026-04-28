//! Integration tests for provider thinking adapter payload mapping.

use std::collections::HashMap;
use std::pin::Pin;

use anyhow::Result;
use ragent_llm::llm::{ChatContent, ChatMessage, ChatRequest, LlmClient};
use ragent_llm::{
    AnthropicProvider, GeminiProvider, GenericOpenAiProvider, HuggingFaceProvider,
    OllamaCloudProvider, OllamaProvider, OpenAiProvider, Provider,
};
use ragent_types::{ThinkingConfig, ThinkingDisplay, ThinkingLevel};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

#[derive(Debug)]
struct CapturedRequest {
    path: String,
    body: Value,
}

fn make_request(model: &str) -> ChatRequest {
    ChatRequest {
        model: model.to_string(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("hello".to_string()),
        }],
        tools: vec![],
        temperature: None,
        top_p: None,
        max_tokens: Some(128),
        system: Some("system".to_string()),
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: Some(5),
        thinking: None,
    }
}

async fn spawn_capture_server() -> Result<(String, oneshot::Receiver<CapturedRequest>)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let (tx, rx) = oneshot::channel();

    tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };

        let mut buffer = Vec::new();
        let header_end = loop {
            let mut chunk = [0_u8; 4096];
            let Ok(read) = socket.read(&mut chunk).await else {
                return;
            };
            if read == 0 {
                return;
            }
            buffer.extend_from_slice(&chunk[..read]);
            if let Some(position) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
                break position + 4;
            }
        };

        let headers = String::from_utf8_lossy(&buffer[..header_end]);
        let path = headers
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or("/")
            .to_string();
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                if name.eq_ignore_ascii_case("content-length") {
                    value.trim().parse::<usize>().ok()
                } else {
                    None
                }
            })
            .unwrap_or(0);

        while buffer.len() < header_end + content_length {
            let mut chunk = vec![0_u8; content_length];
            let Ok(read) = socket.read(&mut chunk).await else {
                return;
            };
            if read == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..read]);
        }

        let body =
            serde_json::from_slice::<Value>(&buffer[header_end..header_end + content_length])
                .unwrap_or(Value::Null);
        let _ = tx.send(CapturedRequest { path, body });

        let response =
            b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        let _ = socket.write_all(response).await;
        let _ = socket.shutdown().await;
    });

    Ok((format!("http://{addr}"), rx))
}

async fn capture_body(
    client: Box<dyn LlmClient>,
    request: ChatRequest,
    receiver: oneshot::Receiver<CapturedRequest>,
) -> CapturedRequest {
    let _stream: Pin<Box<dyn futures::Stream<Item = ragent_llm::llm::StreamEvent> + Send>> =
        client.chat(request).await.expect("request should succeed");
    receiver.await.expect("request should be captured")
}

#[tokio::test]
async fn test_openai_and_generic_openai_map_reasoning_effort_levels() {
    for provider_name in ["openai", "generic_openai"] {
        for (thinking, expected) in [
            (Some(ThinkingConfig::new(ThinkingLevel::Low)), Some("low")),
            (
                Some(ThinkingConfig::new(ThinkingLevel::Medium)),
                Some("medium"),
            ),
            (Some(ThinkingConfig::new(ThinkingLevel::High)), Some("high")),
            (Some(ThinkingConfig::off()), Some("none")),
            (Some(ThinkingConfig::new(ThinkingLevel::Auto)), None),
        ] {
            let (url, rx) = spawn_capture_server().await.expect("server");
            let client = match provider_name {
                "openai" => OpenAiProvider
                    .create_client("test-key", Some(&url), &HashMap::new())
                    .await
                    .expect("openai client"),
                "generic_openai" => GenericOpenAiProvider
                    .create_client("test-key", Some(&url), &HashMap::new())
                    .await
                    .expect("generic client"),
                _ => unreachable!(),
            };

            let mut request = make_request("gpt-5.4");
            request.thinking = thinking;
            let captured = capture_body(client, request, rx).await;

            assert_eq!(captured.path, "/v1/chat/completions");
            assert_eq!(
                captured
                    .body
                    .get("reasoning_effort")
                    .and_then(Value::as_str),
                expected,
                "provider {provider_name} should map thinking correctly"
            );
        }
    }
}

#[tokio::test]
async fn test_anthropic_maps_thinking_payload_variants() {
    let provider = AnthropicProvider;

    let cases = vec![
        (
            ThinkingConfig::new(ThinkingLevel::Low),
            json!({"type": "adaptive", "effort": "low"}),
        ),
        (
            ThinkingConfig::new(ThinkingLevel::Medium),
            json!({"type": "adaptive", "effort": "medium"}),
        ),
        (
            ThinkingConfig::new(ThinkingLevel::High),
            json!({"type": "adaptive", "effort": "high"}),
        ),
        (
            ThinkingConfig::new(ThinkingLevel::Auto),
            json!({"type": "adaptive"}),
        ),
        (ThinkingConfig::off(), json!({"type": "disabled"})),
        (
            ThinkingConfig {
                enabled: true,
                level: ThinkingLevel::High,
                budget_tokens: Some(2048),
                display: None,
            },
            json!({"type": "enabled", "budget_tokens": 2048}),
        ),
        (
            ThinkingConfig {
                enabled: true,
                level: ThinkingLevel::High,
                budget_tokens: None,
                display: Some(ThinkingDisplay::Omitted),
            },
            json!({"type": "disabled"}),
        ),
    ];

    for (thinking, expected) in cases {
        let (url, rx) = spawn_capture_server().await.expect("server");
        let client = provider
            .create_client("test-key", Some(&url), &HashMap::new())
            .await
            .expect("anthropic client");
        let mut request = make_request("claude-sonnet-4-20250514");
        request.thinking = Some(thinking);

        let captured = capture_body(client, request, rx).await;
        assert_eq!(captured.path, "/v1/messages");
        assert_eq!(captured.body["thinking"], expected);
    }
}

#[tokio::test]
async fn test_gemini_maps_thinking_config_variants() {
    let provider = GeminiProvider;
    let cases = vec![
        (
            ThinkingConfig::new(ThinkingLevel::Auto),
            json!({"thinkingLevel": "auto", "includeThoughts": true}),
        ),
        (
            ThinkingConfig::new(ThinkingLevel::Low),
            json!({"thinkingLevel": "low", "includeThoughts": true}),
        ),
        (
            ThinkingConfig::new(ThinkingLevel::Medium),
            json!({"thinkingLevel": "medium", "includeThoughts": true}),
        ),
        (
            ThinkingConfig::new(ThinkingLevel::High),
            json!({"thinkingLevel": "high", "includeThoughts": true}),
        ),
        (
            ThinkingConfig::off(),
            json!({"thinkingLevel": "minimal", "includeThoughts": false}),
        ),
        (
            ThinkingConfig {
                enabled: true,
                level: ThinkingLevel::High,
                budget_tokens: None,
                display: Some(ThinkingDisplay::Omitted),
            },
            json!({"thinkingLevel": "minimal", "includeThoughts": false}),
        ),
    ];

    for (thinking, expected) in cases {
        let (url, rx) = spawn_capture_server().await.expect("server");
        let client = provider
            .create_client("test-key", Some(&url), &HashMap::new())
            .await
            .expect("gemini client");
        let mut request = make_request("gemini-2.5-pro");
        request.thinking = Some(thinking);

        let captured = capture_body(client, request, rx).await;
        assert!(
            captured
                .path
                .starts_with("/v1beta/models/gemini-2.5-pro:streamGenerateContent"),
            "unexpected gemini path: {}",
            captured.path
        );
        assert_eq!(
            captured.body["generationConfig"]["thinkingConfig"],
            expected
        );
    }
}

#[tokio::test]
async fn test_ollama_sends_binary_think_field() {
    // Local Ollama (OpenAI-compatible endpoint) does support the `think` field.
    for (thinking, expected) in [
        (ThinkingConfig::new(ThinkingLevel::Auto), true),
        (ThinkingConfig::new(ThinkingLevel::High), true),
        (ThinkingConfig::off(), false),
    ] {
        let (url, rx) = spawn_capture_server().await.expect("server");
        let client = OllamaProvider::new()
            .create_client("", Some(&url), &HashMap::new())
            .await
            .expect("ollama client");

        let mut request = make_request("qwen3:latest");
        request.thinking = Some(thinking);

        let captured = capture_body(client, request, rx).await;
        assert_eq!(captured.path, "/v1/chat/completions");
        assert_eq!(captured.body["think"], json!(expected));
    }
}

#[tokio::test]
async fn test_ollama_cloud_never_sends_think_field() {
    // Ollama Cloud does not support the `think` parameter, so
    // regardless of the request's thinking config, the body should
    // never contain a `think` key.
    for (thinking, _expected) in [
        (ThinkingConfig::new(ThinkingLevel::Auto), true),
        (ThinkingConfig::new(ThinkingLevel::High), true),
        (ThinkingConfig::off(), false),
    ] {
        let (url, rx) = spawn_capture_server().await.expect("server");
        let client = OllamaCloudProvider::new()
            .create_client("test-key", Some(&url), &HashMap::new())
            .await
            .expect("ollama cloud client");

        let mut request = make_request("qwen3:latest");
        request.thinking = Some(thinking);

        let captured = capture_body(client, request, rx).await;
        assert_eq!(captured.path, "/api/chat");
        assert!(
            !captured.body.as_object().unwrap().contains_key("think"),
            "ollama_cloud must not send a `think` field: {:#?}",
            captured.body
        );
    }
}

#[tokio::test]
async fn test_huggingface_ignores_thinking_payload_fields() {
    let (url, rx) = spawn_capture_server().await.expect("server");
    let client = HuggingFaceProvider
        .create_client("test-key", Some(&url), &HashMap::new())
        .await
        .expect("huggingface client");
    let mut request = make_request("meta-llama/Llama-3.3-70B-Instruct");
    request.thinking = Some(ThinkingConfig::new(ThinkingLevel::High));

    let captured = capture_body(client, request, rx).await;
    assert_eq!(captured.path, "/v1/chat/completions");
    assert!(captured.body.get("thinking").is_none());
    assert!(captured.body.get("reasoning_effort").is_none());
    assert!(captured.body.get("think").is_none());
}
