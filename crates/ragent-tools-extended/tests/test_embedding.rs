//! External tests for `tests` from `crates/ragent-tools-extended/src/memory/embedding.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_tools_extended::memory::embedding::*;

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
    let vec = vec![1.0_f32, -2.5, std::f32::consts::PI, 0.0, f32::MAX];
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
