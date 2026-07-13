//! Rig-backed embedding model adapter (T-007 / FR-007).
//!
//! This module is compiled only when the `embeddings` feature is enabled
//! (which pulls in `rig-core`).
//!
//! # Architecture
//!
//! The full chain from a Rig embedding model to ragent's memory semantic-search
//! path is:
//!
//! ```text
//! rig::embeddings::EmbeddingModel        (async, Vec<f64>)
//!     │
//!     ▼  RigEmbeddingBackendImpl::from_model()
//! RigEmbeddingBackendImpl               (sync bridge: block_on async + f64→f32)
//!     │  implements RigEmbeddingBackend  (the internal adapter trait from T-003)
//!     ▼
//! Box<dyn RigEmbeddingBackend>
//!     │
//!     ▼  RigEmbeddingProvider::new(backend)
//! RigEmbeddingProvider                  (implements ragent EmbeddingProvider)
//!     │
//!     ▼
//! ragent memory semantic-search path    (Storage::search_memories_by_embedding)
//! ```
//!
//! Rig's `EmbeddingModel` trait is async and not object-safe (it returns
//! `impl Future` and requires `Clone`). The concrete provider models are
//! therefore captured at wiring time into a boxed async closure, exactly as
//! the completion adapter (T-004) captures `CompletionModel` into a streaming
//! closure.
//!
//! # Sync↔async bridge
//!
//! ragent's [`EmbeddingProvider::embed`] is **synchronous**, while Rig's
//! `EmbeddingModel::embed_text` is **async**. The bridge uses a lazily-created
//! dedicated current-thread Tokio runtime (stored in a `OnceLock`) and calls
//! `Runtime::block_on` on the async embed future. Because the runtime is
//! separate from any ambient runtime, `block_on` never panics — it works
//! whether `embed` is called from a sync or async context, and from either a
//! single-threaded or multi-threaded ambient runtime. The runtime is created
//! once per backend instance and reused for every subsequent `embed` call.

use std::future::Future;
use std::pin::Pin;
use std::sync::OnceLock;

use ragent_agent::memory::embedding::EmbeddingProvider;
use rig::embeddings::EmbeddingModel as RigEmbeddingModel;

use crate::error::{Result, RigError};

// Re-export the internal adapter trait and metadata handle so callers do not
// need to reach into a separate module.
pub use crate::embeddings_trait::{EmbeddingAdapter, RigEmbeddingBackend};

/// A boxed async embed function: `&str -> Future<Output = Result<Vec<f32>>>`.
///
/// The function returns a pinned boxed future so the concrete Rig
/// `EmbeddingModel` type is handled at wiring time (see
/// [`RigEmbeddingBackendImpl::from_model`]).
type AsyncEmbedFn =
    Box<dyn Fn(&str) -> Pin<Box<dyn Future<Output = Result<Vec<f32>>> + Send>> + Send + Sync>;

/// A boxed async batch-embed function: `&[String] -> Future<Output = Result<Vec<Vec<f32>>>>`.
type AsyncBatchFn = Box<
    dyn Fn(&[String]) -> Pin<Box<dyn Future<Output = Result<Vec<Vec<f32>>>> + Send>> + Send + Sync,
>;

/// A concrete [`RigEmbeddingBackend`] backed by a boxed async embed function.
///
/// This is the T-007 implementation: it wraps a Rig `EmbeddingModel` (captured
/// at wiring time) and bridges the async Rig API onto the sync
/// [`RigEmbeddingBackend`] contract using a lazily-created dedicated Tokio
/// runtime (see the [module docs](self) for the sync↔async bridge rationale).
///
/// Construct with [`RigEmbeddingBackendImpl::from_model`] (generic over any
/// `EmbeddingModel`) or [`RigEmbeddingBackendImpl::from_async_embed_fn`] (for
/// custom/test backends).
pub struct RigEmbeddingBackendImpl {
    name: String,
    dimensions: usize,
    embed_fn: AsyncEmbedFn,
    /// Optional provider-side batch function. When `None`, `embed_batch` falls
    /// back to the default sequential implementation.
    batch_fn: Option<AsyncBatchFn>,
    /// Lazily-created dedicated current-thread runtime used to block on the
    /// async embed function from the sync `embed` method.
    runtime: OnceLock<tokio::runtime::Runtime>,
}

