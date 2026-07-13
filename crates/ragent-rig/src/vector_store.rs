//! Rig-backed vector-store adapter (T-008 / FR-008 / FR-009).
//!
//! This module provides a common, ragent-facing vector-store backend trait
//! ([`RigVectorStoreBackend`]) with three concrete implementations:
//!
//! * **memory** — wraps Rig's own [`InMemoryVectorIndex`] so semantic search
//!   is delegated to Rig's `VectorStoreIndex` trait (FR-008).
//! * **sqlite** — a local SQLite-backed backend that stores documents and
//!   their embeddings in a SQL table and computes similarity in-process.
//! * **http** — a remote backend that talks to a vector-store service over
//!   HTTP (FR-009).
//!
//! All backends share the same sync API and the same embedding bridge
//! ([`RigEmbeddingBackend`]), so callers in `ragent-codeindex`, memory, and
//! `/research` do not need to know which backend is configured.
//!
//! The module is compiled only when a vector-store feature is enabled.

use std::future::Future;
use std::sync::Arc;
use std::sync::OnceLock;

use rig::embeddings::{Embedding, EmbeddingModel as RigEmbeddingModel};

use crate::embeddings_trait::RigEmbeddingBackend;
use crate::error::{Result, RigError};

/// A single document plus its embedding, ready to be stored.
pub type VectorDocument = (String, serde_json::Value, Vec<f32>);

/// A ranked search result: `(similarity_score, document_id, document)`.
pub type VectorSearchResult = (f64, String, serde_json::Value);

/// The internal contract for a Rig-backed vector-store backend.
///
/// Backends are object-safe and expose synchronous methods. The methods are
/// synchronous because ragent's callers (code index, memory, research) are
/// mostly synchronous at the storage boundary; async Rig calls are bridged
/// with a dedicated current-thread runtime inside each backend, exactly as
/// the embedding adapter does in [`crate::embeddings`].
pub trait RigVectorStoreBackend: Send + Sync {
    /// Store or overwrite one or more documents with their embeddings.
    ///
    /// # Errors
    ///
    /// Returns [`RigError::BackendError`] if the underlying store fails.
    fn add_documents(&self, documents: Vec<VectorDocument>) -> Result<()>;

    /// Return the `n` most semantically similar documents for a text query.
    ///
    /// # Errors
    ///
    /// Returns [`RigError::BackendError`] if embedding the query or searching
    /// the store fails.
    fn top_n(&self, query: &str, n: usize) -> Result<Vec<VectorSearchResult>>;

    /// Return only the IDs of the `n` most similar documents.
    ///
    /// # Errors
    ///
    /// Returns [`RigError::BackendError`] if embedding the query or searching
    /// the store fails.
    fn top_n_ids(&self, query: &str, n: usize) -> Result<Vec<(f64, String)>>;

    /// Remove a document by ID.
    ///
    /// # Errors
    ///
    /// Returns [`RigError::BackendError`] if the store cannot delete the row.
    fn delete_document(&self, id: &str) -> Result<()>;

    /// Remove every document from the store.
    ///
    /// # Errors
    ///
    /// Returns [`RigError::BackendError`] if the store cannot clear its data.
    fn clear(&self) -> Result<()>;

    /// Return the number of stored documents.
    fn len(&self) -> usize;

    /// Returns `true` if the store contains no documents.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Human-readable backend name, e.g. `"rig-vector-memory"`.
    fn name(&self) -> &str;
}

/// A thin handle to a configured [`RigVectorStoreBackend`].
///
/// This is the type other crates receive from `ragent-rig`. It dispatches
/// `add_documents`, `top_n`, etc. to the selected concrete backend.
pub struct VectorStoreAdapter {
    backend: Box<dyn RigVectorStoreBackend>,
}

impl std::fmt::Debug for VectorStoreAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VectorStoreAdapter")
            .field("name", &self.backend.name())
            .field("len", &self.backend.len())
            .finish_non_exhaustive()
    }
}

