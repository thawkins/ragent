//! Internal embedded-LLM service wrapper.
//!
//! This module provides a narrow service interface for internal helper tasks.
//! It owns task selection, per-task limits, prompt shaping, timeout handling,
//! and output validation so future call sites do not interact with the raw
//! embedded runtime directly.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};
use tracing::{Instrument, debug, info, info_span, warn};

use crate::config::InternalLlmConfig;
use crate::embedded::{
    EmbeddedInferenceError, EmbeddedRuntime, EmbeddedRuntimeStatus, InferenceControls,
    RuntimeAvailability,
};

const CHARS_PER_TOKEN_BUDGET: usize = 4;

/// Internal helper tasks that may use the embedded LLM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InternalLlmTaskKind {
    /// Generate a short title for a session.
    SessionTitle,
    /// Summarise verbose tool output into a compact human-readable form.
    SummarizeToolOutput,
    /// Compact prompt/context material before it is forwarded elsewhere.
    PromptCompaction,
    /// Return a JSON array of candidate memory snippets worth keeping.
    MemoryPrefilter,
    /// Run a lightweight internal-only chat turn for diagnostics.
    Chat,
}

impl InternalLlmTaskKind {
    /// Returns the stable config key for this task kind.
    #[must_use]
    pub fn as_config_key(self) -> &'static str {
        match self {
            Self::SessionTitle => "session_title",
            Self::SummarizeToolOutput => "summarize_tool_output",
            Self::PromptCompaction => "prompt_compaction",
            Self::MemoryPrefilter => "memory_prefilter",
            Self::Chat => "chat",
        }
    }

    fn system_prompt(self) -> &'static str {
        match self {
            Self::SessionTitle => {
                "Return only a concise session title. One line, plain text, max 8 words."
            }
            Self::SummarizeToolOutput => {
                "Summarize the tool output for internal use. Plain text only."
            }
            Self::PromptCompaction => {
                "Compact the prompt/context while preserving constraints, facts, and requested outcomes. Plain text only."
            }
            Self::MemoryPrefilter => "Return only a JSON array of short memory strings. No prose.",
            Self::Chat => "Respond as a concise internal helper. Plain text only.",
        }
    }

    fn profile(self) -> InternalTaskProfile {
        match self {
            Self::SessionTitle => InternalTaskProfile {
                max_prompt_chars: 12_000,
                max_output_chars: 96,
                max_output_tokens: 16,
            },
            Self::SummarizeToolOutput => InternalTaskProfile {
                max_prompt_chars: 24_000,
                max_output_chars: 2_000,
                max_output_tokens: 160,
            },
            Self::PromptCompaction => InternalTaskProfile {
                max_prompt_chars: 32_000,
                max_output_chars: 4_000,
                max_output_tokens: 224,
            },
            Self::MemoryPrefilter => InternalTaskProfile {
                max_prompt_chars: 16_000,
                max_output_chars: 2_000,
                max_output_tokens: 96,
            },
            Self::Chat => InternalTaskProfile {
                max_prompt_chars: 24_000,
                max_output_chars: 3_000,
                max_output_tokens: 192,
            },
        }
    }

    fn validate_output(
        self,
        output: &str,
        max_output_chars: usize,
    ) -> Result<(), InternalLlmError> {
        let actual_chars = output.chars().count();
        if actual_chars > max_output_chars {
            return Err(InternalLlmError::OutputTooLarge {
                task: self.as_config_key(),
                max_chars: max_output_chars,
                actual_chars,
            });
        }

        match self {
            Self::SessionTitle => validate_session_title(output),
            Self::SummarizeToolOutput | Self::PromptCompaction | Self::Chat => {
                validate_plain_text_output(self, output)
            }
            Self::MemoryPrefilter => validate_memory_prefilter(output),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct InternalTaskProfile {
    max_prompt_chars: usize,
    max_output_chars: usize,
    max_output_tokens: u32,
}

#[derive(Debug, Clone, Copy, Default)]
struct EffectiveTaskLimits {
    max_prompt_chars: usize,
    max_output_chars: usize,
    max_output_tokens: u32,
}

impl EffectiveTaskLimits {
    fn resolve(
        task: InternalLlmTaskKind,
        config: &InternalLlmConfig,
        overrides: InternalTaskLimits,
    ) -> Self {
        let profile = task.profile();
        let config_prompt_chars = config.context_window.saturating_mul(CHARS_PER_TOKEN_BUDGET);
        let max_prompt_chars = overrides
            .max_prompt_chars
            .unwrap_or(profile.max_prompt_chars)
            .min(profile.max_prompt_chars)
            .min(config_prompt_chars.max(1));
        let max_output_chars = overrides
            .max_output_chars
            .unwrap_or(profile.max_output_chars)
            .min(profile.max_output_chars);
        let max_output_tokens = overrides
            .max_output_tokens
            .unwrap_or(profile.max_output_tokens)
            .min(profile.max_output_tokens)
            .min(config.max_output_tokens);

        Self {
            max_prompt_chars,
            max_output_chars,
            max_output_tokens,
        }
    }
}

/// Optional caller-provided task-limit overrides.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InternalTaskLimits {
    /// Optional prompt-size cap in characters.
    pub max_prompt_chars: Option<usize>,
    /// Optional output-size cap in characters.
    pub max_output_chars: Option<usize>,
    /// Optional output-token cap.
    pub max_output_tokens: Option<u32>,
}

/// Request sent to the executor implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InternalLlmExecutionRequest {
    /// The internal task being executed.
    pub task_kind: InternalLlmTaskKind,
    /// Configured embedded model identifier.
    pub model_id: String,
    /// Strict system prompt generated by the service.
    pub system_prompt: String,
    /// User/content prompt for the internal task.
    pub prompt: String,
    /// Effective output-token budget.
    pub max_output_tokens: u32,
    /// End-to-end deadline budget for queue wait and execution.
    pub timeout_ms: u64,
}