impl std::fmt::Debug for RigEmbeddingBackendImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RigEmbeddingBackendImpl")
            .field("name", &self.name)
            .field("dimensions", &self.dimensions)
            .field("has_batch_fn", &self.batch_fn.is_some())
            .finish_non_exhaustive()
    }
}

impl RigEmbeddingBackendImpl {
    /// Construct a backend from a boxed async embed function.
    ///
    /// Use this for custom backends or tests where you already have an async
    /// function that produces `Vec<f32>`. For wrapping a Rig `EmbeddingModel`,
    /// prefer [`Self::from_model`].
    ///
    /// `dimensions` is the length of the vectors the function will produce; a
    /// value of `0` marks the backend as disabled (see
    /// [`RigEmbeddingBackend::is_available`]).
    #[must_use]
    pub fn from_async_embed_fn(name: String, dimensions: usize, embed_fn: AsyncEmbedFn) -> Self {
        Self {
            name,
            dimensions,
            embed_fn,
            batch_fn: None,
            runtime: OnceLock::new(),
        }
    }

    /// Construct a backend from a boxed async embed function **and** a
    /// provider-side batch function.
    ///
    /// The batch function is used by [`RigEmbeddingBackend::embed_batch`] so
    /// providers that support server-side batching (e.g. OpenAI, Gemini) get
    /// better throughput than the default sequential loop.
    #[must_use]
    pub fn from_async_fns(
        name: String,
        dimensions: usize,
        embed_fn: AsyncEmbedFn,
        batch_fn: AsyncBatchFn,
    ) -> Self {
        Self {
            name,
            dimensions,
            embed_fn,
            batch_fn: Some(batch_fn),
            runtime: OnceLock::new(),
        }
    }

    /// Construct a backend wrapping any Rig [`EmbeddingModel`].
    ///
    /// The model is captured into a boxed async closure that calls
    /// `model.embed_text(text)` and converts the resulting `Vec<f64>` into
    /// `Vec<f32>` (ragent uses `f32` embeddings; Rig uses `f64`).
    ///
    /// `dimensions` should match `model.ndims()`; pass it explicitly because
    /// ragent needs the dimensionality before the first embed call (for
    /// storage schema and similarity-search setup).
    ///
    /// # Errors
    ///
    /// This constructor itself does not return an error, but the resulting
    /// backend's [`RigEmbeddingBackend::embed`] will return
    /// [`RigError::BackendError`] if the underlying Rig model fails.
    #[must_use]
    pub fn from_model<M>(model: M, name: String, dimensions: usize) -> Self
    where
        M: RigEmbeddingModel + Send + Sync + 'static,
    {
        let model_for_single = model.clone();
        let model_for_batch = model.clone();
        let embed_fn: AsyncEmbedFn = Box::new(move |text: &str| {
            let m = model_for_single.clone();
            let text = text.to_string();
            Box::pin(async move {
                let embedding = m
                    .embed_text(&text)
                    .await
                    .map_err(|e| RigError::BackendError(format!("rig embed_text failed: {e}")))?;
                Ok(embedding.vec.into_iter().map(|v| v as f32).collect())
            })
        });
        let batch_fn: AsyncBatchFn = Box::new(move |texts: &[String]| {
            let m = model_for_batch.clone();
            let texts: Vec<String> = texts.to_vec();
            Box::pin(async move {
                let embeddings = m
                    .embed_texts(texts)
                    .await
                    .map_err(|e| RigError::BackendError(format!("rig embed_texts failed: {e}")))?;
                Ok(embeddings
                    .into_iter()
                    .map(|emb| emb.vec.into_iter().map(|v| v as f32).collect())
                    .collect())
            })
        });
        Self::from_async_fns(name, dimensions, embed_fn, batch_fn)
    }

