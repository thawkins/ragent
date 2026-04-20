---
title: "TeamManagerInterface"
entity_type: "technology"
type: entity
generated: "2026-04-19T20:07:04.609837091+00:00"
---

# TeamManagerInterface

**Type:** technology

### From: mod

TeamManagerInterface defines the async contract for spawning teammate sessions in multi-agent systems, representing a key abstraction in the ragent-core team coordination architecture. This trait is marked with `#[async_trait::async_trait]` enabling async methods in traits, and requires `Send + Sync` bounds ensuring thread-safe implementations can be shared across async tasks. The interface bridges the M2 and M3 development milestones, where during M2 the registry holds `Option<Arc<dyn TeamManagerInterface>>` as `None` until M3 wiring is complete.

The sole method `spawn_teammate` accepts comprehensive parameters: team name for grouping, teammate name for identification, agent type for specialization, initial prompt for context, optional per-teammate model override, lead model for inheritance when no override is specified, and working directory for file operations. The method returns the spawned agent's ID as a String, which teammates use to identify themselves in subsequent team communications.

This design enables dynamic team scaling where the lead agent can spawn specialized teammates on demand. The optional model parameters support flexible deployment scenarios: teammates can inherit the lead's active model, receive a team-wide model, or have individual model assignments for cost optimization or capability specialization. The trait abstraction allows different TeamManager implementations without affecting tool code, supporting both local process spawning and distributed deployments.

## External Resources

- [async-trait crate documentation for async methods in traits](https://docs.rs/async-trait/latest/async_trait/) - async-trait crate documentation for async methods in traits
- [Strategy pattern for pluggable algorithm implementations](https://en.wikipedia.org/wiki/Strategy_pattern) - Strategy pattern for pluggable algorithm implementations

## Sources

- [mod](../sources/mod.md)
