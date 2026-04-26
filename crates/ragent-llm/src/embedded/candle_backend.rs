//! Candle-based backend for the embedded internal-LLM runtime.
//!
//! This module uses HuggingFace's `candle` framework (pure Rust) to run
//! quantized GGUF-format Llama and Mistral models locally. No C/C++ or LLVM
//! toolchain is required – all inference runs on CPU using Rust-native code.
//!
//! The backend expects:
//! - A `.gguf` model file (Llama/Mistral Q4/Q8 quantisation)
//! - A `tokenizer.json` file in the same directory (standard HuggingFace format)
//!
//! Both files are available on HuggingFace for any supported model.

use anyhow::{Context, Result, bail};
use candle_core::{Device, Tensor, quantized::gguf_file};
use candle_transformers::{generation::LogitsProcessor, models::quantized_llama::ModelWeights};
use ragent_config::InternalLlmConfig;
use rayon::ThreadPoolBuilder;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::Instant,
};
use tokenizers::Tokenizer;
use tracing::{info, warn};

use super::{
    ChatTemplate, EmbeddedBackend, EmbeddedInferenceError, EmbeddedModelManifest,
    EmbeddedRuntimeSettings, InferenceControls,
};

/// Default maximum tokens to generate per inference call.
const DEFAULT_MAX_GEN_TOKENS: u32 = 512;
const WARMUP_MAX_GEN_TOKENS: u32 = 1;
const WARMUP_SYSTEM_PROMPT: &str = "Reply with one short token.";
const WARMUP_USER_PROMPT: &str = "ok";

#[derive(Debug, Clone)]
struct RayonThreadPoolState {
    configured_threads: usize,
    effective_threads: usize,
    detail: String,
}

/// Internal state created during `prepare()` and held for the lifetime of inference.
struct CandleState {
    model: ModelWeights,
    tokenizer: Tokenizer,
    device: Device,
    eos_token_id: u32,
    chat_template: ChatTemplate,
    prompt_prefix_cache: HashMap<String, PromptPrefixCacheEntry>,
}

#[derive(Debug, Clone)]
struct PromptPrefixCacheEntry {
    prefix: String,
    assistant_suffix: &'static str,
}

/// Candle-backed `EmbeddedBackend` implementation.
///
/// After `prepare()` is called the GGUF model and tokenizer are loaded into
/// memory. Inference calls lock the state, build a chat prompt, tokenise it,
/// run the autoregressive generation loop, and return the decoded string.
pub struct CandleBackend {
    state: Mutex<Option<CandleState>>,
    /// Sampling temperature (lower = more deterministic).
    temperature: f64,
    /// RNG seed for reproducible sampling.
    seed: u64,
    /// Hard cap on generated tokens (independent of the per-request limit).
    max_gen_tokens: u32,
    /// Maximum prompt + output tokens supported by the configured runtime.
    context_window: usize,
}

impl CandleBackend {
    /// Creates a new backend from the agent's `InternalLlmConfig`.
    #[must_use]
    pub fn new(config: &InternalLlmConfig) -> Self {
        let _ = thread_pool_state(config.threads);
        Self {
            state: Mutex::new(None),
            temperature: 0.1,
            seed: 42,
            max_gen_tokens: DEFAULT_MAX_GEN_TOKENS,
            context_window: config.context_window.max(1),
        }
    }
}

pub(crate) fn candle_runtime_settings(config: &InternalLlmConfig) -> EmbeddedRuntimeSettings {
    let thread_pool = thread_pool_state(config.threads);
    let gpu_runtime_detail = if candle_core::utils::cuda_is_available() {
        "CUDA-capable Candle build detected, but the internal runtime still keeps GGUF execution on CPU"
    } else if candle_core::utils::metal_is_available() {
        "Metal-capable Candle build detected, but the internal runtime still keeps GGUF execution on CPU"
    } else {
        "this workspace builds Candle without CUDA or Metal support, so GPU offload is unavailable"
    };
    let gpu_offload = if config.gpu_layers == 0 {
        format!("not active; {gpu_runtime_detail}")
    } else {
        format!(
            "requested {} GPU layers, but {}; forcing 0 effective layers",
            config.gpu_layers, gpu_runtime_detail
        )
    };

    EmbeddedRuntimeSettings {
        execution_device: "cpu".to_string(),
        quantized_runtime: "gguf via candle_transformers::models::quantized_llama".to_string(),
        requested_threads: config.threads,
        effective_threads: thread_pool.effective_threads,
        threading: thread_pool_detail(&thread_pool, config.threads),
        requested_gpu_layers: config.gpu_layers,
        effective_gpu_layers: 0,
        gpu_offload,
    }
}

