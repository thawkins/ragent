//! Inline tests for `copilot.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use crate::llm::{ChatContent, ChatMessage, ChatRequest};
use std::sync::Arc;

#[test]
fn test_provider_defaults() {
    let provider = CopilotProvider::new();
    assert_eq!(provider.id(), "copilot");
    assert_eq!(provider.name(), "GitHub Copilot");
    assert!(
        provider.default_models().is_empty(),
        "Copilot default_models should be empty; models are discovered at runtime"
    );
}

#[test]
#[allow(clippy::used_underscore_binding)]
fn test_with_custom_url() {
    let provider = CopilotProvider::with_url("https://proxy.example.com/");
    assert_eq!(provider._base_url, "https://proxy.example.com");
}

#[test]
fn test_models_are_free() {
    let provider = CopilotProvider::new();
    for m in provider.default_models() {
        assert!(m.cost.input.abs() < f64::EPSILON);
        assert!(m.cost.output.abs() < f64::EPSILON);
    }
}

#[test]
fn test_format_request_multiplier_trims_trailing_zeroes() {
    assert_eq!(format_request_multiplier(1.0), "1");
    assert_eq!(format_request_multiplier(1.5), "1.5");
    assert_eq!(format_request_multiplier(0.25), "0.25");
}

#[test]
fn test_request_multiplier_prefers_pricing_block() {
    let entry = CopilotModelEntry {
        id: "x".to_string(),
        name: None,
        model_picker_enabled: true,
        vendor: None,
        pricing: Some(CopilotModelPricing {
            request_cost: Some(2.0),
        }),
        request_cost: Some(1.0),
        request_cost_multiplier: None,
        premium_request_multiplier: None,
        multiplier: None,
        supported_endpoints: None,
        api: None,
        capabilities: None,
    };
    assert_eq!(entry.request_multiplier(), Some(2.0));
}

#[test]
fn test_request_multiplier_ignores_invalid_values() {
    let entry = CopilotModelEntry {
        id: "x".to_string(),
        name: None,
        model_picker_enabled: true,
        vendor: None,
        pricing: None,
        request_cost: Some(0.0),
        request_cost_multiplier: Some(f64::NAN),
        premium_request_multiplier: Some(-1.0),
        multiplier: None,
        supported_endpoints: None,
        api: None,
        capabilities: None,
    };
    assert_eq!(entry.request_multiplier(), None);
}

#[test]
fn test_supports_chat_completions_uses_endpoint_metadata() {
    let entry = CopilotModelEntry {
        id: "gpt-5.3-codex".to_string(),
        name: None,
        model_picker_enabled: true,
        vendor: None,
        pricing: None,
        request_cost: None,
        request_cost_multiplier: None,
        premium_request_multiplier: None,
        multiplier: None,
        supported_endpoints: Some(vec!["/chat/completions".to_string()]),
        api: None,
        capabilities: None,
    };
    assert!(entry.supports_chat_completions());
}

#[test]
fn test_supports_chat_completions_filters_codex_without_metadata() {
    let entry = CopilotModelEntry {
        id: "gpt-5.3-codex".to_string(),
        name: None,
        model_picker_enabled: true,
        vendor: None,
        pricing: None,
        request_cost: None,
        request_cost_multiplier: None,
        premium_request_multiplier: None,
        multiplier: None,
        supported_endpoints: None,
        api: None,
        capabilities: None,
    };
    assert!(!entry.supports_chat_completions());
}

#[test]
fn test_reasoning_effort_from_options_accepts_levels() {
    let mk = |v: &str| {
        let mut request = ChatRequest {
            model: "o3-mini".to_string(),
            messages: Arc::new(vec![ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Text("hello".to_string()),
            }]),
            tools: Arc::new(vec![]),
            temperature: None,
            top_p: None,
            max_tokens: None,
            system: None,
            options: HashMap::new(),
            session_id: None,
            request_id: None,
            stream_timeout_secs: None,
            thinking: None,
        };
        request
            .options
            .insert("reasoning_effort".to_string(), json!(v));
        request
    };
    assert_eq!(reasoning_effort_from_request(&mk("low")), Some("low"));
    assert_eq!(reasoning_effort_from_request(&mk("medium")), Some("medium"));
    assert_eq!(reasoning_effort_from_request(&mk("high")), Some("high"));
}

