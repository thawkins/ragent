//! LiteRT-LM backend for the embedded internal-LLM runtime.
//!
//! This module uses Google's LiteRT-LM edge runtime (via the `litertlm` crate)
//! to run on-device LLM inference with CPU, GPU, or NPU acceleration.
//!
//! The backend expects:
//! - A `.litertlm` model file (converted from GGUF or SafeTensors via the
//!   `litertlm` CLI)
//! - A `tokenizer.json` file in the same directory (standard HuggingFace
//!   format)
//!
//! Both files may be downloaded automatically when `download_policy` allows it.

use anyhow::{Context, Result, bail};
use ragent_config::InternalLlmConfig;
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};
use tracing::{info, warn};

use litertlm::{Backend, Engine, EngineSettings, SamplerParams};

use super::{
    EmbeddedBackend, EmbeddedInferenceError, EmbeddedModelManifest, EmbeddedRuntimeSettings,
    InferenceControls,
};

/// Internal state created during `prepare()` and held for the lifetime of inference.
struct LitertLmState {
    engine: Engine,
    model_path: PathBuf,
}

/// LiteRT-LM–backed `EmbeddedBackend` implementation.
///
/// After `prepare()` is called the `.litertlm` model is loaded into memory via
/// the LiteRT-LM C runtime. Inference calls create a session per request,
/// format the prompt, and collect streamed tokens into a complete response.
pub struct LitertLmBackend {
    state: Mutex<Option<LitertLmState>>,
    /// Sampling temperature (lower = more deterministic).
    temperature: f32,
    /// RNG seed for reproducible sampling.
    seed: i32,
    /// Hard cap on generated tokens (independent of the per-request limit).
    max_gen_tokens: u32,
    /// Maximum prompt + output tokens supported by the configured runtime.
    context_window: usize,
    /// Hardware accelerator selection ("cpu", "gpu", or "npu").
    accelerator: String,
}

impl LitertLmBackend {
    /// Creates a new backend from the agent's `InternalLlmConfig`.
    #[must_use]
    pub fn new(config: &InternalLlmConfig) -> Self {
        Self {
            state: Mutex::new(None),
            temperature: 0.1,
            seed: 42,
            max_gen_tokens: config.max_output_tokens.max(1),
            context_window: config.context_window.max(1),
            accelerator: config.accelerator.clone(),
        }
    }
}

impl EmbeddedBackend for LitertLmBackend {
    fn name(&self) -> &str {
        "litertlm"
    }

    fn prepare(
        &self,
        manifest: &EmbeddedModelManifest,
        model_dir: &Path,
        config: &InternalLlmConfig,
    ) -> Result<()> {
        let litertlm_path = find_litertlm_in_dir(manifest, model_dir)
            .context("No .litertlm file found in embedded model directory")?;

        info!(path = %litertlm_path.display(), "Initialising LiteRT-LM backend");

        let backend = match config.accelerator.as_str() {
            "gpu" => Backend::Gpu,
            "npu" => Backend::Npu,
            _ => Backend::Cpu,
        };

        let max_tokens = i32::try_from(config.context_window).unwrap_or(i32::MAX);

        let settings = EngineSettings::new(&litertlm_path)
            .backend(backend)
            .max_num_tokens(max_tokens);

        let engine = Engine::new(settings).map_err(|e| {
            anyhow::anyhow!(
                "Failed to create LiteRT-LM engine from '{}': {}",
                litertlm_path.display(),
                e
            )
        })?;

        info!(
            model_id = %manifest.model_id,
            path = %litertlm_path.display(),
            accelerator = %config.accelerator,
            "LiteRT-LM engine created"
        );

        let mut guard = self
            .state
            .lock()
            .map_err(|e| anyhow::anyhow!("LitertLmBackend mutex poisoned: {e}"))?;

        *guard = Some(LitertLmState {
            engine,
            model_path: litertlm_path,
        });

        info!(
            model_id = %manifest.model_id,
            "LiteRT-LM backend prepared"
        );
        Ok(())
    }

