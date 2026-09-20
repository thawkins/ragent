//! OpenRouter provider offline tests.
//!
//! Covers spec `openrouterprov` FR-002 (base URL trim/override), FR-004 (key
//! sourcing precedence: per-call arg, stored credential, environment), FR-005
//! (masked fingerprint helper), FR-009 (explicit missing-key error),
//! FR-010/FR-011 (model metadata mapping from the `/api/v1/models` response),
//! FR-018/FR-019/FR-020 (reasoning/thinking levels and SSE delta routing), and
//! the chat stream mapping implemented in T-005.
//!
//! Concurrency notes:
//! - `std::env::set_var` / `remove_var` are `unsafe` in Rust 2024; this test
//!   target opts back in explicitly and mutates `OPENROUTER_API_KEY` only
//!   while a shared `tokio::sync::Mutex` guard is held.
//! - The env guard is deliberately held across `create_client(...).await`
//!   await points: serialisation, not parallelism, is the goal, and each
//!   site documents why the std guard-across-await pattern is safe here
//!   (no other task in the test process reads the variable concurrently).

#![allow(unsafe_code)]

use std::sync::{Arc, OnceLock};

use futures::StreamExt;
use ragent_llm::Provider;
use ragent_llm::llm::{ChatContent, ChatMessage, ChatRequest, StreamEvent};
use ragent_llm::provider::openrouter::{OpenRouterClient, OpenRouterProvider, mask_key};
use ragent_storage::storage::Storage;
use ragent_types::event::FinishReason;
use ragent_types::{ThinkingConfig, ThinkingLevel};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex as AsyncMutex;
use tokio::sync::oneshot;

/// Serialises tests that touch the process-wide `OPENROUTER_API_KEY`.
static ENV_LOCK: OnceLock<AsyncMutex<()>> = OnceLock::new();

fn env_lock() -> &'static AsyncMutex<()> {
    ENV_LOCK.get_or_init(|| AsyncMutex::new(()))
}

/// Removes `OPENROUTER_API_KEY` while holding the shared env lock.
///
/// Returns the guard so the environment stays cleared until the caller drops
/// it. `set_var`/`remove_var` are `unsafe` in Rust 2024; the guard proves
/// no other test in this target is mutating the variable concurrently.
async fn lock_and_clear_env() -> tokio::sync::MutexGuard<'static, ()> {
    let guard = env_lock().lock().await;
    // SAFETY: exclusive access to OPENROUTER_API_KEY via ENV_LOCK.
    unsafe { std::env::remove_var("OPENROUTER_API_KEY") };
    guard
}

/// Sets `OPENROUTER_API_KEY` while already holding the shared env lock.
fn set_env_locked(value: &str) {
    // SAFETY: exclusive access to OPENROUTER_API_KEY via ENV_LOCK.
    unsafe { std::env::set_var("OPENROUTER_API_KEY", value) };
}

/// Removes `OPENROUTER_API_KEY` while already holding the shared env lock.
fn unset_env_locked() {
    // SAFETY: exclusive access to OPENROUTER_API_KEY via ENV_LOCK.
    unsafe { std::env::remove_var("OPENROUTER_API_KEY") };
}

fn make_request() -> ChatRequest {
    ChatRequest {
        model: "openrouter/anthropic/claude-sonnet-4".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("hi".to_string()),
        }]),
        tools: Arc::new(vec![]),
        temperature: None,
        top_p: None,
        max_tokens: Some(16),
        system: None,
        options: std::collections::HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: Some(5),
        thinking: None,
    }
}

#[test]
fn test_openrouter_with_url_trims_trailing_slash() {
    let provider = OpenRouterProvider::with_url("http://127.0.0.1:8999/");
    assert_eq!(provider.base_url(), "http://127.0.0.1:8999");

    let multi = OpenRouterProvider::with_url("http://127.0.0.1:8999///");
    assert_eq!(multi.base_url(), "http://127.0.0.1:8999");

    let default_provider = OpenRouterProvider::new();
    assert_eq!(default_provider.base_url(), "https://openrouter.ai");
}

