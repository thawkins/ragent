//! Integration tests for the Azure AI Foundry provider (audit T-703).
//!
//! Covers the provider's three responsibilities without network access by
//! standing up a loopback HTTP server:
//!
//! - **request build** - `AzureFoundryClient::chat` posts to
//!   `<base>/openai/v1/chat/completions` and parses the OpenAI-compatible SSE
//!   stream;
//! - **auth** - the request carries the Azure `api-key` header (not
//!   `Authorization: Bearer`);
//! - **model discovery** - `discover_azure_foundry_models` parses the
//!   `/openai/models` payload and derives capabilities, and surfaces a redacted
//!   error body on a non-success status.

use std::collections::HashMap;
use std::sync::Arc;

use futures::StreamExt;
use ragent_llm::Provider;
use ragent_llm::llm::{ChatContent, ChatMessage, ChatRequest, StreamEvent};
use ragent_llm::provider::azure_foundry::{AzureFoundryProvider, discover_azure_foundry_models};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn make_request(model: &str) -> ChatRequest {
    ChatRequest {
        model: model.to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("hello".to_string()),
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

/// Serve one request with `status` and `body`, returning the base URL and a
/// receiver carrying the raw request bytes the client sent.
async fn spawn_capture_server(
    status: u16,
    content_type: &str,
    body: String,
) -> anyhow::Result<(String, tokio::sync::oneshot::Receiver<String>)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let content_type = content_type.to_string();
    let (tx, rx) = tokio::sync::oneshot::channel();

    tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        let mut buf = vec![0u8; 16 * 1024];
        let n = socket.read(&mut buf).await.unwrap_or(0);
        let request = String::from_utf8_lossy(&buf[..n]).to_string();
        let _ = tx.send(request);

        let response = format!(
            "HTTP/1.1 {status} OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
    });

    Ok((format!("http://{addr}"), rx))
}

// ---------------------------------------------------------------------------
// 1. request build
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_chat_builds_openai_compatible_request_and_streams_sse() {
    let body = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"hi\"},\"finish_reason\":null}]}\n\n",
        "data: [DONE]\n\n"
    )
    .to_string();
    let (base, _captured) = spawn_capture_server(200, "text/event-stream", body)
        .await
        .expect("server");

    let client = AzureFoundryProvider
        .create_client("test-key", Some(&base), &HashMap::new())
        .await
        .expect("azure foundry client");

    let mut stream = client.chat(make_request("gpt-4o")).await.expect("chat");
    let mut text = String::new();
    let mut finished = false;
    while let Some(event) = stream.next().await {
        match event {
            StreamEvent::TextDelta { text: t } => text.push_str(&t),
            StreamEvent::Finish { .. } => finished = true,
            _ => {}
        }
    }
    assert_eq!(text, "hi", "SSE text delta must be parsed");
    assert!(finished, "[DONE] must yield a Finish event");
}

// ---------------------------------------------------------------------------
// 2. auth + URL shape
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_chat_uses_api_key_header_and_openai_v1_path() {
    let body = "data: [DONE]\n\n".to_string();
    let (base, captured) = spawn_capture_server(200, "text/event-stream", body)
        .await
        .expect("server");

    let client = AzureFoundryProvider
        .create_client("secret-azure-key", Some(&base), &HashMap::new())
        .await
        .expect("azure foundry client");

    // Drive the request to completion so the server reads and reports it.
    let mut stream = client.chat(make_request("gpt-4o")).await.expect("chat");
    while stream.next().await.is_some() {}

    let request = captured.await.expect("captured request");
    assert!(
        request.starts_with("POST /openai/v1/chat/completions "),
        "request line must target the OpenAI-compatible path: {request}"
    );
    assert!(
        request.to_lowercase().contains("api-key: secret-azure-key"),
        "request must carry the Azure api-key header: {request}"
    );
    assert!(
        !request.contains("Authorization: Bearer"),
        "Azure Foundry must not use a Bearer authorization header: {request}"
    );
}

// ---------------------------------------------------------------------------
// 3. model discovery
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_discover_models_parses_capabilities() {
    let body = r#"{
        "data": [
            { "id": "gpt-4o", "object": "model" },
            { "id": "o3-mini", "object": "model" }
        ]
    }"#
    .to_string();
    let (base, captured) = spawn_capture_server(200, "application/json", body)
        .await
        .expect("server");

    let models = discover_azure_foundry_models("test-key", &base)
        .await
        .expect("model discovery");
    assert_eq!(models.len(), 2);

    let gpt = &models[0];
    assert_eq!(gpt.id, "gpt-4o");
    assert_eq!(gpt.provider_id, "azure_foundry");
    assert!(gpt.capabilities.tool_use, "gpt-4* supports tool use");
    assert!(gpt.capabilities.vision, "gpt-4o supports vision");
    assert!(gpt.capabilities.streaming);
    assert!(!gpt.capabilities.reasoning);
    assert_eq!(gpt.context_window, 128_000);

    let o3 = &models[1];
    assert_eq!(o3.id, "o3-mini");
    assert!(o3.capabilities.reasoning, "o3* is a reasoning model");
    assert!(o3.capabilities.tool_use, "o3* supports tool use");

    let request = captured.await.expect("captured request");
    assert!(
        request.starts_with("GET /openai/models?api-version=2024-10-21 "),
        "discovery must hit the Azure OpenAI models endpoint: {request}"
    );
    assert!(request.to_lowercase().contains("api-key: test-key"));
}

#[tokio::test]
async fn test_discover_models_redacts_error_body() {
    // A key-shaped token echoed in the error body must never surface verbatim.
    const KEY_SHAPED_TOKEN: &str = "sk-proj-3f8a2b1c9d0e4f5a6b7c8d9e0f1a2b3c";
    let body = format!("{{\"error\":{{\"message\":\"invalid key {KEY_SHAPED_TOKEN}\"}}}}");
    let (base, _captured) = spawn_capture_server(401, "application/json", body)
        .await
        .expect("server");

    let err = discover_azure_foundry_models("test-key", &base)
        .await
        .expect_err("discovery must fail on a 401")
        .to_string();
    assert!(
        !err.contains(KEY_SHAPED_TOKEN),
        "error body must be redacted before surfacing: {err}"
    );
    assert!(
        err.contains("[REDACTED]"),
        "redacted error should mark the masked span: {err}"
    );
    assert!(err.contains("401"));
}

#[test]
fn test_provider_metadata() {
    let provider = AzureFoundryProvider;
    assert_eq!(provider.id(), "azure_foundry");
    assert_eq!(provider.name(), "Azure AI Foundry");
    assert!(
        provider.default_models().is_empty(),
        "Azure Foundry discovers models dynamically"
    );
}
