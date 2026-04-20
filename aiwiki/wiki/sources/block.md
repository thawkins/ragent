---
title: "Memory Block Implementation in Rust"
source: "block"
type: source
tags: [rust, memory-management, serialization, yaml, markdown, persistence, agent-systems, data-structures]
generated: "2026-04-19T21:54:07.311775923+00:00"
---

# Memory Block Implementation in Rust

This document presents the complete implementation of a memory block system in Rust, designed for the ragent-core crate. The `block.rs` file defines the core data structures and serialization logic for persistent memory storage in an AI agent system. The implementation centers around the `MemoryBlock` struct, which represents a named, scoped unit of persistent memory that can be stored as human-readable Markdown files with YAML frontmatter. This design choice enables version control integration and manual editing while maintaining structured metadata.

The system supports two distinct storage scopes: Global memory stored in the user's home directory (`~/.ragent/memory/`) for cross-project persistence, and Project-specific memory stored within the working directory (`.ragent/memory/`) for context-local data. The implementation includes comprehensive validation for block labels, size limits for content enforcement, read-only flags for protected data, and automatic timestamp management for tracking creation and modification times. The use of serde for YAML serialization and chrono for ISO 8601 timestamp handling demonstrates production-quality Rust patterns for data persistence.

The file also contains extensive unit tests covering serialization roundtrips, label validation, scope resolution, and backward compatibility with legacy plain Markdown files. Helper functions handle frontmatter parsing, directory resolution, and datetime parsing with appropriate fallback behaviors. The builder pattern implementation through `with_*` methods provides an ergonomic API for constructing and modifying memory blocks, while the `BlockScope` enum with its `as_str` and `from_param` methods enables seamless integration with tool parameter parsing.

## Related

### Entities

- [MemoryBlock](../entities/memoryblock.md) — technology
- [BlockScope](../entities/blockscope.md) — technology
- [FrontmatterData](../entities/frontmatterdata.md) — technology

