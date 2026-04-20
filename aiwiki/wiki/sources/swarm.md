---
title: "Ragent Swarm: LLM-Driven Task Decomposition System"
source: "swarm"
type: source
tags: [rust, multi-agent-systems, llm-orchestration, task-decomposition, serde, json-parsing, swarm-intelligence, ragent, agent-teams, parallel-computing]
generated: "2026-04-19T21:08:22.346660054+00:00"
---

# Ragent Swarm: LLM-Driven Task Decomposition System

This document presents the `swarm.rs` module from the `ragent-core` crate, which implements a Fleet-style auto-decomposition system for multi-agent AI teams. The swarm pattern enables a high-level user goal to be automatically broken down into independent, parallelizable subtasks by an LLM, complete with dependency tracking and runtime state management. The module provides robust JSON parsing capabilities that handle common LLM output quirks including markdown code fences, trailing commas, and extraneous whitespace.

The architecture centers on three core data structures: `SwarmSubtask` representing individual work units with their metadata and dependencies, `SwarmDecomposition` as the root container for parsed task lists, and `SwarmState` for tracking the lifecycle of an active swarm execution. The system employs a carefully crafted system prompt that instructs the LLM to generate between 2-8 self-contained subtasks with minimal dependencies, enabling maximum parallel execution across distributed agent contexts. Each subtask can optionally specify agent type and model overrides for specialized processing needs.

The implementation includes comprehensive test coverage validating clean JSON parsing, markdown fence stripping, trailing comma removal, and error handling for malformed responses. The `remove_trailing_commas` function demonstrates defensive programming against a common LLM failure mode where JSON arrays or objects incorrectly terminate with commas. This robustness is essential for production multi-agent systems where LLM output reliability cannot be guaranteed.

## Related

### Entities

- [SwarmSubtask](../entities/swarmsubtask.md) — technology
- [SwarmDecomposition](../entities/swarmdecomposition.md) — technology
- [SwarmState](../entities/swarmstate.md) — technology
- [DECOMPOSITION_SYSTEM_PROMPT](../entities/decomposition-system-prompt.md) — technology

