//! Persistent storage layer.
//!
//! This module is a thin re-export of the canonical implementation in
//! [`ragent_storage`].  The agent crate previously held its own ~2,200-line
//! copy of `Storage`, `SessionRow`, `TodoRow`, `MemoryRow`, the
//! `encrypt_key` / `decrypt_key` / `obfuscate_key` / `deobfuscate_key`
//! helpers, and the knowledge-graph / embedding-search methods
//! (`list_entities`, `list_relationships`, `query_entity_neighbours`,
//! `search_memories_by_embedding`, `has_assistant_messages`); that copy has
//! been consolidated into `ragent-storage` to eliminate the duplication (see
//! `REMPLAN.md` M2 / T2.2).
//!
//! The PERF-004 `has_format_version` cache (previously an agent-only
//! addition) and the knowledge-graph / embedding-search methods have all
//! been ported into the canonical `ragent_storage::Storage`, so behaviour is
//! unchanged.  The KG row types (`KgEntityRow`, `KgRelationshipRow`) and
//! `EmbeddingMatch` live in `ragent-storage` and are mapped by the agent
//! crate's `memory::knowledge_graph` / `memory::embedding` modules into
//! their richer `Entity` / `Relationship` / `SimilarityResult` types.
//!
//! All existing `use crate::storage::{…}` sites in `ragent-agent` continue
//! to resolve via the re-exports below.

pub use ragent_storage::{
    EmbeddingMatch, KgEntityRow, KgRelationshipRow, MemoryRow, RunCostSummaryRow, SessionRow,
    Storage, TodoRow, decrypt_key, deobfuscate_key, encrypt_key, obfuscate_key,
};
