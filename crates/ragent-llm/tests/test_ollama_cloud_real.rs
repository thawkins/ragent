//! Real Ollama Cloud model discovery tests.

use ragent_llm::{OllamaCloudProvider, Provider};

#[tokio::test]
async fn test_ollama_cloud_discovers_models_without_api_key() {
    // Ollama Cloud's /api/tags and /api/show endpoints are publicly readable,
    // so model discovery must succeed even without an API key.
    // SAFETY: called from a single-threaded test before any other test reads
    // this variable.
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
