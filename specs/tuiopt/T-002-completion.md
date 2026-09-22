# T-002 Completion Report: Event-Driven Main Loop

## Summary

Replaced the fixed 50 ms polling loop in `crates/ragent-tui/src/lib.rs` with an
event-driven main loop. The TUI now sleeps until:
- a crossterm input event arrives,
- an event-bus message arrives,
- a shutdown signal arrives, or
- a computed deadline expires for animations/countdowns/status expiry.

This directly satisfies FR-001 (redraw only when needed) and FR-007 (no 20 Hz
idle wake).

## Changes Made

1. **Async crossterm event reader**
   - Spawned a long-lived `tokio::task::spawn_blocking` task that calls
     `crossterm::event::read()` and forwards events through an unbounded async
     channel (`ct_event_tx`/`ct_event_rx`).
   - Removed the old 50 ms timer branch and the `ct_event::poll(Duration::ZERO)`
     busy-read loop from `tokio::select!`.

2. **Dynamic deadline computation**
   - Added `compute_next_deadline(app, last_draw)` at module scope.
   - Default sleep is 60 seconds when idle.
   - Shortened to 1 s when a permission dialog is open (countdown updates).
   - Shortened to 250 ms for active spinners: model loading/download,
     code-index busy state, active benchmark, autopilot pending continue.
   - Wakes at exact deadlines for status expiry, run-cost banner expiry, and
     pending history flush.
   - Wakes every 5 s for code-index/memory stats and every 2 s for active swarm
     polls so their internal debounces fire.
   - Capped by `IDLE_REDRAW_INTERVAL_MS` (250 ms) so missed `needs_redraw`
     requests still render within one interval.

3. **Redraw correctness**
   - Log-record drain now sets `needs_redraw` when records were added.
   - `poll_pending_opt`, `poll_pending_bench`, `poll_swarm_unblock`,
     `finalize_swarm_completion`, and `poll_autopilot_continue` now set
     `needs_redraw` when they mutate visible state.
   - `set_status_info/success/warning/error/working` helpers set `needs_redraw`.
   - The existing `poll_status_expiry` and `poll_run_cost_banner_expiry`
     already set `needs_redraw`.

4. **Preserved behaviour**
   - `refresh_code_index_stats` and `refresh_memory_stats` are now called from
     the main work phase (they were previously inside the 50 ms branch).
   - Memory count atomic copy moved into the work phase.

## Build / Test

- `cargo fmt -p ragent-tui` passed.
- `cargo check -p ragent-tui` passed with no warnings.
- `cargo test -p ragent-tui --lib` passed: 83 tests OK.

## Note

Full `cargo test -p ragent-tui` (integration tests) has unrelated pre-existing
failures in `test_thinking_defaults.rs` (type mismatch) and
`test_memory_panel.rs` (missing `canonical_cache` field), the latter patched
locally for this run. These are not caused by T-002.
