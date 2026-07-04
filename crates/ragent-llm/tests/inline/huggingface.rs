//! Tests for huggingface.rs (M8/T8.4).
//! Compiled as a submodule via #[path], super::* resolves to the source module.

use super::*;
use std::sync::Arc;

#[test]
fn test_provider_id_and_name() {
    let provider = HuggingFaceProvider;
    assert_eq!(provider.id(), "huggingface");
    assert_eq!(provider.name(), "Hugging Face");
}

#[test]
fn test_default_models_non_empty() {
    let provider = HuggingFaceProvider;
    let models = provider.default_models();
    assert!(models.len() >= 4, "Expected at least 4 default models");
}

#[test]
fn test_default_models_have_correct_provider_id() {
    let models = huggingface_default_models();
    for model in &models {
        assert_eq!(model.provider_id, "huggingface");
    }
}

#[test]
fn test_default_models_all_free() {
    let models = huggingface_default_models();
    for model in &models {
        assert_eq!(model.cost.input, 0.0);
        assert_eq!(model.cost.output, 0.0);
    }
}

#[test]
fn test_format_model_display_name() {
    assert_eq!(
        format_model_display_name("meta-llama/Llama-3.1-70B-Instruct"),
        "Llama 3.1 70B Instruct"
    );
    assert_eq!(
        format_model_display_name("mistralai/Mixtral-8x7B-Instruct-v0.1"),
        "Mixtral 8x7B Instruct v0.1"
    );
    assert_eq!(
        format_model_display_name("deepseek-ai/DeepSeek-V4-Pro:together"),
        "DeepSeek V4 Pro (together)"
    );
    assert_eq!(format_model_display_name("plain-model"), "plain model");
}

#[test]
fn test_estimate_context_from_id() {
    assert_eq!(
        estimate_context_from_id("meta-llama/Llama-3.1-70B-Instruct"),
        128_000
    );
    assert_eq!(
        estimate_context_from_id("microsoft/Phi-3-mini-4k-instruct"),
        4_096
    );
    assert_eq!(
        estimate_context_from_id("mistralai/Mixtral-8x7B-Instruct-v0.1"),
        32_000
    );
    assert_eq!(
        estimate_context_from_id("Qwen/Qwen2.5-7B-Instruct-1M:featherless-ai"),
        32_000
    );
    assert_eq!(estimate_context_from_id("some-unknown/model-7b"), 32_000);
}

#[test]
fn test_router_model_to_info_uses_live_provider_metadata() {
    let info = router_model_to_info(HfRouterModelEntry {
        model_id: "deepseek-ai/DeepSeek-V4-Pro".to_string(),
        architecture: Some(HfRouterArchitecture {
            input_modalities: vec!["text".to_string()],
            output_modalities: vec!["text".to_string()],
        }),
        providers: vec![
            HfRouterProviderEntry {
                status: "error".to_string(),
                context_length: Some(65_536),
                max_output: Some(2_048),
                pricing: Some(HfRouterPricing {
                    input: 2.0,
                    output: 8.0,
                }),
                supports_tools: false,
            },
            HfRouterProviderEntry {
                status: "live".to_string(),
                context_length: Some(131_072),
                max_output: Some(8_192),
                pricing: Some(HfRouterPricing {
                    input: 0.9,
                    output: 3.6,
                }),
                supports_tools: true,
            },
        ],
    })
    .expect("model should be included");

    assert_eq!(info.id, "deepseek-ai/DeepSeek-V4-Pro");
    assert_eq!(info.context_window, 131_072);
    assert_eq!(info.max_output, Some(8_192));
    assert!(info.capabilities.tool_use);
    assert_eq!(info.cost.input, 0.9);
    assert_eq!(info.cost.output, 3.6);
}

#[test]
fn test_router_model_to_info_skips_non_text_models() {
    let info = router_model_to_info(HfRouterModelEntry {
        model_id: "black-forest-labs/FLUX.1-dev".to_string(),
        architecture: Some(HfRouterArchitecture {
            input_modalities: vec!["text".to_string()],
            output_modalities: vec!["image".to_string()],
        }),
        providers: vec![HfRouterProviderEntry {
            status: "live".to_string(),
            context_length: None,
            max_output: None,
            pricing: None,
            supports_tools: false,
        }],
    });

    assert!(info.is_none());
}

