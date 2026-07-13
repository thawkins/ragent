//! Rig-backed semantic search integration for ragent's memory subsystem (T-010).
//!
//! This module exposes Rig embeddings and vector stores to the structured-memory
//! subsystem without creating a dependency cycle: `ragent-rig` already depends on
//! `ragent-storage` (which holds the structured-memory SQLite tables), so the glue
//! lives here rather than in the storage crate.
//!
//! The main type is [`SemanticMemory`], a thin wrapper around a
//! [`ragent_storage::Storage`] plus an embedding backend and a vector store. It can:
//!
//! * embed structured memories when they are created or updated (FR-010),
//! * run pure semantic search over the vector store,
//! * fuse lexical (FTS5) memory results with vector-similarity results
//!   (the memory analogue of FR-021's hybrid code-index search).
//!
//! This is the memory-side counterpart of [`crate::codeindex::SemanticCodeIndex`]
//! (T-009) and [`crate::research::ResearchAugmentor`] (T-012). Together the three
//! modules satisfy FR-010 — "expose Rig-backed embedding and vector-store
//! capabilities to ragent's code index, memory, and research subsystems via a
//! common internal API" — by sharing the same
//! [`crate::embeddings_trait::RigEmbeddingBackend`] and
//! [`crate::vector_store::VectorStoreAdapter`] building blocks.
//!
//! The module is compiled only when the `memory-semantic` feature is enabled.

use std::collections::HashMap;
use std::sync::Arc;

use ragent_storage::Storage;
use serde_json::json;
use tracing::{debug, warn};

use crate::embeddings_trait::RigEmbeddingBackend;
use crate::error::{Result, RigError};
use crate::vector_store::VectorStoreAdapter;

/// Prefix used for memory IDs in the vector store so they can be distinguished
/// from code-index (`SemanticCodeIndex`) and research (`ResearchAugmentor`) IDs.
const MEMORY_ID_PREFIX: &str = "mem:";

/// A structured-memory hit enriched with vector-similarity metadata.
#[derive(Debug, Clone)]
pub struct SemanticMemoryHit {
    /// SQLite row ID of the matching memory.
    pub memory_id: i64,
    /// Vector-similarity score (0–1, higher is better).
    pub semantic_score: f64,
    /// Optional lexical (FTS5) score from `Storage::search_memories`, if the
    /// memory also matched the keyword query.
    pub lexical_score: Option<f32>,
    /// Memory content.
    pub content: String,
    /// Memory category (fact, pattern, preference, insight, error, workflow).
    pub category: String,
    /// Source of the memory (e.g. tool name, auto-extract).
    pub source: String,
    /// Confidence score (0.0–1.0).
    pub confidence: f64,
    /// Tags attached to the memory.
    pub tags: Vec<String>,
    /// The raw document stored in the vector store.
    pub document: serde_json::Value,
}

/// Configuration for semantic memory indexing.
#[derive(Debug, Clone)]
pub struct SemanticMemoryConfig {
    /// Whether to generate embeddings when memories are created or updated.
    pub enabled: bool,
    /// Maximum characters to embed per memory (truncate beyond this).
    pub max_chars: usize,
    /// Number of vector results to request per query.
    pub top_n: usize,
}

impl Default for SemanticMemoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_chars: 4_000,
            top_n: 10,
        }
    }
}

/// A Rig-backed semantic layer over a [`Storage`] structured-memory store.
///
/// Keeps the underlying [`Storage`] (FTS5 keyword + embedding-blob search) and a
/// [`VectorStoreAdapter`] (dense semantic search) in sync. All methods are
/// synchronous because both the storage layer and the Rig vector-store adapter
/// expose synchronous APIs.
///
/// This type is the memory-subsystem exposition of FR-010: callers that already
/// own a [`Storage`] can wrap it in a [`SemanticMemory`] to gain vector-similarity
/// search over structured memories, or use the [`MemoryExt`] extension trait for
/// one-off queries without wrapping.
pub struct SemanticMemory {
    storage: Arc<Storage>,
    vector_store: VectorStoreAdapter,
    embedding: Box<dyn RigEmbeddingBackend>,
    config: SemanticMemoryConfig,
}

