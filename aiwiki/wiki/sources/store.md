---
title: "Ragent Core Structured Memory Store Implementation"
source: "store"
type: source
tags: [rust, memory-management, sqlite, fts5, builder-pattern, domain-modeling, serde, serialization, agent-systems, knowledge-base]
generated: "2026-04-19T21:44:33.454527758+00:00"
---

# Ragent Core Structured Memory Store Implementation

This document contains the Rust source code for `store.rs`, a core component of the ragent-core crate that implements structured memory storage with metadata support. The module provides a type-safe, builder-pattern-based API for creating and managing structured memories—individual facts, patterns, preferences, insights, errors, or workflows stored in SQLite with rich metadata including categories, confidence scores, tags, sources, and temporal tracking. The implementation distinguishes itself from the file-based MemoryBlock system by offering fine-grained, queryable storage rather than freeform Markdown documents.

The core abstractions include `StructuredMemory`, a domain type representing a single memory entry with twelve fields including content, category, confidence scoring (0.0-1.0), provenance tracking (source, project, session), tagging support, and access metrics. The `ForgetFilter` enum provides flexible criteria for memory deletion, supporting both precise ID-based removal and broad filter-based cleanup by age, confidence threshold, category, or tag intersection. The module enforces data integrity through compile-time constants defining valid categories and runtime validation methods that return descriptive error messages.

The implementation demonstrates several Rust idioms including the builder pattern for ergonomic struct construction, type-safe enums with struct variants for complex data, and comprehensive unit testing covering validation logic, builder chaining, edge cases like confidence clamping, and filter criterion detection. The code integrates with a larger storage subsystem through `crate::storage::Storage` for CRUD operations and uses SQLite with FTS5 for full-text search capabilities across memory content.

## Related

### Entities

- [StructuredMemory](../entities/structuredmemory.md) — technology
- [ForgetFilter](../entities/forgetfilter.md) — technology

### Concepts

- [Structured Memory Architecture](../concepts/structured-memory-architecture.md)
- [Builder Pattern in Rust](../concepts/builder-pattern-in-rust.md)
- [Confidence Scoring in Knowledge Systems](../concepts/confidence-scoring-in-knowledge-systems.md)
- [Memory Forgetting and Lifecycle Management](../concepts/memory-forgetting-and-lifecycle-management.md)

