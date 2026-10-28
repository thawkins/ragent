//! Inline tests for `gemini.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn test_gemini_discovered_model_to_info_uses_live_thinking_flag() {
    let defaults = gemini_default_models("gemini")
        .into_iter()
        .map(|model| (model.id.clone(), model))
        .collect::<HashMap<_, _>>();
    let model = GeminiDiscoveredModel {
        name: "models/gemini-2.5-pro-preview-05-06".to_string(),
        base_model_id: Some("gemini-2.5-pro-preview-05-06".to_string()),
        display_name: Some("Gemini 2.5 Pro Preview".to_string()),
        input_token_limit: Some(2_000_000),
        output_token_limit: Some(65_536),
        supported_generation_methods: vec!["generateContent".to_string()],
        thinking: Some(true),
    };

    let model = gemini_discovered_model_to_info(model, &defaults).expect("model info");
    assert!(model.capabilities.reasoning);
    assert_eq!(
        model.capabilities.thinking_levels,
        gemini_thinking_levels_for_model("gemini-2.5-pro-preview-05-06")
    );
    assert_eq!(model.context_window, 2_000_000);
    assert_eq!(model.max_output, Some(65_536));
}

// ---------------------------------------------------------------------------
// build_request_body + chat() coverage
// ---------------------------------------------------------------------------

use crate::llm::{ChatRequest, LlmClient, StreamEvent};
use ragent_types::event::FinishReason;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[test]
fn test_gemini_debug_redacts_api_key() {
    let client = GeminiClient::new("super-secret-key", "https://example.com");
    let debug = format!("{client:?}");
    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains("super-secret-key"));
}

#[test]
fn test_gemini_build_request_body_parts_system_and_tools() {
    use crate::llm::ContentPart;
    let client = GeminiClient::new("k", "https://example.com");
    let req = ChatRequest {
        model: "gemini-2.5-pro".to_string(),
        messages: Arc::new(vec![crate::llm::ChatMessage {
            role: "assistant".to_string(),
            content: ChatContent::Parts(vec![
                ContentPart::Text {
                    text: "answer".to_string(),
                },
                ContentPart::ImageUrl {
                    url: "data:image/png;base64,QUJD".to_string(),
                },
                ContentPart::ToolResult {
                    tool_use_id: "call_1".to_string(),
                    content: Arc::from("42"),
                    is_error: false,
                },
                ContentPart::ToolUse {
                    id: "x".to_string(),
                    name: "calc".to_string(),
                    input: serde_json::json!({"expr": "1+1"}),
                },
            ]),
        }]),
        tools: Arc::new(vec![crate::llm::ToolDefinition {
            name: "calc".to_string(),
            description: "Calculate".to_string(),
            parameters: serde_json::json!({"type": "object"}),
        }]),
        temperature: Some(0.4),
        top_p: Some(0.7),
        max_tokens: Some(128),
        system: Some(Arc::from("be terse")),
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    };
    let body = client.build_request_body(&req);
    assert_eq!(body["systemInstruction"]["parts"][0]["text"], "be terse");
    assert_eq!(body["contents"][0]["role"], "model");
    let parts = &body["contents"][0]["parts"];
    assert_eq!(parts[0]["text"], "answer");
    assert_eq!(parts[1]["inlineData"]["mimeType"], "image/png");
    assert_eq!(parts[1]["inlineData"]["data"], "QUJD");
    assert_eq!(parts[2]["functionResponse"]["response"]["result"], "42");
    assert_eq!(parts[3]["functionCall"]["name"], "calc");
    assert_eq!(body["generationConfig"]["maxOutputTokens"], 128);
    assert!(body["tools"].is_array());
}

fn gemini_request() -> ChatRequest {
    ChatRequest {
        model: "gemini-2.5-pro".to_string(),
        messages: Arc::new(vec![crate::llm::ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("hi".to_string()),
        }]),
        tools: Arc::new(vec![]),
        temperature: None,
        top_p: None,
        max_tokens: Some(32),
        system: None,
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: Some(5),
        thinking: None,
    }
}

