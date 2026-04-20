---
title: "Conversation as Event Sourcing"
type: concept
generated: "2026-04-19T15:23:30.250236537+00:00"
---

# Conversation as Event Sourcing

### From: mod

The message architecture embodies event sourcing principles where conversation state is derived from an immutable append-only log of messages. Each `Message` is a fact that occurred at a specific time, identified by a UUID, and never modified after creation—the `updated_at` field notwithstanding, which likely supports edit operations that create new events. This approach provides significant advantages for AI agent systems: complete audit trails of agent reasoning, reproducible debugging by replaying message sequences, and natural support for branching conversations or undo operations. The `ToolCallState` embedded in messages captures not just the intention to call a tool but the actual outcome, making conversations self-contained records of agent behavior. The session-scoped identifier (`session_id`) provides the stream partition key for aggregating related messages. This design contrasts with mutable state approaches and aligns with how modern LLM APIs structure their completion APIs—as sequences of messages that accumulate context.

## External Resources

- [Martin Fowler on Event Sourcing](https://martinfowler.com/eaaDev/EventSourcing.html) - Martin Fowler on Event Sourcing
- [Anthropic Messages API documentation](https://docs.anthropic.com/en/api/messages-api) - Anthropic Messages API documentation

## Sources

- [mod](../sources/mod.md)