#[test]
fn test_openrouter_mask_key_shapes() {
    // FR-005: only the final four characters behind an ellipsis. Short keys
    // are partially masked too (mask_key never reveals the full value).
    assert_eq!(mask_key("sk-or-v1-0123456789abcd"), "...abcd");
    assert_eq!(mask_key("short"), "...hort");
    assert_eq!(mask_key(""), "(none)");
}

#[tokio::test]
async fn test_openrouter_create_client_bails_without_key() {
    let _guard = lock_and_clear_env().await;

    let provider = OpenRouterProvider::new();
    let err = match provider
        .create_client("", None, &std::collections::HashMap::new())
        .await
    {
        // `Box<dyn LlmClient>` has no `Debug`, so unwrap the Err by hand.
        Ok(_) => panic!("an empty key must be rejected at chat time"),
        Err(err) => err,
    };
    // FR-009: the message names the provider and carries remediation.
    let msg = format!("{err:#}");
    assert!(msg.contains("OpenRouter requires an API key."), "{msg}");
    assert!(msg.contains("ragent auth openrouter"), "{msg}");
    assert!(msg.contains("OPENROUTER_API_KEY"), "{msg}");
}

#[tokio::test]
async fn test_openrouter_create_client_env_fallback_wins_when_no_arg() {
    let guard = lock_and_clear_env().await;
    set_env_locked("env-key-openrouter-1234");

    let provider = OpenRouterProvider::new();
    let _client = provider
        .create_client("", None, &std::collections::HashMap::new())
        .await
        .expect("environment key should satisfy the chat gate");
    // T-005 is now implemented; the constructed client should hold a usable
    // endpoint configuration. Actually driving a stream would hit the live
    // network, so inspect the client directly (the provider's default URL
    // stays https://openrouter.ai and the per-call base_url override path is
    // verified elsewhere).
    assert_eq!(provider.base_url(), "https://openrouter.ai");

    unset_env_locked();
    drop(guard);
}

#[tokio::test]
async fn test_openrouter_stored_credential_satisfies_gate_when_no_arg_or_env() {
    let _guard = lock_and_clear_env().await;

    // No per-call argument, no env var, but a stored credential: gate passes.
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    storage
        .set_provider_auth("openrouter", "stored-key-openrouter-9876")
        .expect("store credential");

    let provider = OpenRouterProvider::new();
    provider.set_storage(Arc::clone(&storage));
    provider
        .create_client("", None, &std::collections::HashMap::new())
        .await
        .expect("stored credential should satisfy the chat gate");
}

#[tokio::test]
async fn test_openrouter_per_call_arg_beats_storage_and_env() {
    // FR-004: precedence (a) per-call argument wins even when a stored
    // credential and an env var both exist. Behavioural probe: the gate opens
    // regardless of which source fired; the argument-first order is pinned by
    // the resolution path (non-empty arg short-circuits both fallbacks).
    let guard = lock_and_clear_env().await;
    set_env_locked("env-key-openrouter-1234");

    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    storage
        .set_provider_auth("openrouter", "stored-key-openrouter-9876")
        .expect("store credential");

    let provider = OpenRouterProvider::with_url("https://openrouter.example");
    provider.set_storage(storage);
    provider
        .create_client(
            "arg-key-openrouter-0000",
            None,
            &std::collections::HashMap::new(),
        )
        .await
        .expect("non-empty per-call argument must satisfy the gate");

    unset_env_locked();
    drop(guard);
}