/// Successful internal-LLM response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InternalLlmResult {
    /// The task kind that produced this result.
    pub task_kind: InternalLlmTaskKind,
    /// Configured embedded model identifier.
    pub model_id: String,
    /// Validated output payload.
    pub output: String,
    /// End-to-end request duration in milliseconds.
    pub duration_ms: u128,
    /// Effective limits applied to the task.
    pub effective_limits: InternalTaskLimits,
}

/// Errors surfaced by the internal-LLM service.
#[derive(Debug, Error)]
pub enum InternalLlmError {
    /// The subsystem is disabled in config.
    #[error("internal LLM is disabled")]
    Disabled,
    /// The requested task is not allowlisted in config.
    #[error("internal LLM task '{task}' is not allowlisted")]
    TaskNotAllowed {
        /// Config key for the rejected task.
        task: &'static str,
    },
    /// The prompt was empty after trimming.
    #[error("internal LLM prompt for task '{task}' is empty")]
    EmptyPrompt {
        /// Config key for the rejected task.
        task: &'static str,
    },
    /// The prompt exceeds the resolved per-task limit.
    #[error("internal LLM prompt for task '{task}' exceeds {max_chars} chars ({actual_chars})")]
    PromptTooLarge {
        /// Config key for the rejected task.
        task: &'static str,
        /// Allowed character count.
        max_chars: usize,
        /// Actual character count.
        actual_chars: usize,
    },
    /// The executor returned no meaningful output.
    #[error("internal LLM output for task '{task}' is empty")]
    EmptyOutput {
        /// Config key for the rejected task.
        task: &'static str,
    },
    /// The output exceeds the resolved per-task limit.
    #[error("internal LLM output for task '{task}' exceeds {max_chars} chars ({actual_chars})")]
    OutputTooLarge {
        /// Config key for the rejected task.
        task: &'static str,
        /// Allowed character count.
        max_chars: usize,
        /// Actual character count.
        actual_chars: usize,
    },
    /// The model returned content that failed task-specific validation.
    #[error("internal LLM output for task '{task}' failed validation: {reason}")]
    InvalidOutput {
        /// Config key for the rejected task.
        task: &'static str,
        /// Human-readable explanation of the invalid shape.
        reason: String,
    },
    /// The underlying runtime or backend is unavailable.
    #[error("internal LLM backend unavailable: {message}")]
    Unavailable {
        /// Human-readable availability reason.
        message: String,
    },
    /// The request timed out.
    #[error("internal LLM request timed out after {timeout_ms}ms")]
    Timeout {
        /// Timeout budget in milliseconds.
        timeout_ms: u64,
    },
    /// Generic execution failure from the underlying executor.
    #[error("internal LLM execution failed: {message}")]
    Execution {
        /// Human-readable execution error.
        message: String,
    },
}

