//! Embedded internal-LLM runtime scaffold.
//!
//! This module provides the local-runtime foundation for a small, embedded
//! model used by internal helper workflows. The runtime is lazy, cache-aware,
//! and disabled by default. When the `embedded-llm` feature is enabled the
//! candle backend is compiled in and used for actual inference.
//!
//! If either of the required model files (`.gguf` and `tokenizer.json`) are
//! absent from the local cache, and the configured `download_policy` allows it,
//! they are fetched automatically from their registered HuggingFace URLs on
//! first use.

use anyhow::{Context, Result, bail};
use futures::stream::{self, StreamExt};
use ragent_config::InternalLlmConfig;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Instant;
use thiserror::Error;
use tracing::{info, info_span, warn};

#[cfg(feature = "embedded-llm")]
mod candle_backend;

#[cfg(feature = "embedded-llm")]
pub use candle_backend::CandleBackend;

#[cfg(feature = "embedded-llm")]
use candle_backend::candle_runtime_settings;

/// The maximum allowed embedded-model artifact budget for the Sub-1G runtime.
pub const SUB_1G_MAX_BYTES: u64 = 1_073_741_824;
const MAX_PARALLEL_ARTIFACT_DOWNLOADS: usize = 4;

/// Chat prompt template used by the model.
///
/// The backend formats the system and user prompts using this template
/// before passing them to the tokeniser.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ChatTemplate {
    /// TinyLlama / Zephyr style: `<|system|>…</s><|user|>…</s><|assistant|>`
    #[default]
    TinyLlama,
    /// OpenAI ChatML: `<|im_start|>system…<|im_end|><|im_start|>user…<|im_end|><|im_start|>assistant`
    ChatMl,
}

/// A single file required by an embedded model manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmbeddedModelArtifact {
    /// File name relative to the model cache directory.
    pub file_name: String,
    /// Expected size of the file in bytes.
    pub size_bytes: u64,
    /// Optional SHA-256 checksum for integrity verification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    /// Optional URL used to download the file on demand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
}

/// Metadata for a locally cached embedded model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmbeddedModelManifest {
    /// Unique identifier for the model.
    pub model_id: String,
    /// Human-readable model name.
    pub display_name: String,
    /// Chat prompt template this model was trained with.
    #[serde(default)]
    pub chat_template: ChatTemplate,
    /// Files needed to materialise the model cache.
    #[serde(default)]
    pub artifacts: Vec<EmbeddedModelArtifact>,
}

impl EmbeddedModelManifest {
    /// Returns the total manifest size in bytes.
    #[must_use]
    pub fn total_size_bytes(&self) -> u64 {
        self.artifacts
            .iter()
            .map(|artifact| artifact.size_bytes)
            .sum()
    }
}

/// Runtime availability for the embedded-LLM scaffold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeAvailability {
    /// The compile-time feature is enabled, so a production backend may be added.
    Available,
    /// The compile-time feature is disabled; the scaffold exists but production
    /// inference remains unavailable.
    RequiresFeature,
}

/// Backend hook used by the embedded runtime during initialisation and inference.
pub trait EmbeddedBackend: Send + Sync {
    /// Returns a human-readable backend name.
    fn name(&self) -> &str;

    /// Performs backend-specific model preparation after cache validation.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend cannot prepare the cached model files.
    fn prepare(
        &self,
        manifest: &EmbeddedModelManifest,
        model_dir: &Path,
        config: &InternalLlmConfig,
    ) -> Result<()>;

    /// Run synchronous inference with the given prompts, returning generated text.
    ///
    /// `system_prompt` sets the model's behaviour; `user_prompt` is the request.
    /// The default implementation returns an error. Backends that support
    /// inference must override this method.
    ///
    /// # Errors
    ///
    /// Returns an error if inference is not implemented or fails.
    fn infer(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        max_tokens: u32,
        controls: &InferenceControls,
    ) -> std::result::Result<String, EmbeddedInferenceError> {
        let _ = (system_prompt, user_prompt, max_tokens, controls);
        Err(EmbeddedInferenceError::Other(anyhow::anyhow!(
            "Inference not implemented for backend '{}'",
            self.name()
        )))
    }
}

