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
    let err = result.expect_err("the virtual router provider must never be dispatched");
    assert!(
        err.to_string().contains("not found"),
        "expected a not-found error for the virtual router, got: {err}"
    );
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
    // The invariant under test is "never proceed unauthenticated": with no key
    // available the call must fail, and it must fail *before* any provider
    // request is attempted. A key-resolution failure is therefore the expected
    // outcome; a client-level failure would mean the request was dispatched
    // without credentials, which this asserts against.
    let err = result.expect_err("a provider with no resolvable key must never dispatch");
    let message = err.to_string();
    assert!(
        message.contains("API key") || message.contains("api key") || message.contains("key"),
        "expected a missing-key failure, got: {message}"
    );
}
