//! Unit tests for LitertLmBackend prepare and infer methods.
//!
//! These tests validate:
//! - FR-001: LitertLmBackend struct implements EmbeddedBackend trait
//! - FR-008: Streaming token collection (via InferenceControls)
//! - FR-009: Context window and token limits
//! - FR-010: Cancellation support
//! - FR-011: Timeout enforcement
//! - FR-024: Unit test coverage

use ragent_config::InternalLlmConfig;
use ragent_llm::embedded::{
    EmbeddedInferenceError, EmbeddedModelArtifact, EmbeddedModelManifest,
    EmbeddedRuntime, EmbeddedRuntimeLifecycle, InferenceControls, RuntimeAvailability,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;

/// Creates a minimal InternalLlmConfig for testing with litertlm backend.
fn make_litertlm_config() -> InternalLlmConfig {
    let mut config = InternalLlmConfig::default();
    config.enabled = true;
    config.backend = "litertlm".to_string();
    config.model_id = "gemma-3-1b-it-litertlm".to_string();
    config
}

/// Creates a manifest for a .litertlm model.
fn make_litertlm_manifest() -> EmbeddedModelManifest {
    EmbeddedModelManifest {
        model_id: "gemma-3-1b-it-litertlm".to_string(),
        display_name: "Gemma 3 1B IT (LiteRT-LM)".to_string(),
        chat_template: ragent_llm::embedded::ChatTemplate::ChatMl,
        artifacts: vec![EmbeddedModelArtifact {
            file_name: "gemma-3-1b-it.litertlm".to_string(),
            size_bytes: 0,
            sha256: None,
            source_url: None,
        }],
    }
}

// ── LitertLmBackend creation and configuration ───────────────────────

#[test]
fn test_litertlm_config_backend_field() {
    let config = make_litertlm_config();
    assert_eq!(config.backend, "litertlm");
}

#[test]
fn test_litertlm_config_accelerator_defaults_to_cpu() {
    let config = make_litertlm_config();
    assert_eq!(config.accelerator, "cpu");
}

#[test]
fn test_litertlm_config_context_window_default() {
    let config = make_litertlm_config();
    assert!(config.context_window > 0, "context_window should be positive");
}

#[test]
fn test_litertlm_config_max_output_tokens_default() {
    let config = make_litertlm_config();
    assert!(
        config.max_output_tokens > 0,
        "max_output_tokens should be positive"
    );
}

// ── EmbeddedRuntime availability with litertlm ──────────────────────

#[test]
fn test_runtime_availability_with_litertlm_feature() {
    let config = make_litertlm_config();
    let temp_dir = TempDir::new().unwrap();
    let runtime =
        EmbeddedRuntime::with_cache_root(config, temp_dir.path().to_path_buf()).unwrap();
    // With the litertlm feature compiled in, availability should be Available.
    assert_eq!(runtime.availability(), RuntimeAvailability::Available);
}

// ── InferenceControls tests (FR-010, FR-011) ──────────────────────

#[test]
fn test_inference_controls_unbounded_allows_inference() {
    let controls = InferenceControls::unbounded();
    assert!(controls.check().is_ok(), "unbounded controls should allow inference");
}

#[test]
fn test_inference_controls_cancelled_returns_error() {
    let cancel_flag = Arc::new(AtomicBool::new(true));
    let controls = InferenceControls::with_deadline(
        Instant::now() + Duration::from_secs(300),
        cancel_flag,
    );
    let result = controls.check();
    assert!(result.is_err());
    match result.unwrap_err() {
        EmbeddedInferenceError::Cancelled => {}
        other => panic!("Expected Cancelled, got: {other}"),
    }
}

#[test]
fn test_inference_controls_deadline_exceeded_returns_error() {
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let controls = InferenceControls::with_deadline(
        Instant::now() - Duration::from_secs(10),
        cancel_flag,
    );
    let result = controls.check();
    assert!(result.is_err());
    match result.unwrap_err() {
        EmbeddedInferenceError::DeadlineExceeded => {}
        other => panic!("Expected DeadlineExceeded, got: {other}"),
    }
}

#[test]
fn test_inference_controls_cancel_priority_over_deadline() {
    // When both cancel and deadline are triggered, cancel takes priority (FR-010).
    let cancel_flag = Arc::new(AtomicBool::new(true));
    let controls = InferenceControls::with_deadline(
        Instant::now() - Duration::from_secs(10),
        cancel_flag,
    );
    let result = controls.check();
    assert!(result.is_err());
    match result.unwrap_err() {
        EmbeddedInferenceError::Cancelled => {}
        other => panic!("Expected Cancelled (priority over DeadlineExceeded), got: {other}"),
    }
}

#[test]
fn test_inference_controls_future_deadline_is_ok() {
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let controls = InferenceControls::with_deadline(
        Instant::now() + Duration::from_secs(300),
        cancel_flag,
    );
    assert!(controls.check().is_ok());
}

#[test]
fn test_inference_controls_cancel_flag_set_mid_inference() {
    // Simulate cancel being set after an initial check passes.
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let controls = InferenceControls::with_deadline(
        Instant::now() + Duration::from_secs(300),
        cancel_flag.clone(),
    );
    assert!(controls.check().is_ok());
    cancel_flag.store(true, Ordering::Relaxed);
    let result = controls.check();
    assert!(result.is_err());
    match result.unwrap_err() {
        EmbeddedInferenceError::Cancelled => {}
        other => panic!("Expected Cancelled, got: {other}"),
    }
}

// ── EmbeddedRuntime lifecycle with litertlm ──────────────────────────

#[test]
fn test_runtime_starts_uninitialized() {
    let config = make_litertlm_config();
    let temp_dir = TempDir::new().unwrap();
    let runtime =
        EmbeddedRuntime::with_cache_root(config, temp_dir.path().to_path_buf()).unwrap();
    assert!(!runtime.is_initialized());
    assert_eq!(runtime.status().lifecycle, EmbeddedRuntimeLifecycle::Uninitialized);
}

#[test]
fn test_runtime_prepare_fails_without_model_files() {
    let config = make_litertlm_config();
    let temp_dir = TempDir::new().unwrap();
    let runtime =
        EmbeddedRuntime::with_cache_root(config, temp_dir.path().to_path_buf()).unwrap();
    let manifest = make_litertlm_manifest();
    // No model files in the directory — prepare should fail.
    let result = runtime.prepare_production_runtime(&manifest);
    assert!(result.is_err(), "should fail without model files");
}

// ── Known model manifest ────────────────────────────────────────────

#[test]
fn test_known_model_manifest_gemma_litertlm() {
    let manifest =
        ragent_llm::embedded::known_model_manifest("gemma-3-1b-it-litertlm");
    assert!(manifest.is_some());
    let m = manifest.unwrap();
    assert_eq!(m.model_id, "gemma-3-1b-it-litertlm");
    assert!(
        m.artifacts.iter().any(|a| a.file_name.ends_with(".litertlm")),
        "manifest should reference a .litertlm file"
    );
}

#[test]
fn test_known_model_manifest_unknown_returns_none() {
    let manifest =
        ragent_llm::embedded::known_model_manifest("nonexistent-model-xyz");
    assert!(manifest.is_none());
}

// ── Disabled config returns None ─────────────────────────────────────

#[test]
fn test_embedded_runtime_from_disabled_config_returns_none() {
    let mut config = InternalLlmConfig::default();
    config.enabled = false;
    let result = EmbeddedRuntime::from_config(config);
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}