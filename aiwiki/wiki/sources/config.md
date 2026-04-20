---
title: "Team Configuration System for Multi-Agent Orchestration"
source: "config"
type: source
tags: [rust, multi-agent-systems, configuration-management, orchestration, state-machines, serde, agent-framework, workflow-automation, team-management, ai-agents]
generated: "2026-04-19T21:10:17.445164113+00:00"
---

# Team Configuration System for Multi-Agent Orchestration

This Rust source file defines the core configuration types for managing teams of AI agents in the ragent system. The module establishes a comprehensive framework for team lifecycle management, including team-wide settings, individual teammate states, memory persistence scopes, and quality-gate hooks for workflow automation. The architecture centers around `TeamConfig` as the root configuration object, which serializes to and from `config.json` in the team directory, enabling persistent team state across sessions.

The design implements sophisticated state management through multiple enum types that track both team-level and individual member status transitions. `TeamStatus` manages the overall team lifecycle from creation through completion or disbandment, while `MemberStatus` provides granular tracking of individual agent sessions through states including spawning, working, idle, plan pending, blocked, shutting down, stopped, and failed. This state machine approach enables robust orchestration of distributed agent workflows with clear failure handling and recovery paths.

A notable feature is the memory scope system, which allows agents to maintain persistent context across sessions. The `MemoryScope` enum supports three levels: no persistence, user-global storage in `~/.ragent/agent-memory/`, and project-local storage within the working directory. This enables agents to accumulate knowledge and maintain continuity across task executions. The quality-gate hook system (`HookEntry` and `HookEvent`) provides extensibility, allowing teams to define custom shell commands that execute at specific lifecycle events—such as task creation or completion—enabling integration with external validation, notification, or CI/CD systems.

## Related

### Entities

- [ragent](../entities/ragent.md) — technology
- [serde](../entities/serde.md) — technology
- [chrono](../entities/chrono.md) — technology

### Concepts

- [Multi-Agent Orchestration](../concepts/multi-agent-orchestration.md)
- [Persistent Memory Scope](../concepts/persistent-memory-scope.md)
- [Quality-Gate Hooks](../concepts/quality-gate-hooks.md)
- [State Machine Lifecycle](../concepts/state-machine-lifecycle.md)
- [Configuration-Driven Architecture](../concepts/configuration-driven-architecture.md)

