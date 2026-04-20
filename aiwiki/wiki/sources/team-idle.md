---
title: "TeamIdleTool: Agent Idle State Notification System in Ragent Core"
source: "team_idle"
type: source
tags: [rust, multi-agent-systems, team-coordination, ragent, ai-agents, task-management, hook-pattern, state-machine, distributed-systems, async-rust]
generated: "2026-04-19T19:13:18.644578666+00:00"
---

# TeamIdleTool: Agent Idle State Notification System in Ragent Core

This document describes the `team_idle.rs` source file from the ragent-core crate, which implements the `TeamIdleTool` struct. This tool enables AI agents (teammates) within a multi-agent team system to formally notify their team lead when they have no remaining work assignments. The implementation demonstrates a sophisticated approach to distributed agent coordination, incorporating guard clauses to prevent premature idle states, hook-based extensibility for custom idle validation logic, and comprehensive state management through persistent storage. The tool is part of a broader team management framework that includes task claiming, task completion, and lifecycle hooks for orchestrating collaborative AI workflows.

The `TeamIdleTool` implements the `Tool` trait and provides a JSON-based interface accepting a team name and optional work summary. Its execution follows a carefully designed sequence: first validating input parameters, then checking for any in-progress tasks that would block the idle transition, optionally invoking a configurable `TeammateIdle` hook for custom validation or logging, and finally updating the agent's membership status in team persistent storage. This multi-stage process ensures data consistency and prevents the common failure mode of agents abandoning tasks mid-execution, which could leave tasks in an unrecoverable limbo state within the distributed system.

The hook mechanism represents a particularly flexible design pattern, allowing teams to inject custom business logic into the idle notification workflow. The hook can either accept the idle request (returning `HookOutcome::Success`) or reject it with feedback (returning `HookOutcome::Feedback`), in which case the agent remains in `Working` status. This enables scenarios such as: requiring minimum work summaries before idle, implementing time-based cool-down periods, triggering notification to human supervisors, or dynamically reassigning priority tasks to agents attempting to go idle. The tool's integration with `TaskStore` and `TeamStore` demonstrates the crate's layered persistence architecture for team coordination data.

## Related

### Entities

- [TeamIdleTool](../entities/teamidletool.md) — technology
- [Ragent Project](../entities/ragent-project.md) — product

### Concepts

- [Agent State Machines in Distributed Systems](../concepts/agent-state-machines-in-distributed-systems.md)
- [Hook-Based Extensibility Patterns](../concepts/hook-based-extensibility-patterns.md)
- [Task-Claim Coordination Protocols](../concepts/task-claim-coordination-protocols.md)

