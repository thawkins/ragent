//! Benchmark-facing model selection helpers.

use anyhow::{Context, Result, anyhow, bail};
use futures::StreamExt;
use ragent_core::Config;
use ragent_core::agent::ModelRef;
use ragent_core::agent::fallback_thinking_for_model_ref;
use ragent_core::llm::{ChatContent, ChatMessage, ChatRequest, LlmFinishReason, StreamEvent};
use ragent_core::provider::ProviderRegistry;
use ragent_core::storage::Storage;
use ragent_types::ThinkingConfig;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::command::BenchRunOptions;

const BENCH_TRANSIENT_MAX_RETRIES: u32 = 7;
const BENCH_TRANSIENT_MAX_BACKOFF_SECS: u64 = 30;
const BENCH_TRANSIENT_MAX_ELAPSED_SECS: u64 = 150;

/// Resolved provider/model selection for a benchmark run.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedModelSelection {
    /// Provider ID.
    pub provider_id: String,
    /// Provider display name.
    pub provider_name: String,
    /// Model ID.
    pub model_id: String,
    /// Optional model display name.
    pub model_display_name: Option<String>,
    /// Filesystem-safe provider slug.
    pub provider_slug: String,
    /// Filesystem-safe model slug.
    pub model_slug: String,
    /// Effective model context window, if known.
    pub context_window: Option<usize>,
    /// Effective max output tokens, if known.
    pub max_output_tokens: Option<u32>,
    /// Optional Copilot request multiplier or provider-specific multiplier.
    pub request_multiplier: Option<f64>,
    /// Effective thinking configuration for benchmark requests.
    pub thinking_config: ThinkingConfig,
    /// Effective API base URL for the provider, if configured.
    pub base_url: Option<String>,
}

/// One generated benchmark sample.
#[derive(Debug, Clone, PartialEq)]
pub struct BenchGeneratedSample {
    /// Generated text content.
    pub text: String,
    /// Prompt/input token count if reported.
    pub input_tokens: Option<u64>,
    /// Completion/output token count if reported.
    pub output_tokens: Option<u64>,
    /// Finish reason if reported.
    pub finish_reason: Option<String>,
    /// End-to-end sample generation time in milliseconds.
    pub duration_ms: u64,
}

/// Model runner output for a single benchmark case.
#[derive(Debug, Clone, PartialEq)]
pub struct BenchGenerationResult {
    /// Generated samples.
    pub samples: Vec<BenchGeneratedSample>,
}

/// Synchronous benchmark model runner interface.
pub trait BenchModelRunner: Send + Sync {
    /// Return the resolved model selection backing this runner.
    fn selection(&self) -> &ResolvedModelSelection;

    /// Generate benchmark completion samples for one prompt.
    ///
    /// # Errors
    ///
    /// Returns an error when model generation fails or the run is cancelled.
    fn generate(
        &self,
        prompt: &str,
        options: &BenchRunOptions,
        cancel: &AtomicBool,
    ) -> Result<BenchGenerationResult>;
}

/// Live benchmark model runner backed by the configured provider registry.
pub struct LiveBenchModelRunner {
    selection: ResolvedModelSelection,
    provider_registry: Arc<ProviderRegistry>,
    api_key: String,
}

/// Mock benchmark model runner used by tests.
#[derive(Debug, Clone)]
pub struct MockBenchModelRunner {
    selection: ResolvedModelSelection,
    outputs: Vec<String>,
}

impl MockBenchModelRunner {
    /// Build a mock runner using the given selection and sample outputs.
    #[must_use]
    pub fn new(selection: ResolvedModelSelection, outputs: Vec<String>) -> Self {
        Self { selection, outputs }
    }
}

impl BenchModelRunner for MockBenchModelRunner {
    fn selection(&self) -> &ResolvedModelSelection {
        &self.selection
    }