async fn spawn_gemini_body(body: String) -> String {
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
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}"
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
    });
    format!("http://{addr}")
}

#[tokio::test]
async fn test_gemini_chat_text_tool_call_and_finish() {
    let body = concat!(
        "{\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Hi\"}]}}]}\n",
        "{\"candidates\":[{\"content\":{\"parts\":[{\"functionCall\":{\"name\":\"calc\",\"args\":{\"expr\":\"1+1\"}}}]},\"finishReason\":\"STOP\"}],\"usageMetadata\":{\"promptTokenCount\":5,\"candidatesTokenCount\":2}}\n",
    )
    .to_string();
    let url = spawn_gemini_body(body).await;
    let client = GeminiClient::new("k", &url);
    let events: Vec<StreamEvent> = client
        .chat(gemini_request())
        .await
        .expect("chat")
        .collect()
        .await;
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::TextDelta { text } if text == "Hi"))
    );
    assert!(events.iter().any(|e| matches!(
        e,
        StreamEvent::ToolCallStart { name, .. } if name == "calc"
    )));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::Finish { reason } if *reason == FinishReason::Stop))
    );
    assert!(events.iter().any(|e| matches!(
        e,
        StreamEvent::Usage {
            input_tokens: 5,
            output_tokens: 2
        }
    )));
}

#[tokio::test]
async fn test_gemini_chat_http_error() {
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
        let body = "{\"error\":{\"message\":\"bad key\"}}";
        let response = format!(
            "HTTP/1.1 403 Forbidden\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
    });
    let err = match GeminiClient::new("k", &format!("http://{addr}"))
        .chat(gemini_request())
        .await
    {
        Ok(_) => panic!("expected error"),
        Err(e) => e.to_string(),
    };
    assert!(err.contains("403"), "error: {err}");
}

#[test]
fn test_gemini_discovered_model_unknown_id_thinking_uses_full_levels() {
    // An unrecognised model id with thinking=true falls back to the full
    // reasoning level set (the heuristic returns empty for unknown ids).
    let defaults = gemini_default_models("gemini")
        .into_iter()
        .map(|model| (model.id.clone(), model))
        .collect::<HashMap<_, _>>();
    let model = GeminiDiscoveredModel {
        name: "models/some-future-model".to_string(),
        base_model_id: None,
        display_name: None,
        input_token_limit: None,
        output_token_limit: None,
        supported_generation_methods: vec!["streamGenerateContent".to_string()],
        thinking: Some(true),
    };
    let info = gemini_discovered_model_to_info(model, &defaults).expect("model info");
    assert_eq!(info.id, "some-future-model");
    assert!(info.capabilities.reasoning);
    assert_eq!(info.capabilities.thinking_levels, full_reasoning_levels());
}

#[test]
fn test_gemini_discovered_model_rejects_non_generate_methods() {
    let defaults = HashMap::new();
    let model = GeminiDiscoveredModel {
        name: "models/embed-only".to_string(),
        base_model_id: None,
        display_name: None,
        input_token_limit: None,
        output_token_limit: None,
        supported_generation_methods: vec!["embedContent".to_string()],
        thinking: None,
    };
    assert!(gemini_discovered_model_to_info(model, &defaults).is_none());
}

#[test]
fn test_gemini_build_request_body_non_data_image_url_becomes_text() {
    use crate::llm::ContentPart;
    let client = GeminiClient::new("k", "https://example.com");
    let req = ChatRequest {
        model: "gemini-2.5-pro".to_string(),
        messages: Arc::new(vec![crate::llm::ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Parts(vec![ContentPart::ImageUrl {
                url: "https://example.com/pic.png".to_string(),
            }]),
        }]),
        tools: Arc::new(vec![]),
        temperature: None,
        top_p: None,
        max_tokens: Some(16),
        system: None,
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    };
    let body = client.build_request_body(&req);
    assert_eq!(
        body["contents"][0]["parts"][0]["text"],
        "[Image: https://example.com/pic.png]"
    );
}
