//! Inline tests for `tool_cache.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).
//!
//! These drive the *cache*; the envelope shapes themselves are pinned by
//! `tests/test_wire_encode.rs`, because the bytes now come from
//! `wire::encode::encode_tools` rather than from a builder in this module
//! (FR-003, T-004).

use super::*;

fn sample_tool(name: &str) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        description: format!("Use the {name} tool."),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}

#[test]
fn empty_tool_list_still_parses_for_every_format() {
    for format in ToolFormat::ALL {
        let cached = cached_tools(*format, &[]);
        let payload = match format.tool_envelope() {
            ToolEnvelope::BedrockToolSpec => {
                let value = cached.bedrock_tool_config_object();
                value["tools"].as_array().map(Vec::len)
            }
            _ => cached.openai_tools_array().as_array().map(Vec::len),
        };
        assert_eq!(
            payload,
            Some(0),
            "{format:?}: an empty tool list must serialise to an empty list, not Null"
        );
    }
}

#[test]
fn openai_cache_entry_matches_the_encoder() {
    let tools = [sample_tool("read"), sample_tool("write")];
    let cached = cached_tools(ToolFormat::OpenAi, &tools);
    let value = cached.openai_tools_array();
    let arr = value.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["type"], "function");
    assert_eq!(arr[0]["function"]["name"], "read");
    assert_eq!(
        value,
        encode_tools(OPENAI_CHAT, None, &tools).openai_tools_array()
    );
}

#[test]
fn anthropic_cache_entry_matches_the_encoder() {
    let tools = [sample_tool("bash")];
    let cached = cached_tools(ToolFormat::Anthropic, &tools);
    let value = cached.anthropic_tools_array();
    let arr = value.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["name"], "bash");
    assert!(arr[0].get("input_schema").is_some());
    assert_eq!(
        value,
        encode_tools(ANTHROPIC_MESSAGES, None, &tools).anthropic_tools_array()
    );
}

#[test]
fn gemini_cache_entry_has_function_declarations_wrapper() {
    let tools = [sample_tool("read")];
    let cached = cached_tools(ToolFormat::Gemini, &tools);
    let value = cached.gemini_tools_array();
    let arr = value.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert!(arr[0].get("functionDeclarations").is_some());
    let decls = arr[0]["functionDeclarations"].as_array().unwrap();
    assert_eq!(decls.len(), 1);
    assert_eq!(decls[0]["name"], "read");
    assert_eq!(
        value,
        encode_tools(GEMINI, None, &tools).gemini_tools_array()
    );
}

#[test]
fn bedrock_cache_entry_is_the_whole_tool_config() {
    let tools = [sample_tool("read")];
    let cached = cached_tools(ToolFormat::Bedrock, &tools);
    let value = cached.bedrock_tool_config_object();
    let tools_value = value["tools"].as_array().unwrap();
    assert_eq!(tools_value.len(), 1);
    assert_eq!(tools_value[0]["toolSpec"]["name"], "read");
    assert!(tools_value[0]["toolSpec"].get("inputSchema").is_some());
    assert_eq!(
        value,
        encode_tools(BEDROCK_CONVERSE, None, &tools).bedrock_tool_config_object()
    );
}

#[test]
fn huggingface_cache_entry_prefixes_tool_names() {
    let tools = [sample_tool("read")];
    let cached = cached_tools(ToolFormat::HuggingFace, &tools);
    let value = cached.openai_tools_array();
    let arr = value.as_array().unwrap();
    assert_eq!(arr[0]["function"]["name"], "t_read");
    // HuggingFace shares the OpenAI envelope and differs only by the prefix.
    assert_eq!(
        ToolFormat::HuggingFace.tool_envelope(),
        ToolEnvelope::OpenAiFunction
    );
    assert_eq!(ToolFormat::HuggingFace.name_prefix(), Some(HF_TOOL_PREFIX));
}

#[test]
fn responses_cache_entry_is_the_flattened_function_shape() {
    let tools = [sample_tool("read")];
    let cached = cached_tools(ToolFormat::OpenAiResponses, &tools);
    let value = cached.responses_tools_array();
    let arr = value.as_array().unwrap();
    assert_eq!(arr[0]["type"], "function");
    assert_eq!(arr[0]["name"], "read", "the Responses item is not nested");
    assert!(arr[0].get("function").is_none());
    assert_eq!(
        value,
        encode_tools(OPENAI_RESPONSES, None, &tools).responses_tools_array()
    );
}

#[test]
fn cached_tools_reuses_same_buffer() {
    let t = sample_tool("read");
    let a = cached_tools(ToolFormat::OpenAi, std::slice::from_ref(&t));
    let b = cached_tools(ToolFormat::OpenAi, std::slice::from_ref(&t));
    assert!(Arc::ptr_eq(&a, &b));
}

#[test]
fn every_format_maps_to_a_distinct_envelope_it_actually_emits() {
    // The mapping must be total and honest: the envelope a key reports is the
    // envelope the encoder writes for it, otherwise the TUI and the cache would
    // measure one shape while the provider sent another (FR-003).
    for format in ToolFormat::ALL {
        let tools = [sample_tool("read")];
        let cached = cached_tools(*format, &tools);
        let payload = match format.tool_envelope() {
            ToolEnvelope::BedrockToolSpec => cached.bedrock_tool_config_object(),
            _ => cached.openai_tools_array(),
        };
        let item = match format.tool_envelope() {
            ToolEnvelope::BedrockToolSpec => payload["tools"][0].clone(),
            _ => payload[0].clone(),
        };
        let found = match format.tool_envelope() {
            ToolEnvelope::OpenAiFunction => item.get("function").is_some(),
            ToolEnvelope::AnthropicTool => item.get("input_schema").is_some(),
            ToolEnvelope::GeminiFunctionDeclaration => item.get("functionDeclarations").is_some(),
            ToolEnvelope::BedrockToolSpec => item.get("toolSpec").is_some(),
            ToolEnvelope::ResponsesFlatFunction => {
                item.get("function").is_none() && item.get("name").is_some()
            }
        };
        assert!(
            found,
            "{format:?} reports {:?} but emitted {item}",
            format.tool_envelope()
        );
    }
}
