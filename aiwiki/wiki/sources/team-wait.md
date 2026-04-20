---
title: "TeamWaitTool: Synchronous Coordination for Multi-Agent Teams"
source: "team_wait"
type: source
tags: [rust, multi-agent-systems, coordination, event-driven-architecture, async-await, tokio, agent-orchestration, distributed-systems, synchronization-primitives]
generated: "2026-04-19T19:49:16.257370825+00:00"
---

# TeamWaitTool: Synchronous Coordination for Multi-Agent Teams

The team_wait.rs file implements the TeamWaitTool, a critical synchronization primitive for multi-agent systems that enables a lead agent to block execution until all or specified teammates reach an idle state. This tool addresses the fundamental coordination problem in distributed agent systems: preventing race conditions where a lead agent might proceed with work before teammates have completed their assigned tasks, potentially resulting in duplicated effort or inconsistent state.

The implementation demonstrates sophisticated event-driven architecture principles. Rather than using inefficient polling mechanisms, the tool subscribes to TeammateIdle events on a shared event bus, enabling truly asynchronous waiting with minimal resource overhead. The design includes robust timeout handling (defaulting to 300 seconds), flexible targeting of specific agents or entire teams, and graceful degradation when teammates are already idle at invocation time. The tool also provides rich feedback through emoji-enhanced status summaries and structured metadata for programmatic consumption.

From a systems design perspective, TeamWaitTool exemplifies several important patterns: event-driven coordination over polling, defensive programming against race conditions (note the explicit subscribe-before-check pattern), and comprehensive observability through structured logging. The permission category of "agent:spawn" correctly reflects its role in the agent lifecycle management workflow, typically invoked after team_spawn to ensure proper sequencing of distributed work.

## Related

### Entities

- [TeamWaitTool](../entities/teamwaittool.md) — technology
- [Event Bus](../entities/event-bus.md) — technology
- [TeamStore](../entities/teamstore.md) — technology

### Concepts

- [Event-Driven Coordination](../concepts/event-driven-coordination.md)
- [Race Condition Prevention](../concepts/race-condition-prevention.md)
- [Asynchronous Timeout Patterns](../concepts/asynchronous-timeout-patterns.md)
- [Agent State Machines](../concepts/agent-state-machines.md)
- [Structured Observability](../concepts/structured-observability.md)