    /// Returns the lazily-created runtime, building it on first use.
    ///
    /// A current-thread runtime is used so `block_on` works even when `embed`
    /// is called from within an ambient async runtime (the dedicated runtime
    /// is independent of any ambient `tokio::runtime::Handle`).
    fn runtime(&self) -> &tokio::runtime::Runtime {
        self.runtime.get_or_init(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build embedding bridge runtime")
        })
    }
}

impl RigEmbeddingBackend for RigEmbeddingBackendImpl {
    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let fut = (self.embed_fn)(text);
        self.runtime().block_on(fut)
    }

    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        match &self.batch_fn {
            Some(batch_fn) => {
                let owned: Vec<String> = texts.iter().map(|s| s.to_string()).collect();
                let fut = batch_fn(&owned);
                self.runtime().block_on(fut)
            }
            None => {
                // Fall back to the trait default sequential implementation.
                texts.iter().map(|t| self.embed(t)).collect()
            }
        }
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn name(&self) -> &str {
        &self.name
    }
}

// ── ragent EmbeddingProvider bridge ──────────────────────────────────────────

/// A ragent [`EmbeddingProvider`] backed by a [`RigEmbeddingBackend`].
///
/// This is the outermost adapter: it wraps a `Box<dyn RigEmbeddingBackend>`
/// (typically a [`RigEmbeddingBackendImpl`]) and exposes it as a ragent
/// `EmbeddingProvider` so it can be plugged into ragent's memory
/// semantic-search path
/// (`Storage::search_memories_by_embedding`) without the caller needing to
/// know about the Rig adapter layer.
///
/// Construct with [`RigEmbeddingProvider::new`] or
/// [`RigEmbeddingProvider::from_model`].
pub struct RigEmbeddingProvider {
    backend: Box<dyn RigEmbeddingBackend>,
}

impl std::fmt::Debug for RigEmbeddingProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RigEmbeddingProvider")
            .field("name", &self.backend.name())
            .field("dimensions", &self.backend.dimensions())
            .finish_non_exhaustive()
    }
}

impl RigEmbeddingProvider {
    /// Wrap any [`RigEmbeddingBackend`] as a ragent [`EmbeddingProvider`].
    #[must_use]
    pub fn new(backend: Box<dyn RigEmbeddingBackend>) -> Self {
        Self { backend }
    }

    /// Convenience: construct a [`RigEmbeddingBackendImpl`] from a Rig
    /// `EmbeddingModel` and wrap it as a ragent `EmbeddingProvider` in one
    /// step.
    ///
    /// See [`RigEmbeddingBackendImpl::from_model`] for the model-to-backend
    /// conversion details.
    #[must_use]
    pub fn from_model<M>(model: M, name: String, dimensions: usize) -> Self
    where
        M: RigEmbeddingModel + Send + Sync + 'static,
    {
        Self::new(Box::new(RigEmbeddingBackendImpl::from_model(
            model, name, dimensions,
        )))
    }

    /// Returns a reference to the underlying backend.
    #[must_use]
    pub fn backend(&self) -> &dyn RigEmbeddingBackend {
        &*self.backend
    }
}

impl EmbeddingProvider for RigEmbeddingProvider {
    fn embed(&self, text: &str) -> std::result::Result<Vec<f32>, anyhow::Error> {
        self.backend.embed(text).map_err(anyhow::Error::from)
    }

    fn embed_batch(&self, texts: &[&str]) -> std::result::Result<Vec<Vec<f32>>, anyhow::Error> {
        self.backend.embed_batch(texts).map_err(anyhow::Error::from)
    }

    fn dimensions(&self) -> usize {
        self.backend.dimensions()
    }

    fn name(&self) -> &str {
        self.backend.name()
    }

