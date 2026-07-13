//! Internal adapter trait for the embedding path (T-003).
//!
//! This module defines the stable, ragent-facing contract that the
//! Rig-backed embedding backend (implemented in [`crate::embeddings`] as
//! T-007) satisfies. The trait is intentionally decoupled from `rig-core`
//! specifics so that:
//!
//! * Future Rig API changes only require updating the concrete impl, not the
//!   trait.
//! * Tests can substitute a deterministic mock backend without pulling in
//!   `rig-core` (see the `mock` feature and T-014).
//!
//! This module is compiled unconditionally (it has no `rig-core` dependency),
//! so the trait and metadata handle are available even when the `embeddings`
//! feature is off. The concrete Rig-backed implementation in
//! [`crate::embeddings`] is only compiled when the `embeddings` feature is on.

use crate::error::{Result, RigError};

/// The internal contract for a Rig-backed embedding backend.
///
/// A concrete implementation (T-007, in [`crate::embeddings`]) wraps a
/// `rig::embeddings::EmbeddingModel`, converts text inputs into Rig's
/// embedding request representation, and returns fixed-dimension `f32`
/// vectors.
///
/// The trait is object-safe so it can be stored as
/// `Box<dyn RigEmbeddingBackend>` and wrapped into ragent's
/// `EmbeddingProvider` (see `ragent_agent::memory::embedding`) by the
/// [`crate::embeddings::RigEmbeddingProvider`] adapter struct.
///
/// # Errors
///
/// Every method returns [`crate::error::Result`]; backends surface
/// irrecoverable failures as [`RigError::BackendError`].
pub trait RigEmbeddingBackend: Send + Sync {
    /// Generate an embedding vector for a single text string.
    ///
    /// Returns a `Vec<f32>` of length [`RigEmbeddingBackend::dimensions`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::RigError`] if the embedding computation fails.
    fn embed(&self, text: &str) -> Result<Vec<f32>>;

    /// Generate embedding vectors for a batch of text strings.
    ///
    /// The default implementation calls [`RigEmbeddingBackend::embed`] for
    /// each text sequentially. Backends that support provider-side batching
    /// override this for better throughput.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::RigError`] if any embedding in the batch
    /// fails.
    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        texts.iter().map(|t| self.embed(t)).collect()
    }

    /// Returns the dimensionality of the embedding vectors produced.
    ///
    /// A value of `0` indicates the backend is disabled and will return
    /// empty vectors.
    fn dimensions(&self) -> usize;

    /// Returns the human-readable name of this backend (e.g. `"rig-openai"`).
    fn name(&self) -> &str;

    /// Returns `true` if this backend can produce actual embeddings.
    ///
    /// Defaults to `dimensions() > 0`, matching ragent's
    /// `EmbeddingProvider::is_available` convention.
    fn is_available(&self) -> bool {
        self.dimensions() > 0
    }
}

// ── Blanket forwarding impls for shared-ownership smart pointers ───────────
//
// These let a `Box<dyn RigEmbeddingBackend>` or `Arc<dyn RigEmbeddingBackend>`
// be used wherever a `dyn RigEmbeddingBackend` is expected — e.g. when the
// research augmentor (T-012) stores its embedding backend behind an `Arc` for
// sharing across clones but needs to hand a `Box<dyn RigEmbeddingBackend>` to
// `VectorStoreAdapter::new` when rebuilding the vector store on clone.

impl<T: RigEmbeddingBackend + ?Sized> RigEmbeddingBackend for std::sync::Arc<T> {
    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        (**self).embed(text)
    }

    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        (**self).embed_batch(texts)
    }

    fn dimensions(&self) -> usize {
        (**self).dimensions()
    }

    fn name(&self) -> &str {
        (**self).name()
    }

    fn is_available(&self) -> bool {
        (**self).is_available()
    }
}

impl<T: RigEmbeddingBackend + ?Sized> RigEmbeddingBackend for std::boxed::Box<T> {
    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        (**self).embed(text)
    }

    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        (**self).embed_batch(texts)
    }

    fn dimensions(&self) -> usize {
        (**self).dimensions()
    }

    fn name(&self) -> &str {
        (**self).name()
    }

    fn is_available(&self) -> bool {
        (**self).is_available()
    }
}

