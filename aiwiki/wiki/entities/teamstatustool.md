---
title: "TeamStatusTool"
entity_type: "technology"
type: entity
generated: "2026-04-19T19:33:54.034026397+00:00"
---

# TeamStatusTool

**Type:** technology

### From: team_status

TeamStatusTool is a concrete implementation of the Tool trait within the ragent-core framework, specifically designed to provide comprehensive visibility into the operational state of multi-agent teams. The struct itself is a zero-sized type (unit struct), following a common Rust pattern for stateless service objects that rely entirely on external context and persistent storage rather than internal mutable state. This design choice enables the tool to be instantiated cheaply and used concurrently across multiple execution contexts without synchronization concerns.

The tool's primary responsibility is aggregating distributed state from two persistence layers: the TeamStore, which contains team configuration metadata including member definitions and team-level status, and the TaskStore, which tracks individual task lifecycle information. By combining these data sources, TeamStatusTool produces a holistic view of system health that neither store could provide in isolation. The implementation demonstrates sophisticated error handling through the use of `anyhow` for contextual error propagation and explicit fallback behaviors—for instance, task store failures result in empty task lists rather than complete execution failure, ensuring diagnostic tools remain available even when portions of the system are degraded.

A distinguishing characteristic of this implementation is its dual-output design philosophy. The tool recognizes that agent system outputs serve multiple consumers: human operators needing immediate situational awareness, and downstream automated systems requiring structured data for decision-making. The formatted string output uses visual encoding (emoji status icons) to maximize information density for human scanning, while the JSON metadata field provides normalized, type-safe data for programmatic consumption. This dual-mode output represents a pragmatic approach to interface design in hybrid human-machine systems.

## Diagram

```mermaid
flowchart TD
    subgraph Input["Input Validation"]
        A["JSON Input with team_name"] --> B["Extract team_name parameter"]
        B --> C{"Valid string?"}
        C -->|No| D["Return anyhow error"]
        C -->|Yes| E["Proceed to store loading"]
    end
    
    subgraph Loading["Store Loading"]
        E --> F["find_team_dir(working_dir, team_name)"]
        F --> G{"Team directory exists?"}
        G -->|No| H["Return 'Team not found' error"]
        G -->|Yes| I["TeamStore::load(team_dir)"]
        I --> J["TaskStore::open(team_dir)"]
        J --> K["TaskStore::read()"]
        K -->|Failure| L["Default empty task list"]
        K -->|Success| M["Use loaded tasks"]
    end
    
    subgraph Computation["Statistics Computation"]
        I --> N["Calculate task metrics"]
        L --> N
        M --> N
        N --> O["total_tasks = tasks.len()"]
        O --> P["done_tasks (filter Completed)"]
        P --> Q["in_progress_tasks (filter InProgress)"]
        Q --> R["pending_tasks (filter Pending)"]
    end
    
    subgraph Formatting["Output Formatting"]
        R --> S["Build formatted report lines"]
        S --> T["Iterate members with status icons"]
        T --> U["Collect members as JSON"]
        U --> V["Join lines into content string"]
    end
    
    subgraph Output["ToolOutput"]
        V --> W["Return ToolOutput with content + metadata"]
    end
    
    style D fill:#ffcccc
    style H fill:#ffcccc
    style W fill:#ccffcc
```

## External Resources

- [Anyhow crate documentation for flexible error handling in Rust applications](https://docs.rs/anyhow/latest/anyhow/) - Anyhow crate documentation for flexible error handling in Rust applications
- [Serde serialization framework documentation for Rust data structures](https://serde.rs/) - Serde serialization framework documentation for Rust data structures
- [async-trait crate enabling async methods in Rust traits](https://docs.rs/async-trait/latest/async_trait/) - async-trait crate enabling async methods in Rust traits

## Sources

- [team_status](../sources/team-status.md)
