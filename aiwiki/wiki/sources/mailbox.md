---
title: "RAgent Core Mailbox System: Inter-Agent Message Passing Architecture"
source: "mailbox"
type: source
tags: [rust, async, messaging, inter-process-communication, file-locking, agent-systems, tokio, serde, uuid, ragent-core]
generated: "2026-04-19T21:12:06.195040653+00:00"
---

# RAgent Core Mailbox System: Inter-Agent Message Passing Architecture

The `mailbox.rs` module implements a robust file-backed messaging system for inter-agent communication within the RAgent framework. Each agent in a team—including the lead agent and individual teammates—maintains a dedicated mailbox stored as a JSON file at `mailbox/{agent-id}.json` within the team's directory. This design enables persistent, durable messaging between agents while operating within a single process or across process boundaries through shared filesystem access.

The module introduces several key abstractions: the `MailboxMessage` struct representing individual messages with UUID-based identification, the `Mailbox` struct managing file I/O operations with proper locking semantics, and a comprehensive `MessageType` enum categorizing messages into semantic types like direct messages, broadcasts, plan requests/approvals, idle notifications, and shutdown coordination. The implementation prioritizes reliability through exclusive file locking using the `fs2` crate, preventing race conditions when multiple agents or threads attempt concurrent mailbox access.

A notable architectural feature is the notifier registry pattern, which optimizes message delivery latency. Rather than relying solely on periodic polling—a common anti-pattern in messaging systems that introduces latency and resource waste—the module maintains a process-wide `HashMap` mapping `(team_dir, agent_id)` tuples to `tokio::sync::Notify` handles. When `Mailbox::push` appends a message, it signals any registered notifier, allowing the recipient's async poll loop to wake immediately. This hybrid approach combines the durability of file-backed storage with the responsiveness of in-process signaling, addressing the requirements outlined in Milestone T6 of the project roadmap.

## Related

### Entities

- [MailboxMessage](../entities/mailboxmessage.md) — technology
- [Mailbox](../entities/mailbox.md) — technology
- [tokio::sync::Notify](../entities/tokio-sync-notify.md) — technology
- [fs2 File Locking](../entities/fs2-file-locking.md) — technology

### Concepts

- [File-Backed Messaging with Real-Time Notification](../concepts/file-backed-messaging-with-real-time-notification.md)
- [Agent Coordination Protocols](../concepts/agent-coordination-protocols.md)
- [Process-Wide Singleton Registry Pattern](../concepts/process-wide-singleton-registry-pattern.md)

