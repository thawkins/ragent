//! Integration tests for Milestone 4 — Semantic Search (Embeddings).
//!
//! Tests the embedding provider trait, serialisation utilities, cosine
//! similarity, vector storage in SQLite, and the `memory_search` tool.

use std::path::PathBuf;
use std::sync::Arc;

use ragent_core::memory::embedding::{
    EmbeddingProvider, NoOpEmbedding, cosine_similarity, deserialise_embedding, serialise_embedding,
};
use ragent_core::storage::Storage;
use ragent_core::tool::{Tool, ToolContext};

/// Helper: create a ToolContext with in-memory storage.
fn make_ctx() -> ToolContext {
    let storage = Arc::new(Storage::open_in_memory().unwrap());
    ToolContext {
        session_id: "test-session".to_string(),
        working_dir: PathBuf::from("/tmp/test-project"),
        event_bus: Arc::new(ragent_core::event::EventBus::new(100)),
        storage: Some(storage),
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
        code_index: None,
    }
}

// ── NoOpEmbedding provider ────────────────────────────────────────────────────

#[test]
fn test_noop_provider_returns_empty_vectors() {
    let provider = NoOpEmbedding;
    assert!(!provider.is_available());
    assert_eq!(provider.dimensions(), 0);
    assert_eq!(provider.name(), "noop");
    assert!(provider.embed("hello").unwrap().is_empty());
}

#[test]
fn test_noop_batch_returns_empty() {
    let provider = NoOpEmbedding;
    let result = provider.embed_batch(&["a", "b"]).unwrap();
    assert!(result.is_empty());
}

// ── Cosine similarity ──────────────────────────────────────��──────────────────

#[test]
fn test_cosine_similarity_identical_vectors() {
    let a = vec![1.0, 0.0, 0.0];
    let b = vec![1.0, 0.0, 0.0];
    let sim = cosine_similarity(&a, &b);
    assert!((sim - 1.0).abs() < 1e-6);
}

#[test]
fn test_cosine_similarity_orthogonal_vectors() {
    let a = vec![1.0, 0.0, 0.0];
    let b = vec![0.0, 1.0, 0.0];
    let sim = cosine_similarity(&a, &b);
    assert!(sim.abs() < 1e-6);
}

#[test]
fn test_cosine_similarity_opposite_vectors() {
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
fn test_cosine_similarity_partial_overlap() {
    let a = vec![1.0, 1.0, 0.0];
    let b = vec![1.0, 0.0, 0.0];
    let sim = cosine_similarity(&a, &b);
    // cos(45°) = 1/sqrt(2) ≈ 0.707
    assert!((sim - 0.7071).abs() < 0.01);
}

// ── Embedding serialisation / deserialisation ─────────────────────────────────

#[test]
fn test_serialise_deserialise_roundtrip() {
    let vec = vec![1.0_f32, -2.5, 3.14, 0.0, f32::MIN_POSITIVE];
    let blob = serialise_embedding(&vec);
    assert_eq!(blob.len(), vec.len() * 4);
    let recovered = deserialise_embedding(&blob, vec.len()).unwrap();
    assert_eq!(vec, recovered);
}

#[test]
fn test_deserialise_wrong_dimensions() {
    let vec = vec![1.0_f32, 2.0];
    let blob = serialise_embedding(&vec);
    assert!(deserialise_embedding(&blob, 3).is_err());
}

#[test]
fn test_deserialise_invalid_blob_length() {
    let blob = vec![0u8, 1, 2]; // Not a multiple of 4
    assert!(deserialise_embedding(&blob, 1).is_err());
}

#[test]
fn test_serialise_empty_vector() {
    let vec: Vec<f32> = Vec::new();
    let blob = serialise_embedding(&vec);
    assert!(blob.is_empty());
    let recovered = deserialise_embedding(&blob, 0).unwrap();
    assert!(recovered.is_empty());
}

// ── SQLite embedding storage ──────────────────────────────────────────────────

#[test]
fn test_store_and_retrieve_memory_embedding() {
    let storage = Storage::open_in_memory().unwrap();

    // Create a memory to attach an embedding to.
    let id = storage
        .create_memory(
            "Test memory content",
            "fact",
            "manual",
            0.9,
            "test-project",
            "test-session",
            &["test".to_string()],
        )
        .unwrap();

    // Store an embedding.
    let embedding = vec![0.1_f32, 0.2, 0.3, 0.4];
    let blob = serialise_embedding(&embedding);
    let stored = storage.store_memory_embedding(id, &blob).unwrap();
    assert!(stored);

    // Retrieve and verify.
    let embeddings = storage.list_memory_embeddings().unwrap();
    assert_eq!(embeddings.len(), 1);
    assert_eq!(embeddings[0].0, id);

    let recovered = deserialise_embedding(&embeddings[0].1, 4).unwrap();
    assert_eq!(embedding, recovered);
}

#[test]
fn test_store_and_retrieve_journal_embedding() {
    let storage = Storage::open_in_memory().unwrap();

    // Create a journal entry.
    let id = uuid::Uuid::new_v4().to_string();
    storage
        .create_journal_entry(
            &id,
            "Test title",
            "Test content",
            "test-project",
            "test-session",
            &["test".to_string()],
        )
        .unwrap();

    // Store an embedding.
    let embedding = vec![1.0_f32, 0.0, 0.5];
    let blob = serialise_embedding(&embedding);
    let stored = storage.store_journal_embedding(&id, &blob).unwrap();
    assert!(stored);

    // Retrieve and verify.
    let embeddings = storage.list_journal_embeddings().unwrap();
    assert_eq!(embeddings.len(), 1);
    assert_eq!(embeddings[0].0, id);

    let recovered = deserialise_embedding(&embeddings[0].1, 3).unwrap();
    assert_eq!(embedding, recovered);
}

#[test]
fn test_search_memories_by_embedding() {
    let storage = Storage::open_in_memory().unwrap();

    // Create some memories.
    let id1 = storage
        .create_memory(
            "Rust uses Result for error handling",
            "pattern",
            "manual",
            0.9,
            "test",
            "s1",
            &["rust".to_string()],
        )
        .unwrap();
    let id2 = storage
        .create_memory(
            "Python uses try/except for error handling",
            "pattern",
            "manual",
            0.8,
            "test",
            "s1",
            &["python".to_string()],
        )
        .unwrap();

    // Store embeddings — make id1 similar to the query, id2 different.
    let embed1 = vec![1.0_f32, 0.0, 0.0]; // Same direction as query
    let embed2 = vec![0.0_f32, 1.0, 0.0]; // Orthogonal to query
    storage
        .store_memory_embedding(id1, &serialise_embedding(&embed1))
        .unwrap();
    storage
        .store_memory_embedding(id2, &serialise_embedding(&embed2))
        .unwrap();

    // Search with a query similar to embed1.
    let query = vec![1.0_f32, 0.0, 0.0];
    let results = storage
        .search_memories_by_embedding(&query, 3, 10, 0.5)
        .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].row_id, id1);
    assert!(results[0].score > 0.99);
}

