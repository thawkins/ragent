---
title: "WaitTasksTool: Event-Driven Sub-Agent Task Completion for Rust Agent Systems"
source: "wait_tasks"
type: source
tags: [rust, async, tokio, agent-systems, event-driven, sub-agent, task-management, concurrency, polling-alternative, crdt, mcp]
generated: "2026-04-19T19:51:40.131042383+00:00"
---

# WaitTasksTool: Event-Driven Sub-Agent Task Completion for Rust Agent Systems

This document presents the `WaitTasksTool`, a sophisticated Rust implementation that enables intelligent agents to wait for background sub-agent tasks to complete without resorting to inefficient polling mechanisms. The tool leverages an event-driven architecture using Tokio's async runtime and broadcast channels, subscribing to `Event::SubagentComplete` notifications on a session-scoped event bus. This approach eliminates race conditions through careful ordering of operations—subscribing to events before checking current task state—and provides robust timeout handling with cleanup guarantees.

The implementation demonstrates several advanced Rust patterns including defensive programming against race conditions, proper resource cleanup using RAII-like patterns with explicit increment/decrement waiter counts, and comprehensive error handling with the `anyhow` crate. The tool supports two modes of operation: waiting for specific task IDs provided as parameters, or automatically discovering and waiting for all currently running background tasks. This flexibility makes it suitable for both targeted synchronization scenarios and broad workflow coordination where the exact set of concurrent tasks may not be known in advance.

The output formatting and metadata construction reveal attention to user experience in terminal-based interfaces (TUI), with emoji indicators for success/failure, truncated task IDs for readability, and structured metadata including timing information and output line counts. The 300-second default timeout with configurable override provides reasonable defaults while allowing customization for long-running operations. The permission category of `agent:spawn` indicates this tool is intended for agent orchestration contexts where managing sub-agent lifecycles is a core concern.

## Related

### Entities

- [WaitTasksTool](../entities/waittaskstool.md) — product
- [Tokio](../entities/tokio.md) — technology
- [TaskManager](../entities/taskmanager.md) — technology

### Concepts

- [Event-Driven Architecture](../concepts/event-driven-architecture.md)
- [Race Condition Prevention](../concepts/race-condition-prevention.md)
- [Reference Counting for Event Optimization](../concepts/reference-counting-for-event-optimization.md)
- [Structured Output and TUI Integration](../concepts/structured-output-and-tui-integration.md)

