//! Embedding provider trait and implementations for semantic search.
//!
//! The [`EmbeddingProvider`] trait abstracts the interface for generating
//! vector embeddings from text. Implementations range from a no-op fallback
//! (when embeddings are disabled) to a local ONNX-based sentence-transformer
//! (when the `embeddings` feature is enabled).
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────────┐
//! │   EmbeddingProvider      │  (trait)
//! │   - embed(text)         │
//! │   - embed_batch(texts)  │
//! │   - dimensions()        │
//! │   - name()              │
//! └──────┬───────────────────┘
//!        │
//!   ┌────┴─────┐
//!   │          │
//!   ▼          ▼
//! NoOp     Local (ort)
//! (empty)  (ONNX model)
//! ```
//!
//! # Feature flags
//!
//! - **Default** (`embeddings` disabled): only [`NoOpEmbedding`] is available.
//!   It returns empty vectors, signalling that semantic search is unavailable.
//!   `memory_search` and `journal_search` fall back to FTS5-only mode.
//!
//! - **`embeddings` feature enabled**: [`LocalEmbeddingProvider`] becomes
//!   available, using ONNX Runtime to run a `sentence-transformers` model
//!   (default: `all-MiniLM-L6-v2`, 384-dim vectors).

use anyhow::Result;

/// Abstract interface for generating text embeddings.
///
/// Implementations convert text into fixed-dimension floating-point vectors
/// that can be compared via cosine similarity for semantic search.
///
/// # Thread safety
///
/// All implementations must be `Send + Sync` so they can be shared across
/// async tasks and stored in `Arc`.
pub trait EmbeddingProvider: Send + Sync {
    /// Generate an embedding vector for a single text string.
    ///
    /// Returns a `Vec<f32>` of length [`EmbeddingProvider::dimensions`].
    /// If the provider is disabled or encounters an error, it should return
    /// an empty vector (length 0) rather than panicking.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedding computation fails irrecoverably
    /// (e.g., model not found, OOM).
    fn embed(&self, text: &str) -> Result<Vec<f32>>;

    /// Generate embedding vectors for a batch of text strings.
    ///
    /// The default implementation calls [`EmbeddingProvider::embed`] for each
    /// text sequentially. Implementations that support GPU batching should
    /// override this for better performance.
    ///
    /// # Errors
    ///
    /// Returns an error if any embedding in the batch fails.
    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        texts.iter().map(|t| self.embed(t)).collect()
    }

    /// Returns the dimensionality of the embedding vectors produced.
    ///
    /// A value of 0 indicates that this provider is disabled and will
    /// return empty vectors.
    fn dimensions(&self) -> usize;

    /// Returns the human-readable name of this provider (e.g., "noop", "ort-local").
    fn name(&self) -> &str;

    /// Returns `true` if this provider can produce actual embeddings.
    ///
    /// `NoOpEmbedding` returns `false`; all real providers return `true`.
    fn is_available(&self) -> bool {
        self.dimensions() > 0
    }
}

// ── NoOpEmbedding ─────────────────────────────────────────────────────────────

/// A no-op embedding provider that returns empty vectors.
///
/// Used when the `embeddings` feature is disabled or the user has not
/// enabled semantic search in their configuration. Tools detect empty
/// vectors and fall back to FTS5 keyword search.
///
/// # Examples
///
/// ```
/// use ragent_core::memory::embedding::{EmbeddingProvider, NoOpEmbedding};
///
/// let provider = NoOpEmbedding;
/// assert_eq!(provider.dimensions(), 0);
/// assert!(!provider.is_available());
/// assert!(provider.embed("hello").unwrap().is_empty());
/// ```
pub struct NoOpEmbedding;

impl EmbeddingProvider for NoOpEmbedding {
    fn embed(&self, _text: &str) -> Result<Vec<f32>> {
        Ok(Vec::new())
    }

    fn embed_batch(&self, _texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        Ok(Vec::new())
    }

    fn dimensions(&self) -> usize {
        0
    }

    fn name(&self) -> &str {
        "noop"
    }
}

// ── Utility functions ─────────────────────────────────────────────────────────

