# Agent Type Assignment for `/swarm` — Implementation Plan

## Overview

This plan implements per-team-member agent type assignment in the `/swarm` decomposition and spawn pipeline.

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Design agent type classification schema | FR-002 | S | High | completed | — |
| T-002 | Extend swarm decomposition prompt to emit agent type | FR-002, FR-006 | M | High | completed | T-001 |
| T-003 | Update team member data model to store agent type | FR-003 | S | High | completed | — |
| T-004 | Implement agent type resolver and selector | FR-001, FR-008, FR-009 | M | High | completed | T-001 |
| T-005 | Integrate agent type into `team_spawn` workflow | FR-007 | M | Critical | completed | T-003, T-004 |
| T-006 | Parse explicit agent type hints from task descriptions | FR-004 | S | Medium | completed | T-002 |
| T-007 | Validate assigned agent types against available agents | FR-005, FR-011 | S | High | completed | T-004 |
| T-008 | Add `/swarm` slash-command option for default agent type | FR-010 | S | Medium | completed | T-004 |
| T-009 | Write unit tests for agent type classification | FR-002, FR-008 | M | High | completed | T-001, T-004 |
| T-010 | Write integration tests for `/swarm` agent assignment end-to-end | FR-001, FR-005, FR-007 | L | High | completed | T-005, T-007 |
| T-011 | Update user documentation for `/swarm` agent type assignment | FR-006, FR-012 | S | Medium | completed | T-005 |
## Task Details

### T-001 — Design agent type classification schema

Define the mapping between task kinds and agent types. Decide whether classification is keyword-based, LLM-based, or a hybrid. Document the schema and the default fallback rules.

### T-002 — Extend swarm decomposition prompt to emit agent type

Modify the prompt that decomposes a user request into sub-tasks so that each returned task object includes an `agent_type` field. Update the JSON schema / parser used by the swarm command.

### T-003 — Update team member data model to store agent type

Add an `agent_type: Option<String>` (or strongly typed enum) field to the internal task/member representation maintained by the team runtime while a swarm is active.

### T-004 — Implement agent type resolver and selector

Build a resolver that:

1. Uses the explicit type from the task if present (FR-004).
2. Falls back to classification from task text if no explicit type is given (FR-002).
3. Falls back to `general` when classification is ambiguous (FR-008).
4. Falls back to the lead's current agent when inference is unavailable (FR-009).

### T-005 — Integrate agent type into `team_spawn` workflow

Ensure that when the swarm runtime spawns a teammate, the resolved agent type is passed through to the spawn call (e.g., `team_spawn` or internal equivalent) so the teammate initializes with the correct agent.

### T-006 — Parse explicit agent type hints from task descriptions

Add a lightweight pre-processor or parser that detects explicit agent type hints in task descriptions (for example, `[agent: code-review]`) and strips them before the task is displayed or executed.

### T-007 — Validate assigned agent types against available agents

Before spawning, check the resolved agent type against the runtime agent registry. If unavailable, emit a warning and substitute the fallback agent type (FR-011).

### T-008 — Add `/swarm` slash-command option for default agent type

Extend the `/swarm` parser to accept an optional `--agent` or inline default agent type argument. Apply this default to all sub-tasks that do not have an explicit or inferred type (FR-010).

### T-009 — Write unit tests for agent type classification

Cover:

- Explicit agent type extraction.
- Keyword-based classification.
- Ambiguous fallback to `general`.
- Unknown agent fallback behavior.

### T-010 — Write integration tests for `/swarm` agent assignment end-to-end

Cover:

- A full `/swarm` invocation that produces sub-tasks with assigned agent types.
- Validation that unavailable agent types are substituted with a warning.
- Verification that the spawn call receives the correct agent type.

### T-011 — Update user documentation for `/swarm` agent type assignment

Update the `/swarm` section of the user-facing documentation to describe:

- Automatic agent type inference.
- Explicit agent type hints in task descriptions.
- The swarm-level default agent type option.
- How to inspect assigned agent types via team status.