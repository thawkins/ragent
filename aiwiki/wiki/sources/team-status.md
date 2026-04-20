---
title: "TeamStatusTool: Multi-Agent Team Status Reporting Tool for Ragent Framework"
source: "team_status"
type: source
tags: [rust, multi-agent, orchestration, observability, diagnostics, async, trait, state-management, ragent, ai-agents]
generated: "2026-04-19T19:33:54.033216164+00:00"
---

# TeamStatusTool: Multi-Agent Team Status Reporting Tool for Ragent Framework

This Rust source code implements the `TeamStatusTool`, a diagnostic and monitoring component within the ragent-core framework designed for orchestrating multi-agent systems. The tool provides comprehensive visibility into team composition, member states, and task execution progress, serving as a critical observability mechanism for distributed agent coordination. At its core, the implementation follows a structured pipeline: it validates input parameters, locates team configuration directories, loads persistent state from both team and task stores, computes aggregated statistics, and formats human-readable reports with optional machine-readable metadata.

The architecture demonstrates several important software engineering patterns for agent systems. First, it implements the `Tool` trait using `async_trait`, establishing a consistent interface for all executable capabilities within the framework. The permission system categorizes this as "team:read", reflecting a least-privilege security model. The tool gracefully handles partial failures—specifically, task store corruption doesn't prevent team status reporting—demonstrating resilience in distributed persistence scenarios. State management integrates with the broader `TeamStore` and `TaskStore` abstractions, which handle serialization concerns transparently.

The output formatting deserves particular attention as it bridges human and machine consumers. The `execute` method constructs a multi-line string report using emoji icons to encode member status states (Working, Idle, PlanPending, Blocked, ShuttingDown, Stopped, Spawning, Failed), making team health instantly scannable. Simultaneously, it produces structured JSON metadata containing normalized member information and task statistics, enabling programmatic consumption by other agents or monitoring systems. This dual-format output reflects the tool's role in a hybrid human-in-the-loop and autonomous agent ecosystem where both readability and interoperability matter.

## Related

### Entities

- [TeamStatusTool](../entities/teamstatustool.md) — technology
- [TeamStore](../entities/teamstore.md) — technology
- [TaskStore](../entities/taskstore.md) — technology
- [MemberStatus](../entities/memberstatus.md) — technology

