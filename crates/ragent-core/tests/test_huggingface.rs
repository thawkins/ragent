//! Integration tests for the HuggingFace provider.
//!
//! Tests provider registration, model resolution, and client creation
//! without requiring a real API key or network access.

use ragent_core::provider::create_default_registry;
use std::collections::HashMap;

#[test]
fn test_huggingface_registered_in_default_registry() {
    let registry = create_default_registry();
    let provider = registry.get("huggingface");
    assert!(provider.is_some(), "huggingface should be registered");
    assert_eq!(provider.unwrap().name(), "Hugging Face");
}

#[test]
fn test_huggingface_appears_in_provider_list() {
    let registry = create_default_registry();
    let providers = registry.list();
    let hf = providers.iter().find(|p| p.id == "huggingface");
    assert!(hf.is_some(), "huggingface should appear in provider list");
    assert!(!hf.unwrap().models.is_empty(), "should have default models");
}

#[test]
fn test_huggingface_model_resolution_exact_id() {
    let registry = create_default_registry();
    let model = registry.resolve_model("huggingface", "meta-llama/Llama-3.1-70B-Instruct");
    assert!(model.is_some(), "should resolve exact model ID");
    let model = model.unwrap();
    assert_eq!(model.name, "Llama 3.1 70B");
    assert_eq!(model.provider_id, "huggingface");
    assert!(model.capabilities.streaming);
    assert!(model.capabilities.tool_use);
}

#[test]
fn test_huggingface_model_resolution_display_name() {
    let registry = create_default_registry();
    let model = registry.resolve_model("huggingface", "Llama 3.1 70B");
    assert!(model.is_some(), "should resolve by display name");
}

#[test]
fn test_huggingface_model_resolution_missing() {
    let registry = create_default_registry();
    let model = registry.resolve_model("huggingface", "nonexistent/model-xyz");
    assert!(model.is_none(), "should not resolve unknown model");
}

#[test]
fn test_huggingface_default_models_have_valid_fields() {
    let registry = create_default_registry();
    let provider = registry.get("huggingface").unwrap();
    let models = provider.default_models();

    for model in &models {
        assert_eq!(model.provider_id, "huggingface");
        assert!(!model.id.is_empty(), "model id should not be empty");
        assert!(!model.name.is_empty(), "model name should not be empty");
        assert!(model.context_window > 0, "context window should be > 0");
        assert!(
            model.capabilities.streaming,
            "all HF models should support streaming"
        );
        assert_eq!(model.cost.input, 0.0, "HF free tier should be free");
        assert_eq!(model.cost.output, 0.0, "HF free tier should be free");
    }
}

#[test]
fn test_huggingface_deepseek_r1_has_reasoning() {
    let registry = create_default_registry();
    let model = registry
        .resolve_model("huggingface", "deepseek-ai/DeepSeek-R1")
        .expect("DeepSeek R1 should be in defaults");
    assert!(
        model.capabilities.reasoning,
        "DeepSeek R1 should have reasoning"
    );
    assert!(
        !model.capabilities.tool_use,
        "DeepSeek R1 should not have tool_use"
    );
    assert_eq!(model.context_window, 128_000);
}

#[tokio::test]
async fn test_huggingface_create_client_rejects_empty_key() {
    let registry = create_default_registry();
    let provider = registry.get("huggingface").unwrap();
    let result = provider.create_client("", None, &HashMap::new()).await;
    assert!(result.is_err(), "empty API key should be rejected");
    let err = result.err().unwrap().to_string();
    assert!(
        err.contains("requires an API token"),
        "error should mention token requirement: {err}"
    );
}

#[tokio::test]
async fn test_huggingface_create_client_with_valid_key() {
    let registry = create_default_registry();
    let provider = registry.get("huggingface").unwrap();
    let result = provider
        .create_client("hf_test_token_12345", None, &HashMap::new())
        .await;
    assert!(
        result.is_ok(),
        "valid key should create client successfully"
    );
}

#[tokio::test]
async fn test_huggingface_create_client_custom_base_url() {
    let registry = create_default_registry();
    let provider = registry.get("huggingface").unwrap();
    let result = provider
        .create_client(
            "hf_test_token",
            Some("https://my-endpoint.endpoints.huggingface.cloud"),
            &HashMap::new(),
        )
        .await;
    assert!(result.is_ok(), "custom base URL should work");
}

#[tokio::test]
async fn test_huggingface_create_client_with_options() {
    let registry = create_default_registry();
    let provider = registry.get("huggingface").unwrap();
    let mut options = HashMap::new();
    options.insert("wait_for_model".to_string(), serde_json::Value::Bool(false));
    options.insert("use_cache".to_string(), serde_json::Value::Bool(false));

    let result = provider
        .create_client("hf_test_token", None, &options)
        .await;
    assert!(result.is_ok(), "options should be accepted");
}

#[test]
fn test_huggingface_provider_count_in_registry() {
    let registry = create_default_registry();
    let providers = registry.list();
    let hf_count = providers.iter().filter(|p| p.id == "huggingface").count();
    assert_eq!(hf_count, 1, "should have exactly one huggingface provider");
}

#[test]
fn test_huggingface_qwen_coder_has_tool_use() {
    let registry = create_default_registry();
    let model = registry
        .resolve_model("huggingface", "Qwen/Qwen2.5-Coder-32B-Instruct")
        .expect("Qwen Coder should be in defaults");
    assert!(
        model.capabilities.tool_use,
        "Qwen Coder should support tool_use"
    );
    assert_eq!(model.context_window, 32_000);
}

#[test]
fn test_huggingface_qwen_model() {
    let registry = create_default_registry();
    let model = registry
        .resolve_model("huggingface", "Qwen/Qwen2.5-72B-Instruct")
        .expect("Qwen should be in defaults");
    assert!(model.capabilities.tool_use);
    assert_eq!(model.context_window, 128_000);
}
