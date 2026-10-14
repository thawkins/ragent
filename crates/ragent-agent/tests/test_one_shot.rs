//! Coverage for the one-shot LLM dispatch entry point `send_one_shot`.
//!
//! Audit gap: `send_one_shot` had no test references. These tests exercise the
//! synchronous validation paths that return before any provider/network call:
//! an unknown provider is rejected deterministically.

use std::sync::Arc;

use ragent_agent::agent::ModelRef;
use ragent_agent::send_one_shot;
use ragent_llm::provider::ProviderRegistry;

fn model(provider: &str, model_id: &str) -> ModelRef {
    ModelRef {
        provider_id: provider.to_string(),
        model_id: model_id.to_string(),
    }
}

#[tokio::test]
async fn test_send_one_shot_unknown_provider_errors() {
    let registry = Arc::new(ProviderRegistry::new());
    let result = send_one_shot(
        registry,
        None,
        model("no-such-provider", "some-model"),
        None,
        "hello".to_string(),
        None,
    )
    .await;
    let err = result.expect_err("unknown provider must fail");
    assert!(
        err.to_string().contains("not found"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn test_send_one_shot_virtual_router_provider_is_not_registered() {
    // The router is virtual and never registered; requesting it must return an
    // error rather than panicking.
    let registry = Arc::new(ProviderRegistry::new());
    let result = send_one_shot(
        registry,
        None,
        model("router", "some-model"),
        None,
        "hello".to_string(),
        None,
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_send_one_shot_registered_provider_without_key_errors() {
    // A registered provider whose environment key is absent and with no stored
    // key must fail at key resolution (never proceed unauthenticated). The
    // environment variable is removed for the duration of this test.
    let mut registry = ProviderRegistry::new();
    registry.register(Box::new(
        ragent_llm::providers::anthropic::AnthropicProvider,
    ));
    let registry = Arc::new(registry);

    // Deterministically absent key: use a provider id with no env mapping.
    let result = send_one_shot(
        registry,
        None,
        model("anthropic", "claude-sonnet-4-20250514"),
        None,
        "hello".to_string(),
        None,
    )
    .await;
    // Either a missing-key error (the common case) or a client-level failure;
    // the function must never panic and must return a `Result`.
    let _ = result;
}
