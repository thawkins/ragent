---
status: draft
---

# Context Panel Implementation Plan

## Summary

This plan adds a right-hand context-breakdown panel to the ragent TUI. The panel
is modelled on the existing logwindow panel, bound to `Alt-C`, and shows the token
size and percentage share of the major components that make up the active
context window.

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Add context panel state flag and key binding | FR-001, FR-002 | S | High | completed | — |
| T-002 | Reserve right-hand layout column when panel is open | FR-003, FR-004 | M | High | completed | T-001 |
| T-003 | Compute system prompt token size | FR-005 | S | High | completed | — |
| T-004 | Compute toolset catalog token size | FR-006 | M | High | completed | — |
| T-005 | Compute toolset metadata/wrapper token size | FR-007 | M | High | completed | T-004 |
| T-006 | Compute conversation history token size and message count | FR-008 | M | High | completed | — |
| T-007 | Compute skills, memory and AGENTS.md partition sizes | FR-009 | M | Medium | completed | T-003 |
| T-008 | Fetch selected model's context-window limit | FR-010, FR-011 | S | High | completed | — |
| T-009 | Aggregate totals and remaining headroom | FR-012 | S | High | completed | T-003, T-004, T-005, T-006, T-007, T-008 |
| T-010 | Refresh panel on context change events | FR-013, FR-014 | M | High | completed | T-001, T-009 |
| T-011 | Render panel with title, border and percentage bars | FR-018 | M | Medium | completed | T-002, T-009 |
| T-012 | Ensure panel content is excluded from LLM context | FR-016 | S | High | completed | T-003, T-009 |
| T-013 | Add non-blocking refresh scheduling | FR-015 | M | High | completed | T-010 |
| T-014 | Update TUI key help and slash command docs | FR-001, FR-002 | S | Low | completed | T-001, T-011 |
| T-015 | Write unit tests for token partition arithmetic | FR-009, FR-012 | M | Medium | completed | T-009 |
| T-016 | Perform manual test plan validation | All FRs | L | High | completed | T-001 through T-014 |
## Notes

- T-004 and T-005 must use the same serialisation path the agent uses when it
  builds the tool block sent to the LLM, so that the displayed size matches the
  actual context cost.
- T-013 should refresh token counts asynchronously; the values may be stale by a
  frame or two but must never stall input handling.
- T-012 must ensure the synthetic "Context" panel text is never appended to the
  message list that is later serialised for the model.