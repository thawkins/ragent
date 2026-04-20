---
title: "QuestionTool: User Interaction Mechanism for Agent Sessions"
source: "question"
type: source
tags: [rust, async, event-driven, agent-systems, human-in-the-loop, tokio, tool-system, user-interaction, permission-system, ragent-core]
generated: "2026-04-19T18:53:30.607069559+00:00"
---

# QuestionTool: User Interaction Mechanism for Agent Sessions

The `question.rs` file implements `QuestionTool`, a Rust-based user interaction mechanism within the `ragent-core` crate that enables AI agents to solicit free-text input from users during active sessions. This tool bridges the gap between autonomous agent execution and human-in-the-loop decision-making by publishing permission request events to an event bus and blocking until user responses are received. The implementation leverages Tokio's asynchronous runtime with broadcast channels for event-driven communication, ensuring robust handling of concurrent events while maintaining session isolation through UUID-based request tracking.

The architecture follows a structured event-driven pattern where the tool first subscribes to the event bus before publishing its request, preventing race conditions where responses might arrive before the subscription is active. The tool specifically listens for `Event::PermissionRequested` events with `permission == "question"` and awaits corresponding `Event::UserInput` responses filtered by session and request identifiers. This design enables sophisticated agent behaviors including clarification requests, prioritization assistance, and confirmation flows that require genuine human judgment rather than automated inference.

Error handling is comprehensive, addressing scenarios such as missing parameters, event bus closure, and dropped messages through explicit `anyhow` error propagation. The tool integrates with JSON Schema for parameter validation, specifying a required "question" string field that agents populate when invoking the tool. Metadata tracking includes request IDs, original questions, and user responses, enabling audit trails and debugging capabilities for complex multi-turn agent interactions.

## Related

### Entities

- [QuestionTool](../entities/questiontool.md) — product
- [Tokio](../entities/tokio.md) — technology
- [uuid](../entities/uuid.md) — technology