/// A thin metadata handle to a Rig `EmbeddingModel` instance.
///
/// This struct holds the configuration needed to construct a concrete
/// [`RigEmbeddingBackend`] in T-007. It is separate from the trait so that
/// configuration parsing (T-006) can produce an `EmbeddingAdapter` without
/// needing `rig-core` to be available at parse time.
#[derive(Debug)]
pub struct EmbeddingAdapter {
    provider_alias: String,
    model: String,
    dimensions: usize,
}

impl EmbeddingAdapter {
    /// Creates a new embedding adapter from a provider alias and model name.
    ///
    /// The dimensionality defaults to `1536` (the dimensionality of
    /// `text-embedding-3-small`); T-007 resolves the true dimensionality
    /// from the live model metadata when the backend is constructed.
    ///
    /// # Errors
    ///
    /// Returns [`RigError::ProviderAliasNotFound`] if the alias is empty,
    /// or [`RigError::InvalidConfiguration`] if the model name is empty.
    pub fn new(provider_alias: &str, model: &str) -> Result<Self> {
        if provider_alias.is_empty() {
            return Err(RigError::ProviderAliasNotFound(
                "embedding provider alias must not be empty".to_owned(),
            ));
        }
        if model.is_empty() {
            return Err(RigError::InvalidConfiguration(
                "embedding model name must not be empty".to_owned(),
            ));
        }
        Ok(Self {
            provider_alias: provider_alias.to_owned(),
            model: model.to_owned(),
            // Placeholder: `text-embedding-3-small` is 1536 dimensions.
            dimensions: 1536,
        })
    }

    /// Returns the provider alias this adapter was constructed with.
    pub fn provider_alias(&self) -> &str {
        &self.provider_alias
    }

    /// Returns the model name this adapter was constructed with.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Returns the configured embedding dimensionality.
    pub fn dimensions(&self) -> usize {
        self.dimensions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_stores_metadata() {
        let adapter =
            EmbeddingAdapter::new("rig-openai", "text-embedding-3-small").expect("create adapter");
        assert_eq!(adapter.provider_alias(), "rig-openai");
        assert_eq!(adapter.model(), "text-embedding-3-small");
        assert_eq!(adapter.dimensions(), 1536);
    }

    #[test]
    fn empty_alias_is_rejected() {
        let err = EmbeddingAdapter::new("", "text-embedding-3-small").expect_err("expected error");
        assert!(matches!(err, RigError::ProviderAliasNotFound(_)));
    }

    #[test]
    fn empty_model_is_rejected() {
        let err = EmbeddingAdapter::new("rig-openai", "").expect_err("expected error");
        assert!(matches!(err, RigError::InvalidConfiguration(_)));
    }

    /// A minimal mock backend used to verify the trait is object-safe.
    struct StubBackend {
        dims: usize,
    }

    impl RigEmbeddingBackend for StubBackend {
        fn embed(&self, text: &str) -> Result<Vec<f32>> {
            Ok(vec![text.len() as f32; self.dims])
        }

        fn dimensions(&self) -> usize {
            self.dims
        }

        fn name(&self) -> &str {
            "stub"
        }
    }

    #[test]
    fn boxed_backend_is_object_safe() {
        let backend: Box<dyn RigEmbeddingBackend> = Box::new(StubBackend { dims: 3 });
        assert_eq!(backend.dimensions(), 3);
        assert!(backend.is_available());
        assert_eq!(backend.name(), "stub");

        let vec = backend.embed("hello").expect("embed");
        assert_eq!(vec.len(), 3);
        assert_eq!(vec[0], 5.0); // "hello".len() == 5
    }

    #[test]
    fn embed_batch_uses_default_sequential_impl() {
        let backend = StubBackend { dims: 2 };
        let vecs = backend.embed_batch(&["a", "bb"]).expect("batch");
        assert_eq!(vecs.len(), 2);
        assert_eq!(vecs[0], vec![1.0, 1.0]); // "a".len() == 1
        assert_eq!(vecs[1], vec![2.0, 2.0]); // "bb".len() == 2
    }

    #[test]
    fn zero_dim_backend_is_not_available() {
        let backend = StubBackend { dims: 0 };
        assert!(!backend.is_available());
    }
}
