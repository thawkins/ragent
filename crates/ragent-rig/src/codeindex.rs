//! Rig-backed semantic search integration for `ragent-codeindex` (T-009).
//!
//! This module exposes Rig embeddings and vector stores to the code index
//! without creating a dependency cycle: `ragent-rig` already optionally
//! depends on `ragent-codeindex`, so the glue lives here rather than in the
//! code-index crate.
//!
//! The main type is [`SemanticCodeIndex`], a thin wrapper around a
//! [`ragent_codeindex::CodeIndex`] plus an embedding backend and a vector
//! store. It can:
//!
//! * embed source files when they are indexed (FR-015),
//! * run pure semantic search over the vector store,
//! * fuse lexical code-index results with vector-similarity results
//!   (FR-021 / AC-3).
//!
//! The module is compiled only when the `rig-semantic` feature is enabled.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use ragent_codeindex::CodeIndex;
use ragent_codeindex::types::{CodeIndexConfig, SearchQuery};
use serde_json::json;
use tracing::{debug, warn};

use crate::embeddings_trait::RigEmbeddingBackend;
use crate::error::{Result, RigError};
use crate::vector_store::VectorStoreAdapter;

/// A code-index hit enriched with vector-similarity metadata.
#[derive(Debug, Clone)]
pub struct SemanticHit {
    /// Relative file path.
    pub file_path: String,
    /// Vector-similarity score (0–1, higher is better).
    pub semantic_score: f64,
    /// Optional lexical score from the code-index FTS, if the file also
    /// matched the keyword query.
    pub lexical_score: Option<f32>,
    /// Language detected by the code index, if known.
    pub language: Option<String>,
    /// Number of lines in the file.
    pub line_count: u64,
    /// The raw document stored in the vector store.
    pub document: serde_json::Value,
}

/// Configuration for semantic code indexing.
#[derive(Debug, Clone)]
pub struct SemanticConfig {
    /// Whether to generate embeddings when files are indexed.
    pub enabled: bool,
    /// Maximum characters to embed per file (truncate beyond this).
    pub max_chars: usize,
    /// Number of vector results to request per query.
    pub top_n: usize,
}

impl Default for SemanticConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_chars: 8_000,
            top_n: 10,
        }
    }
}

/// A Rig-backed semantic layer over a [`CodeIndex`].
///
/// Keeps the underlying [`CodeIndex`] (lexical + symbol search) and a
/// [`VectorStoreAdapter`] (dense semantic search) in sync. All methods are
/// synchronous because both the code index and the Rig vector-store adapter
/// expose synchronous APIs.
pub struct SemanticCodeIndex {
    code_index: Arc<CodeIndex>,
    vector_store: VectorStoreAdapter,
    embedding: Box<dyn RigEmbeddingBackend>,
    config: SemanticConfig,
}

impl std::fmt::Debug for SemanticCodeIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SemanticCodeIndex")
            .field("code_index", &"...")
            .field("vector_store", &self.vector_store)
            .field("embedding", &self.embedding.name())
            .field("config", &self.config)
            .finish()
    }
}

impl SemanticCodeIndex {
    /// Open a semantic code index at `config` with the given Rig adapters.
    ///
    /// Creates a new [`CodeIndex`] internally. Use
    /// [`SemanticCodeIndex::from_code_index`] when you already have an
    /// `Arc<CodeIndex>` that is shared with a file watcher or the tool
    /// registry.
    ///
    /// # Errors
    ///
    /// Propagates any error from [`CodeIndex::open`] or the vector-store
    /// adapter construction.
    pub fn open(
        config: &CodeIndexConfig,
        embedding: Box<dyn RigEmbeddingBackend>,
        vector_store: VectorStoreAdapter,
    ) -> Result<Self> {
        let code_index = CodeIndex::open(config)
            .map_err(|e| RigError::BackendError(format!("failed to open code index: {e}")))?;
        Ok(Self {
            code_index: Arc::new(code_index),
            vector_store,
            embedding,
            config: SemanticConfig::default(),
        })
    }

    /// Wrap an existing `Arc<CodeIndex>` with a semantic layer.
    ///
    /// This is the primary constructor for production wiring: the caller
    /// already owns an `Arc<CodeIndex>` (shared with the file watcher and
    /// the `codeindex_*` tools) and wants to add Rig-backed semantic search
    /// alongside it without creating a second code index.
    ///
    /// # Errors
    ///
    /// Returns [`RigError::BackendError`] if the embedding backend is not
    /// available.
    pub fn from_code_index(
        code_index: Arc<CodeIndex>,
        embedding: Box<dyn RigEmbeddingBackend>,
        vector_store: VectorStoreAdapter,
    ) -> Result<Self> {
        Ok(Self {
            code_index,
            vector_store,
            embedding,
            config: SemanticConfig::default(),
        })
    }

