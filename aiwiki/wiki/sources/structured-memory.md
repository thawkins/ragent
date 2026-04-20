---
title: "Structured Memory Tools for Agent Memory Management in Rust"
source: "structured_memory"
type: source
tags: [rust, agent-systems, memory-management, sqlite, fts5, ai-tools, structured-data, full-text-search, knowledge-base, event-driven]
generated: "2026-04-19T18:57:31.761455612+00:00"
---

# Structured Memory Tools for Agent Memory Management in Rust

This document presents a Rust implementation of three specialized tools for managing structured memory in an AI agent system: MemoryStoreTool, MemoryRecallTool, and MemoryForgetTool. These tools provide a sophisticated SQLite-backed memory system that enables agents to store, retrieve, and delete discrete facts, patterns, preferences, insights, errors, and workflows with rich metadata including categories, tags, and confidence scores. Unlike simpler file-based memory systems, this structured approach enables intelligent retrieval through full-text search (FTS5) combined with multi-dimensional filtering, making it suitable for long-term knowledge management in conversational agents.

The architecture implements a clear separation between storage concerns and tool interfaces, with each tool adhering to a common Tool trait that defines standardized methods for name, description, parameter schema, permission categorization, and asynchronous execution. The MemoryStoreTool handles creation of memories with validation for categories against predefined constants, confidence scoring on a 0.0-1.0 scale, and tag normalization. It emits MemoryStored events for observability and returns structured metadata including the generated memory ID. The MemoryRecallTool leverages FTS5 for efficient full-text search while supporting layered filtering by categories, required tags, and minimum confidence thresholds, with automatic access count tracking to identify frequently referenced knowledge. The MemoryForgetTool provides safe deletion capabilities with mandatory filter criteria to prevent accidental data loss, supporting both single-ID deletion and batch operations based on age, confidence, category, or tag combinations.

Permission management is integrated through category-based access control, with write operations classified under "file:write" and read operations under "file:read". The system requires an available SQLite storage backend and operates within a session-scoped context that tracks working directory and session identifiers for project isolation. Event publishing enables external monitoring of memory operations, supporting analytics and debugging workflows. The implementation demonstrates mature Rust patterns including extensive use of Option and Result types for error handling, iterator chains for data transformation, and async_trait for asynchronous trait methods. This structured memory system represents a significant advancement over unstructured text storage, enabling agents to build persistent, queryable knowledge bases that improve over time through confidence scoring and access patterns.

## Related

### Entities

- [MemoryStoreTool](../entities/memorystoretool.md) — technology
- [MemoryRecallTool](../entities/memoryrecalltool.md) — technology
- [MemoryForgetTool](../entities/memoryforgettool.md) — technology
- [StructuredMemory](../entities/structuredmemory.md) — technology

### Concepts

- [Structured Memory Systems](../concepts/structured-memory-systems.md)
- [Full-Text Search with FTS5](../concepts/full-text-search-with-fts5.md)
- [Event-Driven Architecture in Agent Systems](../concepts/event-driven-architecture-in-agent-systems.md)
- [Tool-Based Agent Architecture](../concepts/tool-based-agent-architecture.md)

