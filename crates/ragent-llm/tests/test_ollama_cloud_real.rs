//! Real Ollama Cloud model discovery tests.
//!
//! Note: `std::env::remove_var` is `unsafe` in Rust 2024; the workspace
//! denies `unsafe_code`, so this test target opts back in explicitly.

#![allow(unsafe_code)]

use ragent_llm::{OllamaCloudProvider, Provider};

#[tokio::test]
// `await_holding_lock` is intentional: the guard must stay held across the
// `await` so `OLLAMA_API_KEY` is still unset while discovery runs, and the
// crate-wide lock is what stops a sibling test in this binary setting it.
#[allow(clippy::await_holding_lock)]
#[ignore = "live network: hits the public Ollama Cloud API; run with --ignored"]
async fn test_ollama_cloud_discovers_models_without_api_key() {
    // Ollama Cloud's /api/tags and /api/show endpoints are publicly readable,
    // so model discovery must succeed even without an API key.
    //
    // This crate has no `serial_test` dev-dependency, so isolation comes from
    // the crate-wide env lock declared in `tests/all.rs`; hold it for as long as
    // `OLLAMA_API_KEY` must stay unset.
    let _env_guard = crate::global_env_lock();
    // SAFETY: the crate-wide env lock is held above, so no sibling file in this
    // binary observes the variable while it is removed.
    unsafe { std::env::remove_var("OLLAMA_API_KEY") };

    let provider = OllamaCloudProvider::new();
    let models = provider
        .discover_models()
        .await
        .expect("Ollama Cloud discovery should succeed without an API key");
    assert!(
        !models.is_empty(),
        "Ollama Cloud should return at least one public model"
    );
    for m in &models {
        assert!(!m.id.is_empty(), "model id should not be empty");
        assert_eq!(m.provider_id, "ollama_cloud");
        assert!(m.context_window > 0, "context window should be positive");
    }
}

#[tokio::test]
#[ignore = "live network: hits the public Ollama Cloud API; run with --ignored"]
async fn test_ollama_cloud_discovers_models_with_api_key() {
    let api_key = std::env::var("OLLAMA_API_KEY").unwrap_or_default();
    if api_key.is_empty() {
        eprintln!("OLLAMA_API_KEY not set; skipping authenticated Ollama Cloud discovery test");
        return;
    }

    let provider = OllamaCloudProvider::new();
    let models = provider
        .discover_models()
        .await
        .expect("Ollama Cloud discovery should succeed with a valid API key");
    assert!(
        !models.is_empty(),
        "Ollama Cloud should return at least one model"
    );
    for m in &models {
        assert!(!m.id.is_empty(), "model id should not be empty");
        assert_eq!(m.provider_id, "ollama_cloud");
        assert!(m.context_window > 0, "context window should be positive");
    }
}