/// Controls applied to a single embedded inference request.
#[derive(Debug, Clone)]
pub struct InferenceControls {
    deadline: Option<Instant>,
    cancel_flag: Arc<AtomicBool>,
}

impl InferenceControls {
    /// Creates unbounded controls for tests or callers without deadlines.
    #[must_use]
    pub fn unbounded() -> Self {
        Self {
            deadline: None,
            cancel_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Creates controls with a fixed deadline and caller-owned cancellation flag.
    #[must_use]
    pub fn with_deadline(deadline: Instant, cancel_flag: Arc<AtomicBool>) -> Self {
        Self {
            deadline: Some(deadline),
            cancel_flag,
        }
    }

    /// Returns an error when the request has been cancelled or has missed its deadline.
    ///
    /// # Errors
    ///
    /// Returns [`EmbeddedInferenceError::Cancelled`] or
    /// [`EmbeddedInferenceError::DeadlineExceeded`] when the request should stop.
    pub fn check(&self) -> std::result::Result<(), EmbeddedInferenceError> {
        if self.cancel_flag.load(Ordering::Relaxed) {
            return Err(EmbeddedInferenceError::Cancelled);
        }
        if let Some(deadline) = self.deadline {
            if Instant::now() >= deadline {
                return Err(EmbeddedInferenceError::DeadlineExceeded);
            }
        }
        Ok(())
    }
}

/// Errors returned from embedded inference execution.
#[derive(Debug, Error)]
pub enum EmbeddedInferenceError {
    /// The request exceeded its deadline.
    #[error("embedded inference deadline exceeded")]
    DeadlineExceeded,
    /// The caller cancelled the request.
    #[error("embedded inference was cancelled")]
    Cancelled,
    /// Backend-specific inference failure.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RuntimeState {
    Uninitialized,
    Ready { backend_name: String },
    Failed(String),
}

/// Coarse lifecycle state for the embedded runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddedRuntimeLifecycle {
    /// Runtime has not attempted backend preparation yet.
    Uninitialized,
    /// Runtime successfully prepared a backend.
    Ready,
    /// Runtime previously failed and the last prepare attempt did not succeed.
    Failed,
}

/// Snapshot of embedded-runtime health for observability surfaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedRuntimeStatus {
    /// Configured model identifier.
    pub model_id: String,
    /// Compile-time availability of the embedded backend.
    pub availability: RuntimeAvailability,
    /// Current runtime lifecycle state.
    pub lifecycle: EmbeddedRuntimeLifecycle,
    /// Prepared backend name when the runtime is ready.
    pub backend_name: Option<String>,
    /// Failure detail when the runtime is in the failed state.
    pub detail: Option<String>,
    /// Cache root used for embedded assets.
    pub cache_root: PathBuf,
    /// Model-specific cache directory.
    pub model_dir: PathBuf,
    /// Effective execution settings exposed by the runtime/backend.
    pub settings: EmbeddedRuntimeSettings,
}

/// Effective embedded-runtime execution settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedRuntimeSettings {
    /// Execution device currently used by the runtime.
    pub execution_device: String,
    /// Quantized runtime path used for GGUF inference.
    pub quantized_runtime: String,
    /// Requested CPU thread count from config.
    pub requested_threads: usize,
    /// Effective CPU thread count actually applied by the runtime.
    pub effective_threads: usize,
    /// Human-readable explanation of the thread-control path.
    pub threading: String,
    /// Requested number of GPU-offloaded layers from config.
    pub requested_gpu_layers: u32,
    /// Effective number of GPU-offloaded layers actually applied.
    pub effective_gpu_layers: u32,
    /// Human-readable GPU offload status for the current build/runtime.
    pub gpu_offload: String,
}

/// Lazy embedded-runtime handle.
///
/// Construction validates static configuration but does not download artifacts
/// or touch any inference backend. That work happens only when explicitly
/// requested through [`EmbeddedRuntime::prepare_with_backend`].
pub struct EmbeddedRuntime {
    config: InternalLlmConfig,
    cache_root: PathBuf,
    http: reqwest::Client,
    state: Mutex<RuntimeState>,
    /// Stored backend for inference, set after successful `prepare_with_backend`.
    backend: Mutex<Option<Arc<dyn EmbeddedBackend>>>,
}

#[derive(Debug, Clone)]
struct PendingArtifactDownload {
    artifact: EmbeddedModelArtifact,
    artifact_path: PathBuf,
    source_url: String,
}

impl EmbeddedRuntime {
    /// Creates a dormant runtime handle from configuration, returning `None`
    /// when the subsystem is disabled.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is internally inconsistent.
    pub fn from_config(config: InternalLlmConfig) -> Result<Option<Self>> {
        if !config.enabled {
            return Ok(None);
        }
        Self::new(config).map(Some)
    }

    /// Creates a dormant runtime handle rooted at the default cache path.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid.
    pub fn new(config: InternalLlmConfig) -> Result<Self> {
        let cache_root = default_cache_root()?;
        Self::with_cache_root(config, cache_root)
    }

    /// Creates a dormant runtime handle rooted at a caller-specified cache path.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid.
    pub fn with_cache_root(config: InternalLlmConfig, cache_root: PathBuf) -> Result<Self> {
        validate_config(&config)?;
        Ok(Self {
            config,
            cache_root,
            http: reqwest::Client::new(),
            state: Mutex::new(RuntimeState::Uninitialized),
            backend: Mutex::new(None),
        })
    }

    /// Returns the compile-time runtime availability.
    #[must_use]
    pub fn availability(&self) -> RuntimeAvailability {
        if cfg!(feature = "embedded-llm") {
            RuntimeAvailability::Available
        } else {
            RuntimeAvailability::RequiresFeature
        }
    }

    /// Returns the configured model identifier.
    #[must_use]
    pub fn model_id(&self) -> &str {
        &self.config.model_id
    }

    /// Returns the configured cache root for embedded model assets.
    #[must_use]
    pub fn cache_root(&self) -> &Path {
        &self.cache_root
    }

    /// Returns the cache directory for the configured model.
    #[must_use]
    pub fn model_dir(&self) -> PathBuf {
        self.cache_root.join(&self.config.model_id)
    }

    /// Returns whether backend preparation has completed successfully.
    #[must_use]
    pub fn is_initialized(&self) -> bool {
        self.state
            .lock()
            .map(|state| matches!(*state, RuntimeState::Ready { .. }))
            .unwrap_or(false)
    }

    /// Returns a snapshot of runtime health and current preparation state.
    #[must_use]
    pub fn status(&self) -> EmbeddedRuntimeStatus {
        let (lifecycle, backend_name, detail) = match self.state.lock() {
            Ok(state) => match &*state {
                RuntimeState::Uninitialized => {
                    (EmbeddedRuntimeLifecycle::Uninitialized, None, None)
                }
                RuntimeState::Ready { backend_name } => (
                    EmbeddedRuntimeLifecycle::Ready,
                    Some(backend_name.clone()),
                    None,
                ),
                RuntimeState::Failed(detail) => {
                    (EmbeddedRuntimeLifecycle::Failed, None, Some(detail.clone()))
                }
            },
            Err(error) => (
                EmbeddedRuntimeLifecycle::Failed,
                None,
                Some(format!("embedded runtime mutex poisoned: {error}")),
            ),
        };

        EmbeddedRuntimeStatus {
            model_id: self.config.model_id.clone(),
            availability: self.availability(),
            lifecycle,
            backend_name,
            detail,
            cache_root: self.cache_root.clone(),
            model_dir: self.model_dir(),
            settings: runtime_settings_for_config(&self.config),
        }
    }

