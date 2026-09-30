//! Local ONNX Runtime embedding provider.
//!
//! [`LocalEmbeddingProvider`] uses ONNX Runtime to run a sentence-transformer
//! model (default: `all-MiniLM-L6-v2`) for generating text embeddings
//! entirely on the local machine - no API calls, no external services.
//!
//! # Model loading
//!
//! The ONNX model and tokenizer files are downloaded on first use from
//! HuggingFace to the ragent data directory (`~/.ragent/models/`). Subsequent
//! calls reuse the cached files.
//!
//! # Thread safety
//!
//! The ONNX session is wrapped in a `Mutex` and lazily initialised on first
//! `embed()` call. The provider is `Send + Sync` and safe to share across
//! async tasks via `Arc`.

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::{Context, Result};
use tracing::{debug, info};

use super::EmbeddingProvider;

/// HuggingFace repository for the default model.
const MODEL_REPO: &str = "sentence-transformers/all-MiniLM-L6-v2";

/// Files that must be present for the model to be considered downloaded.
const REQUIRED_FILES: &[&str] = &["model.onnx", "tokenizer.json", "config.json"];

/// Base URL for HuggingFace model hub file downloads.
const HF_BASE_URL: &str = "https://huggingface.co";

/// Upper bound on a single model-file download, in bytes (ANTIPAT 4.3).
///
/// `all-MiniLM-L6-v2`'s `model.onnx` is roughly 90 MiB; the 512 MiB ceiling
/// leaves generous headroom for larger models while ensuring a hostile or
/// mis-sized response cannot exhaust memory. The download stream is aborted
/// once this budget is exceeded rather than buffering the whole body first.
const MAX_MODEL_DOWNLOAD_BYTES: usize = 512 * 1024 * 1024;

/// Local ONNX Runtime embedding provider.
///
/// Loads a sentence-transformer ONNX model and tokeniser, then runs
/// inference to produce embedding vectors.
///
/// # Feature flag
///
/// Only available when the `embeddings` Cargo feature is enabled.
///
/// # Examples
///
/// ```no_run
/// use ragent_tools_extended::memory::embedding::{EmbeddingProvider, LocalEmbeddingProvider};
///
/// let provider = LocalEmbeddingProvider::new(384);
/// let vec = provider.embed("hello world").unwrap();
/// assert_eq!(vec.len(), 384);
/// ```
pub struct LocalEmbeddingProvider {
    /// Embedding vector dimensions (e.g. 384 for all-MiniLM-L6-v2).
    dimensions: usize,
    /// Lazy-initialised ONNX session and tokeniser, protected by a mutex.
    inner: Mutex<LocalEmbeddingInner>,
}

/// Holds the loaded ONNX session and tokeniser.
///
/// Lazily initialised on first `embed()` call.
enum LocalEmbeddingInner {
    /// Not yet initialised.
    Uninit,
    /// Successfully loaded and ready for inference.
    Ready {
        session: ort::session::Session,
        tokenizer: Box<tokenizers::Tokenizer>,
    },
    /// Initialisation failed; store the error so we don't retry indefinitely.
    Failed(String),
}

impl LocalEmbeddingProvider {
    /// Create a new local embedding provider with the given vector dimensions.
    ///
    /// The model is **not** loaded until the first call to [`EmbeddingProvider::embed`].
    /// This keeps startup fast.
    ///
    /// # Arguments
    ///
    /// * `dimensions` - Expected embedding vector length (384 for all-MiniLM-L6-v2).
    pub fn new(dimensions: usize) -> Self {
        Self {
            dimensions,
            inner: Mutex::new(LocalEmbeddingInner::Uninit),
        }
    }

