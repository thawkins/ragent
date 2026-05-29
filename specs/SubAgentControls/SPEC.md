---
status: draft
---

# Specification: Sub-Agent Execution Controls

## Executive Summary

This document specifies controls for managing the execution lifecycle of individual sub-agents within ragent's **Agents Dialog** and **Teams Dialog**. Each executing agent row shall expose **Play/Stop** (suspend/resume) and **Kill** buttons, enabling users to pause, resume, or forcibly terminate specific agents without affecting the rest of the session or team.

## Scope & Objectives

### Scope
- UI affordances (buttons / hotkeys) on every running agent row in the Agents and Teams dialogs.
- State transitions: **Running → Suspended → Running** and **Running/Suspended → Terminated**.
- Visual feedback reflecting the current execution state and the outcome of control actions.
- Prevention of accidental data loss or inconsistent state when suspending agents with pending work.

### Out of Scope
- Granular per-agent resource limits (CPU/memory throttling).
- Automatic suspend/resume based on idle time.
- Undo / resurrection of a killed agent (a killed agent is gone; the user may spawn a replacement).

### Objectives
1. Give operators fine-grained control over background agent execution.
2. Reduce risk of runaway or stuck agents consuming tokens or quota.
3. Maintain clear visual state so users always know which agents are active, paused, or dead.

---

## Requirements

### FR-001 — Control Buttons on Agent Rows

`The <Agents Dialog and Teams Dialog> shall <display a Play/Stop toggle button and a Kill button on every row that represents an executing sub-agent>.`

`While <an agent row is visible in the dialog>, the <UI> shall <keep the buttons enabled unless the agent is already terminated>.`

### FR-002 — Suspend (Stop) Action

`When <the user presses the Stop button (or equivalent hotkey) on a running agent row>, the <orchestrator> shall <send a suspend signal to the designated agent and update its displayed state to Suspended>.`

`Where <the TUI is active>, the <UI> shall <render the Stop button as a filled square (⏹) and the Play button as a hollow triangle (▷)>.`

### FR-003 — Resume (Play) Action

`When <the user presses the Play button on a suspended agent row>, the <orchestrator> shall <resume the agent's event loop and update its displayed state back to Running>.`

`If <the agent's event queue was drained during suspension>, the <orchestrator> shall <restore the queue from a snapshot taken at suspend time before resuming>.`

### FR-004 — Kill Action

`When <the user presses the Kill button on a running or suspended agent row>, the <orchestrator> shall <forcibly terminate the designated agent, reclaim its resources, and remove its row from the active list>.`

`The <system> shall <log the kill event with the agent's ID, the user who triggered it, and a UTC timestamp>.`

### FR-005 — Visual State Indicators

`While <an agent is in the Suspended state>, the <UI> shall <dim the agent's row, append a “⏸ Suspended” badge, and disable input-related columns>.`

`While <an agent is in the Terminating state>, the <UI> shall <replace the buttons with a spinner and the text “Terminating…">.`

### FR-006 — Suspend Warning for Uncommitted Work

`If <a user attempts to suspend an agent that has uncommitted file changes or pending tool calls>, the <system> shall <display a confirmation dialog warning of potential data loss and require an explicit second confirmation before proceeding>.`

`If <the user cancels the confirmation>, the <system> shall <leave the agent in the Running state and make no state change>.`

### FR-007 — Kill Confirmation

`If <a user attempts to kill an agent>, the <system> shall <display a confirmation dialog stating that the action is irreversible and requiring explicit confirmation>.`

`The <system> shall <provide a “Kill & abandon” option and a “Kill & preserve output” option; the latter copies the agent's current output buffer to the session log before termination>.`

### FR-008 — Keyboard Shortcuts

`Where <the TUI is focused on the Agents or Teams dialog>, the <system> shall <support keyboard shortcuts for the three actions: Space for Play/Stop, Shift+K for Kill>.`

`The <system> shall <display the shortcut hints next to the button labels or in a footer help bar>.`

### FR-009 — Error Handling & Feedback

`When <a suspend, resume, or kill operation fails>, the <UI> shall <show a transient toast notification with the error message and leave the agent row in its previous stable state>.`

`If <an agent does not terminate within 10 seconds of a kill request>, the <orchestrator> shall <escalate to a force-kill (SIGKILL equivalent) and report the escalation in the notification>.`

### FR-010 — Teams Dialog Specifics

`While <the Teams Dialog is open>, the <system> shall <synchronise the control buttons with the team runtime so that a kill action also removes the agent from the team's member list and mailbox routing table>.`

`If <a team lead agent is killed>, the <system> shall <prompt the user to appoint a replacement lead or dissolve the team>.`
