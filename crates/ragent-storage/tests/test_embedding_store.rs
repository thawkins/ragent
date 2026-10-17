//! Isolation coverage for the structured-memory and session-message embedding
//! helpers (`store_memory_embedding`, `list_memory_embeddings`,
//! `search_memories_by_embedding`, `get_message_embedding`,
//! `list_message_embeddings`).
//!
//! Embeddings are stored as little-endian `f32` blobs and searched by a
//! caller-supplied cosine-similarity closure. These tests exercise the happy
//! path, the minimum-similarity / limit filters, and the corrupt-blob skip.

use ragent_storage::storage::{EmbeddingMatch, Storage};
use ragent_types::message::Message;

fn serialise_f32(vec: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vec.len() * 4);
    for &val in vec {
        bytes.extend_from_slice(&val.to_le_bytes());
    }
    bytes
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na * nb)
    }
}

fn memory(storage: &Storage, content: &str) -> i64 {
    storage
        .create_memory(content, "fact", "test", 0.8, "/proj", "s1", &[])
        .expect("create memory")
}

#[test]
fn memory_embedding_store_list_and_search() {
    let storage = Storage::open_in_memory().expect("storage");
    let a = memory(&storage, "alpha memory");
    let b = memory(&storage, "beta memory");

    let dims = 4usize;
    let emb_a = vec![1.0_f32, 0.0, 0.0, 0.0];
    let emb_b = vec![0.0_f32, 1.0, 0.0, 0.0];
    assert!(
        storage
            .store_memory_embedding(a, &serialise_f32(&emb_a))
            .expect("store a")
    );
    assert!(
        storage
            .store_memory_embedding(b, &serialise_f32(&emb_b))
            .expect("store b")
    );

    let all = storage.list_memory_embeddings().expect("list");
    assert_eq!(all.len(), 2);
    let ids: Vec<i64> = all.iter().map(|(id, _)| *id).collect();
    assert!(ids.contains(&a) && ids.contains(&b));

    let query = vec![0.95_f32, 0.05, 0.0, 0.0];
    let hits = storage
        .search_memories_by_embedding(&query, dims, 10, 0.0, cosine_similarity)
        .expect("search");
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].row_id, a, "closest match ranked first");
    assert!(hits[0].score > hits[1].score);

    // limit truncates.
    let limited = storage
        .search_memories_by_embedding(&query, dims, 1, 0.0, cosine_similarity)
        .expect("search limited");
    assert_eq!(limited.len(), 1);
    assert_eq!(limited[0].row_id, a);

    // A high min_similarity filters out the unrelated memory.
    let strict = storage
        .search_memories_by_embedding(&query, dims, 10, 0.9, cosine_similarity)
        .expect("search strict");
    assert_eq!(strict.len(), 1);
    assert_eq!(strict[0].row_id, a);
}

#[test]
fn store_memory_embedding_reports_no_row_for_missing_id() {
    let storage = Storage::open_in_memory().expect("storage");
    assert!(
        !storage
            .store_memory_embedding(9999, &serialise_f32(&[1.0, 0.0]))
            .expect("store missing")
    );
}

#[test]
fn search_skips_undecodable_embedding_blobs() {
    let storage = Storage::open_in_memory().expect("storage");
    let good = memory(&storage, "good memory");
    let bad = memory(&storage, "bad memory");

    let dims = 2usize;
    storage
        .store_memory_embedding(good, &serialise_f32(&[1.0, 0.0]))
        .expect("store good");
    // A 3-byte blob cannot decode into 2 little-endian f32s (needs 8 bytes).
    storage
        .store_memory_embedding(bad, &[1, 2, 3])
        .expect("store bad");

    let hits = storage
        .search_memories_by_embedding(&[1.0, 0.0], dims, 10, 0.0, cosine_similarity)
        .expect("search");
    assert_eq!(hits.len(), 1, "only the decodable blob is scored");
    assert_eq!(hits[0].row_id, good);
}

#[test]
fn message_embedding_wrong_dimension_returns_none() {
    let storage = Storage::open_in_memory().expect("storage");
    storage.create_session("sess-1", "/tmp").expect("session");
    let msg = Message::user_text("sess-1", "hello");
    storage.create_message(&msg).expect("create");
    storage
        .store_message_embedding(&msg.id, &serialise_f32(&[1.0, 0.0, 0.0]), 3)
        .expect("store");

    assert!(
        storage
            .get_message_embedding(&msg.id, 3)
            .expect("get")
            .is_some()
    );
    assert!(
        storage
            .get_message_embedding(&msg.id, 5)
            .expect("get other dims")
            .is_none(),
        "a dimensionality mismatch reads as absent"
    );
    assert!(
        storage
            .get_message_embedding("missing-id", 3)
            .expect("get missing")
            .is_none()
    );
}

#[test]
fn message_embedding_upsert_and_list() {
    let storage = Storage::open_in_memory().expect("storage");
    storage.create_session("sess-1", "/tmp").expect("session");
    let msg = Message::user_text("sess-1", "hello");
    storage.create_message(&msg).expect("create");

    storage
        .store_message_embedding(&msg.id, &serialise_f32(&[1.0, 0.0]), 2)
        .expect("store");
    // Upsert replaces the vector and dimensions.
    storage
        .store_message_embedding(&msg.id, &serialise_f32(&[0.0, 1.0, 0.5]), 3)
        .expect("upsert");

    let listed = storage.list_message_embeddings().expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].0, msg.id);
    assert_eq!(listed[0].2, 3, "dimensions updated by the upsert");
    assert_eq!(
        storage.get_message_embedding(&msg.id, 3).expect("get"),
        Some(vec![0.0_f32, 1.0, 0.5])
    );
}

#[test]
fn embedding_match_derives_debug() {
    // Exercise the public `EmbeddingMatch` shape so the type is fully covered.
    let m = EmbeddingMatch {
        row_id: 7,
        score: 0.5,
    };
    assert_eq!(m.row_id, 7);
    assert!((m.score - 0.5).abs() < f32::EPSILON);
    assert!(format!("{m:?}").contains("EmbeddingMatch"));
}