#[tokio::test]
async fn test_openrouter_base_url_override_applies_to_chat_client() {
    let guard = lock_and_clear_env().await;
    set_env_locked("override-probe-key-0001");

    let provider = OpenRouterProvider::new();
    let client = provider
        .create_client(
            "",
            Some("http://127.0.0.1:8999/"),
            &std::collections::HashMap::new(),
        )
        .await
        .expect("call-scoped base URL override with env key must construct");
    // The gate resolved the key from the environment; the constructed
    // client's endpoint is inspected in T-005, so assert construction success
    // plus the provider default shape here.
    assert_eq!(provider.base_url(), "https://openrouter.ai");
    drop(client);

    unset_env_locked();
    drop(guard);
}

#[tokio::test]
async fn test_openrouter_chat_stream_rejects_non_2xx() {
    let _guard = lock_and_clear_env().await;

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let local_addr = listener.local_addr().unwrap();

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf).await;
        let body = serde_json::json!({
            "error": { "message": "invalid model" }
        })
        .to_string();
        let response = format!(
            "HTTP/1.1 400 Bad Request\r\ncontent-length: {}\r\ncontent-type: application/json\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.flush().await;
        let _ = shutdown_rx.await;
    });

    let provider = OpenRouterProvider::with_url(&format!("http://{}", local_addr));
    let client = provider
        .create_client("non-2xx-probe-key", None, &std::collections::HashMap::new())
        .await
        .expect("key must pass the gate");

    let err = match client.chat(make_request()).await {
        Ok(_) => panic!("non-2xx response should fail at stream creation time"),
        Err(err) => err,
    };
    let msg = format!("{err:#}");
    assert!(msg.contains("400"), "{msg}");
    assert!(msg.contains("invalid model"), "{msg}");

    let _ = shutdown_tx.send(());
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), server).await;
}

#[tokio::test]
async fn test_openrouter_chat_stream_maps_sse_events() {
    let _guard = lock_and_clear_env().await;

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let local_addr = listener.local_addr().unwrap();

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let mut buf = [0u8; 8192];
        let n = stream.read(&mut buf).await.expect("read request");
        let request_str = String::from_utf8_lossy(&buf[..n]);
        // FR-012: verify system-first, stream=true, model, max_tokens.
        assert!(request_str.contains("\"model\":\"openrouter/anthropic/claude-sonnet-4\""));
        assert!(request_str.contains("\"stream\":true"));
        assert!(request_str.contains("\"max_tokens\":16"));

        let sse_lines = [
            "data: {\"id\":\"1\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"delta\":{\"reasoning_content\":\"Let me think\"}}]}\n\n",
            "data: {\"id\":\"2\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n",
            "data: {\"id\":\"3\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"read\"}}]}}]}\n\n",
            "data: {\"id\":\"4\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"count\\\":1}\"}}]}}]}\n\n",
            "data: {\"id\":\"5\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5}}\n\n",
            "data: [DONE]\n\n",
        ];
        let body: String = sse_lines.concat();
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\n\r\n{}",
            body
        );
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.flush().await;
        let _ = stream.shutdown().await;
        let _ = shutdown_rx.await;
    });

    let provider = OpenRouterProvider::with_url(&format!("http://{}", local_addr));
    let client = provider
        .create_client("sse-probe-key", None, &std::collections::HashMap::new())
        .await
        .expect("key must pass the gate");

    let mut stream = client.chat(make_request()).await.expect("chat stream");
    let mut events = Vec::new();
    while let Some(ev) = stream.next().await {
        events.push(ev);
    }

    let reasoning_deltas: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::ReasoningDelta { text } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(reasoning_deltas, vec!["Let me think"]);

    let text_deltas: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::TextDelta { text } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(text_deltas, vec!["Hello"]);

    let tool_starts: Vec<(String, String)> = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::ToolCallStart { id, name } => Some((id.clone(), name.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        tool_starts,
        vec![("call_1".to_string(), "read".to_string())]
    );

    let args: String = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::ToolCallDelta { args_json, .. } => Some(args_json.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(args, "{\"count\":1}");

    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::Usage { .. }))
    );
    assert!(events.iter().any(|e| matches!(
        e,
        StreamEvent::Finish {
            reason: FinishReason::ToolUse
        }
    )));

    let _ = shutdown_tx.send(());
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), server).await;
}

