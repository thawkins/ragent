---
title: "TeamShutdownTeammateTool: Graceful Shutdown Coordination for Multi-Agent Teams"
source: "team_shutdown_teammate"
type: source
tags: [rust, multi-agent-systems, agent-coordination, graceful-shutdown, team-management, async-rust, message-passing, state-machine, ragent-core, distributed-systems]
generated: "2026-04-19T19:27:29.926453850+00:00"
---

# TeamShutdownTeammateTool: Graceful Shutdown Coordination for Multi-Agent Teams

This source file implements `TeamShutdownTeammateTool`, a critical component in the ragent-core framework that enables team leads to gracefully shut down teammate agents in a multi-agent system. The tool follows a structured protocol where the lead agent sends a shutdown request to a target teammate's mailbox, updates the team member's status to 'ShuttingDown' in persistent storage, and awaits acknowledgment before termination completes. This design emphasizes safety and coordination over abrupt termination, ensuring that agents can complete in-flight work and acknowledge their shutdown rather than being forcefully killed. The implementation demonstrates robust error handling through the `anyhow` crate, structured data management with `serde_json`, and integration with the broader team management infrastructure including `TeamStore` for persistence and `Mailbox` for inter-agent communication. The tool enforces role-based access through its 'team:manage' permission category, restricting shutdown capabilities to designated lead agents and preventing unauthorized termination requests that could destabilize team operations.

## Related

### Entities

- [TeamShutdownTeammateTool](../entities/teamshutdownteammatetool.md) — technology
- [TeamStore](../entities/teamstore.md) — technology
- [Mailbox](../entities/mailbox.md) — technology