#[test]
fn test_search_memories_by_embedding_min_similarity() {
    let storage = Storage::open_in_memory().unwrap();

    let id = storage
        .create_memory("test content", "fact", "manual", 0.5, "test", "s1", &[])
        .unwrap();

    // Store a weakly similar embedding.
    let embed = vec![0.5_f32, 0.5, 0.707]; // angle ≈ 30° from query
    storage
        .store_memory_embedding(id, &serialise_embedding(&embed))
        .unwrap();

    let query = vec![1.0_f32, 0.0, 0.0];

    // With low threshold, should find it.
    let results = storage
        .search_memories_by_embedding(&query, 3, 10, 0.3)
        .unwrap();
    assert_eq!(results.len(), 1);

    // With high threshold, should not.
    let results = storage
        .search_memories_by_embedding(&query, 3, 10, 0.99)
        .unwrap();
    assert!(results.is_empty());
}

// ── memory_search tool (FTS5 fallback, no embeddings) ──────────────────────────

#[tokio::test]
async fn test_memory_search_fts_fallback() {
    let ctx = make_ctx();
    let tool = ragent_core::tool::memory_search::MemorySearchTool;

    // Store a memory first.
    let store_tool = ragent_core::tool::structured_memory::MemoryStoreTool;
    store_tool
        .execute(
            serde_json::json!({
                "content": "Rust uses Result<T, E> for error handling",
                "category": "pattern",
                "tags": ["rust"],
                "confidence": 0.9
            }),
            &ctx,
        )
        .await
        .unwrap();

    // Search using memory_search (will use FTS5 since NoOpEmbedding).
    let input = serde_json::json!({
        "query": "Rust error handling",
        "scope": "memories"
    });

    let result = tool.execute(input, &ctx).await.unwrap();
    assert!(result.content.contains("Rust"));
}

#[tokio::test]
async fn test_memory_search_no_results() {
    let ctx = make_ctx();
    let tool = ragent_core::tool::memory_search::MemorySearchTool;

    let input = serde_json::json!({
        "query": "nonexistent topic xyz123"
    });

    let result = tool.execute(input, &ctx).await.unwrap();
    assert!(result.content.contains("No memories found"));
}

#[tokio::test]
async fn test_memory_search_no_storage() {
    let ctx = ToolContext {
        session_id: "test".to_string(),
        working_dir: PathBuf::from("/tmp"),
        event_bus: Arc::new(ragent_core::event::EventBus::new(100)),
        storage: None,
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
        code_index: None,
    };
    let tool = ragent_core::tool::memory_search::MemorySearchTool;

    let input = serde_json::json!({
        "query": "test"
    });

    let result = tool.execute(input, &ctx).await;
    assert!(result.is_err());
}