/// Executor abstraction used by [`InternalLlmService`].
#[async_trait]
pub trait InternalLlmExecutor: Send + Sync {
    /// Execute an internal-LLM request.
    ///
    /// # Errors
    ///
    /// Returns an [`InternalLlmError`] when the runtime is unavailable or the
    /// request fails before a validated output is produced.
    async fn execute(
        &self,
        request: InternalLlmExecutionRequest,
    ) -> Result<String, InternalLlmError>;

    /// Returns backend/runtime status when available.
    fn status(&self) -> Option<EmbeddedRuntimeStatus> {
        None
    }

    /// Returns current queue/worker state when available.
    fn queue_status(&self) -> Option<InternalLlmQueueStatus> {
        None
    }
}

/// Snapshot of internal-LLM counters.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InternalLlmMetricsSnapshot {
    /// Number of internal tasks that reached executor dispatch.
    pub attempts: u64,
    /// Number of tasks that completed successfully.
    pub successes: u64,
    /// Number of tasks that failed after dispatch.
    pub failures: u64,
    /// Number of tasks that timed out.
    pub timeouts: u64,
    /// Number of times callers explicitly fell back to another path.
    pub fallbacks: u64,
    /// Most recent task failure, if any.
    pub last_error: Option<String>,
    /// Most recent fallback reason, if any.
    pub last_fallback: Option<String>,
}

/// Snapshot of internal-LLM health for status surfaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InternalLlmStatusSnapshot {
    /// Configured embedded model identifier.
    pub model_id: String,
    /// Configured backend identifier.
    pub backend: String,
    /// Whether the subsystem is enabled in config.
    pub enabled: bool,
    /// Metrics collected by the service.
    pub metrics: InternalLlmMetricsSnapshot,
    /// Embedded runtime/backend status when available.
    pub runtime: Option<EmbeddedRuntimeStatus>,
    /// Queue/worker state when available.
    pub queue: Option<InternalLlmQueueStatus>,
}

/// Snapshot of the dedicated internal-LLM worker queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InternalLlmQueueStatus {
    /// Total active + queued requests that may be admitted.
    pub capacity: usize,
    /// Requests currently admitted to the worker, including the active decode.
    pub in_flight: usize,
    /// Requests waiting behind the active decode.
    pub queued: usize,
    /// Whether the single decode worker is actively processing a request.
    pub worker_busy: bool,
}

#[derive(Debug, Default)]
struct InternalLlmMetrics {
    attempts: AtomicU64,
    successes: AtomicU64,
    failures: AtomicU64,
    timeouts: AtomicU64,
    fallbacks: AtomicU64,
    last_error: Mutex<Option<String>>,
    last_fallback: Mutex<Option<String>>,
}

impl InternalLlmMetrics {
    fn snapshot(&self) -> InternalLlmMetricsSnapshot {
        InternalLlmMetricsSnapshot {
            attempts: self.attempts.load(Ordering::Relaxed),
            successes: self.successes.load(Ordering::Relaxed),
            failures: self.failures.load(Ordering::Relaxed),
            timeouts: self.timeouts.load(Ordering::Relaxed),
            fallbacks: self.fallbacks.load(Ordering::Relaxed),
            last_error: self
                .last_error
                .lock()
                .map(|value| value.clone())
                .unwrap_or_else(|error| Some(format!("metrics mutex poisoned: {error}"))),
            last_fallback: self
                .last_fallback
                .lock()
                .map(|value| value.clone())
                .unwrap_or_else(|error| Some(format!("metrics mutex poisoned: {error}"))),
        }
    }

    fn record_success(&self) {
        self.successes.fetch_add(1, Ordering::Relaxed);
    }

    fn record_failure(&self, error: &InternalLlmError) {
        self.failures.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut last_error) = self.last_error.lock() {
            *last_error = Some(error.to_string());
        }
    }

    fn record_timeout(&self, error: &InternalLlmError) {
        self.timeouts.fetch_add(1, Ordering::Relaxed);
        self.record_failure(error);
    }

    fn record_fallback(&self, reason: &str) {
        self.fallbacks.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut last_fallback) = self.last_fallback.lock() {
            *last_fallback = Some(reason.to_string());
        }
    }
}

