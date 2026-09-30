//! Inline tests for `ollama_cloud.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use serde_json::json;

#[test]
fn test_provider_defaults() {
    let provider = OllamaCloudProvider::new();
    assert_eq!(provider.id(), "ollama_cloud");
    assert_eq!(provider.name(), "Ollama Cloud");
    assert!(provider.default_models().is_empty());
}

#[test]
fn test_with_custom_url() {
    let provider = OllamaCloudProvider::with_url("https://example.com/");
    assert_eq!(provider.base_url, "https://example.com");
}

#[test]
fn test_context_length_parses_top_level_string_fields() {
    let response: OllamaShowResponse = serde_json::from_value(json!({
        "context_length": "1048576",
        "capabilities": []
    }))
    .expect("show response should parse");

    assert_eq!(response.context_length(), Some(1_048_576));
}

#[test]
fn test_context_length_parses_alternate_model_info_keys() {
    let response: OllamaShowResponse = serde_json::from_value(json!({
        "model_info": {
            "llama.context_window": 1_048_576
        },
        "capabilities": []
    }))
    .expect("show response should parse");

    assert_eq!(response.context_length(), Some(1_048_576));
}

#[test]
fn test_has_thinking_from_capabilities() {
    let response: OllamaShowResponse = serde_json::from_value(json!({
        "capabilities": ["vision", "thinking"]
    }))
    .expect("show response should parse");

    assert!(response.has_thinking());
}

#[test]
fn test_has_thinking_from_template_markers() {
    // Template with <!-- think --> marker
    let response: OllamaShowResponse = serde_json::from_value(json!({
        "template": "Some text <!-- think --> thinking block {{ .Content }}",
        "capabilities": []
    }))
    .expect("show response should parse");

    assert!(response.has_thinking());
}

#[test]
fn test_has_thinking_from_template_go_template() {
    // Template with Go template {{--think}} marker
    let response: OllamaShowResponse = serde_json::from_value(json!({
        "template": "{{--think}}\n{{ .Content }}",
        "capabilities": []
    }))
    .expect("show response should parse");

    assert!(response.has_thinking());
}

#[test]
fn test_has_thinking_false_when_no_indicators() {
    let response: OllamaShowResponse = serde_json::from_value(json!({
        "template": "{{ .Content }}",
        "capabilities": ["vision"]
    }))
    .expect("show response should parse");

    assert!(!response.has_thinking());
}