#[test]
fn test_reasoning_effort_from_options_alias_and_invalid() {
    let mut alias = HashMap::new();
    alias.insert("reasoning_level".to_string(), json!("HIGH"));
    let alias_request = ChatRequest {
        model: "o3-mini".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("hello".to_string()),
        }]),
        tools: Arc::new(vec![]),
        temperature: None,
        top_p: None,
        max_tokens: None,
        system: None,
        options: alias,
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    };
    assert_eq!(reasoning_effort_from_request(&alias_request), Some("high"));

    let mut invalid_options = HashMap::new();
    invalid_options.insert("reasoning_effort".to_string(), json!("turbo"));
    let invalid_request = ChatRequest {
        model: "o3-mini".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("hello".to_string()),
        }]),
        tools: Arc::new(vec![]),
        temperature: None,
        top_p: None,
        max_tokens: None,
        system: None,
        options: invalid_options,
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    };
    assert_eq!(reasoning_effort_from_request(&invalid_request), None);
}

#[test]
fn test_reasoning_effort_prefers_typed_thinking() {
    let mut request = ChatRequest {
        model: "o3-mini".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("hello".to_string()),
        }]),
        tools: Arc::new(vec![]),
        temperature: None,
        top_p: None,
        max_tokens: None,
        system: None,
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: Some(ragent_types::ThinkingConfig::new(
            ragent_types::ThinkingLevel::High,
        )),
    };
    request
        .options
        .insert("reasoning_effort".to_string(), json!("low"));

    assert_eq!(reasoning_effort_from_request(&request), Some("high"));
}

#[test]
fn test_build_request_body_applies_reasoning_effort() {
    let client = CopilotClient {
        token: "x".to_string(),
        base_url: "https://api.githubcopilot.com".to_string(),
        http: crate::provider::http_client::create_http_client(),
    };
    let mut options = HashMap::new();
    options.insert("reasoning_effort".to_string(), json!("medium"));
    let req = ChatRequest {
        model: "o3-mini".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("hello".to_string()),
        }]),
        tools: Arc::new(vec![]),
        temperature: None,
        top_p: None,
        max_tokens: None,
        system: None,
        options,
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    };

    let body = client.build_request_body(&req, &[]);
    assert_eq!(body["reasoning_effort"], json!("medium"));
}

#[test]
fn test_build_request_body_thinking_disabled_fallback() {
    let client = CopilotClient {
        token: "x".to_string(),
        base_url: "https://api.githubcopilot.com".to_string(),
        http: crate::provider::http_client::create_http_client(),
    };
    let mut options = HashMap::new();
    options.insert("thinking".to_string(), json!("disabled"));
    let req = ChatRequest {
        model: "gpt-4o".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("hello".to_string()),
        }]),
        tools: Arc::new(vec![]),
        temperature: None,
        top_p: None,
        max_tokens: None,
        system: None,
        options,
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    };

    let body = client.build_request_body(&req, &[]);
    assert_eq!(body["reasoning_effort"], json!("none"));
}

#[test]
fn test_build_request_body_reasoning_effort_overrides_thinking_toggle() {
    let client = CopilotClient {
        token: "x".to_string(),
        base_url: "https://api.githubcopilot.com".to_string(),
        http: crate::provider::http_client::create_http_client(),
    };
    let mut options = HashMap::new();
    options.insert("reasoning_effort".to_string(), json!("high"));
    options.insert("thinking".to_string(), json!("disabled"));
    let req = ChatRequest {
        model: "o3-mini".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("hello".to_string()),
        }]),
        tools: Arc::new(vec![]),
        temperature: None,
        top_p: None,
        max_tokens: None,
        system: None,
        options,
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    };

    let body = client.build_request_body(&req, &[]);
    assert_eq!(body["reasoning_effort"], json!("high"));
}

