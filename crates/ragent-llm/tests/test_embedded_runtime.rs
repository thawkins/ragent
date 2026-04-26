use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use ragent_config::InternalLlmConfig;
use ragent_llm::embedded::{
    EmbeddedBackend, EmbeddedInferenceError, EmbeddedModelArtifact, EmbeddedModelManifest,
    EmbeddedRuntime, EmbeddedRuntimeLifecycle, InferenceControls, RuntimeAvailability,
};
use tempfile::TempDir;

struct FakeBackend {
    calls: Arc<AtomicUsize>,
}

impl EmbeddedBackend for FakeBackend {
    fn name(&self) -> &str {
        "fake"
    }

    fn prepare(
        &self,
        _manifest: &EmbeddedModelManifest,
        model_dir: &Path,
        _config: &InternalLlmConfig,
    ) -> anyhow::Result<()> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        fs::write(model_dir.join("prepared.marker"), b"ok")?;
        Ok(())
    }
}

struct FlakyBackend {
    failures_remaining: Arc<AtomicUsize>,
    calls: Arc<AtomicUsize>,
}

impl EmbeddedBackend for FlakyBackend {
    fn name(&self) -> &str {
        "flaky"
    }

    fn prepare(
        &self,
        _manifest: &EmbeddedModelManifest,
        model_dir: &Path,
        _config: &InternalLlmConfig,
    ) -> anyhow::Result<()> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.failures_remaining.fetch_sub(1, Ordering::SeqCst) > 0 {
            anyhow::bail!("temporary prepare failure");
        }
        fs::write(model_dir.join("prepared.marker"), b"ok")?;
        Ok(())
    }
}

struct DeadlineAwareBackend;

impl EmbeddedBackend for DeadlineAwareBackend {
    fn name(&self) -> &str {
        "deadline-aware"
    }

    fn prepare(
        &self,
        _manifest: &EmbeddedModelManifest,
        model_dir: &Path,
        _config: &InternalLlmConfig,
    ) -> anyhow::Result<()> {
        fs::write(model_dir.join("prepared.marker"), b"ok")?;
        Ok(())
    }