/// Internal service wrapper around the embedded runtime.
pub struct InternalLlmService {
    config: InternalLlmConfig,
    executor: Arc<dyn InternalLlmExecutor>,
    metrics: Arc<InternalLlmMetrics>,
}

impl InternalLlmService {
    /// Returns the effective internal-LLM configuration.
    #[must_use]
    pub fn config(&self) -> &InternalLlmConfig {
        &self.config
    }

    /// Returns a snapshot of current health and counters.
    #[must_use]
    pub fn status_snapshot(&self) -> InternalLlmStatusSnapshot {
        InternalLlmStatusSnapshot {
            model_id: self.config.model_id.clone(),
            backend: self.config.backend.clone(),
            enabled: self.config.enabled,
            metrics: self.metrics.snapshot(),
            runtime: self.executor.status(),
            queue: self.executor.queue_status(),
        }
    }

    /// Records an explicit fallback to a non-embedded path.
    pub fn record_fallback(&self, task_kind: InternalLlmTaskKind, reason: impl Into<String>) {
        let reason = reason.into();
        let _span = info_span!(
            "internal_llm.fallback",
            task = task_kind.as_config_key(),
            model = %self.config.model_id
        )
        .entered();
        self.metrics.record_fallback(&reason);
        warn!(
            task = task_kind.as_config_key(),
            model = %self.config.model_id,
            reason = %reason,
            "Internal LLM fallback used"
        );
    }

    /// Build a service from config when the subsystem is enabled.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded runtime cannot be initialised.
    pub fn from_config(config: InternalLlmConfig) -> anyhow::Result<Option<Self>> {
        if !config.enabled {
            return Ok(None);
        }

        let runtime = EmbeddedRuntime::from_config((&config).into())?.ok_or_else(|| {
            anyhow::anyhow!("internal LLM config was enabled but no runtime was created")
        })?;
        let executor = Arc::new(EmbeddedRuntimeExecutor::new(
            Arc::new(runtime),
            config.max_parallel_requests,
        ));
        Ok(Some(Self::with_executor(config, executor)))
    }

    /// Build a service around a caller-provided executor.
    #[must_use]
    pub fn with_executor(
        config: InternalLlmConfig,
        executor: Arc<dyn InternalLlmExecutor>,
    ) -> Self {
        Self {
            config,
            executor,
            metrics: Arc::new(InternalLlmMetrics::default()),
        }
    }