      fn infer(
          &self,
          system_prompt: &str,
          user_prompt: &str,
          max_tokens: u32,
          controls: &InferenceControls,
      ) -> std::result::Result<String, EmbeddedInferenceError> {
          controls.check()?;
    
          let mut guard = self.state.lock().map_err(|e| {
              EmbeddedInferenceError::Other(anyhow::anyhow!("LitertLmBackend mutex poisoned: {e}"))
          })?;
    
          let state = guard.as_ref().ok_or_else(|| {
              EmbeddedInferenceError::Other(anyhow::anyhow!(
                  "LitertLmBackend not prepared; call prepare() first"
              ))
          })?;
    
          // Combine system and user prompts into a single prompt string.
          let prompt = if system_prompt.is_empty() {
              user_prompt.to_string()
          } else {
              format!("{system_prompt}\n\n{user_prompt}")
          };
    
          // Create a fresh session for this inference request.
          let mut session = state
              .engine
              .create_session(
                  SamplerParams::default()
                      .temperature(self.temperature)
                      .seed(self.seed),
              )
              .map_err(|e| {
                  EmbeddedInferenceError::Other(anyhow::anyhow!(
                      "LiteRT-LM session creation failed: {e}"
                  ))
              })?;
    
          let effective_max_tokens = max_tokens.min(self.max_gen_tokens);
          let mut response = String::new();
          let mut tokens_generated: u32 = 0;
    
          // Track whether generation was interrupted by cancellation/deadline
          // so we can return the appropriate error instead of a partial response.
          let mut interrupted_by_control = false;
    
          // Use streaming generation so we can check cancellation/deadline per token.
          // The litertlm crate's generate_stream() calls on_token for each chunk
          // and blocks until generation completes (is_final=true or error).
          session
              .generate_stream(&prompt, |chunk| {
                  // Check cancellation and deadline between tokens.
                  // FR-010: Respect cancel_flag — abort generation when set.
                  // FR-011: Respect deadline — abort when exceeded.
                  if controls.check().is_err() {
                      interrupted_by_control = true;
                      return false; // Signal to stop generation.
                  }
    
                  if tokens_generated >= effective_max_tokens {
                      return false; // Respect token limit.
                  }
    
                  response.push_str(chunk);
                  tokens_generated += 1;
                  true // Continue generation.
              })
              .map_err(|e| {
                  EmbeddedInferenceError::Other(anyhow::anyhow!("LiteRT-LM generation failed: {e}"))
              })?;
    
          // If the stream was interrupted by cancellation or deadline, return the
          // appropriate error. This must be checked BEFORE the token-limit branch
          // because a cancelled/deadline-exceeded request takes priority.
          if interrupted_by_control {
              // Re-check to determine the specific error variant.
              return Err(controls.check().unwrap_err());
          }
    
          Ok(response)
      }}

/// Produces runtime settings describing the LiteRT-LM backend configuration.
pub(crate) fn litertlm_runtime_settings(config: &InternalLlmConfig) -> EmbeddedRuntimeSettings {
    let accelerator = config.accelerator.as_str();
    let execution_device = match accelerator {
        "gpu" => "GPU (Metal/CUDA/Vulkan)",
        "npu" => "NPU",
        _ => "CPU",
    };
    EmbeddedRuntimeSettings {
        execution_device: execution_device.to_string(),
        quantized_runtime: "litertlm via litert-lm C API".to_string(),
        requested_threads: config.threads,
        effective_threads: 0, // LiteRT-LM manages its own thread pool
        threading: "LiteRT-LM manages its own thread pool internally".to_string(),
        requested_gpu_layers: config.gpu_layers,
        effective_gpu_layers: 0,
        gpu_offload: format!("{accelerator} acceleration via LiteRT-LM"),
    }
}

