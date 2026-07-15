//! Regression tests for Ollama Cloud image serialization.
//!
//! The Ollama Cloud native `/api/chat` endpoint rejects requests where a model
//! that does not support vision receives an `images` array on any message. The
//! provider must therefore only attach images to the most recent user message,
//! dropping historical image parts from earlier turns.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use ragent_llm::llm::{ChatContent, ChatMessage, ChatRequest, ContentPart, LlmClient};
use ragent_llm::{OllamaCloudProvider, Provider};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

#[derive(Debug)]
struct CapturedRequest {
    path: String,
    body: Value,
}

fn make_request(model: &str, messages: Vec<ChatMessage>) -> ChatRequest {
    ChatRequest {
        model: model.to_string(),
        messages: Arc::new(messages),
        tools: Arc::new(vec![]),
        temperature: None,
        top_p: None,
        max_tokens: Some(128),
        system: Some(Arc::from("system")),
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: Some(5),
        thinking: None,
    }
}

async fn spawn_capture_server() -> anyhow::Result<(String, oneshot::Receiver<CapturedRequest>)> {
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
async fn test_ollama_cloud_drops_historical_images_for_text_follow_up() {
    let (url, rx) = spawn_capture_server().await.expect("server");
    let client = OllamaCloudProvider::new()
        .create_client("test-key", Some(&url), &HashMap::new())
        .await
        .expect("ollama cloud client");

    let request = make_request(
        "glm-5.2",
        vec![
            ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Parts(vec![
                    ContentPart::Text {
                        text: "describe this image".to_string(),
                    },
                    ContentPart::ImageUrl {
                        url: "data:image/png;base64,AAA".to_string(),
                    },
                ]),
            },
            ChatMessage {
                role: "assistant".to_string(),
                content: ChatContent::Text("A red square.".to_string()),
            },
            ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Text("now write code for it".to_string()),
            },
        ],
    );

    let captured = capture_body(client, request, rx).await;
    assert_eq!(captured.path, "/api/chat");

    let messages = captured.body["messages"]
        .as_array()
        .expect("messages should be an array");
    assert_eq!(messages.len(), 4, "expected system + 3 chat messages");

    let first_user = messages
        .iter()
        .find(|m| m["role"] == "user" && m["content"] == "describe this image")
        .expect("first user message present");
    assert!(
        first_user["images"].is_null() || first_user["images"].as_array().unwrap().is_empty(),
        "historical user message should not have an images array, got {:?}",
        first_user["images"]
    );

    let last_user = messages
        .iter()
        .find(|m| m["role"] == "user" && m["content"] == "now write code for it")
        .expect("last user message present");
    assert!(
        last_user["images"].is_null() || last_user["images"].as_array().unwrap().is_empty(),
        "text-only follow-up should not have an images array, got {:?}",
        last_user["images"]
    );
}

#[tokio::test]
async fn test_ollama_cloud_includes_images_on_current_user_message() {
    let (url, rx) = spawn_capture_server().await.expect("server");
    let client = OllamaCloudProvider::new()
        .create_client("test-key", Some(&url), &HashMap::new())
        .await
        .expect("ollama cloud client");

    let request = make_request(
        "vision-model",
        vec![
            ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Text("hello".to_string()),
            },
            ChatMessage {
                role: "assistant".to_string(),
                content: ChatContent::Text("Hi there.".to_string()),
            },
            ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Parts(vec![
                    ContentPart::Text {
                        text: "describe this image".to_string(),
                    },
                    ContentPart::ImageUrl {
                        url: "data:image/png;base64,BBB".to_string(),
                    },
                ]),
            },
        ],
    );

    let captured = capture_body(client, request, rx).await;
    assert_eq!(captured.path, "/api/chat");

    let messages = captured.body["messages"]
        .as_array()
        .expect("messages should be an array");

    let first_user = messages
        .iter()
        .find(|m| m["role"] == "user" && m["content"] == "hello")
        .expect("first user message present");
    assert!(
        first_user["images"].is_null() || first_user["images"].as_array().unwrap().is_empty(),
        "older text-only user message should not have images"
    );

    let last_user = messages
        .iter()
        .find(|m| m["role"] == "user" && m["content"] == "describe this image")
        .expect("last user message present");
    assert_eq!(
        last_user["images"]
            .as_array()
            .expect("images array on current user message"),
        &vec![json!("BBB")],
        "current user message should carry stripped base64 images"
    );
}
