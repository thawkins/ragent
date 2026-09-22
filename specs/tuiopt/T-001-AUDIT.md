# T-001 Audit Report: Existing Performance Findings and Benchmarks

## Summary

The `ragent-tui` crate already contains a prior performance review
(`performance_findings.md`) and three Criterion benchmarks. This audit
confirms which findings are still applicable after recent code changes, notes
current source locations, and identifies gaps in benchmark coverage that will
be addressed by later tasks.

## Status of Prior Findings

### 1. Unnecessary/redraw-per-frame of entire TUI
- **Finding source:** `crates/ragent-tui/performance_findings.md` Finding 1.
- **Current state:** Partially addressed.
- **Evidence:** `crates/ragent-tui/src/lib.rs:658-663` already gates `terminal.draw()` on `app.needs_redraw` or a 250 ms minimum refresh interval (`IDLE_REDRAW_INTERVAL_MS`).
- **Still a problem:** `lib.rs:671` uses `tokio::time::sleep(Duration::from_millis(50))`, which wakes the loop 20 times per second, runs the polling arms, and can force redraws at 4 Hz because of the 250 ms timer. This is covered by FR-001 / FR-007 and will be fixed in T-002.

### 2. Quadratic traversal in active-agents panel
- **Finding source:** `performance_findings.md` Finding 2.
- **Current state:** Partially addressed.
- **Evidence:** `crates/ragent-tui/src/layout_active_agents.rs:191-197` now builds a `parent_session_id -> Vec<&TaskEntry>` map once per render, so the traversal is no longer quadratic.
- **Still a problem:** The function still calls `app.active_tasks.clone()` indirectly through borrowing? No — it now borrows (`for task in &app.active_tasks`). However, `format_elapsed` calls `Utc::now()` per row per frame, and the per-frame `custom_names` / `teammate_ids` HashSets are still rebuilt on every render at lines 201-210. FR-008 / T-005 will cache these.

### 3. Per-frame allocations in active panel
- **Finding source:** `performance_findings.md` Finding 3.
- **Current state:** Unchanged.
- **Evidence:** `layout_active_agents.rs:201-210` rebuilds `custom_names` and `teammate_ids` HashSets each frame. FR-008 / T-005.

### 4. UI-blocking synchronous model discovery
- **Finding source:** `performance_findings.md` Finding 4.
- **Current state:** Unchanged.
- **Evidence:** `crates/ragent-tui/src/app/models.rs:1330-1356` still uses `tokio::task::block_in_place(|| handle.block_on(provider.discover_models()))`. The comment explains it is skipped in single-threaded tests. FR-004 / T-003.

### 5. Markdown cache eviction
- **Finding source:** `performance_findings.md` Finding 5.
- **Current state:** Addressed.
- **Evidence:** `crates/ragent-tui/src/app/state.rs:1488` uses `LruCache<u64, String>` with capacity 256. `crates/ragent-tui/src/app/models.rs:196-237` uses `get`, and the old `clear()` path is now a comment about LRU eviction. The cache no longer fully clears on capacity. FR-003 is already partly satisfied here; T-007 will still debounce streaming and reuse the markdown worker thread.

### 6. Blocking git detection at startup
- **Finding source:** `performance_findings.md` Finding 6.
- **Current state:** Out of scope for this optimisation pass.
- **Rationale:** Startup impact is low and the focus is on per-frame / runtime cost. Not included in the tuiopt plan.

### 7. Clipboard image double-copy
- **Finding source:** `performance_findings.md` Finding 7.
- **Current state:** Out of scope.
- **Rationale:** Paste path is not a hot frame-time path.

### 8. Repeated `to_lowercase()` in directory listing
- **Finding source:** `performance_findings.md` Finding 8.
- **Current state:** Not verified during this audit; low priority. Not in the tuiopt plan.

### 9. Hot-path formatting in layout code
- **Finding source:** `performance_findings.md` Finding 9.
- **Current state:** Still present.
- **Evidence:** `layout.rs:4115-4178`, `layout_active_agents.rs`, `layout_teams.rs`, `layout_statusbar.rs` all allocate strings/spans per frame. This will be reduced indirectly by T-002 (fewer redraws) and directly by T-008 (borrowed data, cached strings).

## Additional Issues Identified During Audit

### A. Per-frame storage I/O in teams panel
- **File:** `crates/ragent-tui/src/layout_teams.rs:63-107`.
- **Issue:** `refresh_team_member_session_ids()` is called every frame; `TeamStore::load_by_name` and `TaskStore::open`/`read` read SQLite/json from disk every render; `app.team_members.clone()` copies the whole member list.
- **Requirement:** FR-009.
- **Task:** T-006.

### B. Full-history serialisation before flush
- **File:** `crates/ragent-tui/src/app/state.rs:1828-1860`.
- **Issue:** `flush_history_if_due` converts the entire `input_history` vector to a string on every flush, then writes it in `spawn_blocking`. This is O(n) work on the UI thread.
- **Requirement:** FR-011.
- **Task:** T-011.

### C. Log panel drains unbounded records per frame
- **File:** `crates/ragent-tui/src/lib.rs:619-629`.
- **Issue:** `while let Ok(record) = log_rx.try_recv()` can process hundreds of records in one frame, delaying input handling.
- **Requirement:** FR-001.
- **Task:** T-010.

### D. Message timeline rebuilt every frame
- **File:** `crates/ragent-tui/src/layout.rs:4115-4178`.
- **Issue:** `messages_to_lines`, `build_wrapped_content_lines`, and `paragraph.line_count` run every redraw regardless of whether messages changed.
- **Requirement:** FR-003, FR-006.
- **Task:** T-004.

### E. Many `block_in_place` calls beyond model discovery
- **Files:** `event_handler.rs`, `slash.rs`.
- **Issue:** Spec operations, reverse, research, swarm, and several slash commands still block the UI thread with `tokio::task::block_in_place`. The scope of T-003 is model/provider discovery; the remaining slash/event-handler cases may require a follow-up task, but the spec limits T-003 to model discovery to keep the change surgical.

## Benchmarks

### Existing

| Benchmark file | What it measures | Relevant to tuiopt |
|----------------|------------------|--------------------|
| `benches/bench_markdown.rs` | `render_markdown_to_ascii` at 1/10/100 KB, cache hit/miss | T-007 |
| `benches/bench_history.rs` | `save_history` / `load_history` at 100/500/2,000 entries | T-011 |
| `benches/bench_cursor.rs` | Cursor byte position and insert/delete at 1k/10k/100k | Not directly in scope |

### Missing

| Needed benchmark | Motivation | Task |
|------------------|------------|------|
| Message-list render with 100/500/2,000 messages | Validate T-004 | T-012 |
| Active-agents subpanel with 50/200 tasks | Validate T-005 | T-012 |
| Teams subpanel with 20 teammates | Validate T-006 | T-012 |
| Full-frame render under idle and streaming | Validate T-002 and overall impact | T-012 |

## Build Status

`cargo build -p ragent-tui` completed successfully at the start of the audit with
no warnings emitted.

## Conclusion

The highest-impact remaining issues are:

1. **T-002** — main loop wakes at 20 Hz and redraws up to 4 Hz while idle.
2. **T-003** — model discovery blocks the UI thread.
3. **T-004** — message timeline recomputed every frame.
4. **T-006** — teams panel performs disk I/O every frame.
5. **T-005** — active-agents panel still allocates per-frame HashSets and `Utc::now()` per row.

Markdown cache eviction (Finding 5) is already resolved by the `LruCache`
migration. Benchmarks exist for markdown and history but not for the main render
paths. T-012 will close that gap.
