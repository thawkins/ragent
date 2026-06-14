//! Backend selection and configuration tests for the embedded internal-LLM runtime.

use ragent_config::InternalLlmConfig;
use ragent_llm::embedded::{
    ChatTemplate, EmbeddedModelArtifact, EmbeddedModelManifest, EmbeddedRuntime,
    RuntimeAvailability,
};

fn make_config(backend: &str, model_id: &str) -> InternalLlmConfig {
    let mut config = InternalLlmConfig::default();
    config.backend = backend.to_string();
    config.model_id = model_id.to_string();
    config
}

#[allow(dead_code)]
fn make_manifest(model_id: &str, file_ext: &str) -> EmbeddedModelManifest {
    EmbeddedModelManifest {
        model_id: model_id.to_string(),
        display_name: model_id.to_string(),
        chat_template: ChatTemplate::ChatMl,
        artifacts: vec![EmbeddedModelArtifact {
            file_name: format!("{}.{}", model_id, file_ext),
            size_bytes: 0,
            sha256: None,
            source_url: None,
        }],
    }
}

#[test]
fn test_backend_selection_candle_config() {
    let config = make_config("candle", "smollm2-360m-instruct-q4");
    assert_eq!(config.backend, "candle");
}

#[test]
fn test_availability_reports_available_with_features() {
    // With embedded-llm compiled, availability should be Available.
    let config = InternalLlmConfig::default();
    let runtime = EmbeddedRuntime::with_cache_root(config, std::env::temp_dir()).unwrap();
    if cfg!(feature = "embedded-llm") {
        assert_eq!(runtime.availability(), RuntimeAvailability::Available);
    } else {
        assert_eq!(runtime.availability(), RuntimeAvailability::RequiresFeature);
    }
}
#[test]
fn test_accelerator_config_cpu_default() {
    let config = InternalLlmConfig::default();
    assert_eq!(config.accelerator, "cpu");
}

#[test]
fn test_accelerator_config_gpu() {
    let mut config = InternalLlmConfig::default();
    config.accelerator = "gpu".to_string();
    assert_eq!(config.accelerator, "gpu");
}

#[test]
fn test_accelerator_config_npu() {
    let mut config = InternalLlmConfig::default();
    config.accelerator = "npu".to_string();
    assert_eq!(config.accelerator, "npu");
}

#[test]
fn test_manifest_has_candle_models() {
    assert!(ragent_llm::embedded::known_model_manifest("smollm2-360m-instruct-q4").is_some());
    assert!(ragent_llm::embedded::known_model_manifest("tinyllama-1.1b-chat-q4").is_some());
}
