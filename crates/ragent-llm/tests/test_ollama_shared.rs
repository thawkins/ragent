//! Integration tests for the shared Ollama context-window heuristic (T-307).
//!
//! The single implementation lives in `providers::ollama_shared` and is used by
//! both the local (`ollama`) and cloud (`ollama_cloud`) providers.

use ragent_llm::providers::ollama_shared::estimate_context_window;

#[test]
fn test_estimate_context_window_modern_models_get_128k() {
    // Any model with at least 1B parameters defaults to a 128k window.
    assert_eq!(estimate_context_window("70B"), 131_072);
    assert_eq!(estimate_context_window("8B"), 131_072);
    assert_eq!(estimate_context_window("3B"), 131_072);
    assert_eq!(estimate_context_window("1B"), 131_072);
    assert_eq!(estimate_context_window("1b"), 131_072);
}

#[test]
fn test_estimate_context_window_sub_billion_gets_32k() {
    assert_eq!(estimate_context_window("0.5B"), 32_768);
    assert_eq!(estimate_context_window("0.5b"), 32_768);
}

#[test]
fn test_estimate_context_window_unparsable_defaults_to_128k() {
    // A malformed size string warns and falls back to the 7.0 sentinel, which
    // is treated as a large model (128k).
    assert_eq!(estimate_context_window("unknown"), 131_072);
}