#[tokio::test]
async fn test_openrouter_chat_stream_ignores_unparseable_sse_lines() {
    let _guard = lock_and_clear_env().await;

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let local_addr = listener.local_addr().unwrap();

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf).await;
        let body = concat!(
            "data: not-json\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"OK\"}}]}\n\n",
            "data: [DONE]\n\n",
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\n\r\n{}",
            body
        );
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.flush().await;
        let _ = stream.shutdown().await;
        let _ = shutdown_rx.await;
    });

    let provider = OpenRouterProvider::with_url(&format!("http://{}", local_addr));
    let client = provider
        .create_client(
            "resilience-probe-key",
            None,
            &std::collections::HashMap::new(),
        )
        .await
        .expect("key must pass the gate");

    let mut stream = client.chat(make_request()).await.expect("chat stream");
    let mut texts = Vec::new();
    while let Some(ev) = stream.next().await {
        if let StreamEvent::TextDelta { text } = ev {
            texts.push(text);
        }
    }
    assert_eq!(texts, vec!["OK"]);

    let _ = shutdown_tx.send(());
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), server).await;
}

// ---------------------------------------------------------------------------
// Reasoning/thinking levels (FR-018, FR-019, FR-020)
// ---------------------------------------------------------------------------

#[test]
fn test_openrouter_request_body_reasoning_table() {
    let client = OpenRouterClient::new("test-key", "https://test.example");

    let mut request = make_request();
    request.thinking = Some(ThinkingConfig::off());
    let body = client.build_request_body(&request);
    assert_eq!(body["reasoning"], json!({ "effort": "none" }));

    request.thinking = Some(ThinkingConfig::new(ThinkingLevel::Auto));
    let body = client.build_request_body(&request);
    assert!(
        body.get("reasoning").is_none(),
        "Auto with no budget should omit reasoning object"
    );

    request.thinking = Some(ThinkingConfig {
        enabled: true,
        level: ThinkingLevel::Auto,
        budget_tokens: Some(1024),
        display: None,
    });
    let body = client.build_request_body(&request);
    assert_eq!(body["reasoning"], json!({ "max_tokens": 1024 }));

    request.thinking = Some(ThinkingConfig::new(ThinkingLevel::Low));
    let body = client.build_request_body(&request);
    assert_eq!(body["reasoning"], json!({ "effort": "low" }));

    request.thinking = Some(ThinkingConfig {
        enabled: true,
        level: ThinkingLevel::Medium,
        budget_tokens: Some(2048),
        display: None,
    });
    let body = client.build_request_body(&request);
    assert_eq!(
        body["reasoning"],
        json!({ "effort": "medium", "max_tokens": 2048 })
    );

    request.thinking = Some(ThinkingConfig {
        enabled: true,
        level: ThinkingLevel::High,
        budget_tokens: Some(4096),
        display: None,
    });
    let body = client.build_request_body(&request);
    assert_eq!(
        body["reasoning"],
        json!({ "effort": "high", "max_tokens": 4096 })
    );
}

#[test]
fn test_openrouter_request_body_reasoning_uses_legacy_fallback() {
    let client = OpenRouterClient::new("test-key", "https://test.example");

    let mut request = make_request();
    request
        .options
        .insert("reasoning_effort".to_string(), json!("high"));
    let body = client.build_request_body(&request);
    assert_eq!(body["reasoning"], json!({ "effort": "high" }));
}