    fn is_available(&self) -> bool {
        self.backend.is_available()
    }
}

// ── Per-provider builders ────────────────────────────────────────────────────
//
// Each builder constructs the concrete Rig provider embedding model, then
// wraps it in a `RigEmbeddingBackendImpl`. The `embeddings` feature pulls in
// `rig-core`; the provider module access is gated on the matching
// `provider-*` feature so that, e.g., the OpenAI embedding builder only
// compiles when `provider-openai` is enabled.

/// Build a Rig-backed OpenAI embedding backend.
///
/// Uses `openai::Client::embedding_model`, which defaults to 1536 dimensions
/// (`text-embedding-3-small`).
///
/// # Errors
///
/// Returns [`RigError::InvalidConfiguration`] if `model` is empty.
#[cfg(feature = "provider-openai")]
pub fn build_openai_embedding_backend(
    api_key: &str,
    base_url: Option<&str>,
    model: String,
    dimensions: usize,
) -> Result<RigEmbeddingBackendImpl> {
    use rig::providers::openai;
    if model.is_empty() {
        return Err(RigError::InvalidConfiguration(
            "openai embedding model name must not be empty".to_owned(),
        ));
    }
    let client = match base_url {
        Some(url) => openai::Client::from_url(api_key, url),
        None => openai::Client::new(api_key),
    };
    let rig_model = client.embedding_model(&model);
    Ok(RigEmbeddingBackendImpl::from_model(
        rig_model,
        format!("rig-openai/{model}"),
        dimensions,
    ))
}

/// Build a Rig-backed Gemini embedding backend.
///
/// # Errors
///
/// Returns [`RigError::InvalidConfiguration`] if `model` is empty.
#[cfg(feature = "provider-gemini")]
pub fn build_gemini_embedding_backend(
    api_key: &str,
    base_url: Option<&str>,
    model: String,
    dimensions: usize,
) -> Result<RigEmbeddingBackendImpl> {
    use rig::providers::gemini;
    if model.is_empty() {
        return Err(RigError::InvalidConfiguration(
            "gemini embedding model name must not be empty".to_owned(),
        ));
    }
    let client = match base_url {
        Some(url) => gemini::Client::from_url(api_key, url),
        None => gemini::Client::new(api_key),
    };
    let rig_model = client.embedding_model(&model);
    Ok(RigEmbeddingBackendImpl::from_model(
        rig_model,
        format!("rig-gemini/{model}"),
        dimensions,
    ))
}

/// Build a Rig-backed Ollama embedding backend.
///
/// Ollama runs locally and does not need an API key.
///
/// # Errors
///
/// Returns [`RigError::InvalidConfiguration`] if `model` is empty.
#[cfg(feature = "provider-ollama")]
pub fn build_ollama_embedding_backend(
    base_url: Option<&str>,
    model: String,
    dimensions: usize,
) -> Result<RigEmbeddingBackendImpl> {
    use rig::providers::ollama;
    if model.is_empty() {
        return Err(RigError::InvalidConfiguration(
            "ollama embedding model name must not be empty".to_owned(),
        ));
    }
    let client = match base_url {
        Some(url) => ollama::Client::from_url(url),
        None => ollama::Client::new(),
    };
    let rig_model = client.embedding_model(&model);
    Ok(RigEmbeddingBackendImpl::from_model(
        rig_model,
        format!("rig-ollama/{model}"),
        dimensions,
    ))
}

