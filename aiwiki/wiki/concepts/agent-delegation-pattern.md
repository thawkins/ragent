---
title: "Agent Delegation Pattern"
type: concept
generated: "2026-04-19T16:13:14.935308302+00:00"
---

# Agent Delegation Pattern

### From: plan

The agent delegation pattern represents a sophisticated architectural approach to building multi-agent systems where specialized agents with distinct capabilities are dynamically activated and deactivated based on task requirements. In this implementation, delegation is achieved through a combination of event-driven signaling and metadata-based state management, rather than direct function calls or tight coupling between agents. The pattern enables the construction of systems where agents can have constrained permissions—such as the read-only constraint on the plan agent—while still contributing meaningfully to complex workflows. When delegation occurs, the calling agent suspends its execution loop, preserves its state, and transfers control to a specialized agent that possesses the appropriate capabilities for the current subtask. This suspension and resumption mechanism is mediated through events that carry sufficient context to reconstruct the execution environment upon return. The pattern supports compositionality, allowing new specialized agents to be added without modifying existing agent implementations, as long as they adhere to the delegation protocol. Additionally, the pattern enables auditability and observability, as each delegation event is explicitly published and can be logged, monitored, or intercepted by middleware components.

## External Resources

- [Microsoft Research survey on multi-agent systems and delegation patterns](https://www.microsoft.com/en-us/research/publication/multi-agent-systems-a-survey/) - Microsoft Research survey on multi-agent systems and delegation patterns
- [Temporal workflow platform concepts related to durable execution and agent delegation](https://docs.temporal.io/temporal-explained/introduction) - Temporal workflow platform concepts related to durable execution and agent delegation

## Related

- [Event-Driven Architecture](event-driven-architecture.md)

## Sources

- [plan](../sources/plan.md)
