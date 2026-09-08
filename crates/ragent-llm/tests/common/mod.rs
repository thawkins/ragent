//! Shared helpers for provider streaming tests: a minimal local SSE mock
//! server and a standard `ChatRequest` builder.
//!
//! Extracted from the per-file copies in `test_ollama_tool_handling.rs` and
//! `test_gemini_tool_calls.rs` so a third provider test file does not copy
//! them again.

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;

use ragent_llm::llm::{ChatContent, ChatMessage, ChatRequest, ToolDefinition};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

/// Build a minimal single-user-message chat request for streaming tests.
pub fn make_request(model: &str, tools: Vec<ToolDefinition>) -> ChatRequest {
    ChatRequest {
        model: model.to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("Use the get_weather tool for London.".to_string()),
        }]),
        tools: Arc::new(tools),
        temperature: None,
        top_p: None,
        max_tokens: Some(128),
        system: Some(std::sync::Arc::from("system")),
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: Some(5),
        thinking: None,
    }
}

/// Spawn a one-shot local HTTP server that replies with an SSE
/// `text/event-stream` body and returns its base URL.
pub async fn spawn_sse_server(response_body: String) -> anyhow::Result<String> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };

        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n{response_body}"
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
    });

    Ok(format!("http://{addr}"))
}