impl VectorStoreAdapter {
    /// Open a vector-store backend by name.
    ///
    /// `backend` must be one of the backends compiled in:
    ///
    /// * `"memory"` (requires the `vector-store-memory` feature)
    /// * `"sqlite"` (requires the `vector-store-sqlite` feature)
    /// * `"http"` (requires the `vector-store-http` feature)
    ///
    /// `embedding` is the [`RigEmbeddingBackend`] used to embed queries and,
    /// for the `memory` backend, to build Rig `Embedding` objects.
    /// `connection` is interpreted by the backend (a file path for SQLite,
    /// a base URL for HTTP, ignored for in-memory).
    ///
    /// # Errors
    ///
    /// Returns [`RigError::InvalidConfiguration`] if the backend name is not
    /// recognised, or [`RigError::VectorStoreNotEnabled`] if the matching
    /// feature was not compiled in.
    pub fn new(
        backend: &str,
        connection: Option<&str>,
        embedding: Box<dyn RigEmbeddingBackend>,
    ) -> Result<Self> {
        let backend: Box<dyn RigVectorStoreBackend> = match backend {
            #[cfg(feature = "vector-store-memory")]
            "memory" => Box::new(MemoryVectorStoreBackend::new(embedding)?),
            #[cfg(feature = "vector-store-sqlite")]
            "sqlite" => Box::new(SqliteVectorStoreBackend::new(connection, embedding)?),
            #[cfg(feature = "vector-store-http")]
            "http" => Box::new(HttpVectorStoreBackend::new(connection, embedding)?),
            #[cfg(not(all(
                feature = "vector-store-memory",
                feature = "vector-store-sqlite",
                feature = "vector-store-http"
            )))]
            other => {
                if matches!(other, "memory" | "sqlite" | "http") {
                    return Err(RigError::VectorStoreNotEnabled);
                }
                return Err(RigError::InvalidConfiguration(format!(
                    "unsupported vector store backend: {other}"
                )));
            }
            #[cfg(all(
                feature = "vector-store-memory",
                feature = "vector-store-sqlite",
                feature = "vector-store-http"
            ))]
            other => {
                return Err(RigError::InvalidConfiguration(format!(
                    "unsupported vector store backend: {other}"
                )));
            }
        };
        Ok(Self { backend })
    }

    /// Convenience constructor from a [`ragent_config::RigVectorStoreConfig`].
    ///
    /// # Errors
    ///
    /// Propagates errors from [`VectorStoreAdapter::new`] and returns
    /// [`RigError::InvalidConfiguration`] if vector stores are disabled in the
    /// configuration.
    pub fn from_config(
        config: &ragent_config::RigVectorStoreConfig,
        embedding: Box<dyn RigEmbeddingBackend>,
    ) -> Result<Self> {
        if !config.enabled {
            return Err(RigError::InvalidConfiguration(
                "vector store is disabled in configuration".to_owned(),
            ));
        }
        Self::new(&config.backend, config.connection.as_deref(), embedding)
    }

    /// Store or overwrite documents with their embeddings.
    pub fn add_documents(&self, documents: Vec<VectorDocument>) -> Result<()> {
        self.backend.add_documents(documents)
    }

    /// Return the `n` most semantically similar documents for a query.
    pub fn top_n(&self, query: &str, n: usize) -> Result<Vec<VectorSearchResult>> {
        self.backend.top_n(query, n)
    }

    /// Return only the IDs of the `n` most similar documents.
    pub fn top_n_ids(&self, query: &str, n: usize) -> Result<Vec<(f64, String)>> {
        self.backend.top_n_ids(query, n)
    }

    /// Remove a document by ID.
    pub fn delete_document(&self, id: &str) -> Result<()> {
        self.backend.delete_document(id)
    }

    /// Remove every document.
    pub fn clear(&self) -> Result<()> {
        self.backend.clear()
    }

    /// Return the number of stored documents.
    pub fn len(&self) -> usize {
        self.backend.len()
    }

    /// Returns `true` if the store contains no documents.
    pub fn is_empty(&self) -> bool {
        self.backend.is_empty()
    }

    /// Return the backend name.
    pub fn name(&self) -> &str {
        self.backend.name()
    }
}

// ── Helpers: runtime bridge and embedding model wrapping ────────────────────

/// Lazily create a dedicated current-thread Tokio runtime for sync↔async
/// bridges inside vector-store backends.
fn vector_store_runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build vector-store bridge runtime")
    })
}

/// Convert an f32 embedding (ragent's internal representation) into a Rig
/// `Embedding` with an f64 vector and a document label.
fn f32_to_rig_embedding(document_label: &str, vec: &[f32]) -> Embedding {
    Embedding {
        document: document_label.to_owned(),
        vec: vec.iter().map(|v| f64::from(*v)).collect(),
    }
}

/// A Rig [`EmbeddingModel`] that delegates embedding work to a ragent
/// [`RigEmbeddingBackend`].
///
/// This lets Rig's own [`InMemoryVectorIndex`] use any ragent embedding
/// provider (OpenAI, Gemini, Ollama, or a test backend) without knowing its
/// concrete Rig type.
struct DynEmbeddingModel {
    backend: Arc<dyn RigEmbeddingBackend>,
}