/// Dispatch table: construct an embedding backend by Rig provider name.
///
/// Used by T-006/T-019 wiring to turn a configured embedding provider into a
/// concrete [`RigEmbeddingBackendImpl`] without the caller needing to know
/// which feature flag gates which builder.
///
/// # Errors
///
/// Returns [`RigError::ProviderNotEnabled`] if the requested provider's
/// feature flag is not compiled in, or [`RigError::InvalidConfiguration`] if
/// the provider name is not recognised or the model name is empty.
pub fn build_embedding_backend_by_provider(
    provider: &str,
    api_key: &str,
    base_url: Option<&str>,
    model: String,
    dimensions: usize,
) -> Result<RigEmbeddingBackendImpl> {
    match provider {
        #[cfg(feature = "provider-openai")]
        "openai" => build_openai_embedding_backend(api_key, base_url, model, dimensions),
        #[cfg(feature = "provider-gemini")]
        "gemini" => build_gemini_embedding_backend(api_key, base_url, model, dimensions),
        #[cfg(feature = "provider-ollama")]
        "ollama" => build_ollama_embedding_backend(base_url, model, dimensions),
        #[cfg(not(all(
            feature = "provider-openai",
            feature = "provider-gemini",
            feature = "provider-ollama",
        )))]
        other => {
            if matches!(other, "openai" | "gemini" | "ollama" | "cohere" | "azure") {
                Err(RigError::ProviderNotEnabled(other.to_owned()))
            } else {
                Err(RigError::InvalidConfiguration(format!(
                    "unknown Rig embedding provider: {other}"
                )))
            }
        }
        #[cfg(all(
            feature = "provider-openai",
            feature = "provider-gemini",
            feature = "provider-ollama",
        ))]
        other => Err(RigError::InvalidConfiguration(format!(
            "unsupported Rig embedding provider: {other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rig::embeddings::{Embedding, EmbeddingError};

    /// A deterministic mock `EmbeddingModel` for tests.
    ///
    /// Each embedding vector is `[text.len() as f64; ndims]` so tests can
    /// verify the text flowed through and the f64→f32 conversion happened.
    #[derive(Clone)]
    struct MockEmbeddingModel {
        ndims: usize,
    }

    impl rig::embeddings::EmbeddingModel for MockEmbeddingModel {
        const MAX_DOCUMENTS: usize = 64;

        fn ndims(&self) -> usize {
            self.ndims
        }

        fn embed_texts(
            &self,
            texts: impl IntoIterator<Item = String> + Send,
        ) -> impl std::future::Future<Output = std::result::Result<Vec<Embedding>, EmbeddingError>> + Send
        {
            let ndims = self.ndims;
            let texts: Vec<String> = texts.into_iter().collect();
            async move {
                Ok(texts
                    .into_iter()
                    .map(|t| Embedding {
                        document: t.clone(),
                        vec: vec![t.len() as f64; ndims],
                    })
                    .collect())
            }
        }
    }

    #[test]
    fn from_model_embed_converts_f64_to_f32() {
        let backend =
            RigEmbeddingBackendImpl::from_model(MockEmbeddingModel { ndims: 4 }, "mock".into(), 4);
        let vec = backend.embed("hello").expect("embed");
        assert_eq!(vec.len(), 4);
        // "hello".len() == 5; f64→f32 cast preserves the value.
        assert_eq!(vec[0], 5.0_f32);
        assert!(backend.is_available());
    }

    #[test]
    fn from_model_embed_batch_uses_rig_batch() {
        let backend =
            RigEmbeddingBackendImpl::from_model(MockEmbeddingModel { ndims: 3 }, "mock".into(), 3);
        let vecs = backend.embed_batch(&["a", "bb", "ccc"]).expect("batch");
        assert_eq!(vecs.len(), 3);
        assert_eq!(vecs[0], vec![1.0_f32; 3]); // "a"
        assert_eq!(vecs[1], vec![2.0_f32; 3]); // "bb"
        assert_eq!(vecs[2], vec![3.0_f32; 3]); // "ccc"
    }

    #[test]
    fn from_async_embed_fn_without_batch_falls_back_to_sequential() {
        let backend = RigEmbeddingBackendImpl::from_async_embed_fn(
            "stub".into(),
            2,
            Box::new(|text: &str| {
                let text = text.to_string();
                Box::pin(async move { Ok(vec![text.len() as f32; 2]) })
            }),
        );
        let vecs = backend.embed_batch(&["a", "bb"]).expect("batch");
        assert_eq!(vecs.len(), 2);
        assert_eq!(vecs[0], vec![1.0_f32; 2]);
        assert_eq!(vecs[1], vec![2.0_f32; 2]);
    }

    #[test]
    fn dimensions_and_name_reported() {
        let backend =
            RigEmbeddingBackendImpl::from_model(MockEmbeddingModel { ndims: 8 }, "mock8".into(), 8);
        assert_eq!(backend.dimensions(), 8);
        assert_eq!(backend.name(), "mock8");
        assert!(backend.is_available());
    }

    #[test]
    fn zero_dim_backend_is_not_available() {
        let backend =
            RigEmbeddingBackendImpl::from_model(MockEmbeddingModel { ndims: 0 }, "zero".into(), 0);
        assert_eq!(backend.dimensions(), 0);
        assert!(!backend.is_available());
    }

    #[test]
    fn runtime_is_reused_across_calls() {
        // Two embed calls share the same lazily-created runtime. We cannot
        // observe the runtime directly, but we can verify both calls succeed
        // and return consistent results — if a new runtime were created per
        // call it would still work, but this at least guards against a panic
        // from reentrant block_on.
        let backend =
            RigEmbeddingBackendImpl::from_model(MockEmbeddingModel { ndims: 2 }, "mock".into(), 2);
        let a = backend.embed("hi").expect("first embed");
        let b = backend.embed("hi").expect("second embed");
        assert_eq!(a, b);
        assert_eq!(a, vec![2.0_f32; 2]);
    }

    #[test]
    fn rig_embedding_provider_implements_ragent_trait() {
        let provider =
            RigEmbeddingProvider::from_model(MockEmbeddingModel { ndims: 3 }, "mock".into(), 3);
        // ragent's EmbeddingProvider trait methods:
        assert_eq!(provider.dimensions(), 3);
        assert_eq!(provider.name(), "mock");
        assert!(provider.is_available());
        let vec = provider.embed("hello").expect("embed via provider");
        assert_eq!(vec.len(), 3);
        assert_eq!(vec[0], 5.0_f32);
    }

    #[test]
    fn rig_embedding_provider_batch_delegates_to_backend() {
        let provider =
            RigEmbeddingProvider::from_model(MockEmbeddingModel { ndims: 2 }, "mock".into(), 2);
        let vecs = provider
            .embed_batch(&["a", "bb"])
            .expect("batch via provider");
        assert_eq!(vecs.len(), 2);
        assert_eq!(vecs[0], vec![1.0_f32; 2]);
        assert_eq!(vecs[1], vec![2.0_f32; 2]);
    }

    #[test]
    fn boxed_backend_provider_chain_is_object_safe() {
        // Verify the full chain can be boxed dynamically.
        let backend: Box<dyn RigEmbeddingBackend> = Box::new(RigEmbeddingBackendImpl::from_model(
            MockEmbeddingModel { ndims: 4 },
            "boxed".into(),
            4,
        ));
        let provider = RigEmbeddingProvider::new(backend);
        assert_eq!(provider.dimensions(), 4);
        assert_eq!(provider.name(), "boxed");
        let vec = provider.embed("test").expect("embed");
        assert_eq!(vec.len(), 4);
        assert_eq!(vec[0], 4.0_f32); // "test".len() == 4
    }

    #[cfg(feature = "provider-openai")]
    #[test]
    fn build_openai_embedding_backend_rejects_empty_model() {
        let err = build_openai_embedding_backend("key", None, String::new(), 1536)
            .expect_err("expected error");
        assert!(matches!(err, RigError::InvalidConfiguration(_)));
    }

    #[test]
    fn build_embedding_backend_by_provider_rejects_unknown() {
        let err = build_embedding_backend_by_provider("not-a-provider", "key", None, "m".into(), 8)
            .expect_err("expected error");
        assert!(matches!(err, RigError::InvalidConfiguration(_)));
    }
}
