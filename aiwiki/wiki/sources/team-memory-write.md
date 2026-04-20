---
title: "Team Memory Write Tool Implementation in Rust"
source: "team_memory_write"
type: source
tags: [rust, async, agent-framework, memory-management, file-io, security, serde-json, multi-agent-systems, tool-implementation, persistence]
generated: "2026-04-19T19:18:08.152755006+00:00"
---

# Team Memory Write Tool Implementation in Rust

This document contains the implementation of `TeamMemoryWriteTool`, a Rust-based tool that enables AI agents to write or append content to persistent memory files within a team collaboration framework. The tool is part of a larger agent orchestration system (referred to as "ragent") that provides structured memory management for multi-agent teams. The implementation demonstrates robust error handling using the `anyhow` crate, JSON parameter validation with `serde_json`, and secure file path resolution to prevent directory traversal attacks.

The tool operates within a permission-based architecture, categorized under "team:communicate", and supports two primary write modes: append (default) and overwrite. It integrates with team-specific storage abstractions through `TeamStore` and `MemoryScope` enums, allowing memory to be scoped either per-user or per-project depending on configuration. The implementation includes sophisticated path validation logic that canonicalizes paths and ensures target files remain within designated memory directories, addressing critical security concerns in multi-tenant agent environments.

The codebase reflects modern Rust async patterns using `#[async_trait::async_trait]` for trait implementation, and demonstrates idiomatic error propagation through the `Result` type. The tool's design accommodates flexible memory organization through configurable relative paths, with sensible defaults (MEMORY.md) for quick agent onboarding. Metadata-rich return values provide callers with detailed operation outcomes including byte counts, line counts, and resolved directory paths, enabling comprehensive logging and debugging capabilities in production agent deployments.

## Related

### Entities

- [TeamMemoryWriteTool](../entities/teammemorywritetool.md) — technology
- [MemoryScope](../entities/memoryscope.md) — technology
- [TeamStore](../entities/teamstore.md) — technology
- [ToolContext](../entities/toolcontext.md) — technology
- [ToolOutput](../entities/tooloutput.md) — technology

### Concepts

- [Agent Memory Persistence](../concepts/agent-memory-persistence.md)
- [Path Traversal Prevention](../concepts/path-traversal-prevention.md)
- [Multi-Agent Permission Categories](../concepts/multi-agent-permission-categories.md)
- [Async Tool Trait Pattern](../concepts/async-tool-trait-pattern.md)

