//! Inline tests for `bedrock.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use ragent_types::ToolDefinition;

use super::*;

#[test]
fn test_bedrock_provider_id_and_name() {
    let provider = BedrockProvider;
    assert_eq!(provider.id(), "bedrock");
    assert_eq!(provider.name(), "Amazon Bedrock");
}

#[test]
fn test_bedrock_default_models_non_empty() {
    let models = bedrock_default_models();
    assert!(!models.is_empty());
    assert!(models.len() >= 8, "Expected at least 8 default models");

    // All models should have bedrock provider_id
    for model in &models {
        assert_eq!(model.provider_id, "bedrock");
    }
}

#[test]
fn test_is_anthropic_model() {
    assert!(is_anthropic_model(
        "anthropic.claude-sonnet-4-20250514-v1:0"
    ));
    assert!(is_anthropic_model(
        "anthropic.claude-3-5-haiku-20241022-v1:0"
    ));
    assert!(!is_anthropic_model("amazon.nova-pro-v1:0"));
    assert!(!is_anthropic_model(
        "meta.llama4-maverick-17b-instruct-v1:0"
    ));
    assert!(!is_anthropic_model("mistral.mistral-large-2407-v1:0"));
}

#[test]
fn test_strip_bedrock_suffix() {
    assert_eq!(
        strip_bedrock_suffix("claude-sonnet-4-20250514@bedrock"),
        "claude-sonnet-4-20250514"
    );
    assert_eq!(
        strip_bedrock_suffix("anthropic.claude-sonnet-4-20250514-v1:0"),
        "anthropic.claude-sonnet-4-20250514-v1:0"
    );
    assert_eq!(strip_bedrock_suffix("no-suffix"), "no-suffix");
}

#[test]
fn test_resolve_bedrock_model_id_short_aliases() {
    assert_eq!(
        resolve_bedrock_model_id("claude-sonnet-4-20250514"),
        "anthropic.claude-sonnet-4-20250514-v1:0"
    );
    assert_eq!(
        resolve_bedrock_model_id("claude-opus-4-20250514"),
        "anthropic.claude-opus-4-20250514-v1:0"
    );
    assert_eq!(resolve_bedrock_model_id("nova-pro"), "amazon.nova-pro-v1:0");
    assert_eq!(
        resolve_bedrock_model_id("nova-lite"),
        "amazon.nova-lite-v1:0"
    );
    assert_eq!(
        resolve_bedrock_model_id("nova-micro"),
        "amazon.nova-micro-v1:0"
    );
}

#[test]
fn test_resolve_bedrock_model_id_full_id_passthrough() {
    // Full Bedrock model IDs should pass through unchanged
    assert_eq!(
        resolve_bedrock_model_id("anthropic.claude-sonnet-4-20250514-v1:0"),
        "anthropic.claude-sonnet-4-20250514-v1:0"
    );
}

#[test]
fn test_resolve_bedrock_model_id_with_suffix() {
    assert_eq!(
        resolve_bedrock_model_id("claude-sonnet-4-20250514@bedrock"),
        "anthropic.claude-sonnet-4-20250514-v1:0"
    );
}

#[test]
fn test_build_bedrock_base_url_default() {
    let url = build_bedrock_base_url("us-east-1", None);
    assert_eq!(url, "https://bedrock.us-east-1.amazonaws.com");
}

#[test]
fn test_build_bedrock_base_url_custom_endpoint() {
    let url = build_bedrock_base_url("us-east-1", Some("https://my-vpc-endbedrock.example.com"));
    assert_eq!(url, "https://my-vpc-endbedrock.example.com");
}

#[test]
fn test_build_bedrock_base_url_trailing_slash() {
    let url = build_bedrock_base_url("eu-west-1", Some("https://endpoint.example.com/"));
    assert_eq!(url, "https://endpoint.example.com");
}

