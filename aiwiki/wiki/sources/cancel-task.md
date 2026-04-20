---
title: "CancelTaskTool: Task Cancellation for Background Sub-Agents in Ragent Core"
source: "cancel_task"
type: source
tags: [rust, async, agent-framework, task-management, cancellation, sub-agent, ragent, tool-system, background-task, session-security]
generated: "2026-04-19T17:17:55.778245393+00:00"
---

# CancelTaskTool: Task Cancellation for Background Sub-Agents in Ragent Core

This document details the `CancelTaskTool` implementation within the `ragent-core` crate, a Rust-based agent framework. The tool provides a critical capability for managing asynchronous, long-running sub-agent tasks by allowing agents to cancel background tasks they previously spawned. The implementation demonstrates robust error handling using the `anyhow` crate, strict session-based security controls to prevent unauthorized task cancellation, and structured JSON response formatting. The tool integrates with a `TaskManager` abstraction that handles the actual cancellation logic, while the tool itself focuses on parameter validation, permission verification, and response generation. This architecture reflects common patterns in distributed agent systems where parent-child task relationships must be carefully managed to maintain system integrity and security boundaries between concurrent execution contexts.

## Related

### Entities

- [CancelTaskTool](../entities/canceltasktool.md) — technology
- [TaskManager](../entities/taskmanager.md) — technology
- [ToolContext](../entities/toolcontext.md) — technology
- [Tool](../entities/tool.md) — technology
- [ragent-core](../entities/ragent-core.md) — product

### Concepts

- [Asynchronous Task Cancellation](../concepts/asynchronous-task-cancellation.md)
- [Session-Based Security Isolation](../concepts/session-based-security-isolation.md)
- [Plugin Architecture for Agent Tools](../concepts/plugin-architecture-for-agent-tools.md)
- [Error Handling in Async Rust](../concepts/error-handling-in-async-rust.md)
- [Hierarchical Task Management](../concepts/hierarchical-task-management.md)

