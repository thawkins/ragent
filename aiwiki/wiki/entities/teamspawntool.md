---
title: "TeamSpawnTool"
entity_type: "technology"
type: entity
generated: "2026-04-19T19:31:06.813089609+00:00"
---

# TeamSpawnTool

**Type:** technology

### From: team_spawn

The `TeamSpawnTool` is the central struct defined in this source file, implementing the `Tool` trait to provide teammate spawning capabilities within a multi-agent system. It represents a concrete instantiation of the framework's plugin architecture, allowing dynamic agent creation with configurable parameters including team membership, agent specialization, task assignment, and memory persistence scopes. The struct itself is minimal—essentially a unit struct—demonstrating the trait-based design pattern where behavior is composed through trait implementations rather than encapsulated state.

The tool's implementation spans multiple critical responsibilities: parameter schema definition through JSON Schema, permission category classification for access control, and the core `execute` method that orchestrates the entire teammate lifecycle. The design philosophy emphasizes fail-safe operation with graceful degradation, as evidenced by the explicit handling of missing `TeamManager` infrastructure during the M3 milestone transition period. This pattern of staging functionality behind feature flags or infrastructure availability is common in rapidly evolving AI agent frameworks.

The `TeamSpawnTool` integrates with numerous subsystem components including the event bus for permission workflows, the team manager for agent lifecycle operations, task stores for work item assignment, and team stores for persistent configuration management. Its execution flow demonstrates sophisticated control patterns: synchronous validation, asynchronous permission negotiation with timeout handling, optional model resolution, memory scope configuration, and finally conditional task pre-assignment. The comprehensive tracing instrumentation at all severity levels (`info`, `debug`, `trace`, `warn`, `error`) indicates production-hardened observability practices essential for debugging distributed multi-agent interactions.

## Diagram

```mermaid
flowchart TD
    subgraph InputValidation["Input Validation"]
        A[team_name] --> B[Required Check]
        C[teammate_name] --> D[Required Check]
        E[prompt] --> F[Required Check]
        G[agent_type] --> H[Default: general]
    end
    
    subgraph MultiItemDetection["Multi-Item Detection"]
        F --> I{detect_multi_item_list}
        I -->|numbered ≥3| J[Trigger Permission]
        I -->|bullet ≥3| J
        I -->|letter ≥3| J
        I -->|none| K[Skip Permission]
    end
    
    subgraph PermissionFlow["Permission Workflow"]
        J --> L[Generate UUID]
        L --> M[Subscribe Event Bus]
        M --> N[Publish PermissionRequested]
        N --> O{Wait 300s Timeout}
        O -->|Allowed| P[Continue Execution]
        O -->|Denied| Q[Return Error]
        O -->|Timeout| R[Return Error]
    end
    
    subgraph ModelResolution["Model Resolution"]
        K --> S[Parse model param]
        P --> S
        S --> T{Valid ModelRef?}
        T -->|Yes| U[Use Provider/Model]
        T -->|No| V[Inherit lead model]
    end
    
    subgraph TeamManagerCheck["Team Manager Availability"]
        U --> W{TeamManager exists?}
        V --> W
        W -->|No| X[Return pending_manager]
        W -->|Yes| Y[Spawn Teammate]
    end
    
    subgraph PostSpawn["Post-Spawn Operations"]
        Y --> Z[Optional Task Pre-assignment]
        Z --> AA[Memory Scope Persistence]
        AA --> AB[Build ToolOutput]
    end
```

## External Resources

- [Tokio graceful shutdown patterns for async runtime management](https://tokio.rs/tokio/topics/shutdown) - Tokio graceful shutdown patterns for async runtime management
- [serde_json documentation for JSON handling in Rust](https://serde.rs/json.html) - serde_json documentation for JSON handling in Rust

## Sources

- [team_spawn](../sources/team-spawn.md)
