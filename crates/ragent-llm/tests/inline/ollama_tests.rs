//! Inline tests for `ollama.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn test_format_model_name() {
    let details = OllamaModelDetails {
        parameter_size: "70B".to_string(),
        family: "llama".to_string(),
    };
    assert_eq!(
        format_model_name("llama3.3:latest", &details),
        "Llama3.3 (70B)"
    );
}

#[test]
fn test_estimate_context_window() {
    // Modern local models support 128k regardless of parameter size.
    assert_eq!(estimate_context_window("70B"), 131_072);
    assert_eq!(estimate_context_window("8B"), 131_072);
    assert_eq!(estimate_context_window("3B"), 131_072);
    assert_eq!(estimate_context_window("32B"), 131_072);
    assert_eq!(estimate_context_window("1B"), 131_072);
    // Sub-1B models get a conservative 32k fallback.
    assert_eq!(estimate_context_window("0.5B"), 32_768);
}

#[test]
fn test_provider_defaults() {
    let provider = OllamaProvider::new();
    assert_eq!(provider.id(), "ollama");
    assert_eq!(provider.name(), "Ollama");
    assert!(
        provider.default_models().is_empty(),
        "Ollama default_models should be empty; models are discovered at runtime"
    );
}

#[test]
fn test_with_custom_url() {
    let provider = OllamaProvider::with_url("http://remote:11434/");
    assert_eq!(provider.base_url, "http://remote:11434");
}

// ---------------------------------------------------------------------------
// chat() SSE stream via a local mock server
// ---------------------------------------------------------------------------

use crate::llm::{ChatContent, ChatRequest, LlmClient, StreamEvent, ToolDefinition};
use ragent_types::event::FinishReason;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn req(tools: Vec<ToolDefinition>) -> ChatRequest {
    ChatRequest {
        model: "llama3.3:latest".to_string(),
        messages: Arc::new(vec![crate::llm::ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("hi".to_string()),
        }]),
        tools: Arc::new(tools),
        temperature: None,
        top_p: None,
        max_tokens: Some(64),
        system: Some(Arc::from("sys")),
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: Some(5),
        thinking: None,
    }
}

async fn spawn_ollama_body(body: String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        // Read the request before replying: a client whose request bytes are
        // still in flight when the socket closes sees a TCP RST, surfacing as
        // `client error (Canceled)` on the real HTTP client.
        let mut request_buf = [0u8; 8192];
        let _ = socket.read(&mut request_buf).await;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n{body}"
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
    });
    format!("http://{addr}")
}

fn ollama_client(base_url: String) -> OllamaClient {
    OllamaClient {
        api_key: Some("k".to_string()),
        base_url,
        http: crate::provider::http_client::create_streaming_http_client(),
    }
}

#[tokio::test]
async fn test_ollama_chat_text_and_usage() {
    let sse = concat!(
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hel\"}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"lo\"},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":2}}\n\n",
        "data: [DONE]\n\n",
    )
    .to_string();
    let url = spawn_ollama_body(sse).await;
    let events: Vec<StreamEvent> = ollama_client(url)
        .chat(req(vec![]))
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
            input_tokens: 3,
            output_tokens: 2
        }
    )));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::Finish { reason } if *reason == FinishReason::Stop))
    );
}

#[tokio::test]
async fn test_ollama_chat_http_error() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        // Read the request before replying: a client whose request bytes are
        // still in flight when the socket closes sees a TCP RST, surfacing as
        // `client error (Canceled)` on the real HTTP client.
        let mut request_buf = [0u8; 8192];
        let _ = socket.read(&mut request_buf).await;
        let body = "{\"error\":\"no such model\"}";
        let response = format!(
            "HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
    });
    let err = match ollama_client(format!("http://{addr}"))
        .chat(req(vec![]))
        .await
    {
        Ok(_) => panic!("expected error"),
        Err(e) => e.to_string(),
    };
    assert!(err.contains("404"), "error: {err}");
}

#[tokio::test]
async fn test_ollama_chat_reasoning_and_tool_calls_suppress_narration() {
    let sse = concat!(
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"calling tool\",\"reasoning\":\"think\"}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"c1\",\"function\":{\"name\":\"get_weather\",\"arguments\":\"{\\\"city\\\":\\\"London\\\"}\"}}]}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"after\"},\"finish_reason\":\"tool_calls\"}]}\n\n",
    )
    .to_string();
    let url = spawn_ollama_body(sse).await;
    let events: Vec<StreamEvent> = ollama_client(url)
        .chat(req(vec![]))
        .await
        .expect("chat")
        .collect()
        .await;
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::ReasoningDelta { text } if text == "think"))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::TextDelta { text } if text == "calling tool"))
    );
    assert!(events.iter().any(|e| matches!(
        e,
        StreamEvent::ToolCallStart { id, name } if id == "c1" && name == "get_weather"
    )));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::ToolCallEnd { id } if id == "c1"))
    );
    // Narration after the tool call is suppressed.
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, StreamEvent::TextDelta { text } if text == "after"))
    );
    assert!(
        events.iter().any(
            |e| matches!(e, StreamEvent::Finish { reason } if *reason == FinishReason::ToolUse)
        )
    );
}

#[tokio::test]
async fn test_list_ollama_models_maps_entries() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        // Read the request before replying: a client whose request bytes are
        // still in flight when the socket closes sees a TCP RST, surfacing as
        // `client error (Canceled)` on the real HTTP client.
        let mut request_buf = [0u8; 8192];
        let _ = socket.read(&mut request_buf).await;
        let body = r#"{"models":[{"name":"llama3.3:70b","details":{"parameter_size":"70B","family":"llama"}},{"name":"tiny:0.5b","details":{"parameter_size":""}}]}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
    });
    let models = crate::providers::ollama::list_ollama_models(Some(&format!("http://{addr}")))
        .await
        .expect("models");
    assert_eq!(models.len(), 2);
    assert_eq!(models[0].provider_id, "ollama");
    assert_eq!(models[0].id, "llama3.3:70b");
    assert_eq!(models[0].name, "Llama3.3 (70B)");
    assert!(!models[0].capabilities.vision);
    assert!(models[0].capabilities.tool_use);
    assert_eq!(models[1].name, "Tiny");
}
