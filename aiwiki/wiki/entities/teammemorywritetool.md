---
title: "TeamMemoryWriteTool"
entity_type: "technology"
type: entity
generated: "2026-04-19T19:18:08.153495650+00:00"
---

# TeamMemoryWriteTool

**Type:** technology

### From: team_memory_write

TeamMemoryWriteTool is a concrete implementation of the Tool trait in the ragent agent framework, specifically designed to provide persistent memory capabilities for AI agents operating within team contexts. This struct represents a critical infrastructure component that enables stateful agent interactions by allowing agents to write and append information to files that persist across sessions. The tool is intentionally designed as a zero-sized type (unit struct) with no fields, relying entirely on its method implementations and the ToolContext parameter passed during execution to access necessary runtime state.

The implementation demonstrates sophisticated handling of multi-agent security boundaries. When executing, the tool first resolves the calling agent's identity through the TeamContext, falling back to session-level identifiers when team context is unavailable. It then performs a lookup against the team's configuration to determine memory scope settings—whether memories should be isolated per-agent or shared project-wide. This design enables flexible deployment scenarios where agents might operate with varying levels of persistence and isolation depending on their assigned roles and the team's security posture.

Path security is a paramount concern in this implementation, addressed through multiple layers of validation. The tool canonicalizes both the memory directory and target file paths, then verifies that the resolved path remains within the canonical memory directory boundary. This prevents directory traversal attacks where maliciously crafted paths (e.g., '../../../etc/passwd') might attempt to escape the sandbox. The implementation also gracefully handles path creation, automatically establishing parent directories as needed while maintaining security invariants throughout the process.

## Diagram

```mermaid
flowchart TD
    subgraph Input["Input Validation"]
        A["Parse team_name"] --> B["Parse content"]
        B --> C["Parse optional path/mode"]
    end
    subgraph Identity["Agent Resolution"]
        D["Get agent_id from TeamContext or session"] --> E["Load TeamStore"]
        E --> F["Find member by agent_id"]
    end
    subgraph Scope["Memory Scope Check"]
        F --> G{"memory_scope?"}
        G -->|None| H["Return disabled error"]
        G -->|User/Project| I["Resolve memory directory"]
    end
    subgraph Security["Path Security"]
        I --> J["Canonicalize paths"]
        J --> K{"Path within bounds?"}
        K -->|No| L["Return escape error"]
        K -->|Yes| M["Create directories"]
    end
    subgraph Write["File Operation"]
        M --> N{"Write mode?"}
        N -->|overwrite| O["std::fs::write"]
        N -->|append| P["OpenOptions::append"]
        O --> Q["Return success metadata"]
        P --> Q
    end
    C --> D
    H --> Z["End"]
    L --> Z
    Q --> Z
```

## External Resources

- [Rust std::fs::OpenOptions - used for append mode file operations](https://doc.rust-lang.org/stable/std/fs/struct.OpenOptions.html) - Rust std::fs::OpenOptions - used for append mode file operations
- [anyhow crate - flexible error handling used throughout the implementation](https://docs.rs/anyhow/latest/anyhow/) - anyhow crate - flexible error handling used throughout the implementation
- [serde_json - JSON serialization for tool parameters and metadata](https://docs.rs/serde_json/latest/serde_json/) - serde_json - JSON serialization for tool parameters and metadata

## Sources

- [team_memory_write](../sources/team-memory-write.md)
