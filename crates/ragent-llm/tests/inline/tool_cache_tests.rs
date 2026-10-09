//! Inline tests for `tool_cache.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

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
fn empty_openai_tools_array_is_valid() {
    let cached = build_openai(&[]);
    let value = cached.openai_tools_array();
    assert!(value.is_array());
    assert_eq!(value.as_array().unwrap().len(), 0);
}

#[test]
fn openai_tools_array_matches_expected_shape() {
    let cached = build_openai(&[sample_tool("read"), sample_tool("write")]);
    let value = cached.openai_tools_array();
    let arr = value.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["type"], "function");
    assert_eq!(arr[0]["function"]["name"], "read");
}

#[test]
fn empty_anthropic_tools_array_is_valid() {
    let cached = build_anthropic(&[]);
    let value = cached.anthropic_tools_array();
    assert!(value.is_array());
    assert_eq!(value.as_array().unwrap().len(), 0);
}

#[test]
fn anthropic_tools_array_matches_expected_shape() {
    let cached = build_anthropic(&[sample_tool("bash")]);
    let value = cached.anthropic_tools_array();
    let arr = value.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["name"], "bash");
    assert!(arr[0].get("input_schema").is_some());
}

#[test]
fn empty_gemini_tools_array_is_valid() {
    let cached = build_gemini(&[]);
    let value = cached.gemini_tools_array();
    assert!(value.is_array());
    assert_eq!(value.as_array().unwrap().len(), 0);
}

#[test]
fn gemini_tools_array_has_function_declarations_wrapper() {
    let cached = build_gemini(&[sample_tool("read")]);
    let value = cached.gemini_tools_array();
    let arr = value.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert!(arr[0].get("functionDeclarations").is_some());
    let decls = arr[0]["functionDeclarations"].as_array().unwrap();
    assert_eq!(decls.len(), 1);
    assert_eq!(decls[0]["name"], "read");
}

#[test]
fn empty_bedrock_tool_config_is_valid() {
    let cached = build_bedrock(&[]);
    let value = cached.bedrock_tool_config_object();
    let tools = value["tools"].as_array().unwrap();
    assert!(tools.is_empty(), "tools should be empty");
}

#[test]
fn bedrock_tool_config_matches_expected_shape() {
    let cached = build_bedrock(&[sample_tool("read")]);
    let value = cached.bedrock_tool_config_object();
    let tools = value["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["toolSpec"]["name"], "read");
    assert!(tools[0]["toolSpec"].get("inputSchema").is_some());
}

#[test]
fn huggingface_tools_get_t_prefix() {
    let cached = build_huggingface(&[sample_tool("read")]);
    let value = cached.openai_tools_array();
    let arr = value.as_array().unwrap();
    assert_eq!(arr[0]["function"]["name"], "t_read");
}

#[test]
fn cached_tools_reuses_same_buffer() {
    let t = sample_tool("read");
    let a = cached_tools(ToolFormat::OpenAi, std::slice::from_ref(&t));
    let b = cached_tools(ToolFormat::OpenAi, std::slice::from_ref(&t));
    assert!(Arc::ptr_eq(&a, &b));
}
