---
status: draft
---
# TUI Performance Optimisation Implementation Plan

## Requirements

See `specs/tuiopt/SPEC.md` for the full EARS requirements list.

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Audit existing performance findings and benchmarks | FR-001, FR-003 | S | Medium | completed | — |
| T-002 | Make main loop event-driven and remove 20 Hz idle wake | FR-001, FR-007 | S | High | completed | T-001 |
| T-003 | Convert model/provider discovery to async background tasks | FR-002, FR-004, FR-005 | M | Critical | completed | T-001 |
| T-004 | Cache message timeline and wrapped line counts | FR-003, FR-006 | M | High | completed | T-001 |
| T-005 | Optimise active-agents panel traversal and step counts | FR-003, FR-008 | M | High | completed | T-001 |
| T-006 | Cache teams panel state and remove per-frame storage reads | FR-003, FR-009 | M | High | completed | T-001 |
| T-007 | Reuse markdown worker thread and debounce streaming fragments | FR-002, FR-010 | M | High | completed | T-001 |
| T-008 | Reduce per-frame string clones in status bar and layout | FR-003 | S | Medium | completed | T-001 |
| T-009 | Cache picker list state for model/provider/file/slash menus | FR-003 | S | Medium | completed | T-001 |
| T-010 | Rate-limit log-panel drain per frame | FR-001 | S | Medium | completed | T-002 |
| T-011 | Append-only input-history flush | FR-011 | S | Medium | completed | T-001 |
| T-012 | Add Criterion benchmarks for message list, active agents, and teams panels | FR-001, FR-003 | M | Low | completed | T-004, T-005, T-006 |
| T-013 | Document optimisation changes and run validation tests | FR-012, FR-013 | S | High | completed | T-002, T-003 |
## Task Details

### T-001 — Audit existing performance findings and benchmarks

Read `crates/ragent-tui/performance_findings.md`, the current `benches/`
directory, and the main loop, layout, active-agents, and teams source files.
Confirm which items are already partially addressed and which still apply.
Record the baseline in a short note inside the spec directory.

### T-002 — Make main loop event-driven and remove 20 Hz idle wake

Edit `crates/ragent-tui/src/lib.rs` so the main loop redraws only when
`app.needs_redraw` is true, the minimum refresh interval expires, or an input /
event-bus / background-channel message arrives. Replace the fixed 50 ms sleep
with a long idle timeout. Ensure `needs_redraw` is set by all state-changing paths.

### T-003 — Convert model/provider discovery to async background tasks

Replace UI-thread `block_in_place` + `block_on` calls in
`crates/ragent-tui/src/app/models.rs`, `event_handler.rs`, and `slash.rs` with
`tokio::spawn` tasks that return results through the event bus. Introduce
loading states in the relevant pickers and ensure the UI remains responsive
while discovery runs.

### T-004 — Cache message timeline and wrapped line counts

Cache `Vec<Line>` and wrapped copy-lines in `App` keyed by `(message count,
last message id/hash, width)`. Invalidate on new user message, appended
assistant text, resize, or selection/width change. Cache `paragraph.line_count`
alongside the line cache.

### T-005 — Optimise active-agents panel traversal and step counts

In `crates/ragent-tui/src/layout_active_agents.rs`, build a parent-to-children
map once per render pass and walk it instead of scanning the task list for each
row. Cache per-session step counts and `custom_names` / `teammate_ids` sets in
`App`, updating them only when the source data changes. Avoid cloning
`active_tasks` each frame.

### T-006 — Cache teams panel state and remove per-frame storage reads

Maintain a `TeamCache` in `App` that is updated by team events. In
`crates/ragent-tui/src/layout_teams.rs`, render from the cache and avoid disk
I/O during `render_teams_subpanel`. Remove the per-frame
`refresh_team_member_session_ids`, `team_members.clone()`, and direct storage
reads.

### T-007 — Reuse markdown worker thread and debounce streaming fragments

Convert the per-cache-miss `std::thread::spawn` in the markdown rendering
pipeline to a persistent worker thread (or a small pool) with panic isolation.
Debounce streaming text fragments so conversion is not invoked for every token.
Keep the existing cache but avoid its full clear on eviction.

### T-008 — Reduce per-frame string clones in status bar and layout

In `crates/ragent-tui/src/layout_statusbar.rs` and `layout.rs`, cache status-bar
title strings and responsive layout constraints until their inputs change. Use
borrowed `&str` and `as_deref()` in hot render paths instead of cloning
`Option<String>` fields.

### T-009 — Cache picker list state for model/provider/file/slash menus

Build picker visible lists once when the picker opens or the filter changes;
store them on the picker struct. Update only on data change, selection change,
or scroll. Apply to provider/model/file/slash pickers in `layout.rs`,
`input.rs`, and `app/input_handler.rs`.

### T-010 — Rate-limit log-panel drain per frame

In `crates/ragent-tui/src/lib.rs`, cap the number of tracing records drained
from `log_rx` per frame (for example, 50). Set `needs_redraw = true` only if
records were added.

### T-011 — Append-only input-history flush

In `crates/ragent-tui/src/app/state.rs`, track the number of history entries
already flushed and append only new entries to the history file instead of
serialising the entire vector each time.

### T-012 — Add Criterion benchmarks for message list, active agents, and teams panels

Extend `crates/ragent-tui/benches/` with benchmarks for `render_messages` at
100/500/2000 messages, `render_active_agents_subpanel` at 50/200 tasks, and
`render_teams_subpanel` with 20 teammates. Run the benchmarks before and after
the optimisation work.

### T-013 — Document optimisation changes and run validation tests

Update `crates/ragent-tui/performance_findings.md` with what was changed. Run
the existing TUI unit/integration tests, the new benchmarks, and a manual smoke
test through the TUI to confirm no regressions. Update `CHANGELOG.md` when the
work is complete.