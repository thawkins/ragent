---
title: "Event-Driven Coordination"
type: concept
generated: "2026-04-19T19:49:16.260793277+00:00"
---

# Event-Driven Coordination

### From: team_wait

Event-driven coordination represents a fundamental architectural pattern for managing concurrency in distributed systems, particularly relevant to multi-agent orchestration. Unlike polling-based approaches that repeatedly query state, event-driven systems use publish-subscribe mechanisms where interested parties receive notifications when significant state transitions occur.

The implementation in TeamWaitTool exemplifies this pattern's advantages. By subscribing to TeammateIdle events before entering the wait loop, the tool achieves several critical properties: efficient resource utilization (no CPU consumption during waiting), low latency response (immediate wake on relevant event), and natural scalability (the pattern works identically for 2 or 200 agents). The pattern eliminates the classical trade-off between poll frequency and overhead—frequent polling wastes resources while infrequent polling increases latency.

Practical considerations for event-driven coordination include handling the "missed event" problem through careful ordering of subscription and state check, managing backpressure through bounded channels, and providing timeout mechanisms for liveness guarantees. The code demonstrates sophisticated handling of these concerns: the subscribe-before-check pattern prevents missing idle transitions, Tokio's broadcast channels provide natural flow control, and timeout_at ensures the tool cannot block indefinitely. These techniques combine to create a robust coordination primitive that would be difficult to achieve with polling or direct communication patterns.

## External Resources

- [Microsoft Azure event-driven architecture patterns](https://docs.microsoft.com/en-us/azure/architecture/patterns/event-driven-architecture) - Microsoft Azure event-driven architecture patterns
- [Enterprise Integration Patterns: Event-Driven Consumer](https://www.enterpriseintegrationpatterns.com/patterns/messaging/EventDrivenConsumer.html) - Enterprise Integration Patterns: Event-Driven Consumer
- [Tokio channels tutorial for async message passing](https://tokio.rs/tokio/tutorial/channels) - Tokio channels tutorial for async message passing

## Related

- [Race Condition Prevention](race-condition-prevention.md)

## Sources

- [team_wait](../sources/team-wait.md)