#[test]
fn test_converse_tool_config() {
    let tools = vec![ToolDefinition {
        name: "get_weather".to_string(),
        description: "Get weather for a location".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "location": { "type": "string" }
            }
        }),
    }];

    let config = cached_tools(ToolFormat::Bedrock, &tools).bedrock_tool_config_object();

    // Should have tools array with toolSpec.
    let tool_specs = config["tools"].as_array().unwrap();
    assert_eq!(tool_specs.len(), 1);
    assert_eq!(tool_specs[0]["toolSpec"]["name"], "get_weather");
    assert_eq!(
        tool_specs[0]["toolSpec"]["description"],
        "Get weather for a location"
    );
    // inputSchema should wrap parameters in "json" key
    assert!(tool_specs[0]["toolSpec"]["inputSchema"]["json"].is_object());
}

#[test]
fn test_mime_to_converse_format() {
    assert_eq!(mime_to_converse_format("image/png"), "png");
    assert_eq!(mime_to_converse_format("image/jpeg"), "jpeg");
    assert_eq!(mime_to_converse_format("image/gif"), "gif");
    assert_eq!(mime_to_converse_format("image/webp"), "webp");
    assert_eq!(mime_to_converse_format("image/unknown"), "png"); // Default
}

#[test]
fn test_bedrock_anthropic_request_body() {
    let creds = AwsCredentials {
        access_key: "test".to_string(),
        secret_key: "test".to_string(),
        session_token: None,
        region: "us-east-1".to_string(),
    };
    let client = BedrockAnthropicClient {
        credentials: creds,
        base_url: "https://bedrock.us-east-1.amazonaws.com".to_string(),
        model_id: "anthropic.claude-sonnet-4-20250514-v1:0".to_string(),
        http: crate::provider::http_client::create_streaming_http_client(),
    };

    let request = ChatRequest {
        model: "anthropic.claude-sonnet-4-20250514-v1:0".to_string(),
        messages: std::sync::Arc::new(vec![crate::llm::ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("Hello".to_string()),
        }]),
        tools: std::sync::Arc::new(vec![]),
        temperature: None,
        top_p: None,
        max_tokens: Some(1024),
        system: Some(std::sync::Arc::from("You are helpful")),
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    };

    let body = client.build_request_body(&request);

    // Should have anthropic_version for Bedrock
    assert_eq!(body["anthropic_version"], "bedrock-2023-06-01");
    // Should have system prompt
    assert_eq!(body["system"], "You are helpful");
    // Should have max_tokens
    assert_eq!(body["max_tokens"], 1024);
    // Should have stream enabled
    assert_eq!(body["stream"], true);
}

#[test]
fn test_bedrock_converse_request_body() {
    let creds = AwsCredentials {
        access_key: "test".to_string(),
        secret_key: "test".to_string(),
        session_token: None,
        region: "us-east-1".to_string(),
    };
    let client = BedrockConverseClient {
        credentials: creds,
        base_url: "https://bedrock.us-east-1.amazonaws.com".to_string(),
        model_id: "amazon.nova-pro-v1:0".to_string(),
        http: crate::provider::http_client::create_streaming_http_client(),
    };

    let request = ChatRequest {
        model: "amazon.nova-pro-v1:0".to_string(),
        messages: std::sync::Arc::new(vec![crate::llm::ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("Hello".to_string()),
        }]),
        tools: std::sync::Arc::new(vec![ToolDefinition {
            name: "get_time".to_string(),
            description: "Get current time".to_string(),
            parameters: json!({"type": "object"}),
        }]),
        temperature: Some(0.7),
        top_p: None,
        max_tokens: Some(2048),
        system: Some(std::sync::Arc::from("Be concise")),
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    };

    let body = client.build_request_body(&request);

    // Should have system as array with text
    let system = body["system"].as_array().unwrap();
    assert_eq!(system[0]["text"], "Be concise");

    // Should have inferenceConfig
    // Temperature is f32, JSON float may have precision loss
    let temp = body["inferenceConfig"]["temperature"].as_f64().unwrap();
    assert!((temp - 0.7).abs() < 0.01, "Expected ~0.7, got {temp}");
    assert_eq!(body["inferenceConfig"]["maxTokens"], 2048);

    // Should have toolConfig with toolSpec
    let tools_arr = body["toolConfig"]["tools"].as_array().unwrap();
    assert_eq!(tools_arr.len(), 1);
    assert_eq!(tools_arr[0]["toolSpec"]["name"], "get_time");
}

