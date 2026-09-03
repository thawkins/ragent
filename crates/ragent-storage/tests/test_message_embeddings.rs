#![allow(clippy::assert_is_empty)]
//! Tests for M5 session-message embedding storage helpers.

use ragent_storage::storage::Storage;
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
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

#[test]
fn test_message_embedding_round_trip() {
    let storage = Storage::open_in_memory().expect("storage");
    storage.create_session("sess-1", "/tmp").unwrap();
    let msg = Message::user_text("sess-1", "hello world");
    storage.create_message(&msg).unwrap();

    let embedding = vec![0.1_f32, 0.2, 0.3, 0.4];
    storage
        .store_message_embedding(&msg.id, &serialise_f32(&embedding), embedding.len())
        .unwrap();

    let recovered = storage
        .get_message_embedding(&msg.id, embedding.len())
        .unwrap();
    assert!(recovered.is_some());
    assert_eq!(recovered.unwrap(), embedding);
}

#[test]
fn test_message_embedding_search_by_similarity() {
    let storage = Storage::open_in_memory().expect("storage");
    storage.create_session("sess-1", "/tmp").unwrap();
    let msg_a = Message::user_text("sess-1", "database migration");
    let msg_b = Message::assistant_text("sess-1", "hello world");
    storage.create_message(&msg_a).unwrap();
    storage.create_message(&msg_b).unwrap();

    let dims = 3usize;
    let emb_a = vec![1.0_f32, 0.0, 0.0];
    let emb_b = vec![0.0_f32, 1.0, 0.0];
    storage
        .store_message_embedding(&msg_a.id, &serialise_f32(&emb_a), dims)
        .unwrap();
    storage
        .store_message_embedding(&msg_b.id, &serialise_f32(&emb_b), dims)
        .unwrap();

    let query = vec![0.9_f32, 0.1, 0.0];
    let results = storage
        .search_messages_by_embedding(&query, dims, 1, 0.0, cosine_similarity)
        .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].message_id, msg_a.id);
    assert!(results[0].score > 0.9);
}
