---
status: draft
audit:
  - { time: 1788162887, from: "none", to: "draft", actor: "system" }
---
# Context Panel Specification

## Overview

The TUI shall gain a right-hand side panel that gives the user a live,
quantified breakdown of what currently occupies the active session's context
window. The panel is modelled on the existing logwindow panel and is toggled with
`Alt-C`. It displays the size of each major context partition in tokens and as a
percentage of the current model's reported context-window capacity.

## Motivation

Users frequently do not know why a long conversation suddenly stalls, truncates
or becomes expensive. Showing the relative cost of the system prompt, the tool
catalog, metadata wrappers and the message history makes context pressure
tangible and helps users decide when to compact, change model or disable tools.

## Requirements

### FR-001 (Ubiquitous)
The system shall provide a context information panel that can be displayed
alongside the main TUI chat view.

### FR-002 (Event-driven)
When the user presses `Alt-C`, the system shall toggle the visibility of the
context panel.

### FR-003 (State-driven)
While the context panel is visible, the system shall reserve a vertical
right-hand column of the TUI layout for the panel and render it on every frame.

### FR-004 (State-driven)
While the context panel is hidden, the system shall allocate the full chat
width to the main messages/log area and shall not reserve space for the context
panel.

### FR-005 (Ubiquitous)
The system shall calculate the token size of the active system prompt and
display it in the context panel.

### FR-006 (Ubiquitous)
The system shall calculate the token size of the toolset catalog (the list of
tools, names, descriptions and parameter schemas exposed to the model) and
display it in the context panel.

### FR-007 (Ubiquitous)
The system shall calculate the token size of the toolset metadata (extra
wrappers, XML/JSON envelopes, permission hints or other per-tool overhead sent to
the model) and display it in the context panel.

### FR-008 (Ubiquitous)
The system shall display the token size and message count of the conversation
history currently held in the active session.

### FR-009 (Ubiquitous)
The system shall display the token size of any other key context partitions it
sends to the model, such as skills context, memory injections, agent
instructions, or project guideline (`AGENTS.md`) blocks.

### FR-010 (Ubiquitous)
For every displayed partition, the system shall show the raw token count and
the percentage of the currently selected model's reported context-window size.

### FR-011 (Optional)
Where the model provider does not expose a context-window limit, the system shall
display "unknown" for the percentage values and still show the absolute token
counts.

### FR-012 (Ubiquitous)
The system shall display the sum of all shown partitions and the remaining head
room in tokens and as a percentage.

### FR-013 (Event-driven)
When the context composition changes (for example, after a new message, a tool
call, a model switch, or a compaction event), the system shall refresh the values
shown in the context panel.

### FR-014 (State-driven)
While the panel is visible, the system shall keep the panel content updated
without requiring the user to re-open it.

### FR-015 (Unwanted)
The system shall not block the main agent loop, message input, or tool execution
while the context panel values are being refreshed.

### FR-016 (Unwanted)
The system shall not send the context breakdown itself to the LLM as part of the
conversation history or system context.

### FR-017 (Optional)
Where a partition contributes zero tokens, the system may omit it from the
percentage chart but shall still list it with a count of `0`.

### FR-018 (Ubiquitous)
The panel shall be visually distinguishable from the chat and log areas by a
clear title bar reading "Context" and a border consistent with the active TUI
theme.

## Out of Scope

- Editing values directly from the panel.
- Historical context-size graphs or time-series data.
- Per-provider tokeniser visualisation beyond the provider's reported limit.

## Acceptance Criteria

- `Alt-C` opens and closes the panel reliably from the main chat view.
- All required partitions are visible when the panel is open.
- Token counts refresh automatically after model or session changes.
- The sum of partition sizes and remaining headroom are arithmetically correct.
- The panel never becomes part of the context it is measuring.