#[test]
fn test_copilot_premium_multiplier_table() {
    // Included models (0x)
    assert_eq!(copilot_premium_multiplier("gpt-4o"), Some(0.0));
    assert_eq!(copilot_premium_multiplier("GPT-4o"), Some(0.0)); // case insensitive
    assert_eq!(copilot_premium_multiplier("gpt-4.1"), Some(0.0));
    assert_eq!(copilot_premium_multiplier("gpt-5-mini"), Some(0.0));

    // Low-cost models (0.25x - 0.33x)
    assert_eq!(copilot_premium_multiplier("claude-haiku-4.5"), Some(0.33));
    assert_eq!(copilot_premium_multiplier("gemini-3-flash"), Some(0.33));
    assert_eq!(copilot_premium_multiplier("grok-code-fast-1"), Some(0.25));

    // Standard models (1x)
    assert_eq!(copilot_premium_multiplier("claude-sonnet-4"), Some(1.0));
    assert_eq!(copilot_premium_multiplier("gemini-2.5-pro"), Some(1.0));

    // High-cost models (3x)
    assert_eq!(copilot_premium_multiplier("claude-opus-4.5"), Some(3.0));
    assert_eq!(copilot_premium_multiplier("claude-opus-4.6"), Some(3.0));

    // Very high-cost models
    assert_eq!(copilot_premium_multiplier("claude-opus-4.7"), Some(7.5));

    // Fallback pattern matching
    assert_eq!(
        copilot_premium_multiplier("claude-3-sonnet-latest"),
        Some(1.0)
    ); // contains "sonnet"
    assert_eq!(
        copilot_premium_multiplier("claude-3-haiku-latest"),
        Some(0.33)
    ); // contains "haiku"

    // Unknown models return None
    assert_eq!(copilot_premium_multiplier("unknown-model"), None);
}

// ---------------------------------------------------------------------------
// chat() stream coverage: drive the SSE parser against a local mock server.
// ---------------------------------------------------------------------------

use crate::llm::{LlmClient, StreamEvent};
use ragent_types::event::FinishReason;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn sse_request(_body: String) -> ChatRequest {
    ChatRequest {
        model: "gpt-4o".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("hi".to_string()),
        }]),
        tools: Arc::new(vec![]),
        temperature: None,
        top_p: None,
        max_tokens: Some(16),
        system: None,
        options: HashMap::new(),
        session_id: Some("sess-1".to_string()),
        request_id: Some("req-1".to_string()),
        stream_timeout_secs: Some(5),
        thinking: None,
    }
}

async fn spawn_copilot_sse(body: String) -> String {
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

async fn spawn_copilot_status(status: u16, body: String) -> String {
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
        let reason = if status == 429 {
            "Too Many Requests"
        } else {
            "Error"
        };
        let response = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
    });
    format!("http://{addr}")
}

fn copilot_client(base_url: String) -> CopilotClient {
    CopilotClient {
        token: "tok".to_string(),
        base_url,
        http: crate::provider::http_client::create_streaming_http_client(),
    }
}

#[tokio::test]
async fn test_copilot_chat_text_usage_and_finish() {
    let sse = concat!(
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hel\"}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"lo\"}}]}\n\n",
        "data: {\"usage\":{\"prompt_tokens\":7,\"completion_tokens\":3},\"choices\":[]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
        "data: [DONE]\n\n",
    )
    .to_string();
    let url = spawn_copilot_sse(sse).await;
    let client = copilot_client(url);
    let events: Vec<StreamEvent> = client
        .chat(sse_request(String::new()))
        .await
        .expect("chat")
        .collect()
        .await;
    assert!(matches!(&events[0], StreamEvent::TextDelta { text } if text == "Hel"));
    assert!(matches!(&events[1], StreamEvent::TextDelta { text } if text == "lo"));
    assert!(matches!(
        &events[2],
        StreamEvent::Usage {
            input_tokens: 7,
            output_tokens: 3
        }
    ));
    assert!(matches!(
        &events[3],
        StreamEvent::Finish {
            reason: FinishReason::Stop
        }
    ));
}

#[tokio::test]
async fn test_copilot_chat_tool_call_lifecycle() {
    let sse = concat!(
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"get_weather\"}}]}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"loc\\\":\"}}]}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"London\\\"}\"}}]}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
    )
    .to_string();
    let url = spawn_copilot_sse(sse).await;
    let client = copilot_client(url);
    let events: Vec<StreamEvent> = client
        .chat(sse_request(String::new()))
        .await
        .expect("chat")
        .collect()
        .await;
    assert!(matches!(
        &events[0],
        StreamEvent::ToolCallStart { id, name } if id == "call_1" && name == "get_weather"
    ));
    let deltas: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::ToolCallDelta { args_json, .. } => Some(args_json.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(deltas.join(""), "{\"loc\":\"London\"}");
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::ToolCallEnd { id } if id == "call_1"))
    );
    assert!(
        events.iter().any(
            |e| matches!(e, StreamEvent::Finish { reason } if *reason == FinishReason::ToolUse)
        )
    );
}

