//! Semantic-retrieval augmentation for the research system (T-012).
//!
//! Defines the [`SemanticResearchAugmentor`] trait that the Rig integration
//! implements. Keeping the trait in `ragent-research` avoids a dependency
//! cycle: `ragent-rig` depends on `ragent-research` and can provide a concrete
//! implementation, while `ragent-agent` only needs the trait to pass it through
//! to [`ResearchSession`](crate::session::ResearchSession).

use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::source::Source;

/// What kind of information a semantic hit represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticHitKind {
    /// A prior research item's topic/title.
    PriorTopic,
    /// A source captured in a prior research item.
    PriorSource,
    /// A source captured during the current research run.
    CapturedSource,
    /// A synthesized finding from a prior or current research run.
    Finding,
}

/// One document retrieved by vector similarity during semantic research.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticHit {
    /// Stable identifier, usually prefixed by kind (`prior:`, `source:`).
    pub id: String,
    /// Cosine-similarity score (0–1, higher is better).
    pub score: f64,
    /// Kind of hit.
    pub kind: SemanticHitKind,
    /// Human-readable title.
    pub title: String,
    /// Short text snippet used as the body of an injected source.
    pub snippet: String,
    /// Provider-specific metadata (URL, path, research name, etc.).
    pub payload: serde_json::Value,
}

/// Bridge that lets the research session embed captured sources and retrieve
/// semantically similar prior findings/sources (FR-016 / FR-017 / NFR-006).
#[async_trait]
pub trait SemanticResearchAugmentor: Send + Sync {
    /// Embed and store the captured sources from the current run.
    ///
    /// Returns the number of documents added to the vector store.
    async fn index_sources(&self, sources: &[Source]) -> anyhow::Result<usize>;

    /// Embed and store the current research document (title, topic, sources).
    ///
    /// This makes the current run retrievable by later runs (FR-016).
    async fn index_document(
        &self,
        name: &str,
        title: &str,
        topic: &str,
        sources: &[Source],
    ) -> anyhow::Result<usize>;

    /// Retrieve up to `n` documents that are semantically similar to `topic`.
    ///
    /// Implementations may lazily back-fill prior research items when the
    /// vector store is empty.
    async fn retrieve_for_topic(&self, topic: &str, n: usize) -> anyhow::Result<Vec<SemanticHit>>;
}

/// Wrap an `Arc<T>` as a `Box<dyn SemanticResearchAugmentor>`.
///
/// The orphan rules prevent implementing `From` for this because both `Arc`
/// and `Box` are foreign types, so this free function is provided instead.
pub fn boxed<T: SemanticResearchAugmentor + Send + Sync + 'static>(
    value: Arc<T>,
) -> Box<dyn SemanticResearchAugmentor + Send + Sync + 'static> {
    Box::new(ArcWrapper(value))
}

struct ArcWrapper<T>(Arc<T>);

#[async_trait]
impl<T: SemanticResearchAugmentor + Send + Sync + 'static> SemanticResearchAugmentor
    for ArcWrapper<T>
{
    async fn index_sources(&self, sources: &[Source]) -> anyhow::Result<usize> {
        self.0.index_sources(sources).await
    }

    async fn index_document(
        &self,
        name: &str,
        title: &str,
        topic: &str,
        sources: &[Source],
    ) -> anyhow::Result<usize> {
        self.0.index_document(name, title, topic, sources).await
    }

    async fn retrieve_for_topic(&self, topic: &str, n: usize) -> anyhow::Result<Vec<SemanticHit>> {
        self.0.retrieve_for_topic(topic, n).await
    }
}

/// Wrap an `Arc<T>` as an `Arc<dyn SemanticResearchAugmentor>`.
///
/// Like [`boxed`], this is a free function because the orphan rules block a
/// blanket `From` impl for the foreign `Arc` type.
pub fn arc_boxed<T: SemanticResearchAugmentor + Send + Sync + 'static>(
    value: Arc<T>,
) -> Arc<dyn SemanticResearchAugmentor + Send + Sync + 'static> {
    value
}