fn thread_pool_state(requested_threads: usize) -> RayonThreadPoolState {
    static THREAD_POOL_STATE: OnceLock<RayonThreadPoolState> = OnceLock::new();

    let requested_threads = requested_threads.max(1);
    let state = THREAD_POOL_STATE.get_or_init(|| {
        let build_result = ThreadPoolBuilder::new()
            .num_threads(requested_threads)
            .thread_name(|idx| format!("ragent-internal-llm-{idx}"))
            .build_global();
        let effective_threads = rayon::current_num_threads();
        let detail = match build_result {
            Ok(()) => format!(
                "rayon global pool initialized for internal LLM CPU execution with {effective_threads} threads"
            ),
            Err(error) => format!(
                "rayon global pool was already initialized before internal_llm.threads could be applied: {error}"
            ),
        };
        RayonThreadPoolState {
            configured_threads: requested_threads,
            effective_threads,
            detail,
        }
    });

    state.clone()
}

fn thread_pool_detail(state: &RayonThreadPoolState, requested_threads: usize) -> String {
    if state.configured_threads == requested_threads {
        return state.detail.clone();
    }

    format!(
        "{}; current config requested {requested_threads} threads but the process-global Rayon pool remains fixed at {}",
        state.detail, state.effective_threads
    )
}

impl EmbeddedBackend for CandleBackend {
    fn name(&self) -> &str {
        "candle"
    }

