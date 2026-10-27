//! Inline tests for `openai_responses.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use crate::llm::{ChatMessage, ToolDefinition};
use std::sync::Arc;

#[test]
fn test_provider_id_and_name() {
    let provider = ResponsesApiProvider;
    assert_eq!(provider.id(), "openai_responses");
    assert_eq!(provider.name(), "OpenAI Responses API");
}

#[test]
fn test_default_models() {
    let provider = ResponsesApiProvider;
    let models = provider.default_models();

    assert!(!models.is_empty());
    assert!(models.iter().any(|m| m.id == "gpt-5.6"));
    assert!(models.iter().any(|m| m.id == "o1"));

    // All models should have reasoning capability
    for model in &models {
        assert!(model.capabilities.reasoning);
    }
}

#[test]
fn test_build_request_body_basic() {
    let client = ResponsesApiClient::new("test-key", "https://api.openai.com");
    let request = ChatRequest {
        model: "gpt-5.6".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("Hello".to_string()),
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

    let body = client.build_request_body(&request).expect("body builds");

    assert_eq!(body["model"], "gpt-5.6");
    assert_eq!(body["reasoning"]["effort"], "medium");
    assert!(body["stream"].as_bool().unwrap());

    let input = body["input"].as_array().unwrap();
    assert_eq!(input.len(), 1);
    assert_eq!(input[0]["role"], "user");
    assert_eq!(input[0]["content"], "Hello");
}

#[test]
fn test_build_request_body_with_tools() {
    let client = ResponsesApiClient::new("test-key", "https://api.openai.com");
    let tools = vec![ToolDefinition {
        name: "test_tool".to_string(),
        description: "A test tool".to_string(),
        parameters: json!({"type": "object"}),
    }];

    let request = ChatRequest {
        model: "gpt-5.6".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("Test".to_string()),
        }]),
        tools: Arc::new(tools),
        temperature: None,
        top_p: None,
        max_tokens: Some(1000),
        system: Some("You are helpful".into()),
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    };

    let body = client.build_request_body(&request).expect("body builds");

    assert!(body["tools"].is_array());
    assert_eq!(body["tools"].as_array().unwrap().len(), 1);
    assert_eq!(body["max_output_tokens"], 1000);
    assert_eq!(body["instructions"], "You are helpful");
}

#[test]
fn test_build_request_body_tools_come_from_the_shared_encoder() {
    // FR-003 / T-005: the body's `tools` must *equal* the encoder's payload, not
    // merely "be an array of the right length". An inline array that produced the
    // right shape would satisfy every shape assertion and still be the state
    // FR-003 forbids, so the comparison is against the shared cache.
    let client = ResponsesApiClient::new("test-key", "https://api.openai.com");
    let tools = vec![
        ToolDefinition {
            name: "read".to_string(),
            description: "Read a file".to_string(),
            parameters: json!({"type": "object", "properties": {"path": {"type": "string"}}}),
        },
        ToolDefinition {
            name: "write".to_string(),
            description: "Write a file".to_string(),
            parameters: json!({"type": "object"}),
        },
    ];
    let request = ChatRequest {
        model: "gpt-5.6".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("go".to_string()),
        }]),
        tools: Arc::new(tools),
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

    let body = client.build_request_body(&request).expect("body builds");
    let expected =
        cached_tools(ToolFormat::OpenAiResponses, &request.tools).responses_tools_array();
    assert_eq!(
        body["tools"], expected,
        "the Responses tool array must be the encoder's payload, not a re-derived one"
    );

    let items = body["tools"].as_array().expect("tools is an array");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["type"], "function");
    assert_eq!(items[0]["name"], "read");
    assert_eq!(
        items[0]["parameters"]["properties"]["path"]["type"],
        "string"
    );
    assert!(
        items[0].get("function").is_none(),
        "the Responses item is flattened: {}",
        items[0]
    );
}

#[test]
fn test_build_request_body_omits_tools_when_the_list_is_empty() {
    // Routing the array through the encoder must not quietly add `"tools":[]`:
    // some Responses deployments reject an empty tool list, so an empty registry
    // has to leave the field absent (the guard the inline array carried).
    let client = ResponsesApiClient::new("test-key", "https://api.openai.com");
    let request = ChatRequest {
        model: "gpt-5.6".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("Hello".to_string()),
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

    let body = client.build_request_body(&request).expect("body builds");
    assert!(
        body.get("tools").is_none(),
        "an empty tool list must leave the field absent, not send []: {body}"
    );
}

#[test]
fn test_build_request_body_with_thinking() {
    use ragent_types::thinking::{ThinkingConfig, ThinkingLevel};

    let client = ResponsesApiClient::new("test-key", "https://api.openai.com");
    let request = ChatRequest {
        model: "gpt-5.6".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("Test".to_string()),
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
        thinking: Some(ThinkingConfig {
            enabled: true,
            level: ThinkingLevel::High,
            budget_tokens: None,
            display: None,
        }),
    };

    let body = client.build_request_body(&request).expect("body builds");

    assert_eq!(body["reasoning"]["effort"], "high");
}

#[test]
fn test_build_request_body_encodes_tool_history() {
    // FR-018: tool-use and tool-result parts previously were dropped (text-only
    // filter); they must reach the input list as function_call /
    // function_call_output items.
    let client = ResponsesApiClient::new("test-key", "https://api.openai.com");
    let request = ChatRequest {
        model: "gpt-5.6".to_string(),
        messages: Arc::new(vec![
            ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Text("read notes.txt".to_string()),
            },
            ChatMessage {
                role: "assistant".to_string(),
                content: ChatContent::Parts(vec![
                    ContentPart::Text {
                        text: "reading".to_string(),
                    },
                    ContentPart::ToolUse {
                        id: "call_ab12".to_string(),
                        name: "read".to_string(),
                        input: serde_json::json!({ "path": "notes.txt" }),
                    },
                ]),
            },
            ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Parts(vec![ContentPart::ToolResult {
                    tool_use_id: "call_ab12".to_string(),
                    content: Arc::from("file contents"),
                    is_error: false,
                }]),
            },
        ]),
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

    let body = client.build_request_body(&request).expect("body builds");
    let input = body["input"].as_array().expect("input is array");
    // 1 user text + 1 assistant text + 1 function_call + 1 function_call_output
    assert_eq!(
        input.len(),
        4,
        "all tool constructs reach the input list: {input:?}"
    );
    let kinds: Vec<&str> = input
        .iter()
        .filter_map(|item| item.get("type").and_then(|v| v.as_str()))
        .collect();
    assert!(
        kinds.contains(&"function_call"),
        "ToolUse becomes function_call: {input:?}"
    );
    assert!(
        kinds.contains(&"function_call_output"),
        "ToolResult becomes function_call_output: {input:?}"
    );
    let call = input.iter().find(|i| i["type"] == "function_call").unwrap();
    assert_eq!(call["call_id"], "call_ab12");
    assert_eq!(call["name"], "read");
    let output = input
        .iter()
        .find(|i| i["type"] == "function_call_output")
        .unwrap();
    assert_eq!(output["call_id"], "call_ab12");
    assert_eq!(output["output"], "file contents");
}

#[test]
fn test_build_request_body_wire_loss_on_image() {
    // FR-018: an ImageUrl part has no slot on the Responses input list, so the
    // encoder must refuse the request rather than silently dropping it.
    let client = ResponsesApiClient::new("test-key", "https://api.openai.com");
    let request = ChatRequest {
        model: "gpt-5.6".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Parts(vec![ContentPart::ImageUrl {
                url: "https://example.com/img.png".to_string(),
            }]),
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

    let err = client
        .build_request_body(&request)
        .expect_err("ImageUrl must be WireLoss on Responses");
    let msg = err.to_string();
    assert!(
        msg.contains("wire loss") && msg.contains("ImageUrl") && msg.contains("openai_responses"),
        "WireLoss names construct and provider: {msg}"
    );
    // And it does not issue the request: the anyhow conversion in chat() surfaces it.
    assert!(
        msg.contains("cannot carry"),
        "message states the construct cannot be carried: {msg}"
    );
}
