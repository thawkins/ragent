//! Trait-object handles for the optional Rig-backed semantic layers.
//!
//! `ragent-agent` cannot depend on `ragent-rig` (the dependency runs the
//! other way: `ragent-rig` depends on `ragent-agent` for the embedding
//! bridge). To let the session processor and the `codeindex_search` /
//! `memory_search` tools use Rig semantic search without a compile-time
//! dependency, we define minimal object-safe traits here. The binary crate
//! (which does depend on `ragent-rig`) constructs the concrete
//! `SemanticCodeIndex` / `SemanticMemory` and boxes them into these traits.
//!
//! This mirrors the existing `ConversationMemoryPolicy` pattern used for the
//! Rig conversation-memory adapter (T-011 / FR-014 / FR-020).
//!
//! The handles return plain JSON values so the tools can format results
//! without depending on Rig-specific types.

use std::sync::Arc;

use serde_json::Value;

/// A read-only handle to a Rig-backed semantic code index
/// (`ragent_rig::codeindex::SemanticCodeIndex`).
///
/// Exposes just the hybrid-search operation the `codeindex_search` tool needs
/// (FR-021). The concrete implementation lives in the binary crate.
pub trait SemanticCodeIndexHandle: Send + Sync {
    /// Run a hybrid (lexical + semantic) search and return results as a JSON
    /// array. Each element is an object with `file_path`, `semantic_score`,
    /// `lexical_score`, `language`, `line_count`, and `document` fields,
    /// mirroring `ragent_rig::codeindex::SemanticHit`.
    ///
    /// # Errors
    ///
    /// Returns an error string if the underlying search fails.
    fn hybrid_search(&self, query: &str, n: usize) -> std::result::Result<Vec<Value>, String>;

    /// Returns `true` if the semantic layer is enabled and the embedding
    /// backend is available.
    fn is_available(&self) -> bool;
}

/// A read-only handle to a Rig-backed semantic structured-memory layer
/// (`ragent_rig::memory_semantic::SemanticMemory`).
///
/// Exposes just the semantic-search operation the `memory_search` tool needs
/// (FR-010). The concrete implementation lives in the binary crate.
pub trait SemanticMemoryHandle: Send + Sync {
    /// Run a pure semantic search over structured memories and return
    /// results as a JSON array. Each element is an object with `memory_id`,
    /// `semantic_score`, `content`, `category`, `source`, `confidence`, and
    /// `tags` fields, mirroring `ragent_rig::memory_semantic::SemanticMemoryHit`.
    ///
    /// # Errors
    ///
    /// Returns an error string if the underlying search fails.
    fn semantic_search(
        &self,
        query: &str,
        n: usize,
    ) -> std::result::Result<Vec<Value>, String>;

    /// Returns `true` if the semantic layer is enabled and the embedding
    /// backend is available.
    fn is_available(&self) -> bool;
}

/// Helper to downcast a processor's semantic-code-index OnceLock contents
/// into an `Option<Arc<dyn SemanticCodeIndexHandle>>` for tool contexts.
pub fn semantic_code_index_from_processor(
    lock: &std::sync::OnceLock<Option<Arc<dyn SemanticCodeIndexHandle + Send + Sync>>>,
) -> Option<Arc<dyn SemanticCodeIndexHandle + Send + Sync>> {
    lock.get().and_then(|opt| opt.clone())
}

/// Helper to downcast a processor's semantic-memory OnceLock contents
/// into an `Option<Arc<dyn SemanticMemoryHandle>>` for tool contexts.
pub fn semantic_memory_from_processor(
    lock: &std::sync::OnceLock<Option<Arc<dyn SemanticMemoryHandle + Send + Sync>>>,
) -> Option<Arc<dyn SemanticMemoryHandle + Send + Sync>> {
    lock.get().and_then(|opt| opt.clone())
}