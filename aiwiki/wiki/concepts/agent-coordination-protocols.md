---
title: "Agent Coordination Protocols"
type: concept
generated: "2026-04-19T21:12:06.197906611+00:00"
---

# Agent Coordination Protocols

### From: mailbox

The `MessageType` enum encodes a complete coordination protocol for multi-agent teams, defining the permissible interactions between lead and teammate agents. This protocol design reflects careful analysis of common multi-agent workflow patterns: direct messaging for unstructured communication, broadcasts for one-to-many announcements, structured plan submission and approval for hierarchical task delegation, idle notification for work-stealing or load balancing, and graceful shutdown for clean termination. Each variant represents a distinct conversation pattern with implied expectations about sender, recipient, and response behavior.

The protocol exhibits asymmetry between lead and teammate capabilities, reflecting typical organizational structures. Only the lead can `Broadcast` and send `ShutdownRequest`; only teammates send `PlanRequest` and `IdleNotify`; both can send direct `Message`. This asymmetry prevents protocol confusion—teammates cannot accidentally broadcast to each other, maintaining centralized coordination. The acknowledgment pattern in `ShutdownAck` enables clean termination handshakes, preventing message loss during agent shutdown.

The use of an enum rather than stringly-typed messages provides compile-time safety: the Rust compiler exhaustively checks that all message types are handled. The `serde(rename_all = "snake_case")` attribute ensures JSON compatibility with conventional naming while preserving Rust's PascalCase convention. This protocol design scales to future extensions—new message types can be added without breaking existing handlers, with `#[serde(other)]` or explicit handling providing graceful degradation.

Real-world usage patterns would see heavy `Message` traffic during collaborative problem-solving, `PlanRequest`/`PlanApproved`/`PlanRejected` cycles during task decomposition workflows, `IdleNotify` enabling dynamic work redistribution, and `ShutdownRequest`/`ShutdownAck` ensuring clean team dissolution. The protocol's richness enables sophisticated agent behaviors while the simple JSON encoding maintains interoperability and debuggability. This represents a pragmatic middle ground between unstructured natural language coordination (flexible but ambiguous) and rigid RPC interfaces (precise but brittle).

## External Resources

- [Enterprise Integration Patterns: Conversational patterns](https://www.enterpriseintegrationpatterns.com/patterns/conversation/) - Enterprise Integration Patterns: Conversational patterns
- [Finite-state machine concepts for protocol design](https://en.wikipedia.org/wiki/Finite-state_machine) - Finite-state machine concepts for protocol design

## Sources

- [mailbox](../sources/mailbox.md)