    fn generate(
        &self,
        _prompt: &str,
        options: &BenchRunOptions,
        cancel: &AtomicBool,
    ) -> Result<BenchGenerationResult> {
        if cancel.load(Ordering::Relaxed) {
            bail!("benchmark run cancelled");
        }
        let sample_count = options.samples.max(1);
        let samples = (0..sample_count)
            .map(|idx| BenchGeneratedSample {
                text: self
                    .outputs
                    .get(idx)
                    .cloned()
                    .or_else(|| self.outputs.first().cloned())
                    .unwrap_or_default(),
                input_tokens: Some(16),
                output_tokens: Some(8),
                finish_reason: Some("stop".to_string()),
                duration_ms: 1,
            })
            .collect();
        Ok(BenchGenerationResult { samples })
    }
}

impl LiveBenchModelRunner {
    /// Create a live benchmark runner for the resolved model selection.
    ///
    /// # Errors
    ///
    /// Returns an error when the provider is unknown or stored auth cannot be read.
    pub fn new(
        selection: ResolvedModelSelection,
        provider_registry: Arc<ProviderRegistry>,
        storage: Arc<Storage>,
    ) -> Result<Self> {
        if provider_registry.get(&selection.provider_id).is_none() {
            bail!("unknown provider '{}'", selection.provider_id);
        }
        let api_key = storage
            .get_provider_auth(&selection.provider_id)
            .with_context(|| format!("read auth for provider '{}'", selection.provider_id))?
            .unwrap_or_default();
        Ok(Self {
            selection,
            provider_registry,
            api_key,
        })
    }
}

impl BenchModelRunner for LiveBenchModelRunner {
    fn selection(&self) -> &ResolvedModelSelection {
        &self.selection
    }

    fn generate(
        &self,
        prompt: &str,
        options: &BenchRunOptions,
        cancel: &AtomicBool,
    ) -> Result<BenchGenerationResult> {
        if cancel.load(Ordering::Relaxed) {
            bail!("benchmark run cancelled");
        }

        let provider = self
            .provider_registry
            .get(&self.selection.provider_id)
            .ok_or_else(|| anyhow!("unknown provider '{}'", self.selection.provider_id))?;
        let selection = self.selection.clone();
        let api_key = self.api_key.clone();
        let prompt = prompt.to_string();
        let options = options.clone();

        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(async move {
                let client = provider
                    .create_client(&api_key, selection.base_url.as_deref(), &HashMap::new())
                    .await
                    .with_context(|| {
                        format!(
                            "create benchmark LLM client for {}/{}",
                            selection.provider_id, selection.model_id
                        )
                    })?;

                let mut samples = Vec::with_capacity(options.samples.max(1));
                for _ in 0..options.samples.max(1) {
                    if cancel.load(Ordering::Relaxed) {
                        bail!("benchmark run cancelled");
                    }

                    let retry_started = Instant::now();
                    let mut sample_result = None;
                    'retry: for attempt in 0..=BENCH_TRANSIENT_MAX_RETRIES {
                        if cancel.load(Ordering::Relaxed) {
                            bail!("benchmark run cancelled");
                        }

                        let request = benchmark_chat_request(&selection, &prompt, &options);
                        let started = Instant::now();
                        let mut stream = match client.chat(request).await {
                            Ok(stream) => stream,
                            Err(error) => {
                                let message = format!(
                                    "start benchmark completion for {}/{}: {:#}",
                                    selection.provider_id, selection.model_id, error
                                );
                                if let Some(delay) =
                                    benchmark_retry_delay(&message, attempt, retry_started)
                                {
                                    tokio::time::sleep(delay).await;
                                    continue 'retry;
                                }
                                return Err(anyhow!(format_retry_exhausted_error(
                                    &message,
                                    attempt + 1,
                                    retry_started
                                )));
                            }
                        };

                        let mut text = String::new();
                        let mut input_tokens = None;
                        let mut output_tokens = None;
                        let mut finish_reason = None;
                        let mut provider_error = None;
                        while let Some(event) = stream.next().await {
                            match event {
                                StreamEvent::TextDelta { text: delta } => text.push_str(&delta),
                                StreamEvent::Usage {
                                    input_tokens: input,
                                    output_tokens: output,
                                } => {
                                    input_tokens = Some(input);
                                    output_tokens = Some(output);
                                }
                                StreamEvent::Finish { reason } => {
                                    finish_reason = Some(finish_reason_label(reason));
                                }
                                StreamEvent::Error { message } => {
                                    provider_error = Some(message);
                                }
                                StreamEvent::ReasoningStart
                                | StreamEvent::ReasoningDelta { .. }
                                | StreamEvent::ReasoningEnd
                                | StreamEvent::ToolCallStart { .. }
                                | StreamEvent::ToolCallDelta { .. }
                                | StreamEvent::ToolCallEnd { .. }
                                | StreamEvent::RateLimit { .. } => {}
                            }
                        }
                        if let Some(message) = provider_error {
                            let error_message = format!(
                                "benchmark generation failed for {}/{}: {}",
                                selection.provider_id, selection.model_id, message
                            );
                            if let Some(delay) =
                                benchmark_retry_delay(&error_message, attempt, retry_started)
                            {
                                tokio::time::sleep(delay).await;
                                continue 'retry;
                            }
                            return Err(anyhow!(format_retry_exhausted_error(
                                &error_message,
                                attempt + 1,
                                retry_started
                            )));
                        }

                        sample_result = Some(BenchGeneratedSample {
                            text,
                            input_tokens,
                            output_tokens,
                            finish_reason,
                            duration_ms: started.elapsed().as_millis() as u64,
                        });
                        break;
                    }

                    samples.push(sample_result.ok_or_else(|| {
                        anyhow!(
                            "benchmark generation failed for {}/{} after {} attempt(s)",
                            selection.provider_id,
                            selection.model_id,
                            BENCH_TRANSIENT_MAX_RETRIES + 1
                        )
                    })?);
                }

                Ok(BenchGenerationResult { samples })
            })
    }
}