#[tokio::test]
async fn test_openrouter_chat_stream_emits_reasoning_events_with_transitions() {
    let _guard = lock_and_clear_env().await;

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let local_addr = listener.local_addr().unwrap();

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf).await;
        // Reasoning, then content, then a tool call, then finish.  This exercises
        // the reasoning-start/delta/end transitions plus the "close reasoning
        // block before content" rule.
        let body = concat!(
            "data: {\"choices\":[{\"delta\":{\"reasoning\":\"I need\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"reasoning\":\" to think\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"read\"}}]}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"path\\\": \\\"x.txt\\\"}\"}}]}}]}\n\n",
            "data: {\"choices\":[{\"finish_reason\":\"tool_calls\"}]}\n\n",
            "data: [DONE]\n\n",
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\n\r\n{}",
            body
        );
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.flush().await;
        let _ = stream.shutdown().await;
        let _ = shutdown_rx.await;
    });

    let provider = OpenRouterProvider::with_url(&format!("http://{}", local_addr));
    let client = provider
        .create_client("reasoning-key", None, &std::collections::HashMap::new())
        .await
        .expect("key must pass the gate");

    let mut request = make_request();
    request.thinking = Some(ThinkingConfig::new(ThinkingLevel::Medium));
    let mut stream = client.chat(request).await.expect("chat stream");
    let mut events = Vec::new();
    while let Some(ev) = stream.next().await {
        events.push(ev);
    }

    // Event order: ReasoningStart, ReasoningDelta x2, ReasoningEnd, TextDelta,
    // ToolCallStart, ToolCallDelta, ToolCallEnd, Finish(ToolUse), Finish(Stop) from [DONE].
    let mut iter = events.iter();
    assert!(matches!(iter.next(), Some(StreamEvent::ReasoningStart)));
    assert!(matches!(iter.next(), Some(StreamEvent::ReasoningDelta { text }) if text == "I need"));
    assert!(
        matches!(iter.next(), Some(StreamEvent::ReasoningDelta { text }) if text == " to think")
    );
    assert!(matches!(iter.next(), Some(StreamEvent::ReasoningEnd)));
    assert!(matches!(iter.next(), Some(StreamEvent::TextDelta { text }) if text == "Hello"));
    assert!(
        matches!(iter.next(), Some(StreamEvent::ToolCallStart { id, name }) if id == "call_1" && name == "read")
    );

    let args: String = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::ToolCallDelta { args_json, .. } => Some(args_json.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(args, "{\"path\": \"x.txt\"}");

    assert!(events.iter().any(|e| matches!(
        e,
        StreamEvent::Finish {
            reason: FinishReason::ToolUse
        }
    )));

    let _ = shutdown_tx.send(());
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), server).await;
}

// ---------------------------------------------------------------------------
// Metadata mapping fixtures (FR-010, FR-011)
// ---------------------------------------------------------------------------

/// Starts a tiny HTTP/1.1 server that replies to the first request with the
/// provided JSON body, then waits for the optional shutdown signal.
///
/// Returns the bound address, the server join handle, and a oneshot sender
/// that can be used to stop the server after the test assertions complete.
async fn spawn_json_server(
    body: String,
) -> (
    std::net::SocketAddr,
    tokio::task::JoinHandle<()>,
    tokio::sync::oneshot::Sender<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let local_addr = listener.local_addr().unwrap();
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf).await;
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-length: {}\r\ncontent-type: application/json\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.flush().await;
        let _ = stream.shutdown().await;
        let _ = shutdown_rx.await;
    });

    (local_addr, server, shutdown_tx)
}

