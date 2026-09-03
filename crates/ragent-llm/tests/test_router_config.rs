#![allow(clippy::assert_is_empty)]
//! External tests for `tests` from `crates/ragent-llm/src/providers/router_config.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_llm::providers::router_config::{BoundaryConfig, RouterConfig, Tier, WeightConfig};

#[test]
fn test_default_weights_sum_to_one() {
    let weights = WeightConfig::default();
    let sum = weights.sum();
    assert!(
        (sum - 1.0).abs() < 0.01,
        "default weights should sum to ~1.0, got {sum}"
    );
}

#[test]
fn test_weight_normalisation() {
    let mut weights = WeightConfig::default();
    // Double all weights
    weights.token_count *= 2.0;
    weights.vocabulary_complexity *= 2.0;
    weights.syntax_complexity *= 2.0;
    weights.domain_specificity *= 2.0;
    weights.ambiguity *= 2.0;
    weights.context_dependency *= 2.0;
    weights.reasoning_depth *= 2.0;
    weights.creativity_level *= 2.0;
    weights.emotional_complexity *= 2.0;
    weights.multimodality *= 2.0;
    weights.instruction_complexity *= 2.0;
    weights.knowledge_recency *= 2.0;
    weights.code_complexity *= 2.0;
    weights.mathematical_complexity *= 2.0;
    weights.image_attachment *= 2.0;

    let doubled_sum = weights.sum();
    assert!(
        (doubled_sum - 2.0).abs() < 0.02,
        "doubled sum should be ~2.0, got {doubled_sum}"
    );
    weights.normalise();
    assert!(
        (weights.sum() - 1.0).abs() < 0.001,
        "normalised weights should sum to ~1.0, got {}",
        weights.sum()
    );
}

#[test]
fn test_boundary_validation_valid() {
    let boundaries = BoundaryConfig::default();
    assert!(boundaries.validate().is_ok());
}

#[test]
fn test_boundary_validation_out_of_range() {
    let boundaries = BoundaryConfig {
        simple_medium: -0.1,
        medium_complex: 0.5,
        complex_reasoning: 0.75,
    };
    assert!(boundaries.validate().is_err());
}

#[test]
fn test_boundary_validation_not_ascending() {
    let boundaries = BoundaryConfig {
        simple_medium: 0.5,
        medium_complex: 0.3,
        complex_reasoning: 0.75,
    };
    assert!(boundaries.validate().is_err());
}

#[test]
fn test_default_router_config_has_all_tiers() {
    let config = RouterConfig::default();
    assert!(config.tiers.contains_key("SIMPLE"));
    assert!(config.tiers.contains_key("MEDIUM"));
    assert!(config.tiers.contains_key("COMPLEX"));
    assert!(config.tiers.contains_key("REASONING"));
}

#[test]
fn test_default_router_config_disabled() {
    let config = RouterConfig::default();
    assert!(!config.enabled);
}

#[test]
fn test_tier_from_str_insensitive() {
    assert_eq!(Tier::from_str_insensitive("simple"), Some(Tier::Simple));
    assert_eq!(Tier::from_str_insensitive("MEDIUM"), Some(Tier::Medium));
    assert_eq!(Tier::from_str_insensitive("Complex"), Some(Tier::Complex));
    assert_eq!(
        Tier::from_str_insensitive("reasoning"),
        Some(Tier::Reasoning)
    );
    assert_eq!(Tier::from_str_insensitive("unknown"), None);
}

#[test]
fn test_tier_display() {
    assert_eq!(Tier::Simple.to_string(), "SIMPLE");
    assert_eq!(Tier::Medium.to_string(), "MEDIUM");
    assert_eq!(Tier::Complex.to_string(), "COMPLEX");
    assert_eq!(Tier::Reasoning.to_string(), "REASONING");
}

#[test]
fn test_tier_initial() {
    assert_eq!(Tier::Simple.initial(), 'S');
    assert_eq!(Tier::Medium.initial(), 'M');
    assert_eq!(Tier::Complex.initial(), 'C');
    assert_eq!(Tier::Reasoning.initial(), 'R');
}

#[test]
fn test_tier_config_default_models_empty() {
    let config = RouterConfig::default();
    // Default router tiers are intentionally empty: ragent no longer
    // hard-codes provider/model pairs. Users configure the cluster via
    // `/provider router` or `provider.router` in `ragent.json`.
    for tier in Tier::all() {
        let tc = config.tier_config(*tier);
        assert!(
            tc.models.is_empty(),
            "default tier {:?} should contain no hard-coded models",
            tier
        );
        assert_eq!(tc.timeout_ms, None);
    }
    let all_default_providers: std::collections::HashSet<String> = Tier::all()
        .iter()
        .flat_map(|t| {
            config
                .tier_config(*t)
                .models
                .into_iter()
                .map(|e| e.provider)
        })
        .collect();
    assert!(
        !all_default_providers.contains("ollama"),
        "default tiers should not impose local Ollama models"
    );
    assert!(
        !all_default_providers.contains("anthropic"),
        "default tiers should not silently fall back to Anthropic"
    );
}

#[test]
fn test_router_config_validate() {
    let config = RouterConfig::default();
    assert!(config.validate().is_ok());
}

#[test]
fn test_weight_by_index() {
    let weights = WeightConfig::default();
    assert!((weights.weight_by_index(0) - 0.07).abs() < 1e-10);
    assert!((weights.weight_by_index(6) - 0.08).abs() < 1e-10);
    assert!((weights.weight_by_index(14) - 0.05).abs() < 1e-10);
}

#[test]
fn test_dimension_name() {
    assert_eq!(WeightConfig::dimension_name(0), "token_count");
    assert_eq!(WeightConfig::dimension_name(13), "mathematical_complexity");
    assert_eq!(WeightConfig::dimension_name(14), "image_attachment");
}

#[test]
fn test_config_serde_roundtrip() {
    let config = RouterConfig::default();
    let json = serde_json::to_string_pretty(&config).unwrap();
    let deserialized: RouterConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.enabled, config.enabled);
    assert_eq!(deserialized.context_messages, config.context_messages);
    assert_eq!(deserialized.default_timeout_ms, config.default_timeout_ms);
}