impl DynEmbeddingModel {
    fn new(backend: Arc<dyn RigEmbeddingBackend>) -> Self {
        Self { backend }
    }
}

impl Clone for DynEmbeddingModel {
    fn clone(&self) -> Self {
        Self {
            backend: Arc::clone(&self.backend),
        }
    }
}

impl RigEmbeddingModel for DynEmbeddingModel {
    const MAX_DOCUMENTS: usize = 1024;

    fn ndims(&self) -> usize {
        self.backend.dimensions()
    }

    fn embed_texts(
        &self,
        texts: impl IntoIterator<Item = String> + Send,
    ) -> impl Future<Output = std::result::Result<Vec<Embedding>, rig::embeddings::EmbeddingError>> + Send
    {
        let texts: Vec<String> = texts.into_iter().collect();
        let dims = self.backend.dimensions();
        let backend_ref = &self.backend;
        async move {
            if dims == 0 {
                // Return zero embeddings if the backend is unavailable.
                return Ok(texts
                    .into_iter()
                    .map(|t| Embedding {
                        document: t,
                        vec: Vec::new(),
                    })
                    .collect());
            }
            let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
            let batch = backend_ref.embed_batch(&refs).map_err(|e| {
                rig::embeddings::EmbeddingError::ProviderError(format!(
                    "DynEmbeddingModel embed_batch failed: {e}"
                ))
            })?;
            Ok(batch
                .into_iter()
                .zip(texts)
                .map(|(vec, doc)| {
                    let label: String = doc;
                    f32_to_rig_embedding(&label, &vec)
                })
                .collect())
        }
    }
}

// ── In-memory backend (uses Rig VectorStoreIndex) ─────────────────────────────

#[cfg(feature = "vector-store-memory")]
mod memory_backend {
    use std::sync::Mutex;

    use rig::embeddings::Embedding;
    use rig::one_or_many::OneOrMany;
    use rig::vector_store::VectorStoreIndex;
    use rig::vector_store::in_memory_store::{InMemoryVectorIndex, InMemoryVectorStore};
    use serde_json::Value;

    use super::{
        Arc, DynEmbeddingModel, Result, RigEmbeddingBackend, RigError, RigVectorStoreBackend,
        VectorDocument, VectorSearchResult, f32_to_rig_embedding, vector_store_runtime,
    };
    /// A local, in-memory vector-store backend built on Rig's
    /// [`InMemoryVectorIndex`].
    ///
    /// This backend satisfies the "local backend" requirement of FR-009 and
    /// exercises Rig's native `VectorStoreIndex` trait directly.
    pub struct MemoryVectorStoreBackend {
        name: String,
        index: Mutex<InMemoryVectorIndex<DynEmbeddingModel, Value>>,
        deleted: Mutex<std::collections::HashSet<String>>,
        embedding: Arc<dyn RigEmbeddingBackend>,
    }

