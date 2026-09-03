#![allow(clippy::assert_is_empty)]
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

/// Verifies the Bedrock provider ships an empty default catalog.
/// Models are discovered at runtime via the `ListFoundationModels` API.
#[test]
fn test_bedrock_default_models_empty() {
    let registry = create_default_registry();
    let provider = registry
        .get("bedrock")
        .expect("Bedrock provider should be registered");
    let models = provider.default_models();
    assert!(
        models.is_empty(),
        "Bedrock default_models should be empty; models are discovered at runtime"
    );
}

/// Verifies all discovered/default models have the correct `provider_id`.
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

/// Verifies the static test catalog still contains Claude metadata.
#[test]
fn test_bedrock_static_catalog_has_claude_models() {
    use ragent_llm::provider::bedrock::bedrock_default_models;
    let models = bedrock_default_models();
    let has_claude = models.iter().any(|m| m.id.contains("claude"));
    assert!(
        has_claude,
        "Bedrock static test catalog should include Claude models"
    );
}

/// Verifies the static test catalog still contains Nova metadata.
#[test]
fn test_bedrock_static_catalog_has_nova_models() {
    use ragent_llm::provider::bedrock::bedrock_default_models;
    let models = bedrock_default_models();
    let has_nova = models.iter().any(|m| m.id.contains("nova"));
    assert!(
        has_nova,
        "Bedrock static test catalog should include Nova models"
    );
}

/// Verifies model resolution works for the static Bedrock catalog.
#[test]
fn test_bedrock_model_resolution() {
    use ragent_llm::provider::bedrock::bedrock_default_models;
    let models = bedrock_default_models();
    let model = models
        .iter()
        .find(|m| m.id == "anthropic.claude-sonnet-4-20250514-v1:0");
    assert!(
        model.is_some(),
        "Should resolve Claude Sonnet 4 in static catalog"
    );
    let model = model.unwrap();
    assert_eq!(model.provider_id, "bedrock");
}

/// Verifies that Claude models in the static Bedrock catalog have reasoning capabilities.
#[test]
fn test_bedrock_claude_models_have_reasoning() {
    use ragent_llm::provider::bedrock::bedrock_default_models;
    let models = bedrock_default_models();
    let model = models
        .iter()
        .find(|m| m.id == "anthropic.claude-sonnet-4-20250514-v1:0")
        .expect("Should find Claude Sonnet 4 in static catalog");
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

/// Verifies that Nova models in the static Bedrock catalog have the expected capabilities.
#[test]
fn test_bedrock_nova_models_capabilities() {
    use ragent_llm::provider::bedrock::bedrock_default_models;
    let models = bedrock_default_models();
    let model = models
        .iter()
        .find(|m| m.id == "amazon.nova-pro-v1:0")
        .expect("Should find Nova Pro in static catalog");
    assert!(
        model.capabilities.streaming,
        "Nova Pro should support streaming"
    );
    assert!(
        model.capabilities.tool_use,
        "Nova Pro should support tool use"
    );
}