#[test]
fn test_router_model_to_info_skips_models_without_live_providers() {
    let info = router_model_to_info(HfRouterModelEntry {
        model_id: "some-org/offline-model".to_string(),
        architecture: None,
        providers: vec![HfRouterProviderEntry {
            status: "error".to_string(),
            context_length: Some(8_192),
            max_output: None,
            pricing: None,
            supports_tools: false,
        }],
    });

    assert!(info.is_none());
}

#[test]
fn test_build_request_body_basic() {
    let client = HuggingFaceClient {
        api_key: "test_key".to_string(),
        base_url: HF_API_BASE.to_string(),
        http: reqwest::Client::new(),
        wait_for_model: true,
        use_cache: true,
    };

    let request = ChatRequest {
        model: "meta-llama/Llama-3.1-70B-Instruct".to_string(),
        messages: Arc::new(vec![crate::llm::ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("Hello".to_string()),
        }]),
        tools: Arc::new(vec![]),
        temperature: Some(0.7),
        top_p: None,
        max_tokens: Some(4096),
        system: Some(std::sync::Arc::from("You are helpful.")),
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    };

    let body = client.build_request_body(&request);

    assert_eq!(body["model"], "meta-llama/Llama-3.1-70B-Instruct");
    assert_eq!(body["stream"], true);
    let temp = body["temperature"].as_f64().unwrap();
    assert!((temp - 0.7).abs() < 0.001, "temperature was {temp}");
    assert_eq!(body["max_tokens"], 4096);

    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0]["role"], "system");
    assert_eq!(messages[0]["content"], "You are helpful.");
    assert_eq!(messages[1]["role"], "user");
    assert_eq!(messages[1]["content"], "Hello");
}

#[test]
fn test_build_request_body_with_tools() {
    let client = HuggingFaceClient {
        api_key: "test_key".to_string(),
        base_url: HF_API_BASE.to_string(),
        http: reqwest::Client::new(),
        wait_for_model: true,
        use_cache: true,
    };

    let request = ChatRequest {
        model: "test-model".to_string(),
        messages: Arc::new(vec![]),
        tools: Arc::new(vec![crate::llm::ToolDefinition {
            name: "read".to_string(),
            description: "Read a file".to_string(),
            parameters: json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" }
            }
                          }),
        }]),
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

    let body = client.build_request_body(&request);
    let tools = body["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["type"], "function");
    assert_eq!(tools[0]["function"]["name"], "t_read");
}