/// Returns the first `.litertlm` file found in `model_dir` that is listed in
/// the manifest, falling back to any `.litertlm` file in the directory if
/// none matches.
fn find_litertlm_in_dir(manifest: &EmbeddedModelManifest, model_dir: &Path) -> Option<PathBuf> {
    // Prefer manifest-listed artifacts.
    for artifact in &manifest.artifacts {
        if artifact.file_name.ends_with(".litertlm") {
            let path = model_dir.join(&artifact.file_name);
            if path.exists() {
                return Some(path);
            }
        }
    }
    // Fallback: scan directory.
    if let Ok(entries) = std::fs::read_dir(model_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("litertlm") {
                return Some(path);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedded::{EmbeddedModelArtifact, InferenceControls};
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    #[test]
    fn test_litertlm_backend_name() {
        let config = InternalLlmConfig::default();
        let backend = LitertLmBackend::new(&config);
        assert_eq!(backend.name(), "litertlm");
    }

    #[test]
    fn test_litertlm_new_reads_config_defaults() {
        let mut config = InternalLlmConfig::default();
        config.max_output_tokens = 200;
        config.context_window = 512;
        config.accelerator = "gpu".to_string();
        let backend = LitertLmBackend::new(&config);
        assert_eq!(backend.name(), "litertlm");
        assert_eq!(backend.max_gen_tokens, 200);
        assert_eq!(backend.context_window, 512);
        assert_eq!(backend.accelerator, "gpu");
        assert_eq!(backend.temperature, 0.1);
        assert_eq!(backend.seed, 42);
    }

    #[test]
    fn test_litertlm_new_clamps_max_output_tokens_to_one() {
        let mut config = InternalLlmConfig::default();
        config.max_output_tokens = 0;
        let backend = LitertLmBackend::new(&config);
        assert_eq!(backend.max_gen_tokens, 1);
    }

    #[test]
    fn test_litertlm_new_clamps_context_window_to_one() {
        let mut config = InternalLlmConfig::default();
        config.context_window = 0;
        let backend = LitertLmBackend::new(&config);
        assert_eq!(backend.context_window, 1);
    }

    #[test]
    fn test_litertlm_infer_fails_when_not_prepared() {
        let config = InternalLlmConfig::default();
        let backend = LitertLmBackend::new(&config);
        let controls = InferenceControls::unbounded();
        let result = backend.infer("system", "user", 100, &controls);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("not prepared"),
            "Expected 'not prepared' error, got: {msg}"
        );
    }

    #[test]
    fn test_litertlm_infer_cancel_flag_aborts_before_start() {
        // FR-010: When cancel_flag is set before inference starts,
        // infer() should return Cancelled immediately.
        let config = InternalLlmConfig::default();
        let backend = LitertLmBackend::new(&config);
        let cancel_flag = Arc::new(AtomicBool::new(true)); // Cancelled upfront
        let controls = InferenceControls::with_deadline(
            Instant::now() + Duration::from_secs(300),
            cancel_flag,
        );
        let result = backend.infer("system", "user", 100, &controls);
        assert!(result.is_err());
        match result.unwrap_err() {
            EmbeddedInferenceError::Cancelled => {}
            other => panic!("Expected Cancelled, got: {other}"),
        }
    }

    #[test]
    fn test_litertlm_infer_deadline_exceeded_before_start() {
        // FR-011: When deadline is already past before inference starts,
        // infer() should return DeadlineExceeded immediately.
        let config = InternalLlmConfig::default();
        let backend = LitertLmBackend::new(&config);
        let cancel_flag = Arc::new(AtomicBool::new(false));
        // Deadline in the past
        let controls =
            InferenceControls::with_deadline(Instant::now() - Duration::from_secs(10), cancel_flag);
        let result = backend.infer("system", "user", 100, &controls);
        assert!(result.is_err());
        match result.unwrap_err() {
            EmbeddedInferenceError::DeadlineExceeded => {}
            other => panic!("Expected DeadlineExceeded, got: {other}"),
        }
    }

    #[test]
    fn test_litertlm_prepare_fails_without_litertlm_file() {
        let config = InternalLlmConfig::default();
        let backend = LitertLmBackend::new(&config);
        let manifest = EmbeddedModelManifest {
            model_id: "nonexistent-model".to_string(),
            display_name: "Nonexistent".to_string(),
            chat_template: crate::embedded::ChatTemplate::ChatMl,
            artifacts: vec![EmbeddedModelArtifact {
                file_name: "nonexistent.litertlm".to_string(),
                size_bytes: 0,
                sha256: None,
                source_url: None,
            }],
        };
        let dir = std::env::temp_dir().join("ragent_test_no_litertlm_file");
        let _ = std::fs::create_dir_all(&dir);
        let result = backend.prepare(&manifest, &dir, &config);
        assert!(result.is_err(), "Expected error when .litertlm file is missing");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_litertlm_find_in_manifest_returns_none_when_empty() {
        let manifest = EmbeddedModelManifest {
            model_id: "test".to_string(),
            display_name: "Test".to_string(),
            chat_template: crate::embedded::ChatTemplate::ChatMl,
            artifacts: vec![],
        };
        let dir = std::env::temp_dir();
        assert!(find_litertlm_in_dir(&manifest, &dir).is_none());
    }

    #[test]
    fn test_litertlm_find_in_manifest_returns_existing_file() {
        let dir = std::env::temp_dir().join("ragent_test_find_litertlm");
        let _ = std::fs::create_dir_all(&dir);
        let file_path = dir.join("model.litertlm");
        std::fs::write(&file_path, "fake").unwrap();
        let manifest = EmbeddedModelManifest {
            model_id: "test".to_string(),
            display_name: "Test".to_string(),
            chat_template: crate::embedded::ChatTemplate::ChatMl,
            artifacts: vec![EmbeddedModelArtifact {
                file_name: "model.litertlm".to_string(),
                size_bytes: 5,
                sha256: None,
                source_url: None,
            }],
        };
        let found = find_litertlm_in_dir(&manifest, &dir);
        assert!(found.is_some());
        assert_eq!(found.unwrap().file_name().unwrap(), "model.litertlm");
        let _ = std::fs::remove_file(&file_path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_litertlm_find_in_manifest_falls_back_to_directory_scan() {
        let dir = std::env::temp_dir().join("ragent_test_find_litertlm_fallback");
        let _ = std::fs::create_dir_all(&dir);
        let file_path = dir.join("unlisted-model.litertlm");
        std::fs::write(&file_path, "fake").unwrap();
        // Manifest has no .litertlm artifacts — should fall back to scanning directory.
        let manifest = EmbeddedModelManifest {
            model_id: "test".to_string(),
            display_name: "Test".to_string(),
            chat_template: crate::embedded::ChatTemplate::ChatMl,
            artifacts: vec![],
        };
        let found = find_litertlm_in_dir(&manifest, &dir);
        assert!(found.is_some());
        assert_eq!(
            found.unwrap().file_name().unwrap(),
            "unlisted-model.litertlm"
        );
        let _ = std::fs::remove_file(&file_path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_litertlm_runtime_settings_cpu() {
        let mut config = InternalLlmConfig::default();
        config.accelerator = "cpu".to_string();
        let settings = litertlm_runtime_settings(&config);
        assert_eq!(settings.execution_device, "CPU");
        assert_eq!(settings.quantized_runtime, "litertlm via litert-lm C API");
        assert_eq!(
            settings.threading,
            "LiteRT-LM manages its own thread pool internally"
        );
    }

    #[test]
    fn test_litertlm_runtime_settings_gpu() {
        let mut config = InternalLlmConfig::default();
        config.accelerator = "gpu".to_string();
        let settings = litertlm_runtime_settings(&config);
        assert_eq!(settings.execution_device, "GPU (Metal/CUDA/Vulkan)");
    }

    #[test]
    fn test_litertlm_runtime_settings_npu() {
        let mut config = InternalLlmConfig::default();
        config.accelerator = "npu".to_string();
        let settings = litertlm_runtime_settings(&config);
        assert_eq!(settings.execution_device, "NPU");
    }

    #[test]
    fn test_litertlm_runtime_settings_default_accelerator_is_cpu() {
        let config = InternalLlmConfig::default();
        assert_eq!(config.accelerator, "cpu");
        let settings = litertlm_runtime_settings(&config);
        assert_eq!(settings.execution_device, "CPU");
    }

    #[test]
    fn test_inference_controls_unbounded_is_ok() {
        let controls = InferenceControls::unbounded();
        assert!(controls.check().is_ok());
    }

    #[test]
    fn test_inference_controls_cancel_flag_triggers_cancelled() {
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
    fn test_inference_controls_deadline_exceeded_triggers_error() {
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let controls = InferenceControls::with_deadline(
            Instant::now() - Duration::from_secs(1),
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
    fn test_inference_controls_cancel_takes_priority_over_deadline() {
        // When both cancel and deadline are triggered, cancel takes priority.
        let cancel_flag = Arc::new(AtomicBool::new(true));
        let controls = InferenceControls::with_deadline(
            Instant::now() - Duration::from_secs(1), // Also past deadline
            cancel_flag,
        );
        let result = controls.check();
        assert!(result.is_err());
        match result.unwrap_err() {
            EmbeddedInferenceError::Cancelled => {}
            other => panic!("Expected Cancelled (priority over DeadlineExceeded), got: {other}"),
        }
    }
}