/// Convert an arbitrary string into a filesystem-safe slug.
#[must_use]
pub fn slugify_path_segment(value: &str) -> String {
    let mut slug = String::with_capacity(value.len());
    let mut last_underscore = false;
    for ch in value.chars() {
        let mapped = if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-') {
            ch
        } else {
            '_'
        };
        if mapped == '_' {
            if !last_underscore {
                slug.push(mapped);
            }
            last_underscore = true;
        } else {
            slug.push(mapped);
            last_underscore = false;
        }
    }
    let slug = slug.trim_matches('_').to_string();
    if slug.is_empty() {
        "unknown".to_string()
    } else {
        slug
    }
}

/// Resolve the current `provider/model` selection string.
///
/// # Errors
///
/// Returns an error when the selection is empty or malformed.
pub fn resolve_selected_model(selection: &str) -> Result<ResolvedModelSelection> {
    let trimmed = selection.trim();
    if trimmed.is_empty() {
        bail!("no model is currently selected");
    }
    let (provider_id, model_id) = trimmed
        .split_once('/')
        .ok_or_else(|| anyhow!("selected model must look like 'provider/model'"))?;
    Ok(ResolvedModelSelection {
        provider_id: provider_id.to_string(),
        provider_name: provider_id.to_string(),
        model_id: model_id.to_string(),
        model_display_name: None,
        provider_slug: slugify_path_segment(provider_id),
        model_slug: slugify_path_segment(model_id),
        context_window: None,
        max_output_tokens: None,
        request_multiplier: None,
        thinking_config: ThinkingConfig::off(),
        base_url: None,
    })
}

