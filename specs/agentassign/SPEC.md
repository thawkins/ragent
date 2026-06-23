---
status: implemented
audit:
  - { time: 1782020335, from: "none", to: "draft", actor: "system" }
  - { time: 1782023700, from: "draft", to: "implemented", actor: "agent" }
---
# Agent Type Assignment for `/swarm` — Specification

## Introduction

The `/swarm` slash command decomposes a user prompt into parallel sub-tasks and spawns a team of teammates to work on them concurrently. Currently, all spawned teammates use a default agent type. This specification introduces per-team-member agent type assignment so that the kind of agent spawned for each task is selected automatically from the task description, with optional user override and safe fallbacks.

## Scope

This feature covers:

- Classification of each decomposed sub-task into an appropriate agent type.
- Storage of the selected agent type against each team member / task.
- Validation that the selected agent type is available in the current runtime.
- Passing the selected agent type through to the underlying teammate spawn mechanism.
- User-visible controls for overriding or defaulting the agent type selection.

Out of scope:

- Adding new agent definitions (assumes existing agent registry).
- Changing the behavior of already-running teammates after they have been spawned.

## Requirements

### FR-001 — Ubiquitous

The `/swarm` system shall support assigning a specific agent type to each spawned team member based on the task assigned to that member.

### FR-002 — Event-driven

When the swarm decomposition process creates a sub-task, the system shall classify the sub-task and determine the most appropriate agent type for that sub-task.

### FR-003 — State-driven

While a swarm team is active, the system shall maintain an association between each team member and the agent type assigned to that member.

### FR-004 — Optional

Where a sub-task description explicitly specifies an agent type, the system shall use that agent type for the corresponding team member instead of inferring one.

### FR-005 — Unwanted

The system shall not assign an agent type to a team member unless that agent type is registered and available in the current runtime.

### FR-006 — Ubiquitous

The swarm decomposition output shall include, for each sub-task, the selected agent type alongside the task title and description.

### FR-007 — Event-driven

When a team member is spawned, the system shall pass the assigned agent type to the teammate spawn mechanism.

### FR-008 — State-driven

While the task classification is ambiguous or yields no clear match, the system shall default to the `general` agent type.

### FR-009 — Ubiquitous

The system shall provide a fallback mechanism to use the swarm lead's current agent type when inference is unavailable and no explicit type is provided.

### FR-010 — Optional

Where the user provides a swarm-level default agent type via the `/swarm` command, the system shall apply that default to all sub-tasks that do not specify or infer a more specific agent type.

### FR-011 — Event-driven

When an assigned agent type is unavailable, the system shall emit a warning and substitute the configured fallback agent type before spawning the team member.

### FR-012 — Ubiquitous

The system shall expose the assigned agent type in the team status output so that the user can inspect which agent each teammate is running.
