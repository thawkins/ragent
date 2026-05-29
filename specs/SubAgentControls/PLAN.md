# Implementation Plan: Sub-Agent Execution Controls

## Overview

This plan implements the execution-control affordances defined in `SPEC.md`. The work touches three layers:

1. **Orchestrator / runtime** (`ragent-agent`) — new `AgentHandle` methods for `suspend()`, `resume()`, and `kill()`.
2. **UI** (`ragent-tui`) — new widgets, key handlers, and confirmation dialogs inside the Agents and Teams dialog renderers.
3. **State model** (`ragent-types`) — extension of the agent-state enum to include `Suspended` and `Terminating`.

Implementation is incremental: first extend the shared types, then the runtime, then the UI, then polish with confirmation dialogs and error toasts.

---

## Milestones

### Milestone 1: State Model & Runtime Primitives
**Deliverable:** `ragent-types` and `ragent-agent` can represent and drive `Suspended`/`Terminating` states.

- Add `Suspended` and `Terminating` variants to the agent-status enum.
- Implement `suspend()`, `resume()`, and `kill()` on `AgentHandle` / `SubAgent`.
- Add event types: `AgentSuspended`, `AgentResumed`, `AgentKilled`, `AgentKillTimeout`.
- Unit tests for state transitions (happy path and invalid transitions).

### Milestone 2: UI Rendering & Input Handling
**Deliverable:** The TUI draws working buttons and reacts to clicks / hotkeys.

- Add button widgets (Play ▷, Stop ⏹, Kill ✕) to the agent-row renderer.
- Implement mouse-click hit-testing for the buttons.
- Implement keyboard shortcuts (Space, Shift+K).
- Visual badges for `Suspended` and `Terminating`.

### Milestone 3: Confirmation Dialogs & Safety Guards
**Deliverable:** Users cannot accidentally suspend or kill agents with uncommitted work.

- Suspend confirmation dialog (warn on pending tool calls / uncommitted edits).
- Kill confirmation dialog with “Kill & abandon” vs “Kill & preserve output” options.
- Force-kill escalation after 10-second timeout.
- Toast notification system for error feedback.

### Milestone 4: Teams Dialog Integration
**Deliverable:** Controls work identically in the Teams dialog and update team bookkeeping.

- Propagate kill action to the team runtime (remove from member list, mailbox, task claims).
- Team-lead kill handling (prompt for replacement or dissolve).
- End-to-end integration tests with a mock team.

---

## Tasks

| ID | Title | Requirement | Effort | Priority | Dependencies |
|----|-------|-------------|--------|----------|--------------|
| T-001 | Add `Suspended` and `Terminating` variants to agent-state enum | FR-001, FR-005 | S | Critical | — |
| T-002 | Implement `AgentHandle::suspend()` and event emission | FR-002 | M | Critical | T-001 |
| T-003 | Implement `AgentHandle::resume()` with queue restoration | FR-003 | M | Critical | T-002 |
| T-004 | Implement `AgentHandle::kill()` and resource reclamation | FR-004 | M | Critical | T-001 |
| T-005 | Add suspend/resume/kill events to the event bus | FR-002, FR-003, FR-004 | S | Critical | T-001 |
| T-006 | Render Play/Stop and Kill buttons on each agent row | FR-001 | M | High | T-001 |
| T-007 | Implement mouse-click hit-testing for buttons | FR-001, FR-008 | M | Medium | T-006 |
| T-008 | Implement keyboard shortcuts (Space, Shift+K) | FR-008 | S | Medium | T-006 |
| T-009 | Add visual badges for Suspended and Terminating states | FR-005 | S | High | T-001, T-006 |
| T-010 | Implement suspend confirmation dialog with work-in-progress warning | FR-006 | M | High | T-002, T-006 |
| T-011 | Implement kill confirmation dialog with abandon/preserve options | FR-007 | M | High | T-004, T-006 |
| T-012 | Implement force-kill escalation after 10 s timeout | FR-009 | S | High | T-004 |
| T-013 | Add toast notification system for operation errors | FR-009 | M | Medium | T-006 |
| T-014 | Wire controls into the Teams dialog renderer | FR-010 | M | High | T-006 |
| T-015 | Update team runtime on agent kill (member list, mailbox, tasks) | FR-010 | M | High | T-004, T-014 |
| T-016 | Handle team-lead kill with replacement/dissolve prompt | FR-010 | M | Medium | T-015 |
| T-017 | Unit tests for state transitions and invalid transitions | FR-001–FR-005 | M | Critical | T-001–T-005 |
| T-018 | Integration tests for suspend→resume→kill lifecycle | FR-002–FR-004 | L | High | T-001–T-005 |
| T-019 | Integration tests for Teams dialog kill and lead replacement | FR-010 | L | Medium | T-014–T-016 |
| T-020 | Update TUI help footer with new keyboard shortcuts | FR-008 | S | Low | T-008 |
