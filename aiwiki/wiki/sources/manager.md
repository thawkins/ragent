---
title: "TeamManager: Runtime Orchestration for Multi-Agent Teams in ragent-core"
source: "manager"
type: source
tags: [rust, multi-agent, orchestration, session-management, tokio, async, ai-agents, context-window, compaction, quality-gates, event-driven, mailbox-pattern]
generated: "2026-04-19T21:14:19.317231660+00:00"
---

# TeamManager: Runtime Orchestration for Multi-Agent Teams in ragent-core

This document presents the `TeamManager` implementation in Rust, a core component of the ragent-core crate responsible for orchestrating multi-agent team workflows. The `TeamManager` serves as the central runtime hub that manages the lifecycle of AI teammates, handling their spawning, shutdown, message routing, and error recovery. The module implements sophisticated mechanisms for context window management through automatic session compaction, persistent memory injection via MEMORY.md files, and quality-gate hooks for workflow validation. A key architectural decision is the separation between the manager's control plane and the individual agent sessions, achieved through the `TeammateHandle` abstraction and asynchronous mailbox polling loops. The implementation addresses practical challenges in distributed agent systems: model inheritance hierarchies (teammate-specific > lead model > agent default), recovery from transient API failures versus permanent errors, and team-wide event propagation through an `EventBus` pattern. The code reveals a mature production system with careful attention to race condition prevention (via `spawn_lock`), graceful degradation (hooks that fail open), and operational observability (comprehensive tracing integration). The module's design reflects patterns from actor-based concurrency systems while remaining grounded in Rust's ownership and async ecosystem constraints.

## Related

### Entities

- [TeamManager](../entities/teammanager.md) — technology
- [TeammateHandle](../entities/teammatehandle.md) — technology
- [HookOutcome](../entities/hookoutcome.md) — technology
- [Session Compaction Agent](../entities/session-compaction-agent.md) — technology