/// Resolve the active benchmark model against provider metadata, storage, and config.
///
/// # Errors
///
/// Returns an error when the selected model is malformed or the provider is unknown.
pub fn resolve_model_context(
    selection: &str,
    provider_registry: &ProviderRegistry,
    storage: &Storage,
    config: &Config,
    explicit_thinking: Option<ThinkingConfig>,
) -> Result<ResolvedModelSelection> {
    let mut resolved = resolve_selected_model(selection)?;
    let provider = provider_registry
        .get(&resolved.provider_id)
        .ok_or_else(|| anyhow!("unknown provider '{}'", resolved.provider_id))?;
    resolved.provider_name = provider.name().to_string();

    let model_info = provider_registry.resolve_model(&resolved.provider_id, &resolved.model_id);
    if let Some(model) = &model_info {
        resolved.model_display_name = Some(model.name.clone());
        resolved.context_window = Some(model.context_window);
        resolved.max_output_tokens = model.max_output.and_then(|value| u32::try_from(value).ok());
        resolved.request_multiplier = model.request_multiplier;
    }

    let model_ref = ModelRef {
        provider_id: resolved.provider_id.clone(),
        model_id: resolved.model_id.clone(),
    };
    resolved.thinking_config = explicit_thinking
        .or_else(|| fallback_thinking_for_model_ref(config, provider_registry, &model_ref))
        .unwrap_or_else(ThinkingConfig::off);
    resolved.base_url = resolve_base_url(&resolved.provider_id, storage, config);

    Ok(resolved)
}

fn resolve_base_url(provider_id: &str, storage: &Storage, config: &Config) -> Option<String> {
    match provider_id {
        "copilot" => storage.get_setting("copilot_api_base").ok().flatten(),
        "generic_openai" => storage
            .get_setting("generic_openai_api_base")
            .ok()
            .flatten()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| {
                config
                    .provider
                    .get("generic_openai")
                    .and_then(|provider| provider.api.as_ref())
                    .and_then(|api| api.base_url.clone())
            })
            .or_else(|| {
                std::env::var("GENERIC_OPENAI_API_BASE")
                    .ok()
                    .filter(|s| !s.trim().is_empty())
            }),
        _ => None,
    }
}

fn benchmark_chat_request(
    selection: &ResolvedModelSelection,
    prompt: &str,
    options: &BenchRunOptions,
) -> ChatRequest {
    let deterministic = options.deterministic;
    ChatRequest {
        model: selection.model_id.clone(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text(prompt.to_string()),
        }],
        tools: vec![],
        temperature: if deterministic {
            Some(0.0)
        } else {
            options.temperature
        },
        top_p: if deterministic {
            Some(1.0)
        } else {
            options.top_p
        },
        max_tokens: options.max_tokens,
        system: Some(
            "You are running inside a benchmark harness. Respond with the requested completion only."
                .to_string(),
        ),
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: Some(selection.thinking_config.clone()),
    }
}

fn finish_reason_label(reason: LlmFinishReason) -> String {
    match reason {
        LlmFinishReason::Stop => "stop".to_string(),
        LlmFinishReason::ToolUse => "tool_use".to_string(),
        LlmFinishReason::Length => "length".to_string(),
        LlmFinishReason::ContentFilter => "content_filter".to_string(),
        LlmFinishReason::Cancelled => "cancelled".to_string(),
    }
}

fn benchmark_retry_delay(message: &str, attempt: u32, retry_started: Instant) -> Option<Duration> {
    if is_permanent_benchmark_api_error(message) || !is_retryable_benchmark_error(message) {
        return None;
    }

    if attempt >= BENCH_TRANSIENT_MAX_RETRIES {
        return None;
    }

    let delay = Duration::from_secs(
        (1_u64.checked_shl(attempt + 1).unwrap_or(u64::MAX)).min(BENCH_TRANSIENT_MAX_BACKOFF_SECS),
    );
    let elapsed_after_wait = retry_started.elapsed().saturating_add(delay);
    if elapsed_after_wait > Duration::from_secs(BENCH_TRANSIENT_MAX_ELAPSED_SECS) {
        return None;
    }

    Some(delay)
}

fn format_retry_exhausted_error(message: &str, attempts: u32, retry_started: Instant) -> String {
    format!(
        "{message} (transient benchmark retries exhausted after {attempts} attempt(s) over {}s)",
        retry_started.elapsed().as_secs()
    )
}