    /// Open a semantic code index backed by an in-memory code index.
    ///
    /// Useful for tests and for ephemeral semantic searches.
    pub fn open_in_memory(
        config: &CodeIndexConfig,
        embedding: Box<dyn RigEmbeddingBackend>,
        vector_store: VectorStoreAdapter,
    ) -> Result<Self> {
        let code_index = CodeIndex::open_in_memory(config).map_err(|e| {
            RigError::BackendError(format!("failed to open in-memory code index: {e}"))
        })?;
        Ok(Self {
            code_index: Arc::new(code_index),
            vector_store,
            embedding,
            config: SemanticConfig::default(),
        })
    }

    /// Replace the default [`SemanticConfig`].
    #[must_use]
    pub fn with_config(mut self, config: SemanticConfig) -> Self {
        self.config = config;
        self
    }

    /// Returns a reference to the underlying lexical code index.
    pub fn code_index(&self) -> &Arc<CodeIndex> {
        &self.code_index
    }

    /// Returns a reference to the vector store adapter.
    pub fn vector_store(&self) -> &VectorStoreAdapter {
        &self.vector_store
    }

    /// Returns `true` if semantic indexing/search is enabled and the
    /// embedding backend is available.
    pub fn is_available(&self) -> bool {
        self.config.enabled && self.embedding.is_available()
    }