#[test]
fn test_no_api_key_header_in_signed_request() {
    // FR-016: Verify that SigV4 signing does not produce x-api-key or Bearer auth
    let creds = AwsCredentials {
        access_key: "AKIAIOSFODNN7EXAMPLE".to_string(),
        secret_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
        session_token: None,
        region: "us-east-1".to_string(),
    };

    let mut headers: Vec<(String, String)> = Vec::new();
    super::super::bedrock_sigv4::sign_request(
        "POST",
        "https://bedrock.us-east-1.amazonaws.com/model/test/invoke",
        &mut headers,
        b"{}",
        &creds,
    )
    .unwrap();

    // No x-api-key header
    assert!(!headers.iter().any(|(k, _)| k == "x-api-key"));
    // No Bearer auth
    assert!(
        !headers
            .iter()
            .any(|(k, v)| k == "Authorization" && v.starts_with("Bearer"))
    );
    // Must have AWS4-HMAC-SHA256 auth
    assert!(
        headers
            .iter()
            .any(|(k, v)| k == "Authorization" && v.starts_with("AWS4-HMAC-SHA256"))
    );
}

#[test]
fn test_credentials_not_in_error_messages() {
    // FR-030: Verify that error messages don't contain raw keys
    // We test by checking the resolve function returns credential-related
    // errors without exposing actual key values
    let options = HashMap::new();
    let result = resolve_aws_credentials(&options);
    if let Err(e) = result {
        let msg = e.to_string();
        // The error message should NOT contain actual AWS key patterns
        assert!(!msg.contains("AKIA"));
        assert!(!msg.contains("wJalr"));
    }
}

// ---------------------------------------------------------------------------
// Converse request body, chat() streams, and error classification
// ---------------------------------------------------------------------------

use crate::llm::{ChatRequest, LlmClient, StreamEvent};
use futures::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

fn sample_creds() -> AwsCredentials {
    AwsCredentials {
        access_key: "AKIATEST".to_string(),
        secret_key: "secret".to_string(),
        session_token: None,
        region: "us-east-1".to_string(),
    }
}

#[test]
fn test_converse_request_body_parts_and_tools() {
    let client = BedrockConverseClient {
        credentials: sample_creds(),
        base_url: "https://bedrock.us-east-1.amazonaws.com".to_string(),
        model_id: "amazon.nova-pro-v1:0".to_string(),
        http: crate::provider::http_client::create_streaming_http_client(),
    };
    let req = ChatRequest {
        model: "amazon.nova-pro-v1:0".to_string(),
        messages: Arc::new(vec![
            crate::llm::ChatMessage {
                role: "system".to_string(),
                content: ChatContent::Text("ignored".to_string()),
            },
            crate::llm::ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Parts(vec![
                    crate::llm::ContentPart::Text {
                        text: "describe".to_string(),
                    },
                    crate::llm::ContentPart::ImageUrl {
                        url: "data:image/jpeg;base64,QUJD".to_string(),
                    },
                    crate::llm::ContentPart::ToolUse {
                        id: "tu1".to_string(),
                        name: "calc".to_string(),
                        input: json!({"expr": "2+2"}),
                    },
                    crate::llm::ContentPart::ToolResult {
                        tool_use_id: "tu1".to_string(),
                        content: Arc::from("4"),
                        is_error: false,
                    },
                ]),
            },
            crate::llm::ChatMessage {
                role: "tool".to_string(),
                content: ChatContent::Text("skipped".to_string()),
            },
        ]),
        tools: Arc::new(vec![ToolDefinition {
            name: "calc".to_string(),
            description: "Calculate".to_string(),
            parameters: json!({"type": "object"}),
        }]),
        temperature: Some(0.3),
        top_p: Some(0.6),
        max_tokens: Some(512),
        system: Some(Arc::from("be brief")),
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    };
    let body = client.build_request_body(&req);
    // Only the user message survives (system skipped, unknown role skipped).
    assert_eq!(body["messages"].as_array().expect("msgs").len(), 1);
    let content = &body["messages"][0]["content"];
    assert_eq!(content[0]["text"], "describe");
    assert_eq!(content[1]["image"]["format"], "jpeg");
    assert_eq!(content[1]["image"]["source"]["bytes"], "QUJD");
    assert_eq!(content[2]["toolUse"]["toolUseId"], "tu1");
    assert_eq!(content[3]["toolResult"]["content"][0]["text"], "4");
    assert_eq!(body["system"][0]["text"], "be brief");
    assert_eq!(body["inferenceConfig"]["maxTokens"], 512);
    assert!(body["toolConfig"]["tools"].is_array());
}