    /// Execute one approved internal task with validated output.
    ///
    /// # Errors
    ///
    /// Returns an [`InternalLlmError`] when the task is disabled, invalid, or
    /// the underlying executor fails to produce a valid response.
    pub async fn run_internal_task(
        &self,
        task_kind: InternalLlmTaskKind,
        prompt: &str,
        limits: InternalTaskLimits,
    ) -> Result<InternalLlmResult, InternalLlmError> {
        if !self.config.enabled {
            return Err(InternalLlmError::Disabled);
        }
        if !self.config.allows_task(task_kind.as_config_key()) {
            return Err(InternalLlmError::TaskNotAllowed {
                task: task_kind.as_config_key(),
            });
        }

        let trimmed_prompt = prompt.trim();
        if trimmed_prompt.is_empty() {
            return Err(InternalLlmError::EmptyPrompt {
                task: task_kind.as_config_key(),
            });
        }

        let effective = EffectiveTaskLimits::resolve(task_kind, &self.config, limits);
        let prompt_chars = trimmed_prompt.chars().count();
        if prompt_chars > effective.max_prompt_chars {
            return Err(InternalLlmError::PromptTooLarge {
                task: task_kind.as_config_key(),
                max_chars: effective.max_prompt_chars,
                actual_chars: prompt_chars,
            });
        }

        let request = InternalLlmExecutionRequest {
            task_kind,
            model_id: self.config.model_id.clone(),
            system_prompt: task_kind.system_prompt().to_string(),
            prompt: trimmed_prompt.to_string(),
            max_output_tokens: effective.max_output_tokens,
            timeout_ms: self.config.timeout_ms,
        };
        self.metrics.attempts.fetch_add(1, Ordering::Relaxed);

        let started = Instant::now();
        let inference_span = info_span!(
            "internal_llm.inference",
            task = task_kind.as_config_key(),
            model = %self.config.model_id
        );
        let output = match self
            .executor
            .execute(request.clone())
            .instrument(inference_span)
            .await
        {
            Ok(output) => output,
            Err(err @ InternalLlmError::Timeout { .. }) => {
                let _timeout_span = info_span!(
                    "internal_llm.timeout",
                    task = task_kind.as_config_key(),
                    model = %self.config.model_id
                )
                .entered();
                self.metrics.record_timeout(&err);
                warn!(
                    task = task_kind.as_config_key(),
                    model = %self.config.model_id,
                    elapsed_ms = started.elapsed().as_millis(),
                    error = %err,
                    "Internal LLM request timed out"
                );
                return Err(err);
            }
            Err(err) => {
                self.metrics.record_failure(&err);
                warn!(
                    task = task_kind.as_config_key(),
                    model = %self.config.model_id,
                    elapsed_ms = started.elapsed().as_millis(),
                    error = %err,
                    "Internal LLM request failed"
                );
                return Err(err);
            }
        };

        let normalized = output.trim();
        if normalized.is_empty() {
            let err = InternalLlmError::EmptyOutput {
                task: task_kind.as_config_key(),
            };
            self.metrics.record_failure(&err);
            warn!(
                task = task_kind.as_config_key(),
                model = %self.config.model_id,
                elapsed_ms = started.elapsed().as_millis(),
                error = %err,
                "Internal LLM returned invalid output"
            );
            return Err(err);
        }

        if let Err(err) = task_kind.validate_output(normalized, effective.max_output_chars) {
            self.metrics.record_failure(&err);
            warn!(
                task = task_kind.as_config_key(),
                model = %self.config.model_id,
                elapsed_ms = started.elapsed().as_millis(),
                error = %err,
                "Internal LLM returned invalid output"
            );
            return Err(err);
        }

        let duration_ms = started.elapsed().as_millis();
        self.metrics.record_success();
        debug!(
            task = task_kind.as_config_key(),
            model = %self.config.model_id,
            duration_ms,
            output_chars = normalized.chars().count(),
            "Internal LLM task completed"
        );

        Ok(InternalLlmResult {
            task_kind,
            model_id: self.config.model_id.clone(),
            output: normalized.to_string(),
            duration_ms,
            effective_limits: InternalTaskLimits {
                max_prompt_chars: Some(effective.max_prompt_chars),
                max_output_chars: Some(effective.max_output_chars),
                max_output_tokens: Some(effective.max_output_tokens),
            },
        })
    }
}

#[derive(Clone)]
struct EmbeddedRuntimeExecutor {
    worker: EmbeddedRuntimeWorker,
    worker_tx: mpsc::Sender<EmbeddedRuntimeWorkerRequest>,
    queue_capacity: usize,
}

#[derive(Debug, Default)]
struct EmbeddedRuntimeInitState {
    consecutive_failures: u32,
    next_retry_at: Option<Instant>,
    last_error: Option<String>,
}

#[derive(Clone)]
struct EmbeddedRuntimeWorker {
    runtime: Arc<EmbeddedRuntime>,
    /// Tracks retryable init state for lazy backend preparation.
    init_state: Arc<Mutex<EmbeddedRuntimeInitState>>,
    queue_state: Arc<EmbeddedRuntimeQueueState>,
}

#[derive(Debug, Default)]
struct EmbeddedRuntimeQueueState {
    in_flight: AtomicUsize,
    worker_busy: AtomicBool,
}

struct EmbeddedRuntimeWorkerRequest {
    request: InternalLlmExecutionRequest,
    deadline: Instant,
    cancel_flag: Arc<AtomicBool>,
    response_tx: oneshot::Sender<Result<String, InternalLlmError>>,
}

struct WorkerCancellationGuard {
    cancel_flag: Arc<AtomicBool>,
}

impl Drop for WorkerCancellationGuard {
    fn drop(&mut self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
    }
}

impl EmbeddedRuntimeExecutor {
    fn new(runtime: Arc<EmbeddedRuntime>, queue_capacity: usize) -> Self {
        let queue_capacity = queue_capacity.max(1);
        let worker = EmbeddedRuntimeWorker {
            runtime,
            init_state: Arc::new(Mutex::new(EmbeddedRuntimeInitState::default())),
            queue_state: Arc::new(EmbeddedRuntimeQueueState::default()),
        };
        let (worker_tx, worker_rx) = mpsc::channel(queue_capacity);
        let worker_thread = worker.clone();
        thread::spawn(move || worker_thread.run(worker_rx));
        Self {
            worker,
            worker_tx,
            queue_capacity,
        }
    }
}

