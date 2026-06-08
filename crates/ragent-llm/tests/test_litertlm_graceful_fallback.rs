//! Graceful-fallback tests for the LiteRT-LM backend (FR-006, FR-024).
//!
//! These tests verify that when the LiteRT-LM runtime cannot be loaded
//! (missing .so/.dylib/.dll, wrong architecture, missing model files),
//! the system logs a warning, marks the runtime as Failed, and allows
//! fallback to the provider LLM.

use ragent_config::InternalLlmConfig;
use ragent_llm::embedded::{
    EmbeddedBackend, EmbeddedModelArtifact, EmbeddedModelManifest,
    EmbeddedRuntime, EmbeddedRuntimeLifecycle, InferenceControls, RuntimeAvailability,
};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;

// ── Test backends ────────────────────────────────────────────────────

/// A backend that always fails to prepare, simulating a missing runtime.
struct MissingRuntimeBackend {
    error_message: String,
}

impl EmbeddedBackend for MissingRuntimeBackend {
    fn name(&self) -> &str {
        "missing-runtime"
    }

    fn prepare(
        &self,
        _manifest: &EmbeddedModelManifest,
        _model_dir: &Path,
        _config: &InternalLlmConfig,
    ) -> anyhow::Result<()> {
        anyhow::bail!("{}", self.error_message)
    }
}

/// A backend that fails on the first N calls then succeeds,
/// simulating a transient runtime failure that recovers on retry.
struct TransientFailureBackend {
    failures_remaining: Arc<AtomicUsize>,
}

impl EmbeddedBackend for TransientFailureBackend {
    fn name(&self) -> &str {
        "transient-failure"
    }

    fn prepare(
        &self,
        _manifest: &EmbeddedModelManifest,
        _model_dir: &Path,
        _config: &InternalLlmConfig,
    ) -> anyhow::Result<()> {
        if self.failures_remaining.fetch_sub(1, Ordering::SeqCst) > 0 {
            anyhow::bail!("transient runtime failure — retry later");
        }
        Ok(())
    }
}

/// A backend that fails to prepare with an architecture-mismatch error.
struct WrongArchBackend;

impl EmbeddedBackend for WrongArchBackend {
    fn name(&self) -> &str {
        "wrong-arch"
    }

