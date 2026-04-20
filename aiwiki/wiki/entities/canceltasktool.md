---
title: "CancelTaskTool"
entity_type: "technology"
type: entity
generated: "2026-04-19T17:17:55.778494923+00:00"
---

# CancelTaskTool

**Type:** technology

### From: cancel_task

The `CancelTaskTool` is a concrete implementation of the `Tool` trait within the ragent-core framework, designed specifically for terminating background sub-agent tasks that were previously spawned with asynchronous execution enabled. This tool exemplifies the plugin-based architecture of the ragent system, where capabilities are modularized into discrete tool implementations that can be dynamically invoked by agents during their execution. The tool requires a single parameter—the `task_id` returned when a task was originally created—and enforces strict access controls by verifying that the requesting session matches the parent session of the target task. This security model prevents cross-session interference while enabling legitimate task lifecycle management within multi-agent systems. The implementation leverages Rust's async/await patterns for non-blocking execution and integrates deeply with the framework's error propagation mechanisms to provide clear feedback about cancellation success or failure states.

## Diagram

```mermaid
flowchart TD
    subgraph CancelTaskTool["CancelTaskTool Execution Flow"]
        A["Receive execute() call"] --> B["Extract task_id parameter"]
        B --> C{"task_id valid?"}
        C -->|No| D["Return error: Missing parameter"]
        C -->|Yes| E["Access TaskManager from context"]
        E --> F{"TaskManager available?"}
        F -->|No| G["Return error: Not initialized"]
        F -->|Yes| H["Fetch task entry"]
        H --> I{"Task found?"}
        I -->|No| J["Return: Task not found"]
        I -->|Yes| K{"Same session?"}
        K -->|No| L["Return error: Wrong session"]
        K -->|Yes| M["Call task_manager.cancel_task()"]
        M --> N{"Result"}
        N -->|Ok| O["Return: Task cancelled"]
        N -->|Err| P["Return: Cancel failed"]
    end
    style CancelTaskTool fill:#f9f,stroke:#333,stroke-width:2px
```

## External Resources

- [anyhow crate documentation for flexible error handling in Rust](https://docs.rs/anyhow/latest/anyhow/) - anyhow crate documentation for flexible error handling in Rust
- [serde_json for JSON serialization and schema generation](https://serde.rs/) - serde_json for JSON serialization and schema generation

## Sources

- [cancel_task](../sources/cancel-task.md)
