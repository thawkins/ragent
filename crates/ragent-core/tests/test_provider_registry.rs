//! Tests for test_provider_registry.rs

use ragent_core::provider::*;

// ── Default registry ─────────────────────────────────────────────

#[test]
fn test_default_provider_registry_has_all_providers() {
    let registry = create_default_registry();
    let providers = registry.list();

    let ids: Vec<&str> = providers.iter().map(|p| p.id.as_str()).collect();
    assert!(ids.contains(&"anthropic"), "Missing anthropic provider");
    assert!(ids.contains(&"openai"), "Missing openai provider");
    assert!(
        ids.contains(&"generic_openai"),
        "Missing generic_openai provider"
    );
    assert!(ids.contains(&"copilot"), "Missing copilot provider");
    assert!(
        ids.contains(&"ollama_cloud"),
        "Missing ollama_cloud provider"
    );
    assert!(ids.contains(&"ollama"), "Missing ollama provider");
}

#[test]
fn test_provider_info_has_valid_fields() {
    let registry = create_default_registry();
    let providers = registry.list();

    for provider in &providers {
        assert!(!provider.id.is_empty());
        assert!(!provider.name.is_empty());
    }
}

// ── Provider get ─────────────────────────────────────────────────

#[test]
fn test_provider_registry_get() {
    let registry = create_default_registry();

    let anthropic = registry.get("anthropic");
    assert!(anthropic.is_some());
    assert_eq!(anthropic.unwrap().name(), "Anthropic");

    let openai = registry.get("openai");
    assert!(openai.is_some());
    assert_eq!(openai.unwrap().name(), "OpenAI");

    let generic_openai = registry.get("generic_openai");
    assert!(generic_openai.is_some());
    assert_eq!(generic_openai.unwrap().name(), "Generic OpenAI API");

    let copilot = registry.get("copilot");
    assert!(copilot.is_some());
    assert_eq!(copilot.unwrap().name(), "GitHub Copilot");

    let ollama_cloud = registry.get("ollama_cloud");
    assert!(ollama_cloud.is_some());
    assert_eq!(ollama_cloud.unwrap().name(), "Ollama Cloud");

    let ollama = registry.get("ollama");
    assert!(ollama.is_some());
    assert_eq!(ollama.unwrap().name(), "Ollama");
}

#[test]
fn test_provider_registry_get_nonexistent() {
    let registry = create_default_registry();
    assert!(registry.get("nonexistent").is_none());
}

// ── Provider models ──────────────────────────────────────────────

#[test]
fn test_anthropic_default_models() {
    let registry = create_default_registry();
    let anthropic = registry.get("anthropic").unwrap();
    let models = anthropic.default_models();

    assert!(!models.is_empty());
    for model in &models {
        assert_eq!(model.provider_id, "anthropic");
        assert!(!model.id.is_empty());
        assert!(!model.name.is_empty());
        assert!(model.context_window > 0);
    }
}

#[test]
fn test_openai_default_models() {
    let registry = create_default_registry();
    let openai = registry.get("openai").unwrap();
    let models = openai.default_models();

    assert!(!models.is_empty());
    for model in &models {
        assert_eq!(model.provider_id, "openai");
        assert!(!model.id.is_empty());
        assert!(model.context_window > 0);
    }
}

#[test]
fn test_generic_openai_default_models() {
    let registry = create_default_registry();
    let generic_openai = registry.get("generic_openai").unwrap();
    let models = generic_openai.default_models();

    assert!(!models.is_empty());
    for model in &models {
        assert_eq!(model.provider_id, "generic_openai");
        assert!(!model.id.is_empty());
        assert!(model.context_window > 0);
    }
}

#[test]
fn test_copilot_default_models() {
    let registry = create_default_registry();
    let copilot = registry.get("copilot").unwrap();
    let models = copilot.default_models();

    assert!(!models.is_empty());
    for model in &models {
        assert_eq!(model.provider_id, "copilot");
        assert!(!model.id.is_empty());
    }
}

// ── Model resolution ─────────────────────────────────────────────

#[test]
fn test_resolve_known_model() {
    let registry = create_default_registry();

    let anthropic = registry.get("anthropic").unwrap();
    let first_model = &anthropic.default_models()[0];

    let resolved = registry.resolve_model("anthropic", &first_model.id);
    assert!(resolved.is_some());
    let resolved = resolved.unwrap();
    assert_eq!(resolved.id, first_model.id);
    assert_eq!(resolved.provider_id, "anthropic");
}

#[test]
fn test_resolve_unknown_model() {
    let registry = create_default_registry();
    let resolved = registry.resolve_model("anthropic", "nonexistent-model-xyz");
    assert!(resolved.is_none());
}

#[test]
fn test_resolve_model_unknown_provider() {
    let registry = create_default_registry();
    let resolved = registry.resolve_model("nonexistent", "any-model");
    assert!(resolved.is_none());
}

// ── Model info fields ────────────────────────────────────────────

#[test]
fn test_model_info_serde() {
    use ragent_core::config::{Capabilities, Cost};

    let model = ModelInfo {
        id: "test-model".to_string(),
        provider_id: "test".to_string(),
        name: "Test Model".to_string(),
        cost: Cost {
            input: 3.0,
            output: 15.0,
        },
        capabilities: Capabilities {
            reasoning: true,
            streaming: true,
            vision: true,
            tool_use: true,
        },
        context_window: 200_000,
        max_output: Some(4096),
    };

    let json = serde_json::to_string(&model).unwrap();
    let deserialized: ModelInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, "test-model");
    assert_eq!(deserialized.context_window, 200_000);
    assert_eq!(deserialized.max_output, Some(4096));
    assert!(deserialized.capabilities.reasoning);
}

// ── Empty registry ───────────────────────────────────────────────

#[test]
fn test_empty_provider_registry() {
    let registry = ProviderRegistry::new();
    assert!(registry.list().is_empty());
    assert!(registry.get("anything").is_none());
    assert!(registry.resolve_model("any", "model").is_none());
}

// ── Provider info serde ──────────────────────────────────────────

#[test]
fn test_provider_info_serde() {
    let info = ProviderInfo {
        id: "test".to_string(),
        name: "Test Provider".to_string(),
        models: vec![],
    };

    let json = serde_json::to_string(&info).unwrap();
    let deserialized: ProviderInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, "test");
    assert_eq!(deserialized.name, "Test Provider");
}
