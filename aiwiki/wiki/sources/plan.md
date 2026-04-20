---
title: "Agent Delegation Tools for Planning: PlanEnterTool and PlanExitTool"
source: "plan"
type: source
tags: [rust, agent-delegation, multi-agent-systems, event-driven-architecture, tool-system, planning-agent, async-trait, serde-json, anyhow]
generated: "2026-04-19T16:13:14.933586386+00:00"
---

# Agent Delegation Tools for Planning: PlanEnterTool and PlanExitTool

This Rust source file implements two specialized tool structures, `PlanEnterTool` and `PlanExitTool`, that enable controlled delegation between a main agent and a specialized "plan" sub-agent within the ragent framework. The module provides a mechanism for temporarily transferring task execution to a read-only planning agent that performs codebase analysis and architecture planning without file modification capabilities. The `PlanEnterTool` initiates this delegation by publishing an `AgentSwitchRequested` event and returning metadata that signals the session processor to break the current agent loop, allowing the TUI or consumer to switch to the plan agent. Conversely, `PlanExitTool` restores control to the previous agent by publishing an `AgentRestoreRequested` event, carrying a summary of the planning analysis back into the conversation context. This bidirectional tool pair exemplifies a sophisticated multi-agent orchestration pattern where specialized agents with constrained permissions can be dynamically activated and deactivated while maintaining session continuity and conversational context.

## Related

### Entities

- [PlanEnterTool](../entities/planentertool.md) — technology
- [PlanExitTool](../entities/planexittool.md) — technology
- [ToolContext](../entities/toolcontext.md) — technology

### Concepts

- [Agent Delegation Pattern](../concepts/agent-delegation-pattern.md)
- [Event-Driven Agent Orchestration](../concepts/event-driven-agent-orchestration.md)
- [Capability-Based Agent Permissions](../concepts/capability-based-agent-permissions.md)
- [JSON Schema Validation for Tool Parameters](../concepts/json-schema-validation-for-tool-parameters.md)

