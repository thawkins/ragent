---
title: "TeamTaskCompleteTool"
entity_type: "technology"
type: entity
generated: "2026-04-19T19:41:30.332123642+00:00"
---

# TeamTaskCompleteTool

**Type:** technology

### From: team_task_complete

TeamTaskCompleteTool is the central struct implemented in this source file, serving as a concrete tool implementation within a larger agent framework. This struct encapsulates the capability for an autonomous agent to mark assigned tasks as completed within a team-based workflow system. Unlike simple task completion utilities, this tool implements a sophisticated lifecycle management system that handles validation, permission checking, dependency unblocking, and extensible hook-based workflows. The tool is designed with zero-cost abstractions in mind—the unit struct contains no fields, with all state managed through the execution context and persistent task store.

The implementation follows the Tool trait pattern common in agent frameworks, requiring methods for identification (`name`), capability description (`description`), parameter validation schema (`parameters_schema`), security classification (`permission_category`), and the actual execution logic. The tool's design emphasizes defensibility and auditability: every completion attempt is logged with contextual information including agent identity, team context, and the full task state at time of execution. This logging strategy enables debugging of complex multi-agent scenarios where race conditions or miscommunications might occur.

A distinctive architectural feature is the integration with the `TaskStore` abstraction, which provides durable persistence of team state. The tool does not assume in-memory state but instead coordinates with a filesystem-backed store that enables recovery and cross-process coordination. This persistence model is essential for production deployments where agents may restart or where multiple agent processes might operate on shared team state. The tool also implements graceful degradation patterns, returning structured output with explanatory messages rather than failing with opaque errors when preconditions are not met.

The hook integration mechanism represents a powerful extension point for domain-specific validation. By invoking `run_team_hook` with the `TaskCompleted` event, the tool enables external scripts or services to participate in the completion decision. This could support use cases like automated quality checks, required artifact verification, or notification to downstream systems. The rollback capability—reverting to `InProgress` if hooks reject completion—provides transactional semantics that protect workflow integrity.

## Diagram

```mermaid
flowchart TD
    subgraph Input["Input Validation"]
        I1["Extract team_name"] --> I2["Extract task_id"]
        I2 --> I3["Resolve agent_id"]
    end
    
    subgraph Resolution["Team Resolution"]
        R1["Find team directory"] --> R2["Open TaskStore"]
    end
    
    subgraph Execution["Task Completion"]
        E1["Log debug state"] --> E2["Call store.complete()"]
        E2 --> E3{"Success?"}
        E3 -->|No| E4["Return failure output"]
        E3 -->|Yes| E5["Prepare hook input"]
    end
    
    subgraph Hook["Hook Evaluation"]
        H1["Run TaskCompleted hook"] --> H2{"Outcome?"}
        H2 -->|Feedback| H3["Revert to InProgress"]
        H2 -->|Success| H4["Confirm completion"]
    end
    
    subgraph Output["Response Generation"]
        O1["Format success message"] --> O2["Include metadata JSON"]
    end
    
    I3 --> R1
    R2 --> E1
    E4 --> End1["ToolOutput with error"]
    E5 --> H1
    H3 --> End2["ToolOutput with rejection"]
    H4 --> O1
    O2 --> End3["ToolOutput with confirmation"]
```

## External Resources

- [anyhow crate documentation for flexible error handling in Rust](https://docs.rs/anyhow/latest/anyhow/) - anyhow crate documentation for flexible error handling in Rust
- [Serde serialization framework for Rust data structures](https://serde.rs/) - Serde serialization framework for Rust data structures
- [Tokio tracing for structured, async-aware logging](https://tokio.rs/tokio/topics/tracing) - Tokio tracing for structured, async-aware logging

## Sources

- [team_task_complete](../sources/team-task-complete.md)
