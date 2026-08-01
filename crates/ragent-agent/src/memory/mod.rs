//! Persistent memory system for ragent agents.
//!
//! This module provides structured memories that persist across sessions.
//! Memories are individual facts, patterns, preferences, insights, errors, or
//! workflows stored in SQLite with metadata (category, confidence, tags,
//! source). Freeform Markdown file blocks have been removed in favour of the
//! structured-memory backend.

pub mod embedding;
pub mod extract;
pub mod knowledge_graph;
pub mod store;
pub mod visualisation;

pub use embedding::{
    EmbeddingProvider, NoOpEmbedding, SimilarityResult, cosine_similarity, deserialise_embedding,
    serialise_embedding,
};
pub use extract::{
    ExtractionEngine, MemoryCandidate, SessionMessageSummary, ToolCallSummary, decay_confidence,
};
pub use knowledge_graph::{
    Entity, EntityType, ExtractedEntity, ExtractedRelationship, ExtractionResult, KnowledgeGraph,
    RelationType, Relationship, extract_entities, get_knowledge_graph, store_extraction,
};
pub use store::{ForgetFilter, MEMORY_CATEGORIES, StructuredMemory};
pub use visualisation::{
    AccessHeatmap, AccessHeatmapEntry, GraphEdge, GraphNode, MemoryGraph, TagCloud, TagCloudEntry,
    VisualisationData, generate_graph, generate_heatmap, generate_tag_cloud,
    generate_visualisation,
};
