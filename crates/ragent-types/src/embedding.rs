//! Shared embedding serialisation helpers.
//!
//! These functions encode and decode `Vec<f32>` embeddings as little-endian
//! IEEE-754 byte blobs for SQLite BLOB storage. They live in `ragent-types`
//! (the lowest shared crate) so that both `ragent-storage` and
//! `ragent-tools-extended` can use a single implementation without creating a
//! circular dependency.

/// Serialise a `Vec<f32>` embedding into a byte blob for SQLite BLOB storage.
///
/// Each `f32` is stored in little-endian IEEE 754 format (4 bytes per value).
///
/// # Examples
///
/// ```
/// use ragent_types::embedding::{deserialise_embedding, serialise_embedding};
///
/// let vec = vec![1.0_f32, -2.5, 3.14];
/// let blob = serialise_embedding(&vec);
/// let recovered = deserialise_embedding(&blob, 3).unwrap();
/// assert_eq!(vec, recovered);
/// ```
#[must_use]
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
/// use ragent_types::embedding::{deserialise_embedding, serialise_embedding};
///
/// let vec = vec![0.0_f32, 1.0, 2.0];
/// let blob = serialise_embedding(&vec);
/// assert!(deserialise_embedding(&blob, 3).is_ok());
/// assert!(deserialise_embedding(&blob, 4).is_err()); // wrong dimensions
/// ```
pub fn deserialise_embedding(blob: &[u8], dimensions: usize) -> anyhow::Result<Vec<f32>> {
    if blob.len() != dimensions * 4 {
        anyhow::bail!(
            "Embedding blob length {} does not match expected {} bytes ({} dims × 4)",
            blob.len(),
            dimensions * 4,
            dimensions
        );
    }
    let mut vec = Vec::with_capacity(dimensions);
    for chunk in blob.as_chunks::<4>().0 {
        let val = f32::from_le_bytes(*chunk);
        vec.push(val);
    }
    Ok(vec)
}