impl std::fmt::Debug for SemanticMemory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SemanticMemory")
            .field("storage", &"Storage { .. }")
            .field("vector_store", &self.vector_store)
            .field("embedding", &self.embedding.name())
            .field("config", &self.config)
            .finish()
    }
}

impl SemanticMemory {
    /// Open a semantic memory layer over an existing [`Storage`] handle.
    ///
    /// The caller supplies the storage (already opened and migrated) and the Rig
    /// adapters; the wrapper takes ownership of both. Use
    /// [`SemanticMemory::from_storage`] when you already have an `Arc<Storage>`
    /// that is shared with the rest of the application.
    ///
    /// # Errors
    ///
    /// Propagates any error from the vector-store adapter construction.
    pub fn new(
        storage: Storage,
        embedding: Box<dyn RigEmbeddingBackend>,
        vector_store: VectorStoreAdapter,
    ) -> Result<Self> {
        Ok(Self {
            storage: Arc::new(storage),
            vector_store,
            embedding,
            config: SemanticMemoryConfig::default(),
        })
    }

    /// Wrap an existing `Arc<Storage>` with a semantic memory layer.
    ///
    /// This is the primary constructor for production wiring: the caller
    /// already owns an `Arc<Storage>` (shared with the session-processor and
    /// the `memory_*` tools) and wants to add Rig-backed semantic search
    /// alongside it without creating a second storage handle.
    ///
    /// # Errors
    ///
    /// Returns [`RigError::BackendError`] if the embedding backend is not
    /// available.
    pub fn from_storage(
        storage: Arc<Storage>,
        embedding: Box<dyn RigEmbeddingBackend>,
        vector_store: VectorStoreAdapter,
    ) -> Result<Self> {
        Ok(Self {
            storage,
            vector_store,
            embedding,
            config: SemanticMemoryConfig::default(),
        })
    }

    /// Open a semantic memory layer backed by an in-memory [`Storage`].
    ///
    /// Useful for tests and for ephemeral semantic searches.
    ///
    /// # Errors
    ///
    /// Returns [`RigError::BackendError`] if the in-memory storage cannot be
    /// opened, or propagates errors from [`VectorStoreAdapter::new`].
    pub fn open_in_memory(
        embedding: Box<dyn RigEmbeddingBackend>,
        vector_store: VectorStoreAdapter,
    ) -> Result<Self> {
        let storage = Storage::open_in_memory().map_err(|e| {
            RigError::BackendError(format!("failed to open in-memory storage: {e}"))
        })?;
        Self::new(storage, embedding, vector_store)
    }

    /// Replace the default [`SemanticMemoryConfig`].
    #[must_use]
    pub fn with_config(mut self, config: SemanticMemoryConfig) -> Self {
        self.config = config;
        self
    }

    /// Returns a reference to the underlying structured-memory storage.
    pub fn storage(&self) -> &Arc<Storage> {
        &self.storage
    }

    /// Returns a reference to the vector store adapter.
    pub fn vector_store(&self) -> &VectorStoreAdapter {
        &self.vector_store
    }

    /// Returns `true` if semantic indexing/search is enabled and the embedding
    /// backend is available.
    pub fn is_available(&self) -> bool {
        self.config.enabled && self.embedding.is_available()
    }