    impl std::fmt::Debug for MemoryVectorStoreBackend {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("MemoryVectorStoreBackend")
                .field("name", &self.name)
                .field("len", &self.len())
                .finish_non_exhaustive()
        }
    }

    impl MemoryVectorStoreBackend {
        /// Create an empty in-memory vector store backed by the supplied
        /// embedding backend.
        pub fn new(embedding: Box<dyn RigEmbeddingBackend>) -> Result<Self> {
            let embedding = Arc::from(embedding);
            let model = DynEmbeddingModel::new(Arc::clone(&embedding));
            let store = InMemoryVectorStore::default();
            let index = InMemoryVectorIndex::new(model, store);
            Ok(Self {
                name: "rig-vector-memory".to_owned(),
                index: Mutex::new(index),
                deleted: Mutex::new(std::collections::HashSet::new()),
                embedding,
            })
        }
    }

    impl RigVectorStoreBackend for MemoryVectorStoreBackend {
        fn add_documents(&self, documents: Vec<VectorDocument>) -> Result<()> {
            let rig_docs: Vec<(String, Value, OneOrMany<Embedding>)> = documents
                .into_iter()
                .map(|(id, doc, vec)| {
                    let embedding = OneOrMany::one(f32_to_rig_embedding(&id, &vec));
                    (id, doc, embedding)
                })
                .collect();
            let mut guard = self
                .index
                .lock()
                .map_err(|e| RigError::BackendError(format!("memory store lock poisoned: {e}")))?;
            guard.store.add_documents_with_ids(rig_docs);
            Ok(())
        }

        fn top_n(&self, query: &str, n: usize) -> Result<Vec<VectorSearchResult>> {
            let guard = self
                .index
                .lock()
                .map_err(|e| RigError::BackendError(format!("memory store lock poisoned: {e}")))?;
            let mut results = vector_store_runtime()
                .block_on(guard.top_n::<Value>(query, n))
                .map_err(|e| RigError::BackendError(format!("memory top_n failed: {e}")))?;
            drop(guard);
            let deleted_guard = self
                .deleted
                .lock()
                .map_err(|e| RigError::BackendError(format!("memory deleted set poisoned: {e}")))?;
            results.retain(|(_, id, _)| !deleted_guard.contains(id));
            Ok(results)
        }

        fn top_n_ids(&self, query: &str, n: usize) -> Result<Vec<(f64, String)>> {
            let results = self.top_n(query, n)?;
            Ok(results.into_iter().map(|(s, id, _)| (s, id)).collect())
        }

        fn delete_document(&self, id: &str) -> Result<()> {
            let mut guard = self
                .deleted
                .lock()
                .map_err(|e| RigError::BackendError(format!("memory deleted set poisoned: {e}")))?;
            guard.insert(id.to_owned());
            Ok(())
        }

        fn clear(&self) -> Result<()> {
            let mut guard = self
                .index
                .lock()
                .map_err(|e| RigError::BackendError(format!("memory store lock poisoned: {e}")))?;
            let model = DynEmbeddingModel::new(Arc::clone(&self.embedding));
            *guard = InMemoryVectorIndex::new(model, InMemoryVectorStore::default());
            drop(guard);
            let mut deleted_guard = self
                .deleted
                .lock()
                .map_err(|e| RigError::BackendError(format!("memory deleted set poisoned: {e}")))?;
            deleted_guard.clear();
            Ok(())
        }

        fn len(&self) -> usize {
            let store_len = self.index.lock().map(|g| g.store.len()).unwrap_or(0);
            let deleted_len = self.deleted.lock().map(|g| g.len()).unwrap_or(0);
            store_len.saturating_sub(deleted_len)
        }
        fn name(&self) -> &str {
            &self.name
        }
    }
}

#[cfg(feature = "vector-store-memory")]
pub use memory_backend::MemoryVectorStoreBackend;

// ── SQLite backend (local, SQL-backed) ──────────���────────────────────────────

#[cfg(feature = "vector-store-sqlite")]
mod sqlite_backend {
    use std::path::Path;
    use std::sync::Mutex;

    use rusqlite::{Connection, params};
    use serde_json::Value;

    use super::{
        Result, RigEmbeddingBackend, RigError, RigVectorStoreBackend, VectorDocument,
        VectorSearchResult, bytes_to_f32_vec, f32_vec_to_bytes, rank_by_cosine_similarity,
    };
    /// A local SQLite-backed vector-store backend.
    ///
    /// Embeddings are stored as little-endian f32 byte blobs. Similarity is
    /// computed in-process with cosine similarity, so the backend works with
    /// any embedding dimensionality without needing a vector extension.
    pub struct SqliteVectorStoreBackend {
        name: String,
        conn: Mutex<Connection>,
        embedding: Box<dyn RigEmbeddingBackend>,
    }