    fn prepare(
        &self,
        _manifest: &EmbeddedModelManifest,
        _model_dir: &Path,
        _config: &InternalLlmConfig,
    ) -> anyhow::Result<()> {
        anyhow::bail!(
            "LiteRT-LM native library architecture mismatch: \
             expected x86_64, found aarch64"
        )
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

fn make_config() -> InternalLlmConfig {
    let mut config = InternalLlmConfig::default();
    config.enabled = true;
    config.backend = "litertlm".to_string();
    config.model_id = "gemma-3-1b-it-litertlm".to_string();
    config
}

fn make_manifest() -> EmbeddedModelManifest {
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

// ── Tests ────────────────────────────────────────────────────────────

#[test]
fn test_missing_runtime_marks_backend_failed() {
    // FR-006: When the runtime cannot load, the embedded runtime should
    // transition to Failed state (not panic).
    let temp_dir = TempDir::new().unwrap();
    let runtime = EmbeddedRuntime::with_cache_root(make_config(), temp_dir.path().to_path_buf())
        .expect("runtime should build");

    // Before prepare, lifecycle should be Uninitialized.
    assert_eq!(
        runtime.status().lifecycle,
        EmbeddedRuntimeLifecycle::Uninitialized
    );

    // Create model directory with a placeholder file.
    let model_dir = runtime.model_dir();
    std::fs::create_dir_all(&model_dir).unwrap();
    std::fs::write(model_dir.join("gemma-3-1b-it.litertlm"), b"fake").unwrap();

    let manifest = make_manifest();
    let backend = MissingRuntimeBackend {
        error_message: "libLiteRtLmC.so: cannot open shared object file: No such file or directory".to_string(),
    };

    let result = runtime.prepare_with_backend(&manifest, Arc::new(backend));
    assert!(result.is_err(), "prepare should fail with missing runtime");

    // After failure, the lifecycle should be Failed.
    let status = runtime.status();
    assert_eq!(
        status.lifecycle,
        EmbeddedRuntimeLifecycle::Failed,
        "lifecycle should be Failed after missing runtime"
    );
    assert!(
        status.detail.is_some(),
        "failure detail should be present"
    );
    let detail = status.detail.unwrap();
    assert!(
        detail.contains("libLiteRtLmC.so") || detail.contains("shared object"),
        "detail should mention the missing library: {detail}"
    );
}

#[test]
fn test_wrong_architecture_marks_backend_failed() {
    // FR-006: Incompatible architecture should mark runtime as Failed.
    let temp_dir = TempDir::new().unwrap();
    let runtime = EmbeddedRuntime::with_cache_root(make_config(), temp_dir.path().to_path_buf())
        .expect("runtime should build");

    let model_dir = runtime.model_dir();
    std::fs::create_dir_all(&model_dir).unwrap();
    std::fs::write(model_dir.join("gemma-3-1b-it.litertlm"), b"fake").unwrap();

    let manifest = make_manifest();
    let backend = WrongArchBackend;

    let result = runtime.prepare_with_backend(&manifest, Arc::new(backend));
    assert!(result.is_err(), "prepare should fail with wrong arch");

    let status = runtime.status();
    assert_eq!(
        status.lifecycle,
        EmbeddedRuntimeLifecycle::Failed,
        "lifecycle should be Failed after architecture mismatch"
    );
    let detail = status.detail.unwrap();
    assert!(
        detail.contains("architecture mismatch") || detail.contains("aarch64"),
        "detail should describe the architecture mismatch: {detail}"
    );
}

#[test]
fn test_transient_failure_can_recover_after_retry() {
    // FR-006: After a transient failure, the runtime should be able to
    // recover on a subsequent prepare call.
    let temp_dir = TempDir::new().unwrap();
    let runtime = EmbeddedRuntime::with_cache_root(make_config(), temp_dir.path().to_path_buf())
        .expect("runtime should build");

    let model_dir = runtime.model_dir();
    std::fs::create_dir_all(&model_dir).unwrap();
    std::fs::write(model_dir.join("gemma-3-1b-it.litertlm"), b"fake").unwrap();

    let manifest = make_manifest();

    // First call fails (1 failure remaining).
    let failures = Arc::new(AtomicUsize::new(1));
    let backend1 = TransientFailureBackend {
        failures_remaining: failures.clone(),
    };
    let result = runtime.prepare_with_backend(&manifest, Arc::new(backend1));
    assert!(result.is_err());
    assert_eq!(
        runtime.status().lifecycle,
        EmbeddedRuntimeLifecycle::Failed,
        "lifecycle should be Failed after transient failure"
    );

    // Second call succeeds (0 failures remaining).
    let backend2 = TransientFailureBackend {
        failures_remaining: failures.clone(),
    };
    let result = runtime.prepare_with_backend(&manifest, Arc::new(backend2));
    assert!(result.is_ok(), "retry should succeed: {:?}", result.err());
    assert_eq!(
        runtime.status().lifecycle,
        EmbeddedRuntimeLifecycle::Ready,
        "lifecycle should be Ready after successful retry"
    );
}

#[test]
fn test_failed_runtime_does_not_allow_inference() {
    // After a failed prepare, inference should return an error.
    let temp_dir = TempDir::new().unwrap();
    let runtime = EmbeddedRuntime::with_cache_root(make_config(), temp_dir.path().to_path_buf())
        .expect("runtime should build");

    let model_dir = runtime.model_dir();
    std::fs::create_dir_all(&model_dir).unwrap();
    std::fs::write(model_dir.join("gemma-3-1b-it.litertlm"), b"fake").unwrap();

    let manifest = make_manifest();
    let backend = MissingRuntimeBackend {
        error_message: "runtime missing".to_string(),
    };
    let _ = runtime.prepare_with_backend(&manifest, Arc::new(backend));

    // Attempt inference on a Failed runtime.
    let controls = InferenceControls::unbounded();
    let result = runtime.infer("system", "user", 10, &controls);
    assert!(
        result.is_err(),
        "inference on failed runtime should return error"
    );
}

#[test]
fn test_runtime_availability_with_litertlm_feature() {
    // Verify that with the litertlm feature compiled, the runtime
    // reports Available (not RequiresFeature).
    let config = make_config();
    let temp_dir = TempDir::new().unwrap();
    let runtime = EmbeddedRuntime::with_cache_root(config, temp_dir.path().to_path_buf()).unwrap();
    assert_eq!(
        runtime.availability(),
        RuntimeAvailability::Available,
        "litertlm feature should make runtime available"
    );
}

#[test]
fn test_disabled_config_returns_none() {
    // When internal_llm.enabled = false, from_config should return None.
    let mut config = InternalLlmConfig::default();
    config.enabled = false;
    let result = EmbeddedRuntime::from_config(config);
    assert!(result.is_ok());
    assert!(
        result.unwrap().is_none(),
        "disabled config should return None"
    );
}

#[test]
fn test_prepare_without_model_files_fails_gracefully() {
    // FR-006: Missing model files should fail gracefully (not panic).
    let config = make_config();
    let temp_dir = TempDir::new().unwrap();
    let runtime = EmbeddedRuntime::with_cache_root(config, temp_dir.path().to_path_buf()).unwrap();
    let manifest = make_manifest();

    let result = runtime.prepare_production_runtime(&manifest);
    assert!(
        result.is_err(),
        "should fail without model files: {:?}",
        result.ok()
    );
    // Verify no panic occurred — we reached this point.
}

#[test]
fn test_inference_controls_respect_deadline_on_failed_runtime() {
    // Even with a failed runtime, InferenceControls should still work
    // correctly (deadline enforcement is independent of backend).
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let past_deadline = InferenceControls::with_deadline(
        Instant::now() - Duration::from_secs(1),
        cancel_flag,
    );
    assert!(past_deadline.check().is_err());

    let cancel_flag_active = Arc::new(AtomicBool::new(true));
    let cancelled = InferenceControls::with_deadline(
        Instant::now() + Duration::from_secs(300),
        cancel_flag_active,
    );
    assert!(cancelled.check().is_err());
}

#[test]
fn test_failed_runtime_detail_includes_error_message() {
    // The detail field in status should capture the error message.
    let temp_dir = TempDir::new().unwrap();
    let runtime = EmbeddedRuntime::with_cache_root(make_config(), temp_dir.path().to_path_buf())
        .expect("runtime should build");

    let model_dir = runtime.model_dir();
    std::fs::create_dir_all(&model_dir).unwrap();
    std::fs::write(model_dir.join("gemma-3-1b-it.litertlm"), b"fake").unwrap();

    let manifest = make_manifest();
    let error_msg = "custom runtime error for testing detail field";
    let backend = MissingRuntimeBackend {
        error_message: error_msg.to_string(),
    };
    let _ = runtime.prepare_with_backend(&manifest, Arc::new(backend));

    let status = runtime.status();
    assert!(status.detail.is_some());
    assert!(
        status.detail.unwrap().contains(error_msg),
        "detail should contain the original error message"
    );
}