    /// Validates a manifest against the runtime limits.
    ///
    /// # Errors
    ///
    /// Returns an error when the manifest is empty, contains invalid entries,
    /// or exceeds either the configured or hard Sub-1G limits.
    ///
    /// Artifacts with `size_bytes = 0` are allowed when they carry a `source_url`
    /// (the size is unknown ahead of download). Total-size limits are only enforced
    /// when all artifact sizes are known.
    pub fn validate_manifest(&self, manifest: &EmbeddedModelManifest) -> Result<()> {
        if manifest.model_id.trim().is_empty() {
            bail!("Embedded model manifest must include a model_id");
        }
        if manifest.model_id != self.config.model_id {
            bail!(
                "Embedded model manifest '{}' does not match configured model '{}'",
                manifest.model_id,
                self.config.model_id
            );
        }
        if manifest.artifacts.is_empty() {
            bail!(
                "Embedded model manifest '{}' does not declare any artifacts",
                manifest.model_id
            );
        }

        // Only enforce size limits when all artifact sizes are known (non-zero).
        let total_size_bytes = manifest.total_size_bytes();
        let all_sizes_known = manifest.artifacts.iter().all(|a| a.size_bytes > 0);
        if all_sizes_known {
            if total_size_bytes > SUB_1G_MAX_BYTES {
                bail!(
                    "Embedded model '{}' exceeds the hard Sub-1G limit ({} > {})",
                    manifest.model_id,
                    total_size_bytes,
                    SUB_1G_MAX_BYTES
                );
            }
            if total_size_bytes > self.config.artifact_max_bytes {
                bail!(
                    "Embedded model '{}' exceeds configured artifact_max_bytes ({} > {})",
                    manifest.model_id,
                    total_size_bytes,
                    self.config.artifact_max_bytes
                );
            }
        }

        let mut seen = HashSet::new();
        for artifact in &manifest.artifacts {
            if artifact.file_name.trim().is_empty() {
                bail!(
                    "Embedded model manifest '{}' contains an empty file name",
                    manifest.model_id
                );
            }
            if !seen.insert(artifact.file_name.clone()) {
                bail!(
                    "Embedded model manifest '{}' contains duplicate artifact '{}'",
                    manifest.model_id,
                    artifact.file_name
                );
            }
            // Zero size is allowed only for artifacts that will be downloaded.
            if artifact.size_bytes == 0 && artifact.source_url.is_none() {
                bail!(
                    "Embedded model manifest '{}' contains zero-sized local artifact '{}'",
                    manifest.model_id,
                    artifact.file_name
                );
            }
        }

        Ok(())
    }