    /// Ensure the model is downloaded and the ONNX session is ready.
    ///
    /// This is called on first `embed()`. If the model files are not found
    /// locally, they are downloaded from HuggingFace.
    fn ensure_initialised(&self) -> Result<()> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| anyhow::anyhow!("Embedding provider mutex poisoned: {e}"))?;

        if matches!(*guard, LocalEmbeddingInner::Ready { .. }) {
            return Ok(());
        }

        if let LocalEmbeddingInner::Failed(ref err) = *guard {
            anyhow::bail!("Embedding provider previously failed to initialise: {err}");
        }

        let model_dir = Self::model_dir()?;
        Self::ensure_model_files(&model_dir)?;

        let model_path = model_dir.join("model.onnx");
        debug!("Loading ONNX model from {}", model_path.display());
        let session = ort::session::Session::builder()
            .context("Failed to create ONNX session builder")?
            .commit_from_file(&model_path)
            .context("Failed to load ONNX model - is the file valid?")?;

        let tokenizer_path = model_dir.join("tokenizer.json");
        let tokenizer = tokenizers::Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer.json: {e}"))?;

        info!(
            "Local embedding provider initialised (model: {}, dims: {})",
            MODEL_REPO, self.dimensions
        );

        *guard = LocalEmbeddingInner::Ready {
            session,
            tokenizer: Box::new(tokenizer),
        };
        Ok(())
    }

    /// Return the directory where model files are stored.
    ///
    /// Uses `~/.ragent/models/all-MiniLM-L6-v2/`.
    fn model_dir() -> Result<PathBuf> {
        let base = ragent_config::user_dirs::global_embedding_model_dir()
            .ok_or_else(|| anyhow::anyhow!("no config directory for embedding models"))?;
        Ok(base)
    }

    /// Ensure all required model files exist, downloading any that are missing.
    fn ensure_model_files(model_dir: &PathBuf) -> Result<()> {
        fs::create_dir_all(model_dir)
            .with_context(|| format!("Failed to create model dir: {}", model_dir.display()))?;

        for file in REQUIRED_FILES {
            let path = model_dir.join(file);
            if !path.exists() {
                info!("Downloading model file: {file}");
                Self::download_file(file, &path)?;
            }
        }
        Ok(())
    }

    /// Download a single file from HuggingFace synchronously.
    ///
    /// The async `reqwest` transfer runs on a dedicated OS thread with its own
    /// current-thread Tokio runtime. That thread carries no async-task context,
    /// so a runtime is never constructed inside an async worker thread (which
    /// would panic on a current-thread runtime) and the caller's executor is
    /// never required to host a nested runtime (ANTIPAT 4.3). The blocking join
    /// only happens during lazy model initialisation.
    ///
    /// The response body is streamed and capped at [`MAX_MODEL_DOWNLOAD_BYTES`]
    /// so a hostile or mis-sized download cannot exhaust memory.
    fn download_file(filename: &str, dest: &PathBuf) -> Result<()> {
        let url = format!("{HF_BASE_URL}/{MODEL_REPO}/resolve/main/{filename}");
        let dest = dest.clone();
        let filename_owned = filename.to_string();

        std::thread::Builder::new()
            .name("ragent-model-download".to_string())
            .spawn(move || Self::download_file_blocking(&url, &filename_owned, &dest))
            .context("Failed to spawn model download thread")?
            .join()
            .map_err(|_| anyhow::anyhow!("model download thread panicked"))?
    }

    /// Perform the actual download inside a fresh current-thread runtime.
    ///
    /// Runs only on the dedicated thread spawned by [`Self::download_file`], so
    /// it is never nested inside an async context.
    fn download_file_blocking(url: &str, filename: &str, dest: &PathBuf) -> Result<()> {
        debug!("Downloading {url}");

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("Failed to create Tokio runtime for model download")?;

        rt.block_on(async move {
            let response = reqwest::get(url)
                .await
                .with_context(|| format!("Failed to download {url}"))?;

            if !response.status().is_success() {
                anyhow::bail!("Failed to download {filename}: HTTP {}", response.status());
            }

            let bytes =
                crate::masterfetch::http::read_bytes_capped(response, MAX_MODEL_DOWNLOAD_BYTES)
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to read download response: {e}"))?;

            // `read_bytes_capped` appends a truncation marker and returns the
            // bytes only when the stream exceeded the cap; a model file that
            // large is a failure, not a usable download.
            if bytes.len() > MAX_MODEL_DOWNLOAD_BYTES {
                anyhow::bail!(
                    "model file {filename} exceeded the {MAX_MODEL_DOWNLOAD_BYTES} byte cap"
                );
            }

            let tmp_path = dest.with_extension("tmp");
            fs::write(&tmp_path, &bytes)
                .with_context(|| format!("Failed to write {}", tmp_path.display()))?;
            fs::rename(&tmp_path, dest)
                .with_context(|| format!("Failed to rename tmp to {}", dest.display()))?;

            Ok(())
        })
    }

    /// Run ONNX inference on tokenised input to produce an embedding.
    fn encode(&self, text: &str) -> Result<Vec<f32>> {
        self.ensure_initialised()?;

        let mut guard = self
            .inner
            .lock()
            .map_err(|e| anyhow::anyhow!("Embedding provider mutex poisoned: {e}"))?;

        let tokenizer = match &*guard {
            LocalEmbeddingInner::Ready { tokenizer, .. } => tokenizer,
            _ => anyhow::bail!("Embedding provider not in Ready state"),
        };

        let encoding = tokenizer
            .encode(text, true)
            .map_err(|e| anyhow::anyhow!("Tokenisation failed: {e}"))?;

        let input_ids = encoding.get_ids();
        let attention_mask = encoding.get_attention_mask();
        let seq_len = input_ids.len();

        let input_ids_array = ndarray::Array2::from_shape_vec(
            (1, seq_len),
            input_ids.iter().map(|&id| id as i64).collect::<Vec<_>>(),
        )
        .map_err(|e| anyhow::anyhow!("Failed to create input_ids array: {e}"))?;

        let attention_mask_array = ndarray::Array2::from_shape_vec(
            (1, seq_len),
            attention_mask.iter().map(|&m| m as i64).collect::<Vec<_>>(),
        )
        .map_err(|e| anyhow::anyhow!("Failed to create attention_mask array: {e}"))?;

        let input_ids_value = ort::value::Tensor::<i64>::from_array(input_ids_array.into_dyn())
            .map_err(|e| anyhow::anyhow!("Failed to create input_ids tensor: {e}"))?;
        let attention_mask_value =
            ort::value::Tensor::<i64>::from_array(attention_mask_array.into_dyn())
                .map_err(|e| anyhow::anyhow!("Failed to create attention_mask tensor: {e}"))?;

        let session = match &mut *guard {
            LocalEmbeddingInner::Ready { session, .. } => session,
            _ => anyhow::bail!("Embedding provider not in Ready state"),
        };

        let outputs = session
            .run(ort::inputs! {
                "input_ids" => input_ids_value,
                "attention_mask" => attention_mask_value,
            })
            .map_err(|e| anyhow::anyhow!("ONNX inference failed: {e}"))?;

        let mut output_iter = outputs.iter();
        let (_, first_output) = output_iter
            .next()
            .ok_or_else(|| anyhow::anyhow!("ONNX model returned no outputs"))?;

        let output_view: ndarray::ArrayViewD<f32> = first_output
            .try_extract_array::<f32>()
            .map_err(|e| anyhow::anyhow!("Failed to extract output tensor: {e}"))?;

        let dim = self.dimensions;
        let shape = output_view.shape();

        if shape.len() == 3 && shape[0] == 1 && shape[2] == dim {
            let seq_len_out = shape[1];
            let mut pooled = vec![0.0_f32; dim];
            let mut count: f32 = 0.0;

            for s in 0..seq_len_out {
                let mask_val = if s < attention_mask.len() {
                    attention_mask[s] as f32
                } else {
                    1.0
                };
                count += mask_val;
                for d in 0..dim {
                    pooled[d] += output_view[[0, s, d]] * mask_val;
                }
            }
            if count > 0.0 {
                for val in &mut pooled {
                    *val /= count;
                }
            }

            let norm: f32 = pooled.iter().map(|v| v * v).sum::<f32>().sqrt();
            if norm > 0.0 {
                for val in &mut pooled {
                    *val /= norm;
                }
            }

            Ok(pooled)
        } else if shape.len() == 2 && shape[0] == 1 && shape[1] == dim {
            let mut vec: Vec<f32> = output_view
                .as_slice()
                .ok_or_else(|| anyhow::anyhow!("Output tensor is not contiguous"))?
                .to_vec();
            let norm: f32 = vec.iter().map(|v| v * v).sum::<f32>().sqrt();
            if norm > 0.0 {
                for val in &mut vec {
                    *val /= norm;
                }
            }
            Ok(vec)
        } else {
            anyhow::bail!(
                "Unexpected ONNX output shape: {:?} (expected [1, seq, {dim}] or [1, {dim}])",
                shape
            )
        }
    }
}

impl EmbeddingProvider for LocalEmbeddingProvider {
    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        match self.encode(text) {
            Ok(vec) => Ok(vec),
            Err(e) => {
                if let Ok(mut guard) = self.inner.lock()
                    && matches!(*guard, LocalEmbeddingInner::Uninit)
                {
                    *guard = LocalEmbeddingInner::Failed(e.to_string());
                }
                Err(e)
            }
        }
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn name(&self) -> &str {
        "ort-local"
    }

    fn is_available(&self) -> bool {
        true
    }
}
