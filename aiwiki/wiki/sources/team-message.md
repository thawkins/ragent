---
title: "TeamMessageTool: Agent-to-Agent Direct Messaging System in Rust"
source: "team_message"
type: source
tags: [rust, multi-agent-systems, messaging, tool-framework, async-rust, team-coordination, actor-model, filesystem-persistence, json-schema, serde]
generated: "2026-04-19T19:20:48.308553153+00:00"
---

# TeamMessageTool: Agent-to-Agent Direct Messaging System in Rust

This source code implements `TeamMessageTool`, a Rust-based tool for enabling direct messaging between agents within a team-based architecture. The implementation provides a structured mechanism for one team member to send messages to another by name or agent ID, with robust error handling and integration into a broader tool framework. The code leverages asynchronous traits, JSON schema validation, and filesystem-based persistence through a `Mailbox` system, demonstrating patterns common in multi-agent systems and command-oriented architectures.

The `TeamMessageTool` struct implements a `Tool` trait, making it part of a plugin-style architecture where tools can be dynamically discovered and executed. The tool accepts three required parameters: `team_name` to identify the target team, `to` specifying the recipient (either as an agent ID like "tm-001" or the special "lead" designation), and `content` containing the message text. The implementation handles name resolution through a `resolve_agent_id` helper function that consults team configuration files, allowing users to reference teammates by human-readable names while the system translates these to canonical agent identifiers.

The messaging flow involves locating the team's directory on disk, resolving the recipient identifier, opening or creating a mailbox for that agent, and pushing a structured `MailboxMessage` with appropriate metadata. The system tracks the sender's identity through a `team_context` field in the execution context, defaulting to "lead" when no team context is available. This design supports hierarchical team structures where a lead agent coordinates communication, while also enabling peer-to-peer messaging between team members. The use of `anyhow` for error handling and `serde_json` for serialization reflects modern Rust practices for building reliable, maintainable systems software.

## Related

### Entities

- [TeamMessageTool](../entities/teammessagetool.md) — technology
- [Mailbox](../entities/mailbox.md) — technology
- [TeamStore](../entities/teamstore.md) — technology
- [resolve_agent_id](../entities/resolve-agent-id.md) — technology

### Concepts

- [Multi-Agent Communication Patterns](../concepts/multi-agent-communication-patterns.md)
- [Tool Framework Architecture](../concepts/tool-framework-architecture.md)
- [Filesystem-Based Persistence](../concepts/filesystem-based-persistence.md)
- [JSON Schema as API Contracts](../concepts/json-schema-as-api-contracts.md)