    /// Returns `true` when every artifact already exists and passes integrity checks.
    ///
    /// # Errors
    ///
    /// Returns an error if the manifest is invalid or a file cannot be inspected.
    pub fn artifacts_complete(&self, manifest: &EmbeddedModelManifest) -> Result<bool> {
        self.validate_manifest(manifest)?;
        let model_dir = self.model_dir();
        for artifact in &manifest.artifacts {
            let artifact_path = model_dir.join(&artifact.file_name);
            if !artifact_matches(&artifact_path, artifact)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Ensures the model cache contains every artifact declared by the manifest.
    ///
    /// # Errors
    ///
    /// Returns an error if validation fails, downloads are disallowed, or any
    /// required file cannot be fetched or verified.
    pub fn ensure_artifacts(&self, manifest: &EmbeddedModelManifest) -> Result<()> {
        let _span = info_span!(
            "internal_llm.init",
            phase = "ensure_artifacts",
            model_id = %self.config.model_id
        )
        .entered();
        self.validate_manifest(manifest)?;
        let model_dir = self.model_dir();
        fs::create_dir_all(&model_dir).with_context(|| {
            format!("Failed to create model cache dir '{}'", model_dir.display())
        })?;

        let validation_started = Instant::now();
        let mut pending_downloads = Vec::new();
        for artifact in &manifest.artifacts {
            let artifact_path = model_dir.join(&artifact.file_name);
            if artifact_matches(&artifact_path, artifact)? {
                continue;
            }

            if artifact_path.exists() {
                fs::remove_file(&artifact_path).with_context(|| {
                    format!(
                        "Failed to remove invalid cached artifact '{}'",
                        artifact_path.display()
                    )
                })?;
            }

            let source_url = artifact.source_url.as_deref().ok_or_else(|| {
                anyhow::anyhow!(
                    "Artifact '{}' is missing and has no source_url",
                    artifact.file_name
                )
            })?;

            if matches!(
                self.config.download_policy,
                ragent_config::InternalLlmDownloadPolicy::Never
            ) {
                bail!(
                    "Artifact '{}' is missing and download_policy is 'never'",
                    artifact.file_name
                );
            }

            pending_downloads.push(PendingArtifactDownload {
                artifact: artifact.clone(),
                artifact_path,
                source_url: source_url.to_string(),
            });
        }

        info!(
            model_id = %self.config.model_id,
            artifact_count = manifest.artifacts.len(),
            missing_artifacts = pending_downloads.len(),
            elapsed_ms = validation_started.elapsed().as_millis(),
            "Embedded artifact cache validation complete"
        );

        if pending_downloads.is_empty() {
            return Ok(());
        }

        let download_started = Instant::now();
        download_artifacts_batch(&self.http, &pending_downloads)?;
        info!(
            model_id = %self.config.model_id,
            downloaded_artifacts = pending_downloads.len(),
            elapsed_ms = download_started.elapsed().as_millis(),
            "Embedded artifact download batch complete"
        );

        for pending in &pending_downloads {
            if !artifact_matches(&pending.artifact_path, &pending.artifact)? {
                bail!(
                    "Downloaded artifact '{}' did not match the manifest",
                    pending.artifact.file_name
                );
            }
        }

        Ok(())
    }

    /// Performs lazy backend preparation after the cache is validated.
    ///
    /// # Errors
    ///
    /// Returns an error if artifact validation/download fails or if the backend
    /// cannot prepare the runtime.
    pub fn prepare_with_backend(
        &self,
        manifest: &EmbeddedModelManifest,
        backend: Arc<dyn EmbeddedBackend>,
    ) -> Result<()> {
        self.ensure_artifacts(manifest)?;
        self.prepare_backend(manifest, backend)
    }

    fn prepare_backend(
        &self,
        manifest: &EmbeddedModelManifest,
        backend: Arc<dyn EmbeddedBackend>,
    ) -> Result<()> {
        let _span = info_span!(
            "internal_llm.init",
            phase = "prepare_backend",
            model_id = %self.config.model_id,
            backend = backend.name()
        )
        .entered();
        {
            let state = self
                .state
                .lock()
                .map_err(|e| anyhow::anyhow!("Embedded runtime mutex poisoned: {e}"))?;
            match &*state {
                RuntimeState::Ready { .. } => return Ok(()),
                RuntimeState::Failed(err) => {
                    info!(
                        model_id = %self.config.model_id,
                        backend = backend.name(),
                        previous_error = %err,
                        "Retrying embedded runtime preparation after previous failure"
                    );
                }
                RuntimeState::Uninitialized => {}
            }
        }

        let result = (|| -> Result<()> {
            backend.prepare(manifest, &self.model_dir(), &self.config)?;
            Ok(())
        })();

        let mut state = self
            .state
            .lock()
            .map_err(|e| anyhow::anyhow!("Embedded runtime mutex poisoned: {e}"))?;
        match result {
            Ok(()) => {
                *state = RuntimeState::Ready {
                    backend_name: backend.name().to_string(),
                };
                let mut b = self
                    .backend
                    .lock()
                    .map_err(|e| anyhow::anyhow!("Embedded runtime backend mutex poisoned: {e}"))?;
                *b = Some(backend.clone());
                info!(
                    model_id = %self.config.model_id,
                    backend = backend.name(),
                    "Embedded runtime prepared"
                );
                Ok(())
            }
            Err(err) => {
                warn!(
                    model_id = %self.config.model_id,
                    backend = backend.name(),
                    error = %err,
                    "Embedded runtime preparation failed"
                );
                *state = RuntimeState::Failed(err.to_string());
                Err(err)
            }
        }
    }

    /// Runs inference using the prepared backend.
    ///
    /// # Errors
    ///
    /// Returns an error if the runtime is not ready or the backend fails to generate a response.
    pub fn infer(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        max_tokens: u32,
        controls: &InferenceControls,
    ) -> std::result::Result<String, EmbeddedInferenceError> {
        let b = self.backend.lock().map_err(|e| {
            EmbeddedInferenceError::Other(anyhow::anyhow!(
                "Embedded runtime backend mutex poisoned: {e}"
            ))
        })?;
        match &*b {
            Some(backend) => backend.infer(system_prompt, user_prompt, max_tokens, controls),
            None => Err(EmbeddedInferenceError::Other(anyhow::anyhow!(
                "Embedded runtime for model '{}' is not ready; call prepare_with_backend first",
                self.config.model_id
            ))),
        }
    }

    /// Production initialiser: discovers a GGUF model in the model directory and
    /// prepares the llama.cpp backend.
    ///
    /// # Errors
    ///
    /// Returns an error if the `embedded-llm` feature is not compiled in, if no
    /// model file can be found, or if backend preparation fails.
    pub fn prepare_production_runtime(&self, manifest: &EmbeddedModelManifest) -> Result<()> {
        let _span = info_span!(
            "internal_llm.init",
            phase = "prepare_production_runtime",
            model_id = %self.config.model_id,
            backend = %self.config.backend
        )
        .entered();
        self.validate_manifest(manifest)?;
        #[cfg(feature = "embedded-llm")]
        {
            let backend = Arc::new(CandleBackend::new(&self.config));
            return self.prepare_backend(manifest, backend);
        }
        #[cfg(not(feature = "embedded-llm"))]
        bail!("ragent-llm was built without the embedded-llm feature");
    }

    /// Discovers model files in the local cache (downloading them first if absent),
    /// then initialises the candle backend.
    ///
    /// This is the primary startup path for the agent executor. When the required
    /// files are not yet present and `download_policy` allows it, they are fetched
    /// automatically from their registered HuggingFace URLs.
    ///
    /// # Errors
    ///
    /// Returns an error when the `embedded-llm` feature is absent, the model is
    /// not found locally and cannot be downloaded, or the backend fails to initialise.
    pub fn try_init_from_cache(&self) -> Result<()> {
        #[cfg(not(feature = "embedded-llm"))]
        bail!("ragent-llm was built without the embedded-llm feature");

        #[cfg(feature = "embedded-llm")]
        {
            let model_dir = self.model_dir();
            let manifest_started = Instant::now();

            // Use the registered manifest (with download URLs) when available,
            // otherwise fall back to discovering whatever GGUF files exist locally.
            let (manifest, manifest_source) = if let Some(known) =
                known_model_manifest(&self.config.model_id)
            {
                info!(
                    model_id = %self.config.model_id,
                    model_dir = %model_dir.display(),
                    "Using registered manifest for known model; missing files will be downloaded"
                );
                (known, "registered")
            } else {
                (
                    discover_manifest_in_dir(&self.config.model_id, &model_dir)?,
                    "discovered",
                )
            };

            info!(
                model_id = %self.config.model_id,
                manifest_source,
                artifact_count = manifest.artifacts.len(),
                elapsed_ms = manifest_started.elapsed().as_millis(),
                "Embedded model manifest resolved"
            );

            self.ensure_artifacts(&manifest)?;
            let backend = Arc::new(CandleBackend::new(&self.config));
            self.prepare_backend(&manifest, backend)
        }
    }
}

fn runtime_settings_for_config(config: &InternalLlmConfig) -> EmbeddedRuntimeSettings {
    #[cfg(feature = "embedded-llm")]
    {
        candle_runtime_settings(config)
    }

    #[cfg(not(feature = "embedded-llm"))]
    {
        EmbeddedRuntimeSettings {
            execution_device: "unavailable".to_string(),
            quantized_runtime: "embedded-llm feature not compiled".to_string(),
            requested_threads: config.threads,
            effective_threads: 0,
            threading: "embedded-llm feature disabled in this build".to_string(),
            requested_gpu_layers: config.gpu_layers,
            effective_gpu_layers: 0,
            gpu_offload: "embedded-llm feature disabled in this build".to_string(),
        }
    }
}

fn validate_config(config: &InternalLlmConfig) -> Result<()> {
    if config.backend.trim().is_empty() {
        bail!("internal_llm.backend must not be empty");
    }
    if config.model_id.trim().is_empty() {
        bail!("internal_llm.model_id must not be empty");
    }
    if config.artifact_max_bytes == 0 {
        bail!("internal_llm.artifact_max_bytes must be greater than zero");
    }
    if config.artifact_max_bytes > SUB_1G_MAX_BYTES {
        bail!(
            "internal_llm.artifact_max_bytes exceeds the hard Sub-1G limit ({} > {})",
            config.artifact_max_bytes,
            SUB_1G_MAX_BYTES
        );
    }
    if config.threads == 0 {
        bail!("internal_llm.threads must be at least 1");
    }
    if config.context_window == 0 {
        bail!("internal_llm.context_window must be at least 1");
    }
    if config.max_output_tokens == 0 {
        bail!("internal_llm.max_output_tokens must be at least 1");
    }
    if config.timeout_ms == 0 {
        bail!("internal_llm.timeout_ms must be at least 1");
    }
    if config.max_parallel_requests == 0 {
        bail!("internal_llm.max_parallel_requests must be at least 1");
    }
    Ok(())
}

fn default_cache_root() -> Result<PathBuf> {
    let base = dirs::data_dir()
        .ok_or_else(|| anyhow::anyhow!("No data directory found for embedded model cache"))?;
    Ok(base.join("ragent").join("models").join("embedded"))
}

fn artifact_matches(path: &Path, artifact: &EmbeddedModelArtifact) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }

    // When size_bytes is 0 the size is unknown (e.g. downloadable artifact);
    // skip the size check and rely on sha256 or just existence.
    if artifact.size_bytes > 0 {
        let metadata = fs::metadata(path)
            .with_context(|| format!("Failed to inspect cached artifact '{}'", path.display()))?;
        if metadata.len() != artifact.size_bytes {
            return Ok(false);
        }
    }

    if let Some(expected_sha) = &artifact.sha256 {
        let actual_sha = sha256_file(path)?;
        if actual_sha != *expected_sha {
            return Ok(false);
        }
    }

    Ok(true)
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)
        .with_context(|| format!("Failed to open cached artifact '{}'", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("Failed reading '{}'", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn download_artifacts_batch(
    client: &reqwest::Client,
    downloads: &[PendingArtifactDownload],
) -> Result<()> {
    if downloads.is_empty() {
        return Ok(());
    }

    let concurrency = downloads.len().clamp(1, MAX_PARALLEL_ARTIFACT_DOWNLOADS);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("Failed to create embedded artifact download runtime")?;

    runtime.block_on(async {
        let client = client.clone();
        let results = stream::iter(downloads.iter().cloned())
            .map(|download| {
                let client = client.clone();
                async move { download_artifact_async(client, download).await }
            })
            .buffer_unordered(concurrency)
            .collect::<Vec<_>>()
            .await;

        for result in results {
            result?;
        }
        Ok(())
    })
}

async fn download_artifact_async(
    client: reqwest::Client,
    download: PendingArtifactDownload,
) -> Result<()> {
    let started = Instant::now();
    let _span = info_span!(
        "internal_llm.download",
        artifact = %download.artifact.file_name,
        url = %download.source_url,
        dest = %download.artifact_path.display()
    )
    .entered();

    info!(
        artifact = %download.artifact.file_name,
        url = %download.source_url,
        dest = %download.artifact_path.display(),
        "Downloading embedded model artifact"
    );

    use tokio::io::AsyncWriteExt;

    let response = client
        .get(&download.source_url)
        .send()
        .await
        .with_context(|| format!("Failed to connect to '{}'", download.source_url))?;
    if !response.status().is_success() {
        bail!(
            "Artifact download '{}' returned HTTP {}",
            download.source_url,
            response.status()
        );
    }

    let tmp_path = download.artifact_path.with_extension("part");
    let mut file = tokio::fs::File::create(&tmp_path)
        .await
        .with_context(|| format!("Failed to create temporary file '{}'", tmp_path.display()))?;

    let mut downloaded_bytes: u64 = 0;
    let mut response = response;
    while let Some(chunk) = response.chunk().await.with_context(|| {
        format!(
            "Failed reading response body from '{}'",
            download.source_url
        )
    })? {
        file.write_all(&chunk)
            .await
            .with_context(|| format!("Failed writing to '{}'", tmp_path.display()))?;
        downloaded_bytes += chunk.len() as u64;
    }
    file.flush()
        .await
        .with_context(|| format!("Failed flushing '{}'", tmp_path.display()))?;
    drop(file);

    tokio::fs::rename(&tmp_path, &download.artifact_path)
        .await
        .with_context(|| {
            format!(
                "Failed to move '{}' into place at '{}'",
                tmp_path.display(),
                download.artifact_path.display()
            )
        })?;

    info!(
        artifact = %download.artifact.file_name,
        url = %download.source_url,
        bytes = downloaded_bytes,
        elapsed_ms = started.elapsed().as_millis(),
        dest = %download.artifact_path.display(),
        "Embedded artifact download complete"
    );
    Ok(())
}

/// Returns the pre-registered manifest for a known built-in model, or `None`
/// if the model ID is not recognised.
///
/// Known manifests include download URLs so that missing artifacts are fetched
/// automatically on first use.
pub fn known_model_manifest(model_id: &str) -> Option<EmbeddedModelManifest> {
    match model_id {
        "smollm2-360m-instruct-q4" => Some(EmbeddedModelManifest {
            model_id: "smollm2-360m-instruct-q4".to_string(),
            display_name: "SmolLM2 360M Instruct (Q4_K_M)".to_string(),
            chat_template: ChatTemplate::ChatMl,
            artifacts: vec![
                EmbeddedModelArtifact {
                    file_name: "smollm2-360m-instruct-q4_k_m.gguf".to_string(),
                    size_bytes: 0,
                    sha256: None,
                    source_url: Some(
                        "https://huggingface.co/mfuntowicz/SmolLM2-360M-Instruct-Q4_K_M-GGUF/resolve/main/smollm2-360m-instruct-q4_k_m.gguf"
                            .to_string(),
                    ),
                },
                EmbeddedModelArtifact {
                    file_name: "tokenizer.json".to_string(),
                    size_bytes: 0,
                    sha256: None,
                    source_url: Some(
                        "https://huggingface.co/HuggingFaceTB/SmolLM2-360M-Instruct/resolve/main/tokenizer.json"
                            .to_string(),
                    ),
                },
            ],
        }),
        "tinyllama-1.1b-chat-q4" => Some(EmbeddedModelManifest {
            model_id: "tinyllama-1.1b-chat-q4".to_string(),
            display_name: "TinyLlama 1.1B Chat (Q4_K_M)".to_string(),
            chat_template: ChatTemplate::TinyLlama,
            artifacts: vec![
                EmbeddedModelArtifact {
                    file_name: "tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf".to_string(),
                    size_bytes: 0,
                    sha256: None,
                    source_url: Some(
                        "https://huggingface.co/TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF/resolve/main/tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf"
                            .to_string(),
                    ),
                },
                EmbeddedModelArtifact {
                    file_name: "tokenizer.json".to_string(),
                    size_bytes: 0,
                    sha256: None,
                    source_url: Some(
                        "https://huggingface.co/TinyLlama/TinyLlama-1.1B-Chat-v1.0/resolve/main/tokenizer.json"
                            .to_string(),
                    ),
                },
            ],
        }),
        _ => None,
    }
}

/// Scans `model_dir` for `.gguf` files and builds a minimal manifest from them.
///
/// # Errors
///
/// Returns an error if no GGUF file is found in the directory.
pub fn discover_manifest_in_dir(model_id: &str, model_dir: &Path) -> Result<EmbeddedModelManifest> {
    let mut artifacts = Vec::new();
    if model_dir.exists() {
        if let Ok(entries) = fs::read_dir(model_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("gguf") {
                    let size_bytes = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                    let file_name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or_default()
                        .to_string();
                    artifacts.push(EmbeddedModelArtifact {
                        file_name,
                        size_bytes,
                        sha256: None,
                        source_url: None,
                    });
                }
            }
        }
    }

    if artifacts.is_empty() {
        bail!(
            "No GGUF model file found in '{}' for model '{}'",
            model_dir.display(),
            model_id
        );
    }

    Ok(EmbeddedModelManifest {
        model_id: model_id.to_string(),
        display_name: model_id.to_string(),
        chat_template: ChatTemplate::default(),
        artifacts,
    })
}