#[test]
fn test_bedrock_anthropic_request_body_tools_and_thinking() {
    let client = BedrockAnthropicClient {
        credentials: sample_creds(),
        base_url: "https://bedrock.us-east-1.amazonaws.com".to_string(),
        model_id: "anthropic.claude-sonnet-4-v1:0".to_string(),
        http: crate::provider::http_client::create_streaming_http_client(),
    };
    let req = ChatRequest {
        model: "anthropic.claude-sonnet-4-v1:0".to_string(),
        messages: Arc::new(vec![crate::llm::ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Parts(vec![
                crate::llm::ContentPart::Text {
                    text: "hi".to_string(),
                },
                crate::llm::ContentPart::ImageUrl {
                    url: "data:image/png;base64,QUJD".to_string(),
                },
            ]),
        }]),
        tools: Arc::new(vec![ToolDefinition {
            name: "calc".to_string(),
            description: "Calculate".to_string(),
            parameters: json!({"type": "object"}),
        }]),
        temperature: None,
        top_p: None,
        max_tokens: Some(128),
        system: Some(Arc::from("sys")),
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    };
    let body = client.build_request_body(&req);
    assert_eq!(body["anthropic_version"], "bedrock-2023-06-01");
    assert_eq!(body["messages"][0]["content"][0]["type"], "text");
    assert_eq!(body["messages"][0]["content"][1]["type"], "image");
    assert!(body["tools"].is_array());
}

async fn spawn_bedrock_stream(body: String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/vnd.amazon.eventstream\r\nConnection: close\r\n\r\n{body}"
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
    });
    format!("http://{addr}")
}

async fn spawn_bedrock_status(status: u16, body: String) -> String {
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

fn converse_request() -> ChatRequest {
    ChatRequest {
        model: "amazon.nova-pro-v1:0".to_string(),
        messages: Arc::new(vec![crate::llm::ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("hi".to_string()),
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

#[tokio::test]
async fn test_converse_chat_stream_text_tool_and_finish() {
    let sse = concat!(
        "event: messageStart\n",
        "data: {\"usage\":{\"inputTokens\":9}}\n\n",
        "event: contentBlockStart\n",
        "data: {\"start\":{\"toolUse\":{\"toolUseId\":\"tu1\",\"name\":\"calc\"}}}\n\n",
        "event: contentBlockDelta\n",
        "data: {\"delta\":{\"text\":\"Hi\"}}\n\n",
        "event: contentBlockDelta\n",
        "data: {\"delta\":{\"toolUse\":{\"input\":\"{\\\"x\\\":1}\"}}}\n\n",
        "event: contentBlockStop\n",
        "data: {}\n\n",
        "event: metadata\n",
        "data: {\"usage\":{\"outputTokens\":4}}\n\n",
        "event: messageStop\n",
        "data: {\"stopReason\":\"tool_use\"}\n\n",
    )
    .to_string();
    let url = spawn_bedrock_stream(sse).await;
    let client = BedrockConverseClient {
        credentials: sample_creds(),
        base_url: url,
        model_id: "amazon.nova-pro-v1:0".to_string(),
        http: crate::provider::http_client::create_streaming_http_client(),
    };
    let events: Vec<StreamEvent> = client
        .chat(converse_request())
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
        StreamEvent::ToolCallStart { id, name } if id == "tu1" && name == "calc"
    )));
    assert!(events.iter().any(|e| matches!(
        e,
        StreamEvent::ToolCallDelta { args_json, .. } if args_json == "{\"x\":1}"
    )));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::ToolCallEnd { id } if id == "tu1"))
    );
    assert!(events.iter().any(|e| matches!(
        e,
        StreamEvent::Usage {
            input_tokens: 9,
            output_tokens: 0
        }
    )));
    assert!(
        events.iter().any(
            |e| matches!(e, StreamEvent::Finish { reason } if *reason == FinishReason::ToolUse)
        )
    );
}

