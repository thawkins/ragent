//! Integration tests for the Amazon Bedrock provider.
//!
//! Tests the provider contract: registration, model catalog, client creation,
//! and error handling for missing credentials.

use ragent_llm::provider::create_default_registry;

/// Verifies the Bedrock provider is registered in the default registry.
#[test]
fn test_bedrock_provider_registered() {
    let registry = create_default_registry();
    assert!(
        registry.get("bedrock").is_some(),
        "Bedrock provider should be registered"
    );
}

/// Verifies the Bedrock provider has the correct ID and name.
#[test]
fn test_bedrock_provider_id_and_name() {
    let registry = create_default_registry();
    let provider = registry
        .get("bedrock")
        .expect("Bedrock provider should be registered");
    assert_eq!(provider.id(), "bedrock");
    assert_eq!(provider.name(), "Amazon Bedrock");
}

/// Verifies the Bedrock provider has a non-empty default model catalog.
#[test]
fn test_bedrock_default_models_non_empty() {
    let registry = create_default_registry();
    let provider = registry
        .get("bedrock")
        .expect("Bedrock provider should be registered");
    let models = provider.default_models();
    assert!(!models.is_empty(), "Bedrock should have default models");
    assert!(
        models.len() >= 8,
        "Expected at least 8 default models, got {}",
        models.len()
    );
}

/// Verifies all Bedrock default models have the correct provider_id.
#[test]
fn test_bedrock_models_have_correct_provider_id() {
    let registry = create_default_registry();
    let provider = registry
        .get("bedrock")
        .expect("Bedrock provider should be registered");
    let models = provider.default_models();
    for model in &models {
        assert_eq!(
            model.provider_id, "bedrock",
            "Model '{}' has wrong provider_id: '{}'",
            model.id, model.provider_id
        );
    }
}

/// Verifies Bedrock has Claude models in the catalog.
#[test]
fn test_bedrock_has_claude_models() {
    let registry = create_default_registry();
    let provider = registry
        .get("bedrock")
        .expect("Bedrock provider should be registered");
    let models = provider.default_models();
    let has_claude = models.iter().any(|m| m.id.contains("claude"));
    assert!(
        has_claude,
        "Bedrock should have Claude models in the catalog"
    );
}

/// Verifies Bedrock has Amazon Nova models in the catalog.
#[test]
fn test_bedrock_has_nova_models() {
    let registry = create_default_registry();
    let provider = registry
        .get("bedrock")
        .expect("Bedrock provider should be registered");
    let models = provider.default_models();
    let has_nova = models.iter().any(|m| m.id.contains("nova"));
    assert!(
        has_nova,
        "Bedrock should have Amazon Nova models in the catalog"
    );
}

/// Verifies model resolution works for Bedrock models.
#[test]
fn test_bedrock_model_resolution() {
    let registry = create_default_registry();
    let model = registry.resolve_model("bedrock", "anthropic.claude-sonnet-4-20250514-v1:0");
    assert!(model.is_some(), "Should resolve Claude Sonnet 4 on Bedrock");
    let model = model.unwrap();
    assert_eq!(model.provider_id, "bedrock");
}

/// Verifies client creation fails with actionable error when no AWS credentials.
#[test]
fn test_bedrock_client_creation_fails_without_credentials() {
    let registry = create_default_registry();
    let provider = registry
        .get("bedrock")
        .expect("Bedrock provider should be registered");

    // Note: We cannot safely remove env vars in Rust 2024 (remove_var is unsafe).
    // Instead, we just test with empty options and check the error.
    // If AWS credentials are set in the environment, the test environment
    // will have them and client creation may succeed — that's fine.
    let options = std::collections::HashMap::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(provider.create_client("", None, &options));

    // If no AWS credentials are available, we expect an error
    if std::env::var("AWS_ACCESS_KEY_ID").is_err() {
        assert!(result.is_err(), "Should fail without AWS credentials");

        let error_msg = result.err().unwrap().to_string();
        // FR-002: Error should mention credential sources attempted
        assert!(
            error_msg.contains("AWS") || error_msg.contains("credentials"),
            "Error message should mention AWS credentials: {error_msg}"
        );
    }
    // If credentials ARE set in the environment, client creation succeeds — that's valid too
}

/// Verifies the Bedrock provider appears in the provider list.
#[test]
fn test_bedrock_in_provider_list() {
    let registry = create_default_registry();
    let providers = registry.list();
    let bedrock = providers.iter().find(|p| p.id == "bedrock");
    assert!(bedrock.is_some(), "Bedrock should appear in provider list");
    assert_eq!(bedrock.unwrap().name, "Amazon Bedrock");
}

/// Verifies that Claude models on Bedrock have reasoning capabilities.
#[test]
fn test_bedrock_claude_models_have_reasoning() {
    let registry = create_default_registry();
    let model = registry.resolve_model("bedrock", "anthropic.claude-sonnet-4-20250514-v1:0");
    let model = model.expect("Should resolve Claude Sonnet 4 on Bedrock");
    assert!(
        model.capabilities.reasoning,
        "Claude Sonnet 4 should have reasoning capability"
    );
    assert!(
        model.capabilities.streaming,
        "Claude Sonnet 4 should support streaming"
    );
    assert!(
        model.capabilities.tool_use,
        "Claude Sonnet 4 should support tool use"
    );
}

/// Verifies that Nova models have the expected capabilities.
#[test]
fn test_bedrock_nova_models_capabilities() {
    let registry = create_default_registry();
    let model = registry.resolve_model("bedrock", "amazon.nova-pro-v1:0");
    let model = model.expect("Should resolve Nova Pro on Bedrock");
    assert!(
        model.capabilities.streaming,
        "Nova Pro should support streaming"
    );
    assert!(
        model.capabilities.tool_use,
        "Nova Pro should support tool use"
    );
}
