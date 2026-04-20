---
title: "Ragent Memory Tools: Persistent Memory Management for AI Agents"
source: "memory_write"
type: source
tags: [rust, ai-agents, memory-management, persistent-storage, ragent, tool-system, block-storage, yaml-frontmatter, async-rust]
generated: "2026-04-19T18:35:08.640156330+00:00"
---

# Ragent Memory Tools: Persistent Memory Management for AI Agents

This source code implements a comprehensive memory management system for AI agents in the Ragent framework, providing persistent storage capabilities across sessions. The implementation centers around three core tools—MemoryWriteTool, MemoryReadTool, and MemoryReplaceTool—that enable agents to store, retrieve, and modify information in a structured manner. The system supports two primary storage modes: a legacy flat-file approach using MEMORY.md files, and a modern block-based architecture where each memory block is stored as a separate Markdown file with YAML frontmatter metadata. This dual-mode design ensures backward compatibility while offering enhanced flexibility through labeled, scoped, and metadata-rich memory blocks.

The memory system implements a hierarchical scoping mechanism with three levels: global/user scope (stored in ~/.ragent/memory/), project scope (stored in .ragent/memory/ within the working directory), and cross-project resolution capabilities. Each memory block can be configured with descriptions, size limits, and read-only flags, providing fine-grained control over memory organization. The implementation leverages Rust's async/await patterns with the async-trait crate, uses serde_json for parameter serialization, and integrates with the anyhow crate for robust error handling. The code demonstrates sophisticated software engineering practices including input validation, content size enforcement, timestamp tracking, and surgical text replacement with uniqueness checking to prevent ambiguous edits.

## Related

### Entities

- [MemoryWriteTool](../entities/memorywritetool.md) — technology
- [MemoryReadTool](../entities/memoryreadtool.md) — technology
- [MemoryReplaceTool](../entities/memoryreplacetool.md) — technology
- [FileBlockStorage](../entities/fileblockstorage.md) — technology

