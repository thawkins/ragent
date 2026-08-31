//! Tests for the LLM-backed analysis engine.

use ragent_llm::provider::{ProviderRegistry, create_default_registry};
use ragent_research::analysis::{AnalysisEngine, LlmAnalysisEngine};
use std::sync::Arc;

#[test]
fn validate_provider_rejects_unknown_provider() {
    let registry = Arc::new(ProviderRegistry::new());
    let engine = LlmAnalysisEngine::new(registry, "missing_provider", "some-model");
    let err = engine
        .validate_provider()
        .expect_err("unknown provider must fail validation");
    assert!(err.to_string().contains("missing_provider"));
    assert!(err.to_string().contains("some-model"));
}

#[test]
fn validate_provider_accepts_registered_provider() {
    let registry = Arc::new(create_default_registry());
    let engine = LlmAnalysisEngine::new(registry, "anthropic", "claude-sonnet-4");
    engine
        .validate_provider()
        .expect("registered provider must validate");
}
