//! Rig-backed semantic retrieval augmentation for `/research` (T-012).
//!
//! Implements [`SemanticResearchAugmentor`] from `ragent-research` using a
//! [`VectorStoreAdapter`] and a [`RigEmbeddingBackend`]. The module lives in
//! `ragent-rig` because it needs `rig-core` embedding and vector-store support,
//! while `ragent-research` only defines the trait (avoiding a dependency cycle).
//!
//! The augmentor:
//!
//! * embeds captured web/local sources after the gather phase (FR-017),
//! * embeds the completed research document so later runs can find it,
//! * lazily back-fills prior research item topics when the vector store is empty,
//! * retrieves semantically similar prior findings/sources for a new topic
//!   (FR-016).

use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use ragent_research::{
    ResearchManager, SemanticHit, SemanticHitKind, SemanticResearchAugmentor, Source,
};
use serde_json::json;

use crate::embeddings_trait::RigEmbeddingBackend;
use crate::error::{Result, RigError};
use crate::vector_store::VectorStoreAdapter;

/// A Rig-backed semantic research augmentor.
///
/// Holds a `ResearchManager` (to read prior research items for back-fill),
/// a [`VectorStoreAdapter`] for dense retrieval, and an embedding backend.
pub struct ResearchAugmentor {
    manager: ResearchManager,
    vector_store: VectorStoreAdapter,
    embedding: Arc<dyn RigEmbeddingBackend>,
    max_chars: usize,
}

impl Clone for ResearchAugmentor {
    fn clone(&self) -> Self {
        // Rebuild the vector store with a freshly-boxed clone of the shared
        // embedding backend. The blanket `impl RigEmbeddingBackend for Arc<T>`
        // (see `embeddings_trait`) lets us hand the `Arc` to
        // `VectorStoreAdapter::new`, which expects a `Box<dyn RigEmbeddingBackend>`.
        let embedding_box: Box<dyn RigEmbeddingBackend> =
            Box::new(std::sync::Arc::clone(&self.embedding));
        Self {
            manager: self.manager.clone(),
            vector_store: VectorStoreAdapter::new(self.vector_store.name(), None, embedding_box)
                .expect("research augmentor clone should rebuild vector store"),
            embedding: Arc::clone(&self.embedding),
            max_chars: self.max_chars,
        }
    }
}

impl std::fmt::Debug for ResearchAugmentor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResearchAugmentor")
            .field("research_root", &self.manager.root())
            .field("vector_store", &self.vector_store)
            .field("embedding", &self.embedding.name())
            .finish()
    }
}

impl ResearchAugmentor {
    /// Build a new augmentor.
    ///
    /// Returns an error if the embedding backend is unavailable or the vector
    /// store cannot be opened.
    pub fn new(
        manager: ResearchManager,
        vector_store: VectorStoreAdapter,
        embedding: Box<dyn RigEmbeddingBackend>,
    ) -> Result<Self> {
        if !embedding.is_available() {
            return Err(RigError::BackendError(
                "embedding backend is not available".to_owned(),
            ));
        }
        Ok(Self {
            manager,
            vector_store,
            embedding: Arc::from(embedding),
            max_chars: 8_000,
        })
    }

    /// Convenience constructor from configuration pieces.
    pub fn from_parts(
        manager: ResearchManager,
        vector_store: VectorStoreAdapter,
        embedding: Box<dyn RigEmbeddingBackend>,
    ) -> Result<Self> {
        Self::new(manager, vector_store, embedding)
    }

    fn truncate(&self, text: &str) -> String {
        let end = text.len().min(self.max_chars);
        text[..end].to_string()
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        self.embedding.embed(&self.truncate(text))
    }

    /// When the vector store is empty, index the topic/title of every prior
    /// research item so subsequent runs can retrieve them.
    async fn backfill_prior_research(&self) -> Result<usize> {
        let items =
            self.manager.list(true).await.map_err(|e| {
                RigError::BackendError(format!("failed to list prior research: {e}"))
            })?;
        let mut added = 0;
        for item in items {
            let id = format!("prior:{}:topic", item.name);
            let text = format!("{} {} {}", item.title, item.topic, item.name);
            let embedding = self.embed(&text)?;
            let payload = json!({
                "kind": "prior_topic",
                "name": item.name.as_str(),
                "title": item.title,
                "topic": item.topic,
                "status": item.status.as_str(),
            });
            self.vector_store
                .add_documents(vec![(id, payload, embedding)])?;
            added += 1;
        }
        Ok(added)
    }
}