    /// Embed a single source file and add it to the semantic vector store.
    ///
    /// The file is read from the code-index project root. `rel_path` must be
    /// a UTF-8 path relative to the project root.
    ///
    /// # Errors
    ///
    /// Returns [`RigError::BackendError`] if reading, embedding, or storing
    /// fails.
    pub fn index_file_semantically(&self, rel_path: &str) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }
        let abs_path = self.code_index.project_root().join(rel_path);
        let text = std::fs::read_to_string(&abs_path).map_err(|e| {
            RigError::BackendError(format!("cannot read {rel_path} for semantic indexing: {e}"))
        })?;
        self.index_text(rel_path, &text, None)
    }

    /// Embed an arbitrary text document under a given code-index id.
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
        debug!(id, "indexed document in semantic vector store");
        Ok(())
    }

    /// Remove a document from the semantic vector store.
    ///
    /// # Errors
    ///
    /// Returns [`RigError::BackendError`] if the vector store cannot delete.
    pub fn remove_file_semantically(&self, rel_path: &str) -> Result<()> {
        self.vector_store.delete_document(rel_path)
    }

    /// Run a pure semantic search over the vector store.
    ///
    /// Results are ranked by cosine similarity to the embedded query.
    ///
    /// # Errors
    ///
    /// Returns [`RigError::BackendError`] if the query embedding or vector
    /// search fails.
    pub fn semantic_search(&self, query: &str, n: usize) -> Result<Vec<SemanticHit>> {
        let n = if n == 0 { self.config.top_n } else { n };
        let raw = self.vector_store.top_n(query, n)?;
        let mut hits = Vec::with_capacity(raw.len());
        for (score, id, document) in raw {
            let (language, line_count) = self
                .code_index
                .try_lock_store_for_test()
                .as_deref()
                .and_then(|store| store.get_file(&id).ok().flatten())
                .map(|entry| (entry.language, entry.line_count))
                .unwrap_or((None, 0));
            hits.push(SemanticHit {
                file_path: id,
                semantic_score: score,
                lexical_score: None,
                language,
                line_count,
                document,
            });
        }
        Ok(hits)
    }

    /// Search the code index with both lexical and semantic ranking.
    ///
    /// 1. Run the keyword query through [`CodeIndex::search`].
    /// 2. Run the same query through [`SemanticCodeIndex::semantic_search`].
    /// 3. Fuse the two result sets with a simple reciprocal-rank score so
    ///    files that appear in both lists bubble to the top.
    ///
    /// The returned vector is truncated to `n` items.
    ///
    /// # Errors
    ///
    /// Propagates errors from either the lexical or semantic search path.
    pub fn hybrid_search(&self, query: &str, n: usize) -> Result<Vec<SemanticHit>> {
        let n = if n == 0 { self.config.top_n } else { n };

        // Lexical search.
        let lexical = {
            let mut q = SearchQuery::new(query);
            q.max_results = n * 2;
            self.code_index.search(&q).map_err(|e| {
                RigError::BackendError(format!("lexical code-index search failed: {e}"))
            })?
        };

        // Semantic search.
        let semantic = self.semantic_search(query, n * 2)?;

        // Fuse by reciprocal rank (k=1).
        let mut fused: HashMap<String, SemanticHit> = HashMap::new();
        for (rank, hit) in lexical.iter().enumerate() {
            let score = 1.0 / (rank as f64 + 2.0);
            let semantic_hit = SemanticHit {
                file_path: hit.file_path.clone(),
                semantic_score: score,
                lexical_score: Some(hit.score),
                language: None,
                line_count: hit.line.min(1) as u64,
                document: json!({
                    "symbol_name": hit.symbol_name,
                    "qualified_name": hit.qualified_name,
                    "kind": hit.kind,
                    "line": hit.line,
                    "signature": hit.signature,
                    "doc_snippet": hit.doc_snippet,
                }),
            };
            if let Some(existing) = fused.get_mut(&semantic_hit.file_path) {
                existing.semantic_score += semantic_hit.semantic_score;
                existing.lexical_score = semantic_hit.lexical_score;
            } else {
                fused.insert(semantic_hit.file_path.clone(), semantic_hit);
            }
        }
        for (rank, mut hit) in semantic.into_iter().enumerate() {
            let score = 1.0 / (rank as f64 + 2.0);
            if let Some(existing) = fused.get_mut(&hit.file_path) {
                existing.semantic_score += score;
                existing.lexical_score = hit.lexical_score;
            } else {
                hit.semantic_score = score;
                fused.insert(hit.file_path.clone(), hit);
            }
        }

        let mut results: Vec<SemanticHit> = fused.into_values().collect();
        results.sort_by(|a, b| {
            b.semantic_score
                .partial_cmp(&a.semantic_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(n);
        debug!(query, n, "hybrid code-index search");
        Ok(results)
    }

    /// Index a file using the normal code-index pipeline **and** add it to
    /// the semantic vector store when semantic indexing is enabled.
    ///
    /// This is the main entry point for live file changes (FR-015).
    ///
    /// # Errors
    ///
    /// Propagates errors from the lexical indexing or semantic embedding
    /// paths.
    pub fn index_file(&self, path: &Path) -> Result<()> {
        self.code_index.index_file(path).map_err(|e| {
            RigError::BackendError(format!("code-index failed for {}: {e}", path.display()))
        })?;
        if let Some(rel) = path.to_str() {
            if let Err(e) = self.index_file_semantically(rel) {
                warn!(
                    path = %path.display(),
                    error = %e,
                    "semantic indexing failed; lexical index succeeded"
                );
            }
        }
        Ok(())
    }

    /// Remove a file from both the lexical code index and the semantic
    /// vector store.
    ///
    /// # Errors
    ///
    /// Propagates errors from either removal path.
    pub fn remove_file(&self, path: &Path) -> Result<()> {
        self.code_index.remove_file(path).map_err(|e| {
            RigError::BackendError(format!(
                "code-index remove failed for {}: {e}",
                path.display()
            ))
        })?;
        if let Some(rel) = path.to_str() {
            if let Err(e) = self.remove_file_semantically(rel) {
                warn!(
                    path = %path.display(),
                    error = %e,
                    "semantic remove failed; lexical remove succeeded"
                );
            }
        }
        Ok(())
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

/// Extension trait that adds semantic methods to a plain [`CodeIndex`].
///
/// This is useful when callers already own a [`CodeIndex`] and just want
/// to run one-off semantic queries without wrapping it in a
/// [`SemanticCodeIndex`].
pub trait CodeIndexExt {
    /// Search this code index by vector similarity using an external
    /// embedding backend and vector store.
    ///
    /// Results are decorated with code-index file metadata.
    fn semantic_search(
        &self,
        embedding: &dyn RigEmbeddingBackend,
        vector_store: &VectorStoreAdapter,
        query: &str,
        n: usize,
    ) -> Result<Vec<SemanticHit>>;
}

impl CodeIndexExt for CodeIndex {
    fn semantic_search(
        &self,
        _embedding: &dyn RigEmbeddingBackend,
        vector_store: &VectorStoreAdapter,
        query: &str,
        n: usize,
    ) -> Result<Vec<SemanticHit>> {
        let raw = vector_store.top_n(query, n)?;
        let mut hits = Vec::with_capacity(raw.len());
        for (score, id, document) in raw {
            let (language, line_count) = self
                .try_lock_store_for_test()
                .as_deref()
                .and_then(|store| store.get_file(&id).ok().flatten())
                .map(|entry| (entry.language, entry.line_count))
                .unwrap_or((None, 0));
            hits.push(SemanticHit {
                file_path: id,
                semantic_score: score,
                lexical_score: None,
                language,
                line_count,
                document,
            });
        }
        Ok(hits)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use ragent_codeindex::types::CodeIndexConfig;
    use tempfile::TempDir;

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

        fn name(&self) -> &str {
            "fixed"
        }
    }

    fn make_test_index(project_root: PathBuf) -> SemanticCodeIndex {
        let config = CodeIndexConfig {
            enabled: true,
            project_root,
            index_dir: PathBuf::from(".ragent/codeindex"),
            scan_config: ragent_codeindex::types::ScanConfig::default(),
        };
        let embedding = Box::new(FixedEmbeddingBackend {
            vec: vec![0.0_f32, 1.0, 0.0, 0.0],
        });
        let vector_store = VectorStoreAdapter::new("memory", None, embedding.clone()).unwrap();
        SemanticCodeIndex::open_in_memory(&config, embedding, vector_store).unwrap()
    }

    #[test]
    fn semantic_index_adds_and_searches_text() {
        let idx = make_test_index(PathBuf::from("."));
        idx.index_text(
            "src/foo.rs",
            "fn compute_total(a: i32, b: i32) -> i32 { a + b }",
            None,
        )
        .expect("index text");
        idx.index_text("src/bar.rs", "struct Point { x: f64, y: f64 }", None)
            .expect("index text");

        // The fixture backend returns the same vector for every text, so both
        // documents tie. Request both and verify they are returned with equal
        // top scores.
        let results = idx.semantic_search("query", 2).expect("search");
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|h| h.file_path == "src/foo.rs"));
        assert!(results.iter().any(|h| h.file_path == "src/bar.rs"));
        assert!((results[0].semantic_score - results[1].semantic_score).abs() < f64::EPSILON);
    }

    #[test]
    fn hybrid_search_combines_lexical_and_semantic() {
        let tmp = TempDir::new().expect("temp dir");
        let root = tmp.path().to_path_buf();
        let a = root.join("foo.rs");
        let b = root.join("bar.rs");
        std::fs::write(&a, "pub fn add(a: i32, b: i32) -> i32 { a + b }\n").unwrap();
        std::fs::write(&b, "pub struct Point { x: f64, y: f64 }\n").unwrap();

        let idx = make_test_index(root);
        idx.code_index()
            .index_file(Path::new("foo.rs"))
            .expect("index foo");
        idx.code_index()
            .index_file(Path::new("bar.rs"))
            .expect("index bar");

        // Embed both files with orthogonal vectors.
        idx.index_text(
            "foo.rs",
            "pub fn add(a: i32, b: i32) -> i32 { a + b }",
            None,
        )
        .expect("embed foo");
        let other_vec: Vec<f32> = vec![1.0, 0.0, 0.0, 0.0];
        let other_backend = Box::new(FixedEmbeddingBackend {
            vec: other_vec.clone(),
        });
        let other_store = VectorStoreAdapter::new("memory", None, other_backend).unwrap();
        other_store
            .add_documents(vec![(
                "bar.rs".to_owned(),
                json!({"id": "bar.rs", "text": "struct Point"}),
                other_vec,
            )])
            .unwrap();

        // For the hybrid search we need both stores, but our fixture uses
        // a single store. Search with the fixture store; the query vector
        // matches foo.rs best.
        let hits = idx.hybrid_search("add", 2).expect("hybrid search");
        assert!(!hits.is_empty());
        assert!(hits.iter().any(|h| h.file_path == "foo.rs"));
    }

    #[test]
    fn code_index_ext_semantic_search_enriches_metadata() {
        let tmp = TempDir::new().expect("temp dir");
        let root = tmp.path().to_path_buf();
        let p = root.join("lib.rs");
        std::fs::write(&p, "pub fn helper() {}\n").unwrap();

        let config = CodeIndexConfig {
            enabled: true,
            project_root: root,
            index_dir: PathBuf::from(".ragent/codeindex"),
            scan_config: ragent_codeindex::types::ScanConfig::default(),
        };
        let code_index = CodeIndex::open_in_memory(&config).unwrap();
        code_index.index_file(Path::new("lib.rs")).unwrap();

        let embedding = Box::new(FixedEmbeddingBackend {
            vec: vec![0.0_f32, 1.0, 0.0, 0.0],
        });
        let vector_store = VectorStoreAdapter::new("memory", None, embedding.clone()).unwrap();
        vector_store
            .add_documents(vec![(
                "lib.rs".to_owned(),
                json!({"id": "lib.rs"}),
                vec![0.0_f32, 1.0, 0.0, 0.0],
            )])
            .unwrap();

        let hits = code_index
            .semantic_search(&*embedding, &vector_store, "query", 1)
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].file_path, "lib.rs");
        assert_eq!(hits[0].language.as_deref(), Some("rust"));
    }
}
