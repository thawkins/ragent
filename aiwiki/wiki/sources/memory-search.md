---
title: "MemorySearchTool: Semantic and Keyword-Based Memory Retrieval"
source: "memory_search"
type: source
tags: [rust, semantic-search, vector-embeddings, fts5, memory-management, ai-agents, cosine-similarity, tool-architecture, sqlite, cross-project]
generated: "2026-04-19T17:07:37.127652746+00:00"
---

# MemorySearchTool: Semantic and Keyword-Based Memory Retrieval

The memory_search.rs module implements MemorySearchTool, a sophisticated Rust-based tool for retrieving stored information through multiple search strategies. This tool bridges traditional keyword search with modern semantic similarity techniques, providing a unified interface for querying both structured memories (stored in SQLite with metadata like categories, sources, confidence scores, and tags) and file-based memory blocks. The implementation demonstrates a thoughtful fallback architecture: when vector embeddings are available, it performs cosine similarity search over high-dimensional dense vectors; otherwise, it degrades gracefully to FTS5 full-text search. The module also features lazy embedding generation, automatically computing embeddings for memories that were stored before the embedding capability was enabled.

The tool's design reflects important architectural decisions about AI system memory management. Memory blocks support cross-project scoping with shadowing semantics, allowing project-specific memories to override global defaults—a pattern essential for multi-tenant AI assistants. The search API exposes tunable parameters including similarity thresholds and result limits, giving calling agents control over precision-recall tradeoffs. Event emission enables observability and downstream analytics, tracking search patterns across sessions. This implementation sits within a larger agent framework where tools declare their permission categories (here "file:read") and integrate with a structured event bus for loose coupling.

## Related

### Entities

- [MemorySearchTool](../entities/memorysearchtool.md) — technology
- [NoOpEmbedding](../entities/noopembedding.md) — technology
- [FileBlockStorage](../entities/fileblockstorage.md) — technology

### Concepts

- [Semantic Search with Vector Embeddings](../concepts/semantic-search-with-vector-embeddings.md)
- [FTS5 Full-Text Search](../concepts/fts5-full-text-search.md)
- [Cross-Project Memory Scoping](../concepts/cross-project-memory-scoping.md)
- [Lazy Embedding Generation](../concepts/lazy-embedding-generation.md)
- [Tool Trait Architecture](../concepts/tool-trait-architecture.md)