    impl std::fmt::Debug for SqliteVectorStoreBackend {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("SqliteVectorStoreBackend")
                .field("name", &self.name)
                .field("len", &self.len())
                .finish_non_exhaustive()
        }
    }

    impl SqliteVectorStoreBackend {
        /// Open a SQLite vector store at `path`. If `path` is `None` or
        /// `":memory:"`, an ephemeral in-memory database is used.
        pub fn new(path: Option<&str>, embedding: Box<dyn RigEmbeddingBackend>) -> Result<Self> {
            let conn = match path {
                Some(path) if path != ":memory:" => {
                    if let Some(parent) = Path::new(path).parent() {
                        std::fs::create_dir_all(parent).map_err(|e| {
                            RigError::BackendError(format!(
                                "failed to create vector store directory: {e}"
                            ))
                        })?;
                    }
                    Connection::open(path).map_err(|e| {
                        RigError::BackendError(format!("failed to open sqlite vector store: {e}"))
                    })?
                }
                _ => Connection::open_in_memory().map_err(|e| {
                    RigError::BackendError(format!(
                        "failed to open in-memory sqlite vector store: {e}"
                    ))
                })?,
            };
            conn.execute(
                "CREATE TABLE IF NOT EXISTS rig_vectors (
                    id TEXT PRIMARY KEY,
                    doc TEXT NOT NULL,
                    embedding BLOB NOT NULL
                )",
                [],
            )
            .map_err(|e| {
                RigError::BackendError(format!("failed to create vector store schema: {e}"))
            })?;
            Ok(Self {
                name: "rig-vector-sqlite".to_owned(),
                conn: Mutex::new(conn),
                embedding,
            })
        }

        fn lock_conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
            self.conn
                .lock()
                .map_err(|e| RigError::BackendError(format!("sqlite store lock poisoned: {e}")))
        }

        fn read_all_rows(&self) -> Result<Vec<(String, Value, Vec<f32>)>> {
            let conn = self.lock_conn()?;
            let mut stmt = conn
                .prepare("SELECT id, doc, embedding FROM rig_vectors")
                .map_err(|e| RigError::BackendError(format!("prepare failed: {e}")))?;
            let rows = stmt
                .query_map([], |row| {
                    let id: String = row.get(0)?;
                    let doc_json: String = row.get(1)?;
                    let blob: Vec<u8> = row.get(2)?;
                    let doc: Value = serde_json::from_str(&doc_json).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            1,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?;
                    let embedding = bytes_to_f32_vec(&blob).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            2,
                            rusqlite::types::Type::Blob,
                            Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
                        )
                    })?;
                    Ok((id, doc, embedding))
                })
                .map_err(|e| RigError::BackendError(format!("query failed: {e}")))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .map_err(|e| RigError::BackendError(format!("row mapping failed: {e}")))
        }
    }

    impl RigVectorStoreBackend for SqliteVectorStoreBackend {
        fn add_documents(&self, documents: Vec<VectorDocument>) -> Result<()> {
            let mut conn = self.lock_conn()?;
            let tx = conn
                .transaction()
                .map_err(|e| RigError::BackendError(format!("transaction failed: {e}")))?;
            for (id, doc, vec) in documents {
                let blob = f32_vec_to_bytes(&vec);
                let doc_json = serde_json::to_string(&doc)
                    .map_err(|e| RigError::BackendError(format!("serialize doc failed: {e}")))?;
                tx.execute(
                    "INSERT OR REPLACE INTO rig_vectors (id, doc, embedding) VALUES (?1, ?2, ?3)",
                    params![id, doc_json, blob],
                )
                .map_err(|e| RigError::BackendError(format!("insert failed: {e}")))?;
            }
            tx.commit()
                .map_err(|e| RigError::BackendError(format!("commit failed: {e}")))?;
            Ok(())
        }

        fn top_n(&self, query: &str, n: usize) -> Result<Vec<VectorSearchResult>> {
            let query_vec = self.embedding.embed(query)?;
            let rows = self.read_all_rows()?;
            Ok(rank_by_cosine_similarity(
                &query_vec,
                rows,
                n,
                |(_, doc, _)| doc.clone(),
            ))
        }

        fn top_n_ids(&self, query: &str, n: usize) -> Result<Vec<(f64, String)>> {
            let results = self.top_n(query, n)?;
            Ok(results.into_iter().map(|(s, id, _)| (s, id)).collect())
        }

        fn delete_document(&self, id: &str) -> Result<()> {
            let conn = self.lock_conn()?;
            conn.execute("DELETE FROM rig_vectors WHERE id = ?1", params![id])
                .map_err(|e| RigError::BackendError(format!("delete failed: {e}")))?;
            Ok(())
        }

        fn clear(&self) -> Result<()> {
            let conn = self.lock_conn()?;
            conn.execute("DELETE FROM rig_vectors", [])
                .map_err(|e| RigError::BackendError(format!("clear failed: {e}")))?;
            Ok(())
        }

        fn len(&self) -> usize {
            self.lock_conn()
                .and_then(|conn| {
                    conn.query_row("SELECT COUNT(*) FROM rig_vectors", [], |row| {
                        row.get::<_, i64>(0)
                    })
                    .map(|c| c as usize)
                    .map_err(|e| RigError::BackendError(format!("count failed: {e}")))
                })
                .unwrap_or(0)
        }

        fn name(&self) -> &str {
            &self.name
        }
    }
}

#[cfg(feature = "vector-store-sqlite")]
pub use sqlite_backend::SqliteVectorStoreBackend;

// ── HTTP remote backend ─────────────────────────────────────────────────────

#[cfg(feature = "vector-store-http")]
mod http_backend {
    use serde::{Deserialize, Serialize};