/// Compute the cosine similarity between two embedding vectors.
///
/// Returns a value in `[-1.0, 1.0]` where `1.0` means identical direction.
/// Returns `0.0` if either vector has zero magnitude (all zeros).
///
/// # Panics
///
/// Panics if the vectors have different lengths.
///
/// # Examples
///
/// ```
/// use ragent_core::memory::embedding::cosine_similarity;
///
/// let a = vec![1.0, 0.0, 0.0];
/// let b = vec![1.0, 0.0, 0.0];
/// assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);
///
/// let c = vec![0.0, 1.0, 0.0];
/// assert!((cosine_similarity(&a, &c)).abs() < 1e-6);
/// ```
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(
        a.len(),
        b.len(),
        "Embedding vectors must have the same length"
    );
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

/// Serialise a `Vec<f32>` embedding into a byte blob for SQLite BLOB storage.
///
/// Each `f32` is stored in little-endian IEEE 754 format (4 bytes per value).
///
/// # Examples
///
/// ```
/// use ragent_core::memory::embedding::{serialise_embedding, deserialise_embedding};
///
/// let vec = vec![1.0_f32, -2.5, 3.14];
/// let blob = serialise_embedding(&vec);
/// let recovered = deserialise_embedding(&blob, 3).unwrap();
/// assert_eq!(vec, recovered);
/// ```
pub fn serialise_embedding(vec: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vec.len() * 4);
    for &val in vec {
        bytes.extend_from_slice(&val.to_le_bytes());
    }
    bytes
}

/// Deserialise a byte blob back into a `Vec<f32>`.
///
/// # Errors
///
/// Returns an error if the blob length is not a multiple of 4 bytes or
/// does not match the expected `dimensions`.
///
/// # Examples
///
/// ```
/// use ragent_core::memory::embedding::{serialise_embedding, deserialise_embedding};
///
/// let vec = vec![0.0_f32, 1.0, 2.0];
/// let blob = serialise_embedding(&vec);
/// assert!(deserialise_embedding(&blob, 3).is_ok());
/// assert!(deserialise_embedding(&blob, 4).is_err()); // wrong dimensions
/// ```
pub fn deserialise_embedding(blob: &[u8], dimensions: usize) -> Result<Vec<f32>> {
    if blob.len() != dimensions * 4 {
        anyhow::bail!(
            "Embedding blob length {} does not match expected {} bytes ({} dims × 4)",
            blob.len(),
            dimensions * 4,
            dimensions
        );
    }
    let mut vec = Vec::with_capacity(dimensions);
    for chunk in blob.chunks_exact(4) {
        let val = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        vec.push(val);
    }
    Ok(vec)
}

/// A scored search result from semantic search.
///
/// Pairs an item's row ID with its cosine similarity score to the query.
#[derive(Debug, Clone)]
pub struct SimilarityResult {
    /// Row ID of the matching memory or journal entry.
    pub row_id: i64,
    /// Cosine similarity score in `[-1.0, 1.0]`. Higher = more similar.
    pub score: f32,
}

// ── Local ONNX Embedding Provider (feature-gated) ────────────────────────────

#[cfg(feature = "embeddings")]
mod local;
#[cfg(feature = "embeddings")]
pub use local::LocalEmbeddingProvider;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noop_embed_returns_empty() {
        let provider = NoOpEmbedding;
        assert_eq!(provider.embed("hello").unwrap(), Vec::<f32>::new());
    }

    #[test]
    fn test_noop_batch_returns_empty() {
        let provider = NoOpEmbedding;
        assert_eq!(
            provider.embed_batch(&["a", "b"]).unwrap(),
            Vec::<Vec<f32>>::new()
        );
    }

    #[test]
    fn test_noop_dimensions_zero() {
        assert_eq!(NoOpEmbedding.dimensions(), 0);
        assert!(!NoOpEmbedding.is_available());
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim + 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_zero_vector() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 2.0, 3.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn test_serialise_deserialise_roundtrip() {
        let vec = vec![1.0_f32, -2.5, 3.14, 0.0, f32::MAX];
        let blob = serialise_embedding(&vec);
        let recovered = deserialise_embedding(&blob, 5).unwrap();
        assert_eq!(vec, recovered);
    }

    #[test]
    fn test_deserialise_wrong_dimensions() {
        let vec = vec![1.0_f32, 2.0];
        let blob = serialise_embedding(&vec);
        assert!(deserialise_embedding(&blob, 3).is_err());
    }

    #[test]
    fn test_deserialise_invalid_blob() {
        let blob = vec![0u8, 1, 2]; // Not a multiple of 4
        assert!(deserialise_embedding(&blob, 1).is_err());
    }
}
