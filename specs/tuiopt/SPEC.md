---
status: draft
audit:
  - { time: 1787547184, from: "none", to: "draft", actor: "system" }
---
# TUI Performance Optimisation Review

## Context

The `ragent-tui` crate provides the full-screen ratatui interface for ragent. As
sessions grow (long message histories, many active sub-agents, large teams, and
streaming assistant output), several UI paths have become expensive on a
per-frame basis. The current implementation performs redundant work, blocks the UI
thread with synchronous async calls, and redraws at a fixed rate even when
nothing has changed. This specification defines a focused optimisation effort
that improves responsiveness, reduces CPU/battery use, and keeps large sessions
smooth.

The scope is limited to the `crates/ragent-tui` crate and its existing
benchmarks. The goal is to make targeted, measurable improvements without
rewriting the TUI architecture.

## Requirements

### Ubiquitous requirements

FR-001: The TUI **shall** redraw only when state has changed, a user input
arrives, or a periodic minimum refresh interval expires.

FR-002: The TUI **shall** keep all network, disk, and long-running computation
off the UI thread by delegating work to background tasks and reacting to events.

FR-003: The TUI **shall** avoid recomputing expensive derived data on every
frame by caching results until the underlying state changes.

### Event-driven requirements

FR-004: When a long-running operation (for example, model discovery, spec load,
or team storage read) is triggered from the UI, the TUI **shall** display a
loading state and post the result via the event bus or a channel; it **shall**
not call `block_on` or `block_in_place` from the UI thread.

FR-005: When a background task completes, the TUI **shall** update `App` state,
set the redraw flag, and render the new data without polling.

FR-006: When a message is appended during streaming, the TUI **shall** update
only the affected message region and cached derived lines instead of rebuilding
the entire message timeline.

### State-driven requirements

FR-007: While the TUI is idle and no state has changed, the main loop **shall**
sleep until the next event rather than waking at a fixed 20 Hz cadence.

FR-008: While the active-agents panel is visible, its render path **shall** use
precomputed parent-to-child references and cached step counts rather than
scanning the full task list for every row.

FR-009: While the teams panel is visible, its render path **shall** read from an
in-memory cache that is updated by team events rather than reloading task and
team storage from disk each frame.

### Optional requirements

FR-010: The markdown-to-text pipeline **may** reuse a single long-lived worker
thread instead of spawning a new OS thread for every cache miss.

FR-011: The input-history flush **may** use an append-only file write and track
the number of entries already flushed to avoid serialising the entire history on
every flush.

### Unwanted requirements

FR-012: The TUI **shall not** introduce new unsafe code, new public API
surfaces, or behavioural changes to slash commands and key bindings beyond the
optimisations described.

FR-013: Optimisations **shall not** remove existing accessibility, logging, or
status information; only the cost of producing it **shall** be reduced.

## Glossary

- **UI thread** — the async task that owns `App`, handles crossterm input, and
calls `terminal.draw()`.
- **Event bus** — the internal tokio pub/sub used to propagate state changes
throughout the TUI and other crates.
- **Derived data** — rendered lines, wrapped text, parent-to-child maps, step
counts, status-bar strings, and other values computed from `App` state.
- **Background task** — a `tokio::spawn` or `tokio::task::spawn_blocking` task
that runs independently of the UI thread and communicates results via channels
or the event bus.

## References

- `crates/ragent-tui/performance_findings.md` — prior performance review.
- `crates/ragent-tui/benches/` — existing Criterion benchmarks.