    use super::{
        Result, RigEmbeddingBackend, RigError, RigVectorStoreBackend, VectorDocument,
        VectorSearchResult, vector_store_runtime,
    };
    /// A minimal remote vector-store backend that communicates over HTTP.
    ///
    /// The protocol is intentionally simple:
    ///
    /// * `POST {base_url}/vectors` with a JSON array of
    ///   [`HttpVectorDocument`] to add documents.
    /// * `GET {base_url}/vectors/search?query={query}&n={n}` returning a JSON
    ///   array of [`HttpSearchResult`].
    /// * `DELETE {base_url}/vectors/{id}` to delete a document.
    /// * `DELETE {base_url}/vectors` to clear the remote index.
    ///
    /// This satisfies the "remote backend" requirement of FR-009 behind the
    /// same [`RigVectorStoreBackend`] trait used by the local backends.
    pub struct HttpVectorStoreBackend {
        name: String,
        base_url: String,
        client: reqwest::Client,
        /// The embedding backend is kept for future local fallback support.
        #[allow(dead_code)]
        embedding: Box<dyn RigEmbeddingBackend>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct HttpVectorDocument {
        id: String,
        document: serde_json::Value,
        embedding: Vec<f32>,
    }

    #[derive(Debug, Deserialize)]
    struct HttpSearchResult {
        score: f64,
        id: String,
        document: serde_json::Value,
    }

    impl std::fmt::Debug for HttpVectorStoreBackend {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("HttpVectorStoreBackend")
                .field("name", &self.name)
                .field("base_url", &self.base_url)
                .finish_non_exhaustive()
        }
    }

    impl HttpVectorStoreBackend {
        /// Create a remote vector-store backend pointing at `connection`.
        ///
        /// `connection` must be a valid HTTP(S) URL.
        pub fn new(
            connection: Option<&str>,
            embedding: Box<dyn RigEmbeddingBackend>,
        ) -> Result<Self> {
            let base_url = connection
                .map(std::string::String::from)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    RigError::InvalidConfiguration(
                        "http vector store requires a connection URL".to_owned(),
                    )
                })?;
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .map_err(|e| {
                    RigError::BackendError(format!("failed to build http vector store client: {e}"))
                })?;
            Ok(Self {
                name: "rig-vector-http".to_owned(),
                base_url,
                client,
                embedding,
            })
        }
    }

    impl RigVectorStoreBackend for HttpVectorStoreBackend {
        fn add_documents(&self, documents: Vec<VectorDocument>) -> Result<()> {
            let payload: Vec<HttpVectorDocument> = documents
                .into_iter()
                .map(|(id, document, embedding)| HttpVectorDocument {
                    id,
                    document,
                    embedding,
                })
                .collect();
            let url = format!("{}/vectors", self.base_url);
            let request = self.client.post(&url).json(&payload);
            let response = vector_store_runtime()
                .block_on(request.send())
                .map_err(|e| RigError::BackendError(format!("http add_documents failed: {e}")))?;
            if !response.status().is_success() {
                return Err(RigError::BackendError(format!(
                    "http add_documents returned {}",
                    response.status()
                )));
            }
            Ok(())
        }

        fn top_n(&self, query: &str, n: usize) -> Result<Vec<VectorSearchResult>> {
            let url = format!("{}/vectors/search?query={}&n={}", self.base_url, query, n);
            let request = self.client.get(&url);
            let response = vector_store_runtime()
                .block_on(request.send())
                .map_err(|e| RigError::BackendError(format!("http top_n failed: {e}")))?;
            if !response.status().is_success() {
                return Err(RigError::BackendError(format!(
                    "http top_n returned {}",
                    response.status()
                )));
            }
            let results: Vec<HttpSearchResult> = vector_store_runtime()
                .block_on(response.json())
                .map_err(|e| {
                RigError::BackendError(format!("http top_n decode failed: {e}"))
            })?;
            Ok(results
                .into_iter()
                .map(|r| (r.score, r.id, r.document))
                .collect())
        }

        fn top_n_ids(&self, query: &str, n: usize) -> Result<Vec<(f64, String)>> {
            let results = self.top_n(query, n)?;
            Ok(results.into_iter().map(|(s, id, _)| (s, id)).collect())
        }

        fn delete_document(&self, id: &str) -> Result<()> {
            let url = format!("{}/vectors/{}", self.base_url, id);
            let request = self.client.delete(&url);
            let response = vector_store_runtime()
                .block_on(request.send())
                .map_err(|e| RigError::BackendError(format!("http delete failed: {e}")))?;
            if !response.status().is_success() {
                return Err(RigError::BackendError(format!(
                    "http delete returned {}",
                    response.status()
                )));
            }
            Ok(())
        }

        fn clear(&self) -> Result<()> {
            let url = format!("{}/vectors", self.base_url);
            let request = self.client.delete(&url);
            let response = vector_store_runtime()
                .block_on(request.send())
                .map_err(|e| RigError::BackendError(format!("http clear failed: {e}")))?;
            if !response.status().is_success() {
                return Err(RigError::BackendError(format!(
                    "http clear returned {}",
                    response.status()
                )));
            }
            Ok(())
        }

        fn len(&self) -> usize {
            // The remote store does not expose a cheap length call in this
            // minimal protocol, so we report 0 to avoid an extra round-trip.
            0
        }

        fn name(&self) -> &str {
            &self.name
        }
    }
}

