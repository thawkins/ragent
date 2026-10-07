//! Integration tests for ANTIPAT Milestone M5.6 (provider consistency).
//!
//! Covers:
//! - 3.5 stream-timeout model: the named constants exist with the documented
//!   values and the first-byte budget is the larger of the two.
//! - 3.6 secret handling: providers' `create_client` register their key with
//!   the shared redaction registry, so `redact_secrets` masks keys that the
//!   regex layer alone would not catch.
//! - 5.2 `data:` line handling: the Ollama Cloud NDJSON parser still accepts
//!   prefix-less frames (native `/api/chat`), and the OpenAI Responses parser
//!   skips non-`data:` lines instead of slicing them by a fixed offset.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use futures::StreamExt;
use ragent_llm::llm::{ChatContent, ChatMessage, ChatRequest, LlmClient, StreamEvent};
use ragent_llm::provider::http_client::{
    DEFAULT_STREAM_TIMEOUT_SECS, STREAM_CHUNK_IDLE_TIMEOUT_SECS,
};
use ragent_llm::provider::openai_responses::ResponsesApiClient;
use ragent_llm::{
    AnthropicProvider, GeminiProvider, OllamaCloudProvider, OllamaProvider, Provider,
};
use tokio::io::AsyncWriteExt;
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

/// Serves a single one-shot HTTP response with the given content type and body.
async fn spawn_sse_server(content_type: &str, body: String) -> anyhow::Result<String> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let content_type = content_type.to_string();

    tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nConnection: close\r\n\r\n{body}"
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
    });

    Ok(format!("http://{addr}"))
}

/// Serves a single error response with the given status and body.
async fn spawn_error_server(status: u16, body: String) -> anyhow::Result<String> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let body = body.replace('\n', " ");

    tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        let response = format!(
            "HTTP/1.1 {status} Error\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
    });

    Ok(format!("http://{addr}"))
}

// ---------------------------------------------------------------------------
// 3.5 stream-timeout model
// ---------------------------------------------------------------------------

#[test]
fn test_stream_timeout_constants_are_named_and_ordered() {
    // ANTIPAT 3.5: the first-byte budget (600s) is deliberately larger than the
    // per-chunk idle guard (120s) - a model may think for minutes before its
    // first token, whereas an inter-chunk gap of that length means a stall.
    assert_eq!(DEFAULT_STREAM_TIMEOUT_SECS, 600);
    assert_eq!(STREAM_CHUNK_IDLE_TIMEOUT_SECS, 120);
    // First-byte budget must exceed the per-chunk idle guard. Evaluated in a
    // const block so an accidental future change fails to compile.
    const {
        assert!(DEFAULT_STREAM_TIMEOUT_SECS > STREAM_CHUNK_IDLE_TIMEOUT_SECS);
    }
}

// ---------------------------------------------------------------------------
// 3.6 secret handling
// ---------------------------------------------------------------------------

/// A unique provider key. It deliberately has no secret-looking prefix and no
/// `token=`/`api_key=` assignment, so the regex layer of `redact_secrets`
/// cannot mask it - only the registry layer (registered by `create_client`)
/// can. That makes the assertion below a genuine registry check.
const UNIQUE_KEY: &str = "unit-test-credential-9f3a7c2e1b";

#[tokio::test]
async fn test_anthropic_create_client_registers_secret() {
    let provider = AnthropicProvider;
    let _client = provider
        .create_client(UNIQUE_KEY, None, &HashMap::new())
        .await
        .expect("anthropic client");
    assert!(
        !ragent_types::sanitize::redact_secrets(UNIQUE_KEY).contains(UNIQUE_KEY),
        "anthropic create_client must register its key with the redaction registry"
    );
}

#[tokio::test]
async fn test_gemini_create_client_registers_secret_via_constructor() {
    let provider = GeminiProvider;
    let _client = provider
        .create_client(UNIQUE_KEY, None, &HashMap::new())
        .await
        .expect("gemini client");
    assert!(
        !ragent_types::sanitize::redact_secrets(UNIQUE_KEY).contains(UNIQUE_KEY),
        "gemini create_client must register its key with the redaction registry"
    );
}

