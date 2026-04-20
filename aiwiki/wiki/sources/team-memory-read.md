---
title: "Team Memory Read Tool Implementation"
source: "team_memory_read"
type: source
tags: [rust, ai-agents, memory-systems, multi-agent, filesystem-security, path-traversal-prevention, async-trait, serde-json, tool-architecture]
generated: "2026-04-19T19:15:44.484266788+00:00"
---

# Team Memory Read Tool Implementation

This Rust source file implements `TeamMemoryReadTool`, a tool that enables AI agents to read from persistent memory directories within a team-based architecture. The tool forms part of a larger multi-agent system where agents can store and retrieve contextual information across sessions, facilitating long-term memory and knowledge persistence. The implementation demonstrates sophisticated security considerations including path traversal prevention through canonicalization checks, scoping mechanisms to control memory visibility (user-level vs project-level vs disabled), and integration with a team configuration system that manages agent membership and permissions.

The tool operates within a structured permission category (`team:communicate`) and accepts parameters for team identification and file path resolution. When executed, it validates the requesting agent's membership in the specified team, determines the appropriate memory scope based on configuration, and safely reads file contents while preventing directory escape attacks. The implementation showcases patterns for secure filesystem operations in Rust, including careful error handling with the `anyhow` crate, JSON schema validation for tool parameters, and metadata-rich return values that provide diagnostic information about the read operation.

This component exemplifies modern approaches to AI agent memory systems, where persistence enables more sophisticated agent behaviors including continuity across conversations, learned preferences, and collaborative knowledge building within agent teams. The architectural separation between session-scoped and persistent memory, combined with team-based access controls, reflects production-grade design patterns for multi-tenant AI systems.

## Related

### Entities

- [TeamMemoryReadTool](../entities/teammemoryreadtool.md) — technology
- [MemoryScope](../entities/memoryscope.md) — technology
- [TeamStore](../entities/teamstore.md) — technology
- [ToolContext](../entities/toolcontext.md) — technology

### Concepts

- [Path Traversal Prevention](../concepts/path-traversal-prevention.md)
- [Agent Memory Persistence](../concepts/agent-memory-persistence.md)
- [Multi-Agent Team Architecture](../concepts/multi-agent-team-architecture.md)
- [Tool Parameter Schema Validation](../concepts/tool-parameter-schema-validation.md)
- [Asynchronous Tool Execution](../concepts/asynchronous-tool-execution.md)