fn is_retryable_benchmark_error(message: &str) -> bool {
    let lower = message.to_lowercase();
    lower.contains("429")
        || lower.contains("503")
        || lower.contains("504")
        || lower.contains("408")
        || lower.contains("service unavailable")
        || lower.contains("server overloaded")
        || lower.contains("rate limit")
        || lower.contains("temporarily unavailable")
        || lower.contains("timed out")
        || lower.contains("timeout")
        || lower.contains("connection reset")
        || lower.contains("connection closed")
        || lower.contains("broken pipe")
        || lower.contains("unexpected eof")
        || lower.contains("http2 error")
        || lower.contains("h2 protocol error")
}

fn is_permanent_benchmark_api_error(message: &str) -> bool {
    let lower = message.to_lowercase();
    if lower.contains("invalid_request_error")
        || lower.contains("model_not_supported")
        || lower.contains("access denied for model")
        || lower.contains("invalid or expired api token")
    {
        return true;
    }

    extract_benchmark_error_status_code(message)
        .map(|code| (400..500).contains(&code) && code != 408 && code != 429)
        .unwrap_or(false)
}

fn extract_benchmark_error_status_code(message: &str) -> Option<u16> {
    let bytes = message.as_bytes();
    for window in bytes.windows(3) {
        if window.iter().all(u8::is_ascii_digit) {
            let code = std::str::from_utf8(window).ok()?.parse::<u16>().ok()?;
            if (100..=599).contains(&code) {
                return Some(code);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{
        BENCH_TRANSIENT_MAX_BACKOFF_SECS, BENCH_TRANSIENT_MAX_ELAPSED_SECS, benchmark_retry_delay,
        extract_benchmark_error_status_code, format_retry_exhausted_error,
        is_permanent_benchmark_api_error,
    };
    use std::time::{Duration, Instant};

    #[test]
    fn test_extract_benchmark_error_status_code_reads_http_style_messages() {
        assert_eq!(
            extract_benchmark_error_status_code("Ollama Cloud API error (503 Service Unavailable)"),
            Some(503)
        );
        assert_eq!(
            extract_benchmark_error_status_code("HTTP 429 Too Many Requests"),
            Some(429)
        );
    }

    #[test]
    fn test_is_permanent_benchmark_api_error_ignores_retryable_statuses() {
        assert!(!is_permanent_benchmark_api_error(
            "OpenAI API error (429 Too Many Requests): rate limited"
        ));
        assert!(!is_permanent_benchmark_api_error(
            "Ollama Cloud API error (503 Service Unavailable): server overloaded"
        ));
    }

    #[test]
    fn test_benchmark_retry_delay_grows_and_caps_for_retryable_errors() {
        let started = Instant::now();
        assert_eq!(
            benchmark_retry_delay("HTTP 503 Service Unavailable", 0, started),
            Some(Duration::from_secs(2))
        );
        assert_eq!(
            benchmark_retry_delay("HTTP 503 Service Unavailable", 3, started),
            Some(Duration::from_secs(16))
        );
        assert_eq!(
            benchmark_retry_delay("HTTP 503 Service Unavailable", 6, started),
            Some(Duration::from_secs(BENCH_TRANSIENT_MAX_BACKOFF_SECS))
        );
    }

    #[test]
    fn test_benchmark_retry_delay_stops_after_budget_is_spent() {
        let started = Instant::now() - Duration::from_secs(BENCH_TRANSIENT_MAX_ELAPSED_SECS);
        assert_eq!(
            benchmark_retry_delay("HTTP 503 Service Unavailable", 0, started),
            None
        );
        assert_eq!(
            benchmark_retry_delay("HTTP 400 Bad Request", 0, Instant::now()),
            None
        );
    }

    #[test]
    fn test_format_retry_exhausted_error_reports_attempts_and_elapsed_time() {
        let started = Instant::now() - Duration::from_secs(9);
        let message = format_retry_exhausted_error("HTTP 503 Service Unavailable", 5, started);
        assert!(message.contains("503"));
        assert!(message.contains("5 attempt(s)"));
        assert!(message.contains("over 9s") || message.contains("over 10s"));
    }
}