// ---------------------------------------------------------------------------
// 3.6 provider error-body redaction (audit T-107)
// ---------------------------------------------------------------------------

/// A key-shaped token that the regex layer of `redact_secrets` masks but the
/// registry does not need to know about. It mimics a credential a provider
/// might echo back inside an HTTP error body.
const KEY_SHAPED_TOKEN: &str = "sk-proj-3f8a2b1c9d0e4f5a6b7c8d9e0f1a2b3c";

#[tokio::test]
async fn test_ollama_error_body_is_redacted() {
    // The error body a provider echoes can carry a credential; it must never
    // reach the surfaced `anyhow::Error` in cleartext.
    let body = format!("{{\"error\":{{\"message\":\"invalid api key {KEY_SHAPED_TOKEN}\"}}}}");
    let url = spawn_error_server(401, body).await.expect("server");

    let client = OllamaProvider::new()
        .create_client("test-key", Some(&url), &HashMap::new())
        .await
        .expect("ollama client");

    let err = client
        .chat(make_request("llama3"))
        .await
        .err()
        .expect("chat must fail on a 401 response");
    let message = err.to_string();
    assert!(
        !message.contains(KEY_SHAPED_TOKEN),
        "provider error body must be redacted before surfacing: {message}"
    );
    assert!(
        message.contains("[REDACTED]"),
        "redacted error should mark the masked span: {message}"
    );
}

// ---------------------------------------------------------------------------
// 5.2 `data:` line handling
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_ollama_cloud_accepts_prefixless_ndjson_frames() {
    // ANTIPAT 5.2: the native Ollama `/api/chat` endpoint streams NDJSON, so a
    // frame has no `data: ` prefix; such a line must be parsed, not skipped.
    let body = concat!("{\"message\":{\"content\":\"hi\"}}\n", "{\"done\":true}\n").to_string();
    let url = spawn_sse_server("application/x-ndjson", body)
        .await
        .expect("server");

    let client = OllamaCloudProvider::new()
        .create_client("test-key", Some(&url), &HashMap::new())
        .await
        .expect("ollama cloud client");

    let mut stream: Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>> =
        client.chat(make_request("glm-5.2")).await.expect("chat");

    let mut text = String::new();
    let mut finished = false;
    while let Some(event) = stream.next().await {
        match event {
            StreamEvent::TextDelta { text: t } => text.push_str(&t),
            StreamEvent::Finish { .. } => finished = true,
            _ => {}
        }
    }

    assert_eq!(text, "hi", "prefix-less NDJSON frame must be parsed");
    assert!(finished, "done frame must yield a Finish event");
}

#[tokio::test]
async fn test_openai_responses_skips_non_data_lines() {
    // ANTIPAT 5.2: a non-`data:` line (here an SSE comment/keep-alive) must be
    // skipped, not sliced by a fixed offset that would corrupt the frame.
    let body = concat!(
        ": keep-alive\n\n",
        "data: {\"type\":\"response.output_text.delta\",\"delta\":\"hello\"}\n\n",
        "data: [DONE]\n\n"
    )
    .to_string();
    let url = spawn_sse_server("text/event-stream", body)
        .await
        .expect("server");

    let client = ResponsesApiClient::new("test-key", &url);
    let mut stream: Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>> = client
        .chat(make_request("gpt-5.6"))
        .await
        .expect("responses chat");

    let mut text = String::new();
    let mut finished = false;
    while let Some(event) = stream.next().await {
        match event {
            StreamEvent::TextDelta { text: t } => text.push_str(&t),
            StreamEvent::Finish { .. } => finished = true,
            StreamEvent::Error { message } => panic!("unexpected stream error: {message}"),
            _ => {}
        }
    }

    assert_eq!(text, "hello", "data frame after a comment line must parse");
    assert!(finished, "[DONE] must yield a Finish event");
}