#[tokio::test]
async fn test_bedrock_anthropic_chat_stream_blocks() {
    let sse = concat!(
        "event: message_start\n",
        "data: {\"message\":{\"usage\":{\"input_tokens\":3}}}\n\n",
        "event: content_block_delta\n",
        "data: {\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Yo\"}}\n\n",
        "event: message_delta\n",
        "data: {\"delta\":{\"stop_reason\":\"max_tokens\"},\"usage\":{\"output_tokens\":1}}\n\n",
    )
    .to_string();
    let url = spawn_bedrock_stream(sse).await;
    let client = BedrockAnthropicClient {
        credentials: sample_creds(),
        base_url: url,
        model_id: "anthropic.claude-sonnet-4-v1:0".to_string(),
        http: crate::provider::http_client::create_streaming_http_client(),
    };
    let events: Vec<StreamEvent> = client
        .chat(converse_request())
        .await
        .expect("chat")
        .collect()
        .await;
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::TextDelta { text } if text == "Yo"))
    );
    assert!(
        events.iter().any(
            |e| matches!(e, StreamEvent::Finish { reason } if *reason == FinishReason::Length)
        )
    );
}

#[tokio::test]
async fn test_bedrock_converse_error_classification() {
    let throttling = r#"{"error":{"type":"ThrottlingException","message":"slow down"}}"#;
    let url = spawn_bedrock_status(429, throttling.to_string()).await;
    let client = BedrockConverseClient {
        credentials: sample_creds(),
        base_url: url,
        model_id: "amazon.nova-pro-v1:0".to_string(),
        http: crate::provider::http_client::create_streaming_http_client(),
    };
    let err = match client.chat(converse_request()).await {
        Ok(_) => panic!("expected error"),
        Err(e) => e.to_string(),
    };
    assert!(err.contains("ThrottlingException"), "error: {err}");

    let denied = r#"{"error":{"type":"AccessDeniedException","message":"no"}}"#;
    let url = spawn_bedrock_status(403, denied.to_string()).await;
    let client = BedrockConverseClient {
        credentials: sample_creds(),
        base_url: url,
        model_id: "amazon.nova-pro-v1:0".to_string(),
        http: crate::provider::http_client::create_streaming_http_client(),
    };
    let err = match client.chat(converse_request()).await {
        Ok(_) => panic!("expected error"),
        Err(e) => e.to_string(),
    };
    assert!(err.contains("access denied"), "error: {err}");
}

#[test]
fn test_build_bedrock_base_url_endpoint_override() {
    assert_eq!(
        build_bedrock_base_url("eu-west-1", None),
        "https://bedrock.eu-west-1.amazonaws.com"
    );
    assert_eq!(
        build_bedrock_base_url("us-east-1", Some("https://vpce.example.com/")),
        "https://vpce.example.com"
    );
}