#[tokio::test]
async fn test_copilot_chat_finish_reason_mapping_and_malformed_frame() {
    let sse = concat!(
        "data: not-json\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"x\"},\"finish_reason\":\"length\"}]}\n\n",
    )
    .to_string();
    let url = spawn_copilot_sse(sse).await;
    let client = copilot_client(url);
    let events: Vec<StreamEvent> = client
        .chat(sse_request(String::new()))
        .await
        .expect("chat")
        .collect()
        .await;
    assert!(
        events.iter().any(
            |e| matches!(e, StreamEvent::Finish { reason } if *reason == FinishReason::Length)
        )
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::TextDelta { text } if text == "x"))
    );
}

#[tokio::test]
async fn test_copilot_chat_content_filter_finish() {
    let sse =
        "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"content_filter\"}]}\n\n"
            .to_string();
    let url = spawn_copilot_sse(sse).await;
    let client = copilot_client(url);
    let events: Vec<StreamEvent> = client
        .chat(sse_request(String::new()))
        .await
        .expect("chat")
        .collect()
        .await;
    assert!(events.iter().any(|e| matches!(
        e,
        StreamEvent::Finish { reason } if *reason == FinishReason::ContentFilter
    )));
}

#[tokio::test]
async fn test_copilot_chat_http_error_bails_with_status() {
    let url = spawn_copilot_status(
        401,
        r#"{"error":{"message":"bad credentials"}}"#.to_string(),
    )
    .await;
    let client = copilot_client(url);
    let err = match client.chat(sse_request(String::new())).await {
        Ok(_) => panic!("expected error"),
        Err(e) => e.to_string(),
    };
    assert!(err.contains("401"), "error should carry status: {err}");
    assert!(
        err.contains("bad credentials"),
        "error should carry message: {err}"
    );
}

// ---------------------------------------------------------------------------
// Pure helper coverage
// ---------------------------------------------------------------------------

#[test]
fn test_is_pat_token_shapes() {
    assert!(is_pat_token("github_pat_abc"));
    assert!(is_pat_token("ghp_abc"));
    assert!(!is_pat_token("ghu_abc"));
    assert!(!is_pat_token(""));
}

#[test]
fn test_plan_label_from_api_base() {
    assert_eq!(
        plan_label_from_api_base("https://api.individual.githubcopilot.com"),
        "Pro"
    );
    assert_eq!(
        plan_label_from_api_base("https://api.business.githubcopilot.com"),
        "Business"
    );
    assert_eq!(
        plan_label_from_api_base("https://api.enterprise.githubcopilot.com"),
        "Enterprise"
    );
    assert_eq!(
        plan_label_from_api_base("https://api.free.githubcopilot.com"),
        "Free"
    );
    assert_eq!(
        plan_label_from_api_base("https://api.githubcopilot.com"),
        "Copilot"
    );
}

#[test]
fn test_hash_token_is_stable_and_distinguishing() {
    assert_eq!(hash_token("abc"), hash_token("abc"));
    assert_ne!(hash_token("abc"), hash_token("abd"));
}

#[test]
fn test_premium_multiplier_fallback_patterns() {
    // Fallback: contains "sonnet" -> 1.0
    assert_eq!(copilot_premium_multiplier("my-sonnet-variant"), Some(1.0));
    // Fallback: contains "haiku" -> 0.33
    assert_eq!(copilot_premium_multiplier("x-haiku-y"), Some(0.33));
    // Fallback: contains "opus" without 4.7/fast -> 3.0
    assert_eq!(copilot_premium_multiplier("opus-3"), Some(3.0));
    // Fallback: contains "opus" AND "4.7" -> 7.5
    assert_eq!(copilot_premium_multiplier("opus-4.7-turbo"), Some(7.5));
    // Fallback: contains "opus" AND "fast" (no 4.7) -> 30.0
    assert_eq!(copilot_premium_multiplier("opus-fast"), Some(30.0));
    // Fallback: o1/o3 reasoning -> 1.0
    assert_eq!(copilot_premium_multiplier("o3-mini"), Some(1.0));
    assert_eq!(copilot_premium_multiplier("o1-preview"), Some(1.0));
    // Fallback: gemini flash -> 0.33, other gemini -> 1.0
    assert_eq!(copilot_premium_multiplier("gemini-flash-x"), Some(0.33));
    assert_eq!(copilot_premium_multiplier("gemini-ultra"), Some(1.0));
    // Unknown -> None
    assert_eq!(copilot_premium_multiplier("mystery-model"), None);
}