impl EmbeddedRuntimeWorker {
    /// Performs lazy backend initialisation from the local model cache.
    ///
    /// Returns `Ok(true)` when this call performed a cold-start init.
    fn try_init_runtime(
        &self,
        deadline: Instant,
        timeout_ms: u64,
    ) -> Result<bool, InternalLlmError> {
        ensure_deadline(deadline, timeout_ms)?;

        // Fast path: already ready.
        if self.runtime.is_initialized() {
            return Ok(false);
        }

        let mut state = self.init_state.lock().unwrap_or_else(|e| e.into_inner());
        if self.runtime.is_initialized() {
            return Ok(false);
        }

        if let Some(next_retry_at) = state.next_retry_at {
            let now = Instant::now();
            if now < next_retry_at {
                let retry_in_ms = next_retry_at.duration_since(now).as_millis();
                let last_error = state
                    .last_error
                    .clone()
                    .unwrap_or_else(|| "embedded runtime init previously failed".to_string());
                return Err(InternalLlmError::Unavailable {
                    message: format!(
                        "embedded runtime retry backoff active for {retry_in_ms}ms after previous init failure: {last_error}"
                    ),
                });
            }
        }

        let started = Instant::now();
        match self.runtime.try_init_from_cache() {
            Ok(()) => {
                ensure_deadline(deadline, timeout_ms)?;
                let elapsed_ms = started.elapsed().as_millis();
                *state = EmbeddedRuntimeInitState::default();
                info!(
                    model = %self.runtime.model_id(),
                    elapsed_ms,
                    "Embedded runtime initialized"
                );
                Ok(true)
            }
            Err(error) => {
                state.consecutive_failures = state.consecutive_failures.saturating_add(1);
                let retry_delay = retry_backoff_delay(state.consecutive_failures);
                state.next_retry_at = if retry_delay.is_zero() {
                    None
                } else {
                    Some(Instant::now() + retry_delay)
                };
                state.last_error = Some(error.to_string());
                warn!(
                    model = %self.runtime.model_id(),
                    attempt = state.consecutive_failures,
                    retry_in_ms = retry_delay.as_millis(),
                    elapsed_ms = started.elapsed().as_millis(),
                    error = %error,
                    "Embedded runtime init failed"
                );
                Err(InternalLlmError::Unavailable {
                    message: format!("embedded runtime init failed: {error:#}"),
                })
            }
        }
    }

    fn run(&self, mut worker_rx: mpsc::Receiver<EmbeddedRuntimeWorkerRequest>) {
        while let Some(job) = worker_rx.blocking_recv() {
            self.queue_state.worker_busy.store(true, Ordering::Release);
            let result = self.execute_job(&job);
            self.queue_state.worker_busy.store(false, Ordering::Release);
            self.queue_state.in_flight.fetch_sub(1, Ordering::AcqRel);
            let _ = job.response_tx.send(result);
        }
    }

    fn execute_job(&self, job: &EmbeddedRuntimeWorkerRequest) -> Result<String, InternalLlmError> {
        if job.cancel_flag.load(Ordering::Relaxed) {
            return Err(InternalLlmError::Execution {
                message: "internal LLM request was cancelled before execution".to_string(),
            });
        }

        ensure_deadline(job.deadline, job.request.timeout_ms)?;
        let cold_start = self.try_init_runtime(job.deadline, job.request.timeout_ms)?;
        ensure_deadline(job.deadline, job.request.timeout_ms)?;

        let started = Instant::now();
        let controls = InferenceControls::with_deadline(job.deadline, Arc::clone(&job.cancel_flag));
        let result = self
            .runtime
            .infer(
                &job.request.system_prompt,
                &job.request.prompt,
                job.request.max_output_tokens,
                &controls,
            )
            .map_err(|error| map_inference_error(error, job.request.timeout_ms));
        if cold_start {
            match &result {
                Ok(_) => info!(
                    task = job.request.task_kind.as_config_key(),
                    model = %self.runtime.model_id(),
                    elapsed_ms = started.elapsed().as_millis(),
                    "First request after cold start completed"
                ),
                Err(error) => warn!(
                    task = job.request.task_kind.as_config_key(),
                    model = %self.runtime.model_id(),
                    elapsed_ms = started.elapsed().as_millis(),
                    error = %error,
                    "First request after cold start failed"
                ),
            }
        }
        result
    }