    fn infer(
        &self,
        _system_prompt: &str,
        _user_prompt: &str,
        _max_tokens: u32,
        controls: &InferenceControls,
    ) -> Result<String, EmbeddedInferenceError> {
        loop {
            controls.check()?;
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

fn test_config() -> InternalLlmConfig {
    let mut config = InternalLlmConfig::default();
    config.enabled = true;
    config.model_id = "smollm2-360m-instruct-q4".to_string();
    config
}

fn test_manifest(size_bytes: u64) -> EmbeddedModelManifest {
    EmbeddedModelManifest {
        model_id: "smollm2-360m-instruct-q4".to_string(),
        display_name: "Test Model".to_string(),
        chat_template: ragent_llm::ChatTemplate::default(),
        artifacts: vec![EmbeddedModelArtifact {
            file_name: "model.gguf".to_string(),
            size_bytes,
            sha256: None,
            source_url: None,
        }],
    }
}

#[test]
fn test_embedded_runtime_from_disabled_config_returns_none() {
    let runtime = EmbeddedRuntime::from_config(InternalLlmConfig::default()).unwrap();
    assert!(runtime.is_none());
}

#[test]
fn test_embedded_runtime_rejects_manifest_above_sub1g_limit() {
    let temp_dir = TempDir::new().unwrap();
    let runtime = EmbeddedRuntime::with_cache_root(test_config(), temp_dir.path().to_path_buf())
        .expect("runtime should build");
    let manifest = test_manifest(1_073_741_825);
    let err = runtime
        .validate_manifest(&manifest)
        .unwrap_err()
        .to_string();
    assert!(err.contains("Sub-1G"), "unexpected error: {err}");
}

#[test]
fn test_embedded_runtime_is_lazy_and_prepares_only_once() {
    let temp_dir = TempDir::new().unwrap();
    let runtime = EmbeddedRuntime::with_cache_root(test_config(), temp_dir.path().to_path_buf())
        .expect("runtime should build");
    let manifest = test_manifest(4);

    assert!(!runtime.is_initialized(), "runtime should start dormant");
    let model_dir = runtime.model_dir();
    fs::create_dir_all(&model_dir).unwrap();
    fs::write(model_dir.join("model.gguf"), b"rust").unwrap();

    let calls = Arc::new(AtomicUsize::new(0));
    let backend = FakeBackend {
        calls: calls.clone(),
    };

    runtime
        .prepare_with_backend(&manifest, Arc::new(backend))
        .expect("first prepare should work");
    let backend2 = FakeBackend {
        calls: calls.clone(),
    };
    runtime
        .prepare_with_backend(&manifest, Arc::new(backend2))
        .expect("second prepare should be a no-op");

    assert!(runtime.is_initialized(), "runtime should now be ready");
    assert_eq!(calls.load(Ordering::SeqCst), 1, "backend should run once");
}

#[test]
fn test_embedded_runtime_reports_cache_hit_and_miss() {
    let temp_dir = TempDir::new().unwrap();
    let runtime = EmbeddedRuntime::with_cache_root(test_config(), temp_dir.path().to_path_buf())
        .expect("runtime should build");
    let manifest = test_manifest(4);

    assert!(
        !runtime.artifacts_complete(&manifest).unwrap(),
        "cache should miss before artifact exists"
    );

    let model_dir = runtime.model_dir();
    fs::create_dir_all(&model_dir).unwrap();
    fs::write(model_dir.join("model.gguf"), b"rust").unwrap();

    assert!(
        runtime.artifacts_complete(&manifest).unwrap(),
        "cache should hit after artifact exists"
    );
}

#[test]
fn test_embedded_runtime_status_tracks_lifecycle() {
    let temp_dir = TempDir::new().unwrap();
    let runtime = EmbeddedRuntime::with_cache_root(test_config(), temp_dir.path().to_path_buf())
        .expect("runtime should build");
    let manifest = test_manifest(4);

    let initial = runtime.status();
    assert_eq!(initial.lifecycle, EmbeddedRuntimeLifecycle::Uninitialized);
    match initial.availability {
        RuntimeAvailability::Available => {
            assert_eq!(initial.settings.execution_device, "cpu");
            assert!(initial.settings.quantized_runtime.contains("gguf"));
        }
        RuntimeAvailability::RequiresFeature => {
            assert_eq!(initial.settings.execution_device, "unavailable");
            assert!(initial.settings.quantized_runtime.contains("not compiled"));
        }
    }
    assert_eq!(initial.settings.requested_gpu_layers, 0);
    assert_eq!(initial.settings.effective_gpu_layers, 0);

    let model_dir = runtime.model_dir();
    fs::create_dir_all(&model_dir).unwrap();
    fs::write(model_dir.join("model.gguf"), b"rust").unwrap();
    let backend = FakeBackend {
        calls: Arc::new(AtomicUsize::new(0)),
    };
    runtime
        .prepare_with_backend(&manifest, Arc::new(backend))
        .expect("prepare should succeed");

    let ready = runtime.status();
    assert_eq!(ready.lifecycle, EmbeddedRuntimeLifecycle::Ready);
    assert_eq!(ready.backend_name.as_deref(), Some("fake"));
}

#[test]
fn test_embedded_runtime_status_surfaces_requested_and_effective_settings() {
    let temp_dir = TempDir::new().unwrap();
    let mut config = test_config();
    config.threads = 2;
    config.gpu_layers = 6;
    let runtime = EmbeddedRuntime::with_cache_root(config, temp_dir.path().to_path_buf())
        .expect("runtime should build");

    let status = runtime.status();
    match status.availability {
        RuntimeAvailability::Available => {
            assert_eq!(status.settings.execution_device, "cpu");
            assert!(status.settings.quantized_runtime.contains("gguf"));
            assert!(status.settings.effective_threads >= 1);
        }
        RuntimeAvailability::RequiresFeature => {
            assert_eq!(status.settings.execution_device, "unavailable");
            assert!(status.settings.quantized_runtime.contains("not compiled"));
            assert_eq!(status.settings.effective_threads, 0);
        }
    }
    assert_eq!(status.settings.requested_threads, 2);
    assert!(!status.settings.threading.is_empty());
    assert_eq!(status.settings.requested_gpu_layers, 6);
    assert_eq!(status.settings.effective_gpu_layers, 0);
    match status.availability {
        RuntimeAvailability::Available => {
            assert!(
                status
                    .settings
                    .gpu_offload
                    .contains("forcing 0 effective layers")
            );
        }
        RuntimeAvailability::RequiresFeature => {
            assert!(status.settings.gpu_offload.contains("disabled"));
        }
    }
}

#[test]
fn test_embedded_runtime_can_retry_after_prepare_failure() {
    let temp_dir = TempDir::new().unwrap();
    let runtime = EmbeddedRuntime::with_cache_root(test_config(), temp_dir.path().to_path_buf())
        .expect("runtime should build");
    let manifest = test_manifest(4);

    let model_dir = runtime.model_dir();
    fs::create_dir_all(&model_dir).unwrap();
    fs::write(model_dir.join("model.gguf"), b"rust").unwrap();

    let failures_remaining = Arc::new(AtomicUsize::new(1));
    let calls = Arc::new(AtomicUsize::new(0));

    let err = runtime
        .prepare_with_backend(
            &manifest,
            Arc::new(FlakyBackend {
                failures_remaining: failures_remaining.clone(),
                calls: calls.clone(),
            }),
        )
        .unwrap_err()
        .to_string();
    assert!(err.contains("temporary prepare failure"));
    assert_eq!(
        runtime.status().lifecycle,
        EmbeddedRuntimeLifecycle::Failed,
        "first failed prepare should mark runtime failed"
    );

    runtime
        .prepare_with_backend(
            &manifest,
            Arc::new(FlakyBackend {
                failures_remaining,
                calls: calls.clone(),
            }),
        )
        .expect("second prepare should retry and succeed");

    assert_eq!(calls.load(Ordering::SeqCst), 2, "prepare should be retried");
    assert_eq!(
        runtime.status().lifecycle,
        EmbeddedRuntimeLifecycle::Ready,
        "successful retry should restore ready state"
    );
}

#[test]
fn test_embedded_runtime_inference_controls_stop_backend_work() {
    let temp_dir = TempDir::new().unwrap();
    let runtime = EmbeddedRuntime::with_cache_root(test_config(), temp_dir.path().to_path_buf())
        .expect("runtime should build");
    let manifest = test_manifest(4);

    let model_dir = runtime.model_dir();
    fs::create_dir_all(&model_dir).unwrap();
    fs::write(model_dir.join("model.gguf"), b"rust").unwrap();

    runtime
        .prepare_with_backend(&manifest, Arc::new(DeadlineAwareBackend))
        .expect("prepare should succeed");

    let controls = InferenceControls::with_deadline(
        Instant::now() + Duration::from_millis(20),
        Arc::new(AtomicBool::new(false)),
    );
    let err = runtime
        .infer("system", "user", 8, &controls)
        .expect_err("deadline-aware backend should stop on timeout");
    assert!(matches!(err, EmbeddedInferenceError::DeadlineExceeded));
}
