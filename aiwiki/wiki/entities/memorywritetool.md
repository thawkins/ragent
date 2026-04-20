---
title: "MemoryWriteTool"
entity_type: "technology"
type: entity
generated: "2026-04-19T18:35:08.640841231+00:00"
---

# MemoryWriteTool

**Type:** technology

### From: memory_write

MemoryWriteTool is a core component of the Ragent framework's persistent memory system, designed to enable AI agents to store information across sessions. This tool implements a sophisticated dual-mode write operation that balances backward compatibility with modern structured storage capabilities. In its block-based mode, the tool creates named memory blocks as individual Markdown files with YAML frontmatter, supporting metadata such as descriptions, size limits, and timestamps. The tool enforces strict validation rules, including label format requirements (lowercase with hyphens) and content size limits, while providing flexible write modes including append and overwrite operations.

The implementation demonstrates advanced Rust programming patterns, leveraging the async-trait crate for asynchronous trait implementation and integrating deeply with the framework's storage abstraction layer through FileBlockStorage. The tool's execute method handles complex logic paths, distinguishing between block-based and legacy operations based on input parameters. When operating in append mode, the tool automatically injects ISO 8601 timestamp separators between entries, creating an auditable history of memory modifications. The tool also implements read-only protection, preventing accidental modification of protected memory blocks through explicit permission checks before any write operation.

MemoryWriteTool plays a critical role in the agent's learning and adaptation capabilities, serving as the primary interface for knowledge persistence. By supporting both user-global and project-local scopes, the tool enables sophisticated memory organization strategies where agents can maintain universal knowledge alongside project-specific context. The tool's integration with the broader Ragent ecosystem is evident in its use of shared types like ToolContext and ToolOutput, and its coordination with cross-project resolution mechanisms that allow memory blocks to be discovered across scope boundaries.

## Diagram

```mermaid
flowchart TD
    subgraph Input["Input Processing"]
        I1["JSON Parameters"] --> I2["Extract content, scope, label, mode"]
        I2 --> I3["Validate label format"]
    end
    
    subgraph Decision{"Mode Selection"}
        I3 --> D1{"label provided?"}
        D1 -->|Yes| BlockMode["Block-Based Mode"]
        D1 -->|No| LegacyMode["Legacy Mode"]
    end
    
    subgraph BlockWrite["Block-Based Write"]
        BW1["Load existing block"] --> BW2{"Check read-only"}
        BW2 -->|Blocked| BW_ERR["Return error"]
        BW2 -->|Writable| BW3{"mode == 'overwrite'?"}
        BW3 -->|Yes| BW_NEW["Create new block with content"]
        BW3 -->|No| BW_APPEND["Append with timestamp separator"]
        BW_NEW --> BW4["Check content limit"]
        BW_APPEND --> BW4
        BW4 --> BW5["Save via FileBlockStorage"]
    end
    
    subgraph LegacyWrite["Legacy Write"]
        LW1["Resolve memory directory"] --> LW2["Open/create file"]
        LW2 --> LW3["Write content"]
    end
    
    BlockMode --> BlockWrite
    LegacyMode --> LegacyWrite
    BW5 --> Output["Return ToolOutput with metadata"]
    LW3 --> Output
```

## External Resources

- [async-trait crate documentation for Rust async trait implementations](https://docs.rs/async-trait/latest/async_trait/) - async-trait crate documentation for Rust async trait implementations
- [Serde serialization framework documentation](https://serde.rs/) - Serde serialization framework documentation

## Sources

- [memory_write](../sources/memory-write.md)
