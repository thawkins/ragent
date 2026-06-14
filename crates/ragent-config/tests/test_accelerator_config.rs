use ragent_config::{Config, InternalLlmConfig};

#[test]
fn test_accelerator_default_is_cpu() {
    let config = InternalLlmConfig::default();
    assert_eq!(config.accelerator, "cpu");
}

#[test]
fn test_accelerator_deserialize_explicit() {
    let config: Config = serde_json::from_str(
        r#"{
            "internal_llm": {
                "enabled": true,
                "accelerator": "gpu"
            }
        }"#,
    )
    .unwrap();
    assert_eq!(config.internal_llm.accelerator, "gpu");
}

#[test]
fn test_accelerator_deserialize_npu() {
    let config: Config = serde_json::from_str(
        r#"{
            "internal_llm": {
                "enabled": true,
                "accelerator": "npu"
            }
        }"#,
    )
    .unwrap();
    assert_eq!(config.internal_llm.accelerator, "npu");
}

#[test]
fn test_accelerator_not_serialized_when_default() {
    let config = InternalLlmConfig::default();
    let json = serde_json::to_string(&config).unwrap();
    // "accelerator" should be omitted when it's the default value "cpu"
    assert!(!json.contains("accelerator"));
}

#[test]
fn test_accelerator_serialized_when_non_default() {
    let mut config = InternalLlmConfig::default();
    config.accelerator = "gpu".to_string();
    let json = serde_json::to_string(&config).unwrap();
    assert!(json.contains("accelerator"));
    assert!(json.contains("gpu"));
}

#[test]
fn test_accelerator_merge_preserves_explicit() {
    let mut base = Config::default();
    base.internal_llm.accelerator = "gpu".to_string();

    let overlay: Config = serde_json::from_str(
        r#"{
            "internal_llm": {
                "enabled": true
            }
        }"#,
    )
    .unwrap();

    let merged = Config::merge(base, overlay);
    // Overlay didn't specify accelerator, so base should be preserved
    assert_eq!(merged.internal_llm.accelerator, "gpu");
}

#[test]
fn test_accelerator_merge_overrides() {
    let mut base = Config::default();
    base.internal_llm.accelerator = "gpu".to_string();

    let overlay: Config = serde_json::from_str(
        r#"{
            "internal_llm": {
                "accelerator": "npu"
            }
        }"#,
    )
    .unwrap();

    let merged = Config::merge(base, overlay);
    assert_eq!(merged.internal_llm.accelerator, "npu");
}

#[test]
fn test_backend_default_is_candle() {
    let config = InternalLlmConfig::default();
    assert_eq!(config.backend, "candle", "default backend should be candle");
}

#[test]
fn test_backend_and_accelerator_independent() {
    let config: Config = serde_json::from_str(
        r#"{
            "internal_llm": {
                "enabled": true,
                "backend": "candle",
                "accelerator": "gpu"
            }
        }"#,
    )
    .unwrap();
    assert_eq!(config.internal_llm.backend, "candle");
    assert_eq!(config.internal_llm.accelerator, "gpu");
}
