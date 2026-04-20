---
title: "Cross-Project Memory Sharing in Ragent Core"
source: "cross_project"
type: source
tags: [rust, memory-management, cross-project, agent-framework, configuration, scope-resolution, knowledge-base, trait-abstraction, testing]
generated: "2026-04-19T21:49:17.721627032+00:00"
---

# Cross-Project Memory Sharing in Ragent Core

This Rust source file implements a cross-project memory sharing system for the Ragent agent framework, enabling intelligent agents to access memory blocks across both global and project-scoped contexts. The module provides three core capabilities: resolving block labels with configurable precedence rules, listing all available labels with deduplication, and performing cross-project searches with substring matching. The system operates through a hierarchical scope model where global memory blocks stored in `~/.ragent/memory/` can be accessed from any project, while project-specific blocks take precedence when `project_override` is enabled. The implementation uses a trait-based storage abstraction (`BlockStorage`) to remain backend-agnostic, with concrete implementations like `FileBlockStorage` handling the actual persistence. Comprehensive test coverage validates the resolution logic, including scenarios for disabled cross-project mode, project override behavior, and coexisting blocks without override.

The cross-project memory system addresses a fundamental challenge in agent-based development: maintaining contextual continuity across different projects while respecting project boundaries. When `cross_project.enabled` is true, agents can leverage shared knowledge bases such as coding patterns, architectural guidelines, or persona definitions without duplicating them in every project. The `ResolvedBlock` type captures resolution metadata including which scope won and whether shadowing occurred, enabling transparent debugging of memory precedence. The configuration granularity allows teams to enable global search independently from override behavior, supporting workflows where global templates provide defaults but project customizations are preserved. This design reflects principles from configuration management systems and software package managers, where scope precedence and shadowing are well-established patterns for managing namespaced resources.

## Related

### Entities

- [Ragent](../entities/ragent.md) — product
- [tempfile](../entities/tempfile.md) — technology
- [FileBlockStorage](../entities/fileblockstorage.md) — technology

### Concepts

- [Cross-Project Memory Scope Resolution](../concepts/cross-project-memory-scope-resolution.md)
- [Configuration-Driven Feature Flags](../concepts/configuration-driven-feature-flags.md)
- [Memory Block Shadowing](../concepts/memory-block-shadowing.md)
- [Trait-Based Storage Abstraction](../concepts/trait-based-storage-abstraction.md)