#[async_trait]
impl SemanticResearchAugmentor for ResearchAugmentor {
    async fn index_sources(&self, sources: &[Source]) -> anyhow::Result<usize> {
        let mut docs = Vec::new();
        for src in sources {
            let (id, title, body, kind) = match src {
                Source::Web {
                    url, title, body, ..
                } => (format!("source:{url}"), title.clone(), body.clone(), "web"),
                Source::Local { path, body, .. } => (
                    format!("source:{path}"),
                    path.clone(),
                    body.clone(),
                    "local",
                ),
                Source::Other { label, body, .. } => (
                    format!("source:{label}"),
                    label.clone(),
                    body.clone(),
                    "other",
                ),
                Source::Spec {
                    spec_id, relevance, ..
                } => (
                    format!("source:{spec_id}"),
                    spec_id.clone(),
                    relevance.clone(),
                    "spec",
                ),
            };
            if body.is_empty() {
                continue;
            }
            let embedding = self.embed(&body).map_err(anyhow::Error::from)?;
            let payload = json!({
                "kind": kind,
                "title": title,
                "text": self.truncate(&body),
            });
            docs.push((id, payload, embedding));
        }
        let count = docs.len();
        if count == 0 {
            return Ok(0);
        }
        self.vector_store
            .add_documents(docs)
            .map_err(anyhow::Error::from)?;
        Ok(count)
    }
    async fn index_document(
        &self,
        name: &str,
        title: &str,
        topic: &str,
        sources: &[Source],
    ) -> anyhow::Result<usize> {
        let mut docs = Vec::new();
        let topic_text = format!("{title} {topic}");
        let embedding = self.embed(&topic_text).map_err(anyhow::Error::from)?;
        docs.push((
            format!("prior:{name}:topic"),
            json!({
                "kind": "prior_topic",
                "name": name,
                "title": title,
                "topic": topic,
            }),
            embedding,
        ));

        for (idx, src) in sources.iter().enumerate() {
            let (id, title, body, kind) = match src {
                Source::Web { title, body, .. } => (
                    format!("prior:{name}:source:{idx}"),
                    title.clone(),
                    body.clone(),
                    "web",
                ),
                Source::Local { path, body, .. } => (
                    format!("prior:{name}:source:{idx}"),
                    path.clone(),
                    body.clone(),
                    "local",
                ),
                Source::Other { label, body, .. } => (
                    format!("prior:{name}:source:{idx}"),
                    label.clone(),
                    body.clone(),
                    "other",
                ),
                Source::Spec {
                    spec_id, relevance, ..
                } => (
                    format!("prior:{name}:source:{idx}"),
                    spec_id.clone(),
                    relevance.clone(),
                    "spec",
                ),
            };
            if body.is_empty() {
                continue;
            }
            let embedding = self.embed(&body).map_err(anyhow::Error::from)?;
            docs.push((
                id,
                json!({
                    "kind": kind,
                    "research_name": name,
                    "title": title,
                    "text": self.truncate(&body),
                }),
                embedding,
            ));
        }

        let count = docs.len();
        if count == 0 {
            return Ok(0);
        }
        self.vector_store
            .add_documents(docs)
            .map_err(anyhow::Error::from)?;
        Ok(count)
    }
    async fn retrieve_for_topic(&self, topic: &str, n: usize) -> anyhow::Result<Vec<SemanticHit>> {
        if n == 0 {
            return Ok(Vec::new());
        }

        // Lazily backfill prior research topics if the store is empty.
        if self.vector_store.is_empty() {
            match self.backfill_prior_research().await {
                Ok(count) => tracing::info!(count, "research: back-filled prior research topics"),
                Err(e) => tracing::warn!(error = %e, "research: prior-research backfill failed"),
            }
        }

        let raw = self
            .vector_store
            .top_n(topic, n)
            .map_err(anyhow::Error::from)?;
        let mut hits = Vec::with_capacity(raw.len());
        let mut seen = HashSet::new();
        for (score, id, document) in raw {
            if !seen.insert(id.clone()) {
                continue;
            }
            let kind = if id.starts_with("prior:") {
                SemanticHitKind::PriorTopic
            } else {
                SemanticHitKind::PriorSource
            };
            let title = document
                .get("title")
                .and_then(|v| v.as_str())
                .or_else(|| document.get("name").and_then(|v| v.as_str()))
                .unwrap_or(&id)
                .to_string();
            let snippet = document
                .get("topic")
                .and_then(|v| v.as_str())
                .or_else(|| document.get("text").and_then(|v| v.as_str()))
                .unwrap_or("")
                .to_string();
            hits.push(SemanticHit {
                id,
                score,
                kind,
                title,
                snippet,
                payload: document,
            });
        }
        Ok(hits)
    }
}