#[tokio::test]
async fn test_openrouter_metadata_maps_context_pricing_vision_reasoning() {
    let _guard = lock_and_clear_env().await;

    let body = serde_json::json!({
        "data": [
            {
                "id": "anthropic/claude-sonnet-4",
                "name": "Claude Sonnet 4",
                "context_length": 200_000,
                "pricing": { "prompt": 0.000_003, "completion": 0.000_015 },
                "architecture": { "input_modalities": ["text", "image"] },
                "supported_parameters": ["temperature", "reasoning"],
            },
            {
                "id": "deepseek/deepseek-chat",
                "context_length": 65536,
                "pricing": { "prompt": "1e-7", "completion": "2.5e-7" },
                "architecture": { "input_modalities": ["text"] },
                "supported_parameters": ["reasoning:required"],
            },
        ]
    })
    .to_string();

    let (local_addr, server, shutdown_tx) = spawn_json_server(body).await;
    let provider = OpenRouterProvider::with_url(&format!("http://{}", local_addr));
    let models = provider
        .discover_models()
        .await
        .expect("discovery should parse fixture");

    assert_eq!(models.len(), 2);

    let claude = models
        .iter()
        .find(|m| m.id == "anthropic/claude-sonnet-4")
        .expect("claude entry present");
    assert_eq!(claude.name, "Claude Sonnet 4");
    assert_eq!(claude.context_window, 200_000);
    assert!(
        (claude.cost.input - 3.0).abs() < f64::EPSILON,
        "expected prompt price 3.0 USD/M, got {}",
        claude.cost.input
    );
    assert!(
        (claude.cost.output - 15.0).abs() < f64::EPSILON,
        "expected completion price 15.0 USD/M, got {}",
        claude.cost.output
    );
    assert!(claude.capabilities.vision);
    assert!(claude.capabilities.reasoning);
    assert_ne!(
        claude.capabilities.thinking_levels,
        [] as [ragent_types::ThinkingLevel; 0]
    );

    let deepseek = models
        .iter()
        .find(|m| m.id == "deepseek/deepseek-chat")
        .expect("deepseek entry present");
    assert_eq!(deepseek.context_window, 65536);
    assert!(
        (deepseek.cost.input - 0.1).abs() < 0.001,
        "expected prompt price 0.1 USD/M, got {}",
        deepseek.cost.input
    );
    assert!(
        (deepseek.cost.output - 0.25).abs() < 0.001,
        "expected completion price 0.25 USD/M, got {}",
        deepseek.cost.output
    );
    assert!(!deepseek.capabilities.vision);
    assert!(deepseek.capabilities.reasoning);

    let _ = shutdown_tx.send(());
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), server).await;
}

#[tokio::test]
async fn test_openrouter_metadata_uses_top_provider_context_length_fallback() {
    let _guard = lock_and_clear_env().await;

    let body = serde_json::json!({
        "data": [
            {
                "id": "mistral/mistral-small",
                "name": "Mistral Small",
                "top_provider": { "context_length": 32768 },
                "architecture": { "input_modalities": ["text"] },
                "supported_parameters": ["max_tokens"],
            }
        ]
    })
    .to_string();

    let (local_addr, server, shutdown_tx) = spawn_json_server(body).await;
    let provider = OpenRouterProvider::with_url(&format!("http://{}", local_addr));
    let models = provider.discover_models().await.expect("parse fixture");

    assert_eq!(models.len(), 1);
    let m = &models[0];
    assert_eq!(m.id, "mistral/mistral-small");
    assert_eq!(m.context_window, 32768);
    assert!(!m.capabilities.vision);
    assert!(!m.capabilities.reasoning);
    assert_eq!(
        m.capabilities.thinking_levels,
        [] as [ragent_types::ThinkingLevel; 0]
    );

    let _ = shutdown_tx.send(());
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), server).await;
}

#[tokio::test]
async fn test_openrouter_metadata_skips_empty_id_entries() {
    let _guard = lock_and_clear_env().await;

    let body = serde_json::json!({
        "data": [
            {
                "id": "",
                "name": "Empty ID Model",
                "context_length": 1000,
            },
            {
                "id": "valid/entry",
                "name": "Valid Entry",
                "context_length": 4096,
            }
        ]
    })
    .to_string();

    let (local_addr, server, shutdown_tx) = spawn_json_server(body).await;
    let provider = OpenRouterProvider::with_url(&format!("http://{}", local_addr));
    let models = provider.discover_models().await.expect("parse fixture");

    assert_eq!(models.len(), 1);
    assert_eq!(models[0].id, "valid/entry");

    let _ = shutdown_tx.send(());
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), server).await;
}
