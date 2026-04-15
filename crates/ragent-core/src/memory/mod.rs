//! Persistent memory system for ragent agents.
//!
//! This module provides structured, named memory blocks that persist across
//! sessions. Memory blocks are stored as Markdown files with YAML frontmatter
//! in `.ragent/memory/` (project scope) or `~/.ragent/memory/` (global scope).
//!
//! # Block format
//!
//! Each block file (e.g. `patterns.md`) contains:
//!
//! ```markdown
//! ---
//! label: patterns
//! description: Coding patterns observed in this project
//! scope: project
//! limit: 2048
//! read_only: false
//! created_at: 2025-01-15T10:30:00Z
//! updated_at: 2025-01-15T10:30:00Z
//! ---
//!
//! Block content goes here.
//! ```

pub mod block;
pub mod compact;
pub mod cross_project;
pub mod defaults;
pub mod embedding;
pub mod extract;
pub mod import_export;
pub mod journal;
pub mod knowledge_graph;
pub mod migrate;
pub mod storage;
pub mod store;
pub mod visualisation;

pub use block::{BlockScope, MemoryBlock};
pub use compact::{
    CompactionResult, CompactionTrigger, DedupResult, EvictionResult, FullCompactionResult,
    apply_dedup_merge, compact_block_content, compact_blocks, deduplicate_memory,
    evict_stale_memories, merge_content, merge_tags, run_compaction,
};
pub use cross_project::{
    ResolvedBlock, list_all_labels, resolve_block, search_blocks_cross_project,
};
pub use embedding::{
    EmbeddingProvider, NoOpEmbedding, SimilarityResult, cosine_similarity, deserialise_embedding,
    serialise_embedding,
};
pub use extract::{
    ExtractionEngine, MemoryCandidate, SessionMessageSummary, ToolCallSummary, decay_confidence,
};
pub use import_export::{
    ExportResult, ImportResult, MemoryBlocksExport, MemoryExport, export_all, import_claude_code,
    import_cline, import_ragent,
};
pub use journal::{JournalEntry, JournalEntrySummary};
pub use knowledge_graph::{
    Entity, EntityType, ExtractedEntity, ExtractedRelationship, ExtractionResult, KnowledgeGraph,
    RelationType, Relationship, extract_entities, get_knowledge_graph, store_extraction,
};
pub use storage::{BlockStorage, FileBlockStorage, load_all_blocks};
pub use store::{ForgetFilter, MEMORY_CATEGORIES, StructuredMemory};
pub use visualisation::{
    AccessHeatmap, AccessHeatmapEntry, GraphEdge, GraphNode, MemoryGraph, TagCloud, TagCloudEntry,
    Timeline, TimelineEntry, VisualisationData, generate_graph, generate_heatmap,
    generate_tag_cloud, generate_timeline, generate_visualisation,
};