#[test]
fn test_build_request_body_with_tool_results() {
    let client = HuggingFaceClient {
        api_key: "test_key".to_string(),
        base_url: HF_API_BASE.to_string(),
        http: reqwest::Client::new(),
        wait_for_model: true,
        use_cache: true,
    };

    let request = ChatRequest {
        model: "test-model".to_string(),
        messages: Arc::new(vec![crate::llm::ChatMessage {
            role: "tool".to_string(),
            content: ChatContent::Parts(vec![ContentPart::ToolResult {
                tool_use_id: "call_123".to_string(),
                content: "file contents here".to_string().into(),
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

    let body = client.build_request_body(&request);
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["role"], "tool");
    assert_eq!(messages[0]["tool_call_id"], "call_123");
    assert_eq!(messages[0]["content"], "file contents here");
}

#[test]
fn test_build_request_body_with_tool_use() {
    let client = HuggingFaceClient {
        api_key: "test_key".to_string(),
        base_url: HF_API_BASE.to_string(),
        http: reqwest::Client::new(),
        wait_for_model: true,
        use_cache: true,
    };

    let request = ChatRequest {
        model: "test-model".to_string(),
        messages: Arc::new(vec![crate::llm::ChatMessage {
            role: "assistant".to_string(),
            content: ChatContent::Parts(vec![ContentPart::ToolUse {
                id: "call_456".to_string(),
                name: "write_file".to_string(),
                input: json!({"path": "test.txt", "content": "hello"}),
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

    let body = client.build_request_body(&request);
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["role"], "assistant");
    let tool_calls = messages[0]["tool_calls"].as_array().unwrap();
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0]["id"], "call_456");
    assert_eq!(tool_calls[0]["function"]["name"], "t_write_file");
}

#[test]
fn test_hf_error_response_parsing() {
    let json_str = r#"{"error": "Model is currently loading", "estimated_time": 45.2}"#;
    let err: HfErrorResponse = serde_json::from_str(json_str).unwrap();
    assert_eq!(err.error, "Model is currently loading");
    assert!((err.estimated_time.unwrap() - 45.2).abs() < f64::EPSILON);
}

#[test]
fn test_hf_error_response_no_time() {
    let json_str = r#"{"error": "Unauthorized"}"#;
    let err: HfErrorResponse = serde_json::from_str(json_str).unwrap();
    assert_eq!(err.error, "Unauthorized");
    assert!(err.estimated_time.is_none());
}

#[test]
fn test_parse_hf_rate_limit_headers_present() {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("x-ratelimit-limit", "100".parse().unwrap());
    headers.insert("x-ratelimit-remaining", "95".parse().unwrap());

    let event = parse_hf_rate_limit_headers(&headers);
    assert!(event.is_some());
    if let Some(StreamEvent::RateLimit {
        requests_used_pct,
        tokens_used_pct,
    }) = event
    {
        assert!((requests_used_pct.unwrap() - 5.0).abs() < 0.1);
        assert!(tokens_used_pct.is_none());
    } else {
        panic!("Expected RateLimit event");
    }
}

#[test]
fn test_parse_hf_rate_limit_headers_absent() {
    let headers = reqwest::header::HeaderMap::new();
    let event = parse_hf_rate_limit_headers(&headers);
    assert!(event.is_none());
}

#[test]
fn test_build_request_body_no_system_prompt() {
    let client = HuggingFaceClient {
        api_key: "test_key".to_string(),
        base_url: HF_API_BASE.to_string(),
        http: reqwest::Client::new(),
        wait_for_model: true,
        use_cache: true,
    };

    let request = ChatRequest {
        model: "test-model".to_string(),
        messages: Arc::new(vec![crate::llm::ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("Hi".to_string()),
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

    let body = client.build_request_body(&request);
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["role"], "user");
}

#[test]
fn test_build_request_includes_stream_options() {
    let client = HuggingFaceClient {
        api_key: "test_key".to_string(),
        base_url: HF_API_BASE.to_string(),
        http: reqwest::Client::new(),
        wait_for_model: true,
        use_cache: true,
    };

    let request = ChatRequest {
        model: "test-model".to_string(),
        messages: Arc::new(vec![]),
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

    let body = client.build_request_body(&request);
    assert_eq!(body["stream"], true);
    assert_eq!(body["stream_options"]["include_usage"], true);
}

#[test]
fn test_safe_tool_name_and_strip() {
    assert_eq!(HuggingFaceClient::safe_tool_name("search"), "t_search");
    assert_eq!(
        HuggingFaceClient::safe_tool_name("write_file"),
        "t_write_file"
    );
    assert_eq!(HuggingFaceClient::strip_tool_prefix("t_search"), "search");
    assert_eq!(
        HuggingFaceClient::strip_tool_prefix("t_write_file"),
        "write_file"
    );
    // If the model returns a name without prefix, pass through unchanged
    assert_eq!(
        HuggingFaceClient::strip_tool_prefix("unknown_tool"),
        "unknown_tool"
    );
}

#[test]
fn test_system_prompt_rewriting() {
    let tools = vec![
        crate::llm::ToolDefinition {
            name: "search".to_string(),
            description: "Search".to_string(),
            parameters: json!({}),
        },
        crate::llm::ToolDefinition {
            name: "write_file".to_string(),
            description: "Write a file".to_string(),
            parameters: json!({}),
        },
    ];

    let prompt = "You have tools: search, write_file. Use search to find code.";
    let rewritten = HuggingFaceClient::rewrite_system_prompt(prompt, &tools);
    assert!(rewritten.contains("t_search"));
    assert!(rewritten.contains("t_write_file"));
    assert!(!rewritten.contains(" search"));
}
