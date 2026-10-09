//! Inline tests for `ollama.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn test_format_model_name() {
    let details = OllamaModelDetails {
        parameter_size: "70B".to_string(),
        family: "llama".to_string(),
    };
    assert_eq!(
        format_model_name("llama3.3:latest", &details),
        "Llama3.3 (70B)"
    );
}

#[test]
fn test_estimate_context_window() {
    // Modern local models support 128k regardless of parameter size.
    assert_eq!(estimate_context_window("70B"), 131_072);
    assert_eq!(estimate_context_window("8B"), 131_072);
    assert_eq!(estimate_context_window("3B"), 131_072);
    assert_eq!(estimate_context_window("32B"), 131_072);
    assert_eq!(estimate_context_window("1B"), 131_072);
    // Sub-1B models get a conservative 32k fallback.
    assert_eq!(estimate_context_window("0.5B"), 32_768);
}

#[test]
fn test_provider_defaults() {
    let provider = OllamaProvider::new();
    assert_eq!(provider.id(), "ollama");
    assert_eq!(provider.name(), "Ollama");
    assert!(
        provider.default_models().is_empty(),
        "Ollama default_models should be empty; models are discovered at runtime"
    );
}

#[test]
fn test_with_custom_url() {
    let provider = OllamaProvider::with_url("http://remote:11434/");
    assert_eq!(provider.base_url, "http://remote:11434");
}