#[cfg(feature = "vector-store-http")]
pub use http_backend::HttpVectorStoreBackend;

// ── Shared similarity utilities ─────────────────────────────────────────────

/// Compute the cosine similarity between two f32 vectors.
///
/// Returns `None` if either vector has zero magnitude.
fn cosine_similarity(a: &[f32], b: &[f32]) -> Option<f64> {
    if a.len() != b.len() {
        return None;
    }
    let mut dot = 0.0_f64;
    let mut norm_a = 0.0_f64;
    let mut norm_b = 0.0_f64;
    for (x, y) in a.iter().zip(b.iter()) {
        let xf = f64::from(*x);
        let yf = f64::from(*y);
        dot = (xf).mul_add(yf, dot);
        norm_a = (xf).mul_add(xf, norm_a);
        norm_b = (yf).mul_add(yf, norm_b);
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 {
        return None;
    }
    Some(dot / denom)
}

/// Rank stored documents by cosine similarity to a query vector and return
/// the top `n` results, mapping each row into a [`VectorSearchResult`].
fn rank_by_cosine_similarity<T>(
    query: &[f32],
    rows: Vec<(String, serde_json::Value, Vec<f32>)>,
    n: usize,
    extract_doc: impl Fn(&(String, serde_json::Value, Vec<f32>)) -> T,
) -> Vec<(f64, String, T)> {
    let mut scored: Vec<(f64, String, T)> = rows
        .into_iter()
        .filter_map(|row| {
            let (id, _, vec) = &row;
            let score = cosine_similarity(query, vec)?;
            Some((score, id.clone(), extract_doc(&row)))
        })
        .collect();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(n);
    scored
}

/// Encode a vector of f32 values into a little-endian byte blob.
fn f32_vec_to_bytes(vec: &[f32]) -> Vec<u8> {
    vec.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// Decode a little-endian byte blob back into f32 values.
fn bytes_to_f32_vec(bytes: &[u8]) -> std::result::Result<Vec<f32>, String> {
    if !bytes.len().is_multiple_of(4) {
        return Err(format!(
            "embedding blob length {} is not a multiple of 4",
            bytes.len()
        ));
    }
    let mut vec = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        let arr: [u8; 4] = chunk.try_into().map_err(|e| format!("chunk error: {e}"))?;
        vec.push(f32::from_le_bytes(arr));
    }
    Ok(vec)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embeddings_trait::RigEmbeddingBackend;

    /// A deterministic embedding backend for tests: each text is embedded as
    /// `[text.len() as f32; dimensions]`.
    struct TestEmbeddingBackend {
        dims: usize,
    }

    /// An embedding backend that always returns a fixed vector.
    ///
    /// Used in semantic-search tests where we need predictable query
    /// embeddings independent of the query string length.
    struct FixedEmbeddingBackend {
        vec: Vec<f32>,
    }

    impl RigEmbeddingBackend for TestEmbeddingBackend {
        fn embed(&self, text: &str) -> Result<Vec<f32>> {
            Ok(vec![text.len() as f32; self.dims])
        }

        fn dimensions(&self) -> usize {
            self.dims
        }

        fn name(&self) -> &str {
            "test"
        }
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

    #[test]
    fn cosine_similarity_handles_orthogonal_vectors() {
        let a = vec![1.0_f32, 0.0];
        let b = vec![0.0_f32, 1.0];
        let score = cosine_similarity(&a, &b);
        assert!(score.is_some());
        assert!((score.unwrap() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn cosine_similarity_returns_none_for_zero_vector() {
        assert!(cosine_similarity(&[0.0_f32, 0.0], &[1.0_f32, 0.0]).is_none());
    }

    #[test]
    fn f32_byte_round_trip() {
        let original = vec![1.5_f32, -2.25, 0.0, 42.0];
        let bytes = f32_vec_to_bytes(&original);
        let decoded = bytes_to_f32_vec(&bytes).expect("decode");
        assert_eq!(original, decoded);
    }

    #[test]
    fn vector_store_adapter_accepts_memory_backend() {
        let adapter =
            VectorStoreAdapter::new("memory", None, Box::new(TestEmbeddingBackend { dims: 4 }))
                .expect("create memory adapter");
        assert_eq!(adapter.name(), "rig-vector-memory");
        assert!(adapter.is_empty());
    }

    #[test]
    fn memory_backend_add_and_top_n() {
        // Use a fixed query vector [0,1,0,0] so doc "b" ([0,1,0,0]) ranks
        // above doc "a" ([1,0,0,0]) regardless of string lengths.
        let backend = MemoryVectorStoreBackend::new(Box::new(FixedEmbeddingBackend {
            vec: vec![0.0_f32, 1.0, 0.0, 0.0],
        }))
        .expect("create memory backend");
        let docs = vec![
            (
                "a".to_owned(),
                serde_json::json!("short"),
                vec![1.0_f32, 0.0, 0.0, 0.0],
            ),
            (
                "b".to_owned(),
                serde_json::json!("loooooong"),
                vec![0.0_f32, 1.0, 0.0, 0.0],
            ),
        ];
        backend.add_documents(docs).expect("add");
        assert_eq!(backend.len(), 2);

        let results = backend.top_n("query", 1).expect("top_n");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, "b");
    }

    #[test]
    fn memory_backend_delete_and_clear() {
        let backend = MemoryVectorStoreBackend::new(Box::new(TestEmbeddingBackend { dims: 2 }))
            .expect("create");
        backend
            .add_documents(vec![(
                "x".to_owned(),
                serde_json::json!(1),
                vec![1.0_f32; 2],
            )])
            .expect("add");
        assert_eq!(backend.len(), 1);
        backend.delete_document("x").expect("delete");
        assert!(backend.is_empty());
        backend
            .add_documents(vec![(
                "y".to_owned(),
                serde_json::json!(2),
                vec![2.0_f32; 2],
            )])
            .expect("add");
        backend.clear().expect("clear");
        assert!(backend.is_empty());
    }

    #[cfg(feature = "vector-store-sqlite")]
    #[test]
    fn sqlite_backend_add_and_top_n() {
        let backend = SqliteVectorStoreBackend::new(
            Some(":memory:"),
            Box::new(FixedEmbeddingBackend {
                vec: vec![0.0_f32, 1.0, 0.0, 0.0],
            }),
        )
        .expect("create sqlite backend");
        let docs = vec![
            (
                "a".to_owned(),
                serde_json::json!("short"),
                vec![1.0_f32, 0.0, 0.0, 0.0],
            ),
            (
                "b".to_owned(),
                serde_json::json!("loooooong"),
                vec![0.0_f32, 1.0, 0.0, 0.0],
            ),
        ];
        backend.add_documents(docs).expect("add");
        assert_eq!(backend.len(), 2);

        let results = backend.top_n("query", 1).expect("top_n");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, "b");
    }

    #[cfg(feature = "vector-store-sqlite")]
    #[test]
    fn sqlite_backend_delete_and_clear() {
        let backend = SqliteVectorStoreBackend::new(
            Some(":memory:"),
            Box::new(TestEmbeddingBackend { dims: 2 }),
        )
        .expect("create");
        backend
            .add_documents(vec![(
                "x".to_owned(),
                serde_json::json!(1),
                vec![1.0_f32; 2],
            )])
            .expect("add");
        assert_eq!(backend.len(), 1);
        backend.delete_document("x").expect("delete");
        assert!(backend.is_empty());
        backend
            .add_documents(vec![(
                "y".to_owned(),
                serde_json::json!(2),
                vec![2.0_f32; 2],
            )])
            .expect("add");
        backend.clear().expect("clear");
        assert!(backend.is_empty());
    }

    #[cfg(feature = "vector-store-http")]
    #[test]
    fn http_backend_requires_connection_url() {
        let err = VectorStoreAdapter::new("http", None, Box::new(TestEmbeddingBackend { dims: 4 }))
            .expect_err("expected error");
        assert!(matches!(err, RigError::InvalidConfiguration(_)));
    }

    #[test]
    fn unsupported_backend_fails() {
        let err =
            VectorStoreAdapter::new("unknown", None, Box::new(TestEmbeddingBackend { dims: 4 }))
                .expect_err("expected error");
        assert!(matches!(err, RigError::InvalidConfiguration(_)));
    }
}