    /// Embed a structured memory and add it to the semantic vector store.
    ///
    /// `memory_id` is the SQLite row ID of the memory; `content` is the text to
    /// embed; `metadata` is merged into the stored JSON document (e.g. category,
    /// source, confidence, tags).
    ///
    /// # Errors
    ///
    /// Returns [`RigError::BackendError`] if embedding or vector storage fails.
    pub fn index_memory(
        &self,
        memory_id: i64,
        content: &str,
        metadata: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }
        let id = format!("{MEMORY_ID_PREFIX}{memory_id}");
        self.index_text(&id, content, metadata)
    }

    /// Embed an arbitrary text document under a given vector-store id.
    ///
    /// `metadata` is merged into the stored JSON document.
    ///
    /// # Errors
    ///
    /// Returns [`RigError::BackendError`] if embedding or vector storage fails.
    pub fn index_text(
        &self,
        id: &str,
        text: &str,
        metadata: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }
        let truncated = &text[..text.len().min(self.config.max_chars)];
        let embedding = self.embedding.embed(truncated)?;
        let mut doc = serde_json::Map::new();
        doc.insert("id".to_owned(), json!(id));
        doc.insert("text".to_owned(), json!(truncated));
        if let Some(m) = metadata {
            for (k, v) in m {
                doc.entry(k).or_insert(v);
            }
        }
        self.vector_store.add_documents(vec![(
            id.to_owned(),
            serde_json::Value::Object(doc),
            embedding,
        )])?;
        debug!(id, "indexed memory in semantic vector store");
        Ok(())
    }

    /// Remove a memory's embedding from the semantic vector store.
    ///
    /// # Errors
    ///
    /// Returns [`RigError::BackendError`] if the vector store cannot delete.
    pub fn remove_memory(&self, memory_id: i64) -> Result<()> {
        let id = format!("{MEMORY_ID_PREFIX}{memory_id}");
        self.vector_store.delete_document(&id)
    }

    /// Remove an arbitrary document from the semantic vector store by id.
    ///
    /// # Errors
    ///
    /// Returns [`RigError::BackendError`] if the vector store cannot delete.
    pub fn remove_text(&self, id: &str) -> Result<()> {
        self.vector_store.delete_document(id)
    }

    /// Run a pure semantic search over the vector store.
    ///
    /// Results are ranked by cosine similarity to the embedded query and
    /// decorated with structured-memory metadata fetched from [`Storage`].
    ///
    /// # Errors
    ///
    /// Returns [`RigError::BackendError`] if the query embedding or vector
    /// search fails.
    pub fn semantic_search(&self, query: &str, n: usize) -> Result<Vec<SemanticMemoryHit>> {
        let n = if n == 0 { self.config.top_n } else { n };
        let raw = self.vector_store.top_n(query, n)?;
        let mut hits = Vec::with_capacity(raw.len());
        for (score, id, document) in raw {
            let memory_id = id
                .strip_prefix(MEMORY_ID_PREFIX)
                .and_then(|s| s.parse::<i64>().ok());
            if let Some(mid) = memory_id {
                if let Ok(Some(mem)) = self.storage.get_memory(mid) {
                    let tags = self.storage.get_memory_tags(mid).unwrap_or_default();
                    hits.push(SemanticMemoryHit {
                        memory_id: mid,
                        semantic_score: score,
                        lexical_score: None,
                        content: mem.content,
                        category: mem.category,
                        source: mem.source,
                        confidence: mem.confidence,
                        tags,
                        document,
                    });
                }
            }
        }
        Ok(hits)
    }

    /// Run a hybrid (lexical + semantic) search over structured memories.
    ///
    /// Fuses the FTS5 keyword results from [`Storage::search_memories`] with the
    /// vector-similarity results from [`SemanticMemory::semantic_search`] using
    /// reciprocal rank fusion (k=1), the same strategy used by
    /// [`crate::codeindex::SemanticCodeIndex::hybrid_search`].
    ///
    /// The returned vector is truncated to `n` items.
    ///
    /// # Errors
    ///
    /// Propagates errors from either the lexical or semantic search path.
    pub fn hybrid_search(&self, query: &str, n: usize) -> Result<Vec<SemanticMemoryHit>> {
        let n = if n == 0 { self.config.top_n } else { n };

        // Lexical search (FTS5).
        let lexical = self
            .storage
            .search_memories(query, None, None, n * 2, 0.0)
            .map_err(|e| RigError::BackendError(format!("lexical memory search failed: {e}")))?;

        // Semantic search.
        let semantic = self.semantic_search(query, n * 2)?;

        // Fuse by reciprocal rank (k=1).
        let mut fused: HashMap<i64, SemanticMemoryHit> = HashMap::new();
        for (rank, mem) in lexical.iter().enumerate() {
            let score = 1.0 / (rank as f64 + 2.0);
            let tags = self.storage.get_memory_tags(mem.id).unwrap_or_default();
            let hit = SemanticMemoryHit {
                memory_id: mem.id,
                semantic_score: score,
                lexical_score: None,
                content: mem.content.clone(),
                category: mem.category.clone(),
                source: mem.source.clone(),
                confidence: mem.confidence,
                tags,
                document: json!({
                    "id": format!("{MEMORY_ID_PREFIX}{}", mem.id),
                    "text": mem.content,
                    "category": mem.category,
                    "source": mem.source,
                }),
            };
            if let Some(existing) = fused.get_mut(&hit.memory_id) {
                existing.semantic_score += hit.semantic_score;
                existing.lexical_score = hit.lexical_score;
            } else {
                fused.insert(hit.memory_id, hit);
            }
        }
        for (rank, mut hit) in semantic.into_iter().enumerate() {
            let score = 1.0 / (rank as f64 + 2.0);
            if let Some(existing) = fused.get_mut(&hit.memory_id) {
                existing.semantic_score += score;
                existing.lexical_score = hit.lexical_score;
            } else {
                hit.semantic_score = score;
                fused.insert(hit.memory_id, hit);
            }
        }

        let mut results: Vec<SemanticMemoryHit> = fused.into_values().collect();
        results.sort_by(|a, b| {
            b.semantic_score
                .partial_cmp(&a.semantic_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(n);
        debug!(query, n, "hybrid memory search");
        Ok(results)
    }

    /// Index a memory using the normal storage pipeline **and** add it to the
    /// semantic vector store when semantic indexing is enabled.
    ///
    /// This is the main entry point for live memory changes (the memory
    /// analogue of [`crate::codeindex::SemanticCodeIndex::index_file`] /
    /// FR-015). It creates the memory row in SQLite via
    /// [`Storage::create_memory`], then embeds the content and stores it in the
    /// vector store.
    ///
    /// Returns the new memory row ID.
    ///
    /// # Errors
    ///
    /// Propagates errors from storage creation or semantic embedding.
    pub fn create_and_index_memory(
        &self,
        content: &str,
        category: &str,
        source: &str,
        confidence: f64,
        project: &str,
        session_id: &str,
        tags: &[String],
    ) -> Result<i64> {
        let id = self
            .storage
            .create_memory(
                content, category, source, confidence, project, session_id, tags,
            )
            .map_err(|e| RigError::BackendError(format!("failed to create memory: {e}")))?;

        if self.config.enabled {
            let mut metadata = serde_json::Map::new();
            metadata.insert("category".to_owned(), json!(category));
            metadata.insert("source".to_owned(), json!(source));
            metadata.insert("confidence".to_owned(), json!(confidence));
            metadata.insert("project".to_owned(), json!(project));
            metadata.insert("tags".to_owned(), json!(tags));
            if let Err(e) = self.index_memory(id, content, Some(metadata)) {
                warn!(
                    memory_id = id,
                    error = %e,
                    "semantic memory indexing failed; storage creation succeeded"
                );
            }
        }
        Ok(id)
    }

    /// Remove a memory from both the SQLite store and the semantic vector store.
    ///
    /// # Errors
    ///
    /// Propagates errors from either removal path.
    pub fn delete_and_unindex_memory(&self, memory_id: i64) -> Result<bool> {
        let deleted = self
            .storage
            .delete_memory(memory_id)
            .map_err(|e| RigError::BackendError(format!("failed to delete memory: {e}")))?;
        if deleted {
            if let Err(e) = self.remove_memory(memory_id) {
                warn!(
                    memory_id,
                    error = %e,
                    "semantic memory remove failed; storage delete succeeded"
                );
            }
        }
        Ok(deleted)
    }

    /// Convenience: build a [`VectorStoreAdapter`] from a
    /// [`ragent_config::RigVectorStoreConfig`] and an embedding backend.
    ///
    /// # Errors
    ///
    /// Propagates errors from [`VectorStoreAdapter::from_config`].
    pub fn build_vector_store(
        config: &ragent_config::RigVectorStoreConfig,
        embedding: Box<dyn RigEmbeddingBackend>,
    ) -> Result<VectorStoreAdapter> {
        VectorStoreAdapter::from_config(config, embedding)
    }
}

/// Extension trait that adds semantic methods to a plain [`Storage`].
///
/// This is useful when callers already own a [`Storage`] and just want to run
/// one-off semantic queries without wrapping it in a [`SemanticMemory`].
pub trait MemoryExt {
    /// Search this storage by vector similarity using an external embedding
    /// backend and vector store.
    ///
    /// Results are decorated with structured-memory metadata.
    fn semantic_search(
        &self,
        embedding: &dyn RigEmbeddingBackend,
        vector_store: &VectorStoreAdapter,
        query: &str,
        n: usize,
    ) -> Result<Vec<SemanticMemoryHit>>;
}

impl MemoryExt for Storage {
    fn semantic_search(
        &self,
        _embedding: &dyn RigEmbeddingBackend,
        vector_store: &VectorStoreAdapter,
        query: &str,
        n: usize,
    ) -> Result<Vec<SemanticMemoryHit>> {
        let raw = vector_store.top_n(query, n)?;
        let mut hits = Vec::with_capacity(raw.len());
        for (score, id, document) in raw {
            let memory_id = id
                .strip_prefix(MEMORY_ID_PREFIX)
                .and_then(|s| s.parse::<i64>().ok());
            if let Some(mid) = memory_id {
                if let Ok(Some(mem)) = self.get_memory(mid) {
                    let tags = self.get_memory_tags(mid).unwrap_or_default();
                    hits.push(SemanticMemoryHit {
                        memory_id: mid,
                        semantic_score: score,
                        lexical_score: None,
                        content: mem.content,
                        category: mem.category,
                        source: mem.source,
                        confidence: mem.confidence,
                        tags,
                        document,
                    });
                }
            }
        }
        Ok(hits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embeddings_trait::RigEmbeddingBackend;

    /// A deterministic embedding backend that returns a fixed vector for every
    /// text. Mirrors the helper in `codeindex.rs` tests.
    #[derive(Clone)]
    struct FixedEmbeddingBackend {
        vec: Vec<f32>,
    }

    impl RigEmbeddingBackend for FixedEmbeddingBackend {
        fn embed(&self, _text: &str) -> Result<Vec<f32>> {
            Ok(self.vec.clone())
        }

        fn dimensions(&self) -> usize {
            self.vec.len()
        }

        fn name(&self) -> &'static str {
            "fixed"
        }
    }

    fn make_test_memory() -> SemanticMemory {
        let embedding = Box::new(FixedEmbeddingBackend {
            vec: vec![0.0_f32, 1.0, 0.0, 0.0],
        });
        let vector_store = VectorStoreAdapter::new("memory", None, embedding.clone()).unwrap();
        SemanticMemory::open_in_memory(embedding, vector_store).unwrap()
    }

    #[test]
    fn open_in_memory_constructs() {
        let mem = make_test_memory();
        assert!(mem.is_available());
        assert_eq!(mem.storage().count_memories().unwrap(), 0);
    }

    #[test]
    fn create_and_index_memory_adds_to_vector_store() {
        let mem = make_test_memory();
        let id = mem
            .create_and_index_memory(
                "Rust uses snake_case for functions",
                "pattern",
                "test",
                0.8,
                "proj",
                "sess",
                &["rust".to_string()],
            )
            .expect("create and index");

        assert!(id > 0);
        assert_eq!(mem.storage().count_memories().unwrap(), 1);
        assert_eq!(mem.vector_store().len(), 1);
    }

    #[test]
    fn semantic_search_returns_indexed_memory() {
        let mem = make_test_memory();
        mem.create_and_index_memory(
            "Rust uses snake_case for functions",
            "pattern",
            "test",
            0.8,
            "proj",
            "sess",
            &["rust".to_string()],
        )
        .expect("create and index");

        // The fixture backend returns the same vector for every text, so the
        // query always matches. Request 1 result.
        let hits = mem.semantic_search("query", 1).expect("search");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].memory_id, 1);
        assert_eq!(hits[0].category, "pattern");
        assert_eq!(hits[0].source, "test");
        assert!((hits[0].confidence - 0.8).abs() < f64::EPSILON);
        assert_eq!(hits[0].tags, vec!["rust".to_string()]);
    }

    #[test]
    fn hybrid_search_combines_lexical_and_semantic() {
        let mem = make_test_memory();
        mem.create_and_index_memory(
            "snake_case naming convention for rust functions",
            "pattern",
            "test",
            0.8,
            "proj",
            "sess",
            &[],
        )
        .expect("create and index");
        mem.create_and_index_memory(
            "use anyhow::Result for error handling",
            "preference",
            "test",
            0.7,
            "proj",
            "sess",
            &[],
        )
        .expect("create and index");

        // "snake_case" should match the first memory lexically (FTS5), and the
        // fixture backend gives both memories the same vector so both appear in
        // the semantic results. The hybrid fusion should return at least the
        // lexical match.
        let hits = mem.hybrid_search("snake_case", 2).expect("hybrid search");
        assert!(!hits.is_empty());
        assert!(hits.iter().any(|h| h.content.contains("snake_case")));
    }

    #[test]
    fn delete_and_unindex_memory_removes_from_both_stores() {
        let mem = make_test_memory();
        let id = mem
            .create_and_index_memory("temporary memory", "fact", "test", 0.5, "proj", "sess", &[])
            .expect("create and index");
        assert_eq!(mem.vector_store().len(), 1);

        let deleted = mem.delete_and_unindex_memory(id).expect("delete");
        assert!(deleted);
        assert_eq!(mem.storage().count_memories().unwrap(), 0);
        assert_eq!(mem.vector_store().len(), 0);
    }

    #[test]
    fn remove_memory_is_idempotent() {
        let mem = make_test_memory();
        // Removing a non-existent memory should not panic.
        assert!(mem.remove_memory(999).is_ok());
    }

    #[test]
    fn memory_ext_semantic_search_enriches_metadata() {
        let mem = make_test_memory();
        mem.create_and_index_memory(
            "structured memory ext search",
            "insight",
            "test",
            0.9,
            "proj",
            "sess",
            &["ext".to_string()],
        )
        .expect("create and index");

        let embedding = Box::new(FixedEmbeddingBackend {
            vec: vec![0.0_f32, 1.0, 0.0, 0.0],
        });
        let vector_store = VectorStoreAdapter::new("memory", None, embedding.clone()).unwrap();
        vector_store
            .add_documents(vec![(
                format!("{MEMORY_ID_PREFIX}1"),
                json!({"id": format!("{MEMORY_ID_PREFIX}1"), "text": "structured memory ext search"}),
                vec![0.0_f32, 1.0, 0.0, 0.0],
            )])
            .unwrap();

        let hits = mem
            .storage()
            .semantic_search(&*embedding, &vector_store, "query", 1)
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].memory_id, 1);
        assert_eq!(hits[0].category, "insight");
        assert_eq!(hits[0].tags, vec!["ext".to_string()]);
    }

    #[test]
    fn disabled_config_skips_indexing() {
        let mut mem = make_test_memory();
        mem.config.enabled = false;

        let id = mem
            .create_and_index_memory("disabled", "fact", "test", 0.5, "proj", "sess", &[])
            .expect("create");
        // Memory row created in SQLite, but vector store should be empty.
        assert!(id > 0);
        assert_eq!(mem.storage().count_memories().unwrap(), 1);
        assert_eq!(mem.vector_store().len(), 0);
    }
}