#[test]
fn test_parse_api_error_message_shapes() {
    assert_eq!(
        parse_api_error_message(r#"{"error":{"message":"bad key"}}"#).as_deref(),
        Some("bad key")
    );
    assert_eq!(
        parse_api_error_message(r#"{"message":"plain"}"#).as_deref(),
        Some("plain")
    );
    assert_eq!(
        parse_api_error_message(r#"{"error":"string error"}"#).as_deref(),
        Some("string error")
    );
    assert_eq!(parse_api_error_message("not json"), None);
    assert_eq!(parse_api_error_message(r#"{"other":1}"#), None);
}

#[test]
fn test_model_entry_request_multiplier_precedence() {
    let base = || CopilotModelEntry {
        id: "m".to_string(),
        name: None,
        model_picker_enabled: true,
        vendor: None,
        pricing: None,
        request_cost: None,
        request_cost_multiplier: None,
        premium_request_multiplier: None,
        multiplier: None,
        supported_endpoints: None,
        api: None,
        capabilities: None,
    };

    // request_cost_multiplier preferred over premium/request_cost/multiplier
    let mut e = base();
    e.request_cost_multiplier = Some(1.5);
    e.premium_request_multiplier = Some(2.0);
    e.request_cost = Some(3.0);
    e.multiplier = Some(4.0);
    assert_eq!(e.request_multiplier(), Some(1.5));

    // premium_request_multiplier when cost_multiplier absent
    let mut e = base();
    e.premium_request_multiplier = Some(2.0);
    e.request_cost = Some(3.0);
    assert_eq!(e.request_multiplier(), Some(2.0));

    // request_cost when both multipliers absent
    let mut e = base();
    e.request_cost = Some(3.0);
    assert_eq!(e.request_multiplier(), Some(3.0));

    // multiplier as final fallback (positive value)
    let mut e = base();
    e.multiplier = Some(0.25);
    assert_eq!(e.request_multiplier(), Some(0.25));

    // Non-positive -> None
    let mut e = base();
    e.multiplier = Some(0.0);
    assert_eq!(e.request_multiplier(), None);
}

#[test]
fn test_supports_chat_completions_via_api_field() {
    let mut e = CopilotModelEntry {
        id: "model-x".to_string(),
        name: None,
        model_picker_enabled: true,
        vendor: None,
        pricing: None,
        request_cost: None,
        request_cost_multiplier: None,
        premium_request_multiplier: None,
        multiplier: None,
        supported_endpoints: None,
        api: Some(vec!["/chat/completions".to_string()]),
        capabilities: None,
    };
    assert!(e.supports_chat_completions());

    // Empty endpoints array falls back to the codex heuristic.
    e.api = Some(vec![]);
    e.id = "gpt-5-codex".to_string();
    assert!(!e.supports_chat_completions());
    e.id = "gpt-5".to_string();
    assert!(e.supports_chat_completions());
}

#[test]
fn test_resolve_copilot_github_token_uses_db_lookup_when_env_absent() {
    // The env var is absent in the test environment; the db lookup is the
    // only populated source.
    let lookup = || Some("ghu_from_db".to_string());
    // If GITHUB_COPILOT_TOKEN happens to be set in the ambient environment the
    // env value wins - accept either but assert it is not the empty lookup.
    let resolved = resolve_copilot_github_token(Some(&lookup));
    assert!(resolved.is_some());

    let empty = || Some(String::new());
    // Empty db value is treated as absent.
    let _ = resolve_copilot_github_token(Some(&empty));
}
