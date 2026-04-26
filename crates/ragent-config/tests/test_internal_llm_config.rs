use ragent_config::{Config, InternalLlmDownloadPolicy};

#[test]
fn test_internal_llm_defaults_are_safe() {
    let config = Config::default();

    assert!(!config.internal_llm.enabled);
    assert_eq!(config.internal_llm.backend, "embedded");
    assert_eq!(config.internal_llm.model_id, "smollm2-360m-instruct-q4");
    assert_eq!(config.internal_llm.artifact_max_bytes, 1_073_741_824);
    assert_eq!(config.internal_llm.threads, 4);
    assert_eq!(config.internal_llm.context_window, 4096);
    assert_eq!(config.internal_llm.max_output_tokens, 1_024);
    assert_eq!(config.internal_llm.timeout_ms, 300_000);
    assert_eq!(config.internal_llm.max_parallel_requests, 2);
    assert_eq!(
        config.internal_llm.download_policy,
        InternalLlmDownloadPolicy::OnDemand
    );
    assert!(config.internal_llm.allows_task("session_title"));
    assert!(config.internal_llm.allows_task("chat"));
    assert!(!config.internal_llm.session_title_enabled);
    assert!(!config.internal_llm.prompt_context_enabled);
    assert!(!config.internal_llm.memory_extraction_enabled);
}

#[test]
fn test_internal_llm_deserializes_partial_config() {
    let config: Config = serde_json::from_str(
        r#"{
            "internal_llm": {
                "enabled": true,
                "threads": 8,
                "download_policy": "never",
                "session_title_enabled": true
            }
        }"#,
    )
    .unwrap();

    assert!(config.internal_llm.enabled);
    assert_eq!(config.internal_llm.threads, 8);
    assert_eq!(
        config.internal_llm.download_policy,
        InternalLlmDownloadPolicy::Never
    );
    assert!(config.internal_llm.session_title_enabled);
    assert_eq!(config.internal_llm.model_id, "smollm2-360m-instruct-q4");
}

#[test]
fn test_internal_llm_merge_only_overrides_explicit_fields() {
    let mut base = Config::default();
    base.internal_llm.enabled = true;
    base.internal_llm.model_id = "base-model".to_string();
    base.internal_llm.threads = 2;

    let overlay: Config = serde_json::from_str(
        r#"{
            "internal_llm": {
                "model_id": "overlay-model",
                "prompt_context_enabled": true
            }
        }"#,
    )
    .unwrap();

    let merged = Config::merge(base, overlay);

    assert!(
        merged.internal_llm.enabled,
        "overlay should not reset enabled"
    );
    assert_eq!(merged.internal_llm.model_id, "overlay-model");
    assert_eq!(merged.internal_llm.threads, 2);
    assert!(merged.internal_llm.prompt_context_enabled);
    assert!(!merged.internal_llm.memory_extraction_enabled);
}