    fn try_acquire_slot(&self, queue_capacity: usize) -> Result<(), InternalLlmError> {
        self.queue_state
            .in_flight
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                (current < queue_capacity).then_some(current + 1)
            })
            .map(|_| ())
            .map_err(|current| InternalLlmError::Unavailable {
                message: format!(
                    "internal LLM worker overloaded: {current}/{queue_capacity} requests already active or queued"
                ),
            })
    }

    fn release_slot(&self) {
        self.queue_state.in_flight.fetch_sub(1, Ordering::AcqRel);
    }

    fn queue_status(&self, queue_capacity: usize) -> InternalLlmQueueStatus {
        let in_flight = self.queue_state.in_flight.load(Ordering::Acquire);
        let worker_busy = self.queue_state.worker_busy.load(Ordering::Acquire);
        let queued = in_flight.saturating_sub(usize::from(worker_busy && in_flight > 0));
        InternalLlmQueueStatus {
            capacity: queue_capacity,
            in_flight,
            queued,
            worker_busy,
        }
    }
}

fn retry_backoff_delay(failures: u32) -> Duration {
    if failures <= 1 {
        return Duration::from_secs(0);
    }

    let exponent = failures.saturating_sub(2).min(5);
    Duration::from_secs(1_u64 << exponent)
}

#[async_trait]
impl InternalLlmExecutor for EmbeddedRuntimeExecutor {
    async fn execute(
        &self,
        request: InternalLlmExecutionRequest,
    ) -> Result<String, InternalLlmError> {
        if self.worker.runtime.availability() == RuntimeAvailability::RequiresFeature {
            return Err(InternalLlmError::Unavailable {
                message: format!(
                    "ragent-llm was built without the embedded-llm feature (model '{}', task '{}')",
                    self.worker.runtime.model_id(),
                    request.task_kind.as_config_key()
                ),
            });
        }

        self.worker.try_acquire_slot(self.queue_capacity)?;

        let deadline = Instant::now() + Duration::from_millis(request.timeout_ms);
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let _cancellation_guard = WorkerCancellationGuard {
            cancel_flag: Arc::clone(&cancel_flag),
        };
        let (response_tx, response_rx) = oneshot::channel();

        if self
            .worker_tx
            .try_send(EmbeddedRuntimeWorkerRequest {
                request,
                deadline,
                cancel_flag,
                response_tx,
            })
            .is_err()
        {
            self.worker.release_slot();
            return Err(InternalLlmError::Unavailable {
                message: format!(
                    "internal LLM worker overloaded: queue depth exceeded configured capacity {}",
                    self.queue_capacity
                ),
            });
        }

        match response_rx.await {
            Ok(result) => result,
            Err(_) => {
                self.worker.release_slot();
                Err(InternalLlmError::Unavailable {
                    message: "internal LLM worker stopped before delivering a response".to_string(),
                })
            }
        }
    }

    fn status(&self) -> Option<EmbeddedRuntimeStatus> {
        Some(self.worker.runtime.status())
    }

    fn queue_status(&self) -> Option<InternalLlmQueueStatus> {
        Some(self.worker.queue_status(self.queue_capacity))
    }
}

fn ensure_deadline(deadline: Instant, timeout_ms: u64) -> Result<(), InternalLlmError> {
    if Instant::now() >= deadline {
        return Err(InternalLlmError::Timeout { timeout_ms });
    }
    Ok(())
}

fn map_inference_error(error: EmbeddedInferenceError, timeout_ms: u64) -> InternalLlmError {
    match error {
        EmbeddedInferenceError::DeadlineExceeded | EmbeddedInferenceError::Cancelled => {
            InternalLlmError::Timeout { timeout_ms }
        }
        EmbeddedInferenceError::Other(error) => InternalLlmError::Unavailable {
            message: format!("embedded runtime inference failed: {error:#}"),
        },
    }
}