    fn prepare(
        &self,
        manifest: &EmbeddedModelManifest,
        model_dir: &Path,
        _config: &InternalLlmConfig,
    ) -> Result<()> {
        let prepare_started = Instant::now();
        let gguf_path = find_gguf_in_dir(manifest, model_dir)
            .context("No GGUF file found in embedded model directory")?;

        info!(path = %gguf_path.display(), "Initialising candle backend");

        // Load tokenizer from `tokenizer.json` in the same directory.
        let tokenizer_started = Instant::now();
        let tokenizer = load_tokenizer(model_dir)?;
        let eos_token_id = find_eos_token(&tokenizer, &manifest.chat_template);
        let tokenizer_elapsed_ms = tokenizer_started.elapsed().as_millis();
        info!(
            model_id = %manifest.model_id,
            elapsed_ms = tokenizer_elapsed_ms,
            "Candle tokenizer loaded"
        );

        // Load quantised model weights from the GGUF file.
        let model_load_started = Instant::now();
        let mut file = std::fs::File::open(&gguf_path)
            .with_context(|| format!("Cannot open GGUF file: {}", gguf_path.display()))?;

        let content = gguf_file::Content::read(&mut file)
            .map_err(|e| anyhow::anyhow!("Failed to read GGUF metadata: {e}"))?;

        let device = Device::Cpu;
        let model = ModelWeights::from_gguf(content, &mut file, &device)
            .map_err(|e| anyhow::anyhow!("Failed to load model weights from GGUF: {e}"))?;
        let model_load_elapsed_ms = model_load_started.elapsed().as_millis();
        info!(
            model_id = %manifest.model_id,
            path = %gguf_path.display(),
            elapsed_ms = model_load_elapsed_ms,
            "Candle model weights loaded"
        );

        let mut guard = self
            .state
            .lock()
            .map_err(|e| anyhow::anyhow!("CandleBackend mutex poisoned: {e}"))?;
        let mut state = CandleState {
            model,
            tokenizer,
            device,
            eos_token_id,
            chat_template: manifest.chat_template.clone(),
            prompt_prefix_cache: HashMap::new(),
        };

        let warmup_started = Instant::now();
        match run_warmup(&mut state, self.seed, self.temperature) {
            Ok(()) => info!(
                model_id = %manifest.model_id,
                elapsed_ms = warmup_started.elapsed().as_millis(),
                "Candle backend warmup completed"
            ),
            Err(error) => warn!(
                model_id = %manifest.model_id,
                elapsed_ms = warmup_started.elapsed().as_millis(),
                error = %error,
                "Candle backend warmup failed"
            ),
        }

        *guard = Some(state);

        info!(
            model_id = %manifest.model_id,
            path = %gguf_path.display(),
            tokenizer_load_ms = tokenizer_elapsed_ms,
            model_load_ms = model_load_elapsed_ms,
            warmup_ms = warmup_started.elapsed().as_millis(),
            elapsed_ms = prepare_started.elapsed().as_millis(),
            "Candle backend prepared"
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
        let mut guard = self.state.lock().map_err(|e| {
            EmbeddedInferenceError::Other(anyhow::anyhow!("CandleBackend mutex poisoned: {e}"))
        })?;

        let state = guard.as_mut().ok_or_else(|| {
            EmbeddedInferenceError::Other(anyhow::anyhow!(
                "CandleBackend not prepared; call prepare() first"
            ))
        })?;

        let prompt = format_prompt_cached(state, system_prompt, user_prompt);

        run_generation(
            state,
            &prompt,
            max_tokens.min(self.max_gen_tokens),
            self.context_window,
            self.seed,
            self.temperature,
            controls,
        )
    }
}

// ── Prompt formatting ─────────────────────────────────────────────────────────

/// Format a system + user message pair using cached prompt-prefix assembly.
fn format_prompt_cached(state: &mut CandleState, system_prompt: &str, user_prompt: &str) -> String {
    let entry = state
        .prompt_prefix_cache
        .entry(system_prompt.to_string())
        .or_insert_with(|| build_prompt_prefix_entry(&state.chat_template, system_prompt));
    let mut prompt = String::with_capacity(
        entry.prefix.len() + user_prompt.len() + entry.assistant_suffix.len(),
    );
    prompt.push_str(&entry.prefix);
    prompt.push_str(user_prompt);
    prompt.push_str(entry.assistant_suffix);
    prompt
}

fn build_prompt_prefix_entry(
    template: &ChatTemplate,
    system_prompt: &str,
) -> PromptPrefixCacheEntry {
    let (system_prefix, user_prefix, assistant_suffix) = prompt_fragments(template);
    let mut prefix =
        String::with_capacity(system_prefix.len() + system_prompt.len() + user_prefix.len());
    prefix.push_str(system_prefix);
    prefix.push_str(system_prompt);
    prefix.push_str(user_prefix);
    PromptPrefixCacheEntry {
        prefix,
        assistant_suffix,
    }
}

fn prompt_fragments(template: &ChatTemplate) -> (&'static str, &'static str, &'static str) {
    match template {
        ChatTemplate::TinyLlama => ("<|system|>\n", "</s>\n<|user|>\n", "</s>\n<|assistant|>\n"),
        ChatTemplate::ChatMl => (
            "<|im_start|>system\n",
            "<|im_end|>\n<|im_start|>user\n",
            "<|im_end|>\n<|im_start|>assistant\n",
        ),
    }
}

fn run_warmup(
    state: &mut CandleState,
    seed: u64,
    temperature: f64,
) -> std::result::Result<(), EmbeddedInferenceError> {
    let controls = InferenceControls::unbounded();
    let prompt = format_prompt_cached(state, WARMUP_SYSTEM_PROMPT, WARMUP_USER_PROMPT);
    let _ = run_generation(
        state,
        &prompt,
        WARMUP_MAX_GEN_TOKENS,
        usize::MAX,
        seed,
        temperature,
        &controls,
    )?;
    Ok(())
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Autoregressive generation loop.
fn run_generation(
    state: &mut CandleState,
    prompt: &str,
    max_new_tokens: u32,
    context_window: usize,
    seed: u64,
    temperature: f64,
    controls: &InferenceControls,
) -> std::result::Result<String, EmbeddedInferenceError> {
    controls.check()?;

    // Tokenise prompt (add BOS token).
    let encoding = state
        .tokenizer
        .encode(prompt, true)
        .map_err(|e| EmbeddedInferenceError::Other(anyhow::anyhow!("Tokenisation failed: {e}")))?;

    let prompt_tokens: Vec<u32> = encoding.get_ids().to_vec();
    if prompt_tokens.is_empty() {
        return Err(EmbeddedInferenceError::Other(anyhow::anyhow!(
            "Prompt tokenised to zero tokens"
        )));
    }

    let total_requested_tokens = prompt_tokens.len().saturating_add(max_new_tokens as usize);
    if total_requested_tokens > context_window {
        return Err(EmbeddedInferenceError::Other(anyhow::anyhow!(
            "prompt requires {total_requested_tokens} tokens ({} prompt + {} output) which exceeds context window {context_window}",
            prompt_tokens.len(),
            max_new_tokens
        )));
    }

    let mut logits_processor = LogitsProcessor::new(seed, Some(temperature), None);

    // First forward pass: feed the entire prompt and get the first generated token.
    controls.check()?;
    let input = Tensor::new(prompt_tokens.as_slice(), &state.device)
        .map_err(|e| {
            EmbeddedInferenceError::Other(anyhow::anyhow!("Prompt tensor build failed: {e}"))
        })?
        .unsqueeze(0)
        .map_err(|e| {
            EmbeddedInferenceError::Other(anyhow::anyhow!("Prompt batch reshape failed: {e}"))
        })?;
    let logits = state.model.forward(&input, 0).map_err(|e| {
        EmbeddedInferenceError::Other(anyhow::anyhow!("Forward pass (prefill) failed: {e}"))
    })?;
    let logits = logits.squeeze(0).map_err(|e| {
        EmbeddedInferenceError::Other(anyhow::anyhow!("Prefill squeeze failed: {e}"))
    })?;
    let first_token = logits_processor
        .sample(&logits)
        .map_err(|e| EmbeddedInferenceError::Other(anyhow::anyhow!("Sampling failed: {e}")))?;

    if first_token == state.eos_token_id {
        return Ok(String::new());
    }

    let mut output_tokens: Vec<u32> = vec![first_token];
    let kv_offset = prompt_tokens.len();

    // Autoregressive decode loop.
    for i in 1..max_new_tokens as usize {
        controls.check()?;
        let last = *output_tokens.last().expect("output_tokens non-empty");
        let input = Tensor::new(&[last], &state.device)
            .map_err(|e| {
                EmbeddedInferenceError::Other(anyhow::anyhow!("Decode tensor build failed: {e}"))
            })?
            .unsqueeze(0)
            .map_err(|e| {
                EmbeddedInferenceError::Other(anyhow::anyhow!(
                    "Decode batch reshape failed at step {i}: {e}"
                ))
            })?;
        let logits = state
            .model
            .forward(&input, kv_offset + i - 1)
            .map_err(|e| {
                EmbeddedInferenceError::Other(anyhow::anyhow!(
                    "Forward pass (decode step {i}) failed: {e}"
                ))
            })?;
        let logits = logits.squeeze(0).map_err(|e| {
            EmbeddedInferenceError::Other(anyhow::anyhow!("Decode squeeze failed at step {i}: {e}"))
        })?;
        let next_token = logits_processor.sample(&logits).map_err(|e| {
            EmbeddedInferenceError::Other(anyhow::anyhow!("Sampling failed at step {i}: {e}"))
        })?;

        if next_token == state.eos_token_id {
            break;
        }
        output_tokens.push(next_token);
    }

    // Decode back to text.
    controls.check()?;
    state
        .tokenizer
        .decode(&output_tokens, true)
        .map_err(|e| EmbeddedInferenceError::Other(anyhow::anyhow!("Detokenisation failed: {e}")))
}

/// Loads the tokenizer from `tokenizer.json` in the model directory.
fn load_tokenizer(model_dir: &Path) -> Result<Tokenizer> {
    let path = model_dir.join("tokenizer.json");
    if !path.exists() {
        bail!(
            "tokenizer.json not found at '{}'. \
            Please download it from the model's HuggingFace repository and \
            place it alongside the .gguf file.",
            path.display()
        );
    }
    Tokenizer::from_file(&path)
        .map_err(|e| anyhow::anyhow!("Failed to load tokenizer from '{}': {e}", path.display()))
}

/// Returns the EOS token ID for the given chat template, falling back to
/// common variants before using the standard Llama EOS id (2).
fn find_eos_token(tokenizer: &Tokenizer, template: &ChatTemplate) -> u32 {
    // Template-specific primary candidates.
    let primary = match template {
        ChatTemplate::ChatMl => "<|im_end|>",
        ChatTemplate::TinyLlama => "</s>",
    };
    if let Some(id) = tokenizer.token_to_id(primary) {
        return id;
    }
    // Universal fallbacks.
    for candidate in &["</s>", "<|endoftext|>", "<|im_end|>", "<eos>", "[EOS]"] {
        if let Some(id) = tokenizer.token_to_id(candidate) {
            return id;
        }
    }
    2 // Standard Llama/Mistral EOS token id.
}

/// Returns the first `.gguf` file found in `model_dir` that is listed in the manifest,
/// falling back to any `.gguf` file in the directory if none matches.
fn find_gguf_in_dir(manifest: &EmbeddedModelManifest, model_dir: &Path) -> Option<PathBuf> {
    for artifact in &manifest.artifacts {
        if artifact.file_name.ends_with(".gguf") {
            let path = model_dir.join(&artifact.file_name);
            if path.exists() {
                return Some(path);
            }
        }
    }
    if let Ok(entries) = std::fs::read_dir(model_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("gguf") {
                return Some(path);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{build_prompt_prefix_entry, prompt_fragments};
    use crate::embedded::ChatTemplate;

    #[test]
    fn test_prompt_prefix_entry_builds_tinyllama_prefix_and_suffix() {
        let entry = build_prompt_prefix_entry(&ChatTemplate::TinyLlama, "System prompt");
        assert_eq!(entry.prefix, "<|system|>\nSystem prompt</s>\n<|user|>\n");
        assert_eq!(entry.assistant_suffix, "</s>\n<|assistant|>\n");
    }

    #[test]
    fn test_prompt_fragments_build_chatml_layout() {
        let (system_prefix, user_prefix, assistant_suffix) =
            prompt_fragments(&ChatTemplate::ChatMl);
        assert_eq!(system_prefix, "<|im_start|>system\n");
        assert_eq!(user_prefix, "<|im_end|>\n<|im_start|>user\n");
        assert_eq!(assistant_suffix, "<|im_end|>\n<|im_start|>assistant\n");
    }
}
