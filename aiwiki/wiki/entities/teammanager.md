---
title: "TeamManager"
entity_type: "technology"
type: entity
generated: "2026-04-19T19:31:06.813601963+00:00"
---

# TeamManager

**Type:** technology

### From: team_spawn

The `TeamManager` represents a critical infrastructure component referenced throughout the `TeamSpawnTool` implementation, though notably external to this specific source file. It serves as the authoritative subsystem for team lifecycle management, agent coordination, and resource allocation within the broader agent framework. The code comments explicitly identify `TeamManager` as a "M3" (Milestone 3) dependency, indicating planned but not yet fully integrated functionality that the current implementation anticipates and gracefully accommodates.

When available, the `TeamManager` provides the `spawn_teammate` method—a core operation accepting team identifier, teammate name, agent type classification, initial prompt, optional model override, reference to the active model, and working directory context. This signature reveals the `TeamManager`'s comprehensive responsibilities spanning identity management, process isolation, model configuration inheritance, and filesystem-based workspace organization. The asynchronous nature of this operation (`await`) suggests the `TeamManager` may handle network communication, container orchestration, or process spawning depending on the underlying execution environment.

The defensive coding patterns around `TeamManager` availability—checking `is_none()`, logging warnings with structured fields, and returning semantically meaningful pending states—demonstrate mature engineering practices for feature flagging and incremental rollout. This design enables development and testing of the `TeamSpawnTool` interface while the `TeamManager` implementation matures, reducing coupling and allowing parallel development tracks. The integration points with `TeamStore` for configuration persistence and `TaskStore` for work item management suggest the `TeamManager` operates as a facade coordinating multiple specialized subsystems rather than a monolithic implementation.

## External Resources

- [Rust Option type for safe null handling patterns](https://doc.rust-lang.org/std/option/enum.Option.html) - Rust Option type for safe null handling patterns
- [Rust design patterns including facade pattern usage](https://doc.rust-lang.org/book/ch17-03-oo-design-patterns.html) - Rust design patterns including facade pattern usage

## Sources

- [team_spawn](../sources/team-spawn.md)

### From: manager

The `TeamManager` is the central runtime orchestrator for multi-agent teams in the ragent-core system, implemented as a thread-safe, Arc-shared struct that manages the complete lifecycle of AI teammates. It serves as the concrete implementation of the `TeamManagerInterface` trait, enabling integration with the broader tool ecosystem including the `team_spawn` tool. The struct maintains critical state including the team name, lead session ID for event routing, the absolute path to the team's persistent directory, and a RwLock-protected HashMap of active `TeammateHandle` instances indexed by agent ID.

The `TeamManager` embeds several sophisticated subsystems: a `SessionProcessor` for executing agent operations, an `EventBus` for publishing lifecycle events, configurable mailbox polling intervals (default 500ms), and a `Mutex`-based spawn lock to prevent race conditions during concurrent member creation. A notable design feature is the `active_model` field, which captures the lead's current model selection and serves as a fallback for teammates spawned without explicit model overrides—critical for the reconciliation loop where no ToolContext model is available.

The implementation demonstrates production-grade patterns including graceful error handling through the `anyhow` crate, structured logging via `tracing`, and careful resource management with explicit shutdown semantics. The manager's methods span the full operational domain: `spawn_teammate_internal` for creating new sessions with injected team context, `shutdown_teammate` and `shutdown_all` for graceful termination, `approve_plan` for human-in-the-loop approval workflows, and `reconcile_spawning_members` for recovering from crashes by restoring persisted member states. The polling-based mailbox architecture with push-based wakeup via `tokio::sync::Notify` balances efficiency with responsiveness.