fn validate_session_title(output: &str) -> Result<(), InternalLlmError> {
    if output.contains('\n') {
        return Err(InternalLlmError::InvalidOutput {
            task: InternalLlmTaskKind::SessionTitle.as_config_key(),
            reason: "title must be a single line".to_string(),
        });
    }
    if output.starts_with("```") || output.starts_with('#') || output.starts_with("- ") {
        return Err(InternalLlmError::InvalidOutput {
            task: InternalLlmTaskKind::SessionTitle.as_config_key(),
            reason: "title must be plain text without markdown or list markers".to_string(),
        });
    }
    Ok(())
}

fn validate_plain_text_output(
    task_kind: InternalLlmTaskKind,
    output: &str,
) -> Result<(), InternalLlmError> {
    if output.starts_with("```") {
        return Err(InternalLlmError::InvalidOutput {
            task: task_kind.as_config_key(),
            reason: "output must be plain text without markdown fences".to_string(),
        });
    }
    if !output.chars().any(char::is_alphanumeric) {
        return Err(InternalLlmError::InvalidOutput {
            task: task_kind.as_config_key(),
            reason: "output must contain meaningful text".to_string(),
        });
    }
    Ok(())
}

fn validate_memory_prefilter(output: &str) -> Result<(), InternalLlmError> {
    let values: Vec<String> =
        serde_json::from_str(output).map_err(|err| InternalLlmError::InvalidOutput {
            task: InternalLlmTaskKind::MemoryPrefilter.as_config_key(),
            reason: format!("expected JSON array of strings: {err}"),
        })?;

    if values.is_empty() {
        return Err(InternalLlmError::InvalidOutput {
            task: InternalLlmTaskKind::MemoryPrefilter.as_config_key(),
            reason: "JSON array must not be empty".to_string(),
        });
    }
    if values.len() > 20 {
        return Err(InternalLlmError::InvalidOutput {
            task: InternalLlmTaskKind::MemoryPrefilter.as_config_key(),
            reason: "JSON array must contain at most 20 strings".to_string(),
        });
    }
    if values
        .iter()
        .any(|value| value.trim().is_empty() || value.chars().count() > 200)
    {
        return Err(InternalLlmError::InvalidOutput {
            task: InternalLlmTaskKind::MemoryPrefilter.as_config_key(),
            reason: "each JSON string must be non-empty and at most 200 chars".to_string(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use tokio::sync::mpsc;

    use super::{
        EmbeddedRuntimeExecutor, EmbeddedRuntimeInitState, EmbeddedRuntimeQueueState,
        EmbeddedRuntimeWorker, InternalLlmExecutionRequest, InternalLlmExecutor,
        InternalLlmTaskKind, retry_backoff_delay,
    };
    use crate::embedded::EmbeddedRuntime;

    #[test]
    fn test_retry_backoff_delay_allows_immediate_first_retry() {
        assert_eq!(retry_backoff_delay(0).as_secs(), 0);
        assert_eq!(retry_backoff_delay(1).as_secs(), 0);
        assert_eq!(retry_backoff_delay(2).as_secs(), 1);
        assert_eq!(retry_backoff_delay(3).as_secs(), 2);
        assert_eq!(retry_backoff_delay(4).as_secs(), 4);
    }

    #[tokio::test]
    async fn test_executor_releases_slot_when_worker_stops_before_response() {
        let mut config = ragent_config::InternalLlmConfig::default();
        config.enabled = true;
        let runtime = Arc::new(EmbeddedRuntime::new(config).expect("runtime should build"));
        let worker = EmbeddedRuntimeWorker {
            runtime,
            init_state: Arc::new(Mutex::new(EmbeddedRuntimeInitState::default())),
            queue_state: Arc::new(EmbeddedRuntimeQueueState::default()),
        };
        let (worker_tx, mut worker_rx) = mpsc::channel(1);
        let executor = EmbeddedRuntimeExecutor {
            worker: worker.clone(),
            worker_tx,
            queue_capacity: 1,
        };

        tokio::spawn(async move {
            let _job = worker_rx.recv().await;
        });

        let error = executor
            .execute(InternalLlmExecutionRequest {
                task_kind: InternalLlmTaskKind::Chat,
                model_id: "test-model".to_string(),
                system_prompt: "system".to_string(),
                prompt: "prompt".to_string(),
                max_output_tokens: 8,
                timeout_ms: 50,
            })
            .await
            .expect_err("worker drop should surface an error");

        assert!(matches!(error, super::InternalLlmError::Unavailable { .. }));
        assert_eq!(executor.queue_status().expect("queue status").in_flight, 0);
    }
}
