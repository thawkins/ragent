---
title: "SwarmState"
entity_type: "technology"
type: entity
generated: "2026-04-19T21:08:22.347993114+00:00"
---

# SwarmState

**Type:** technology

### From: swarm

SwarmState captures the complete runtime lifecycle of an active swarm execution, serving as the authoritative record for orchestration and monitoring purposes. The struct maintains provenance information including the original prompt that initiated decomposition and the resulting SwarmDecomposition, enabling full reconstruction of the planning phase. The team_name field references the ephemeral agent team created to execute the decomposed subtasks, linking the abstract plan to concrete runtime resources.

Boolean flags track execution progress through the spawn and completion phases. The `spawned` field indicates whether the orchestrator has instantiated agent processes for all subtasks, transitioning from planning to execution. The `completed` flag signals that results have been collected and the swarm has finished, potentially including aggregation of partial results from distributed agents. These discrete state markers enable idempotent operations and safe recovery from failures at any lifecycle stage.

The state structure supports distributed systems concerns through its serializable design, allowing SwarmState instances to be persisted, replicated, or transmitted across network boundaries. This capability enables horizontal scaling where different orchestrator nodes can assume responsibility for swarm management, and facilitates debugging by capturing complete execution context. The inclusion of both original prompt and decomposition provides auditability for compliance and optimization purposes.

## Sources

- [swarm](../sources/swarm.md)
