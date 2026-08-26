# RESOURCEPLAN.md — CPU Creep & Resource Leak Remediation Plan

## Problem Statement

`ragent` exhibits steadily increasing CPU utilisation as more agents run, even with
no visible activity. Over a long session, CPU creeps toward 100% and the entire
system lags. Exiting and restarting `ragent` clears the condition, confirming this
is a **resource accumulation problem** (leaked tokio tasks, unbounded data
structures, busy-poll loops, and missing `Drop` cleanup) rather than a single hot
path.

A parallel investigation across all 17 crates (5 explore agents + direct source
verification) identified **four root-cause categories** that combine to produce the
observed behaviour. This document ranks them by impact and defines a concrete
remediation plan.

---

## Root-Cause Categories

### A. Leaked / orphaned tokio tasks (CPU creep — primary cause)

Spawned `tokio::spawn` tasks whose `JoinHandle` is dropped and which are never
aborted. They continue running indefinitely, each contributing wakeups and CPU
time. The single most damaging instance is the LLM "waiting" notice task.

### B. Busy-poll loops (sustained low-grade CPU spin)

Several background loops wake on a fixed timer even when idle, doing filesystem
I/O or channel polling every 50–500 ms for the entire process lifetime.

### C. Unbounded memory growth (RSS creep, GC pressure)

Maps and Vecs that are never evicted. As memory grows, the allocator and the
tokio runtime spend increasing time on allocation/deallocation and cache misses,
contributing to the "system lag" sensation.

### D. Missing `Drop` / Arc cycles (resources never released)

`AgentManager`, `TeamManager`, and `SessionManager` have no (or no-op) `Drop`
implementations. A `TeamManager`↔`SessionProcessor` `Arc` cycle prevents either
from ever being dropped. MCP child processes and browser processes are not
killed on drop, leaking OS processes.

---

## Findings — Ranked by Impact

### CRITICAL

| ID | Category | File | Pattern |
|----|----------|------|---------|
| R-1 | A | `crates/ragent-agent/src/session/loop_steps.rs:908–934` | **LLM "waiting" notice task never aborted on API error/retry.** `notice_handle` is spawned at line 908 but only `.abort()`ed at line 1049 *after* the stream is successfully created. Every error path (`continue 'retry` at line 1027, `bail!` at lines 1018 and 1043) leaves the task running. Since `first_event_arrived` is never set on failure, the task loops forever publishing `AgentNotice` every 30 s. **N retry failures = N zombie notice tasks.** This is the smoking gun for cumulative CPU creep under any provider instability. |
| R-2 | C | `crates/ragent-agent/src/task/mod.rs:263` | **`AgentManager.tasks` DashMap never removes completed entries.** `drain_completed` marks `reported = true` but never calls `remove`/`retain`. Every sub-agent task ever spawned stays in memory for the process lifetime, each holding its full `result: Arc<str>` (up to 32 KB) and prompt. |
| R-3 | C | `crates/ragent-agent/src/session/mod.rs:126–141` | **Global `session_state_cache` static never pruned.** A `OnceLock<Mutex<HashMap<String, Arc<Mutex<SessionState>>>>` accumulates one entry per session ID (including every sub-agent child session) permanently. Each holds a `Vec<ChatMessage>` that can be hundreds of KB. |
| R-4 | D | `crates/ragent-agent/src/task/mod.rs` (no `Drop`) | **`AgentManager` has no `Drop` impl.** Detached sub-agent tasks hold `Arc<SessionProcessor>` (and its storage, caches, event bus) indefinitely. A stalled sub-agent pins the entire processor. |
| R-5 | D | `crates/ragent-agent/src/team/manager.rs` (no `Drop`) | **`TeamManager` has no `Drop` impl.** Team mailbox poll loops (500 ms disk-polling) and teammate agent loops are never forcibly stopped when a team is abandoned without `team_cleanup`. Orphaned poll loops accumulate, each doing filesystem I/O twice per second forever. |
| R-6 | D | `TeamManager` ↔ `SessionProcessor` | **`Arc` cycle.** `TeamManager` holds `Arc<SessionProcessor>` and `SessionProcessor` (via tools) holds back-references that keep the `TeamManager` alive. Neither can ever be dropped, so all team resources leak for the process lifetime. |

### HIGH

| ID | Category | File | Pattern |
|----|----------|------|---------|
| R-7 | B | `crates/ragent-codeindex/src/worker.rs:242–319` | **CodeIndex worker thread busy-spins every 50 ms forever.** `try_recv()` + `std::thread::sleep(50ms)` even when completely idle — 20 wakeups/second for the entire process lifetime when the index is active. |
| R-8 | B | `crates/ragent-tools-core/src/askpass.rs:168–217` | **AskPass `watch_loop` polls directory every 100 ms indefinitely.** 10 wakeups/second doing `read_dir` for the entire duration of any sudo-capable bash command. |
| R-9 | B | `crates/ragent-tools-core/src/bg.rs:381–418` | **`BackgroundCommand` `waiter_task` polls `try_wait()` every 100 ms.** 10 wakeups/second per background shell command for the command's entire lifetime (a 30-min build = 18,000 wakeups). |
| R-10 | C | `crates/ragent-tui/src/app/state.rs:1139` | **TUI `messages: Vec<Message>` unbounded.** Only trimmed by `/clear`. Long sessions accumulate every message with all `MessagePart`s (tool results up to 12 KB each). |
| R-11 | C | `crates/ragent-tui/src/app/state.rs:1278` | **TUI `log_entries: Vec<LogEntry>` unbounded.** Never trimmed; `log_line_cache` mirrors it, doubling memory. |
| R-12 | C | `crates/ragent-agent/src/background/mod.rs:94` | **`drained_ids: HashSet<String>` never cleared.** Each background shell command adds a permanent string ID. |
| R-13 | C | `crates/ragent-agent/src/session/processor.rs:190` | **`read_timestamps: HashMap<PathBuf, u64>` never pruned.** A code-reading session touching thousands of files accumulates thousands of entries. |
| R-14 | A | `crates/ragent-agent/src/orchestrator/registry.rs:91–98` | **Orchestrator mailbox loop — handle dropped, runs forever.** `while let Some(req) = rx.recv().await` with the `JoinHandle` dropped. Re-registration spawns another infinite loop; old ones never exit. |
| R-15 | D | `crates/ragent-agent/src/llm/mcp/` (MCP client) | **MCP stdio child processes leak on drop.** No `Drop` impl, no `kill_on_drop`, and `Arc::into_inner` fails when shared. Child processes persist after the MCP client is discarded. |
| R-16 | D | `crates/ragent-tools-extended/src/browser/launch.rs:253–278` | **Browser `Child` handle dropped without `detach()` or `kill_on_drop`.** Repeated `browser setup` spawns zombie Chrome processes (~200–500 MB RSS each). |

### MEDIUM

| ID | Category | File | Pattern |
|----|----------|------|---------|
| R-17 | B | `crates/ragent-tools-core/src/bg.rs:206–227` | `cancel()` polls `is_done()` every 50 ms for 10 s (200 wakeups). Use `Notify` instead. |
| R-18 | B | `crates/ragent-agent/src/team/manager.rs:995` | Team mailbox poll loop per teammate (500 ms disk poll). Handle dropped. |
| R-19 | A | `crates/ragent-agent/src/team/manager.rs:856` | Teammate agent loop `JoinHandle` not stored — cannot be force-aborted. |
| R-20 | A | `crates/ragent-agent/src/orchestrator/coordinator.rs:392–477` | `start_job_async` handle dropped; panics leave jobs stuck "running". |
| R-21 | C | `crates/ragent-agent/src/session/processor.rs:169` | `llm_client_cache` HashMap never pruned (each holds a `reqwest::Client` connection pool). |
| R-22 | C | `crates/ragent-tui/src/app/state.rs:1181` | `llm_request_stats: Vec<LlmRequestStat>` unbounded. |
| R-23 | C | `crates/ragent-storage/src/storage.rs:360` | SQLite WAL — no explicit `wal_autocheckpoint` or `wal_checkpoint(TRUNCATE)` on shutdown. WAL file can grow to hundreds of MB. |
| R-24 | C | `crates/ragent-agent/src/team/manager.rs:988` | `cancel_flags` not removed on normal teammate completion. |

### LOW / DEFERRED

| ID | Category | File | Pattern |
|----|----------|------|---------|
| R-25 | C | `crates/ragent-agent/src/agent/mod.rs:57` | `PROMPT_CONTEXT_CACHE` — all-or-nothing `clear()`, no per-entry eviction. Bounded by working dir count (low cardinality). |
| R-26 | C | `crates/ragent-agent/src/reference/fuzzy.rs:30` | `PROJECT_FILE_CACHE` — never individually evicted. Bounded by working dir count. |
| R-27 | C | `crates/ragent-agent/src/session/processor.rs:209` | `skill_body_cache` — never pruned. Bounded by skill count (<20). |
| R-28 | C | `crates/ragent-agent/src/session/processor.rs:125` | `team_context_cache` — TTL comment but no eviction. Bounded by session count in teams. |
| R-29 | — | `crates/ragent-agent/src/session/mod.rs:331–343` | `SessionManager::drop` is a no-op. Safe once R-4/R-6 are fixed. |

### VERIFIED SAFE (no action)

| Item | Location | Why |
|------|----------|-----|
| LRU file-read cache | `processor.rs` | Correctly bounded at 256 entries. |
| TUI `md_render_cache` | `tui/state.rs` | Correctly bounded at 256 entries. |
| CodeIndex `tree_cache` | `codeindex/tree_cache.rs` | Correctly bounded, configurable. |
| `std::thread::spawn` in `reference/resolve.rs` | `resolve.rs:184` | Joined via `JoinHandle::join`. |
| `spawn_blocking` (~50 sites) | All crates | Bounded by tokio blocking pool. |
| `AbortOnDrop` guard | `processor.rs:580` | Correct pattern — adopt elsewhere. |

---

## Remediation Plan

Each phase is independently shippable. Phase 1 addresses the primary CPU-creep
cause; later phases address memory and structural leaks.

### Phase 1 — Stop the CPU bleed (leaked tasks + busy-poll loops)

**Goal:** eliminate the tasks and loops that accumulate wakeups over a session.

- [ ] **R-1** — Abort `notice_handle` on all early-return / error / retry paths in
  `loop_steps.rs`. Set `first_event_arrived` and call `notice_handle.abort()`
  before every `continue 'retry` and `bail!`. Wrap the handle in the existing
  `AbortOnDrop` guard (`processor.rs:580`) so it is aborted on scope exit
  regardless of path. *(verify: stress-test with a provider that returns 429s on
  every call; confirm no `AgentNotice` events publish after the turn ends.)*

- [ ] **R-7** — Replace `std::thread::sleep(50ms)` in `codeindex/worker.rs:318`
  with `event_rx.recv_timeout(Duration::from_millis(50))` so the thread blocks on
  the channel and wakes only on events or timeout. Remove the separate
  `try_recv` + `sleep` pattern. *(verify: idle CPU drops to ~0 with codeindex
  enabled.)*

- [ ] **R-8** — Increase `askpass.rs` `POLL_INTERVAL` from 100 ms to 500 ms–1 s,
  or replace the `read_dir` poll with a `notify::Watcher` on the request
  directory so the loop blocks until the filesystem changes. *(verify: `sudo`
  prompt path shows negligible CPU.)*

- [ ] **R-9** — Replace the `try_wait()` poll loop in `bg.rs:381–418` with
  `child.wait().await` (async-aware). Use `tokio::select!` with the cancel signal.
  *(verify: a long-running background command shows zero polling wakeups.)*

- [ ] **R-17** — Replace the 50 ms `is_done()` poll in `bg.rs:206–227` with a
  `tokio::sync::Notify` signalled by `waiter_task` on exit. *(verify: cancel of
  a running command returns promptly without 200 wakeups.)*

### Phase 2 — Bound the memory growth

**Goal:** stop RSS from growing without limit across a long session.

- [ ] **R-2** — In `AgentManager::drain_completed` (`task/mod.rs:827`), after
  collecting reported entries, `retain` entries that are reported, not running,
  and have `waiter_count == 0`. Add a `reap_reported()` helper for sync tasks.
  *(verify: spawn 50 sub-agents, confirm `tasks.len()` returns toward 0.)*

- [ ] **R-3** — Add `remove_session_state(session_id)` to `SessionManager` and
  call it when a sub-agent task completes (pairs with R-2) and on
  `archive_session`. *(verify: session_state_cache map size stays bounded.)*

- [ ] **R-10** — Add a configurable `MAX_TUI_MESSAGES` cap (e.g. 500) with FIFO
  eviction in the TUI message push path. Also trims `message_line_cache`.
  *(verify: long session RSS plateaus.)*

- [ ] **R-11** — Add a `MAX_LOG_ENTRIES` cap (e.g. 1000) with FIFO eviction in
  `add_log_entry`. Also trims `log_line_cache`. *(verify: log panel memory
  plateaus.)*

- [ ] **R-12** — Clear `drained_ids` entries alongside the existing
  `tasks`/`sessions` cleanup in `background/mod.rs`. *(verify: set size stable.)*

- [ ] **R-13** — Cap `read_timestamps` at e.g. 2000 entries (random eviction or
  clear on new user message). *(verify: map size bounded after reading many
  files.)*

- [ ] **R-21** — Bound `llm_client_cache` to 4–8 entries with simple LRU.
  *(verify: cycling through 20 models does not accumulate 20 clients.)*

- [ ] **R-22** — Cap `llm_request_stats` to a rolling window (e.g. 1000).
  *(verify: vec length bounded.)*

- [ ] **R-23** — Add `PRAGMA wal_autocheckpoint = 500` and call
  `PRAGMA wal_checkpoint(TRUNCATE)` on graceful shutdown. *(verify: WAL file
  stays small.)*

### Phase 3 — Add `Drop` cleanup and break the Arc cycle

**Goal:** ensure all spawned work is aborted and OS resources are released when
their owners are dropped.

- [ ] **R-4** — Add `impl Drop for AgentManager` that sets all `cancel_flags` to
  `true` and aborts stored `JoinHandle`s. Store the outer `JoinHandle` from
  `spawn_background` in `TaskEntry` (new field `handle: Option<JoinHandle<()>>`)
  so `Drop` can abort it. Adopt the `AbortOnDrop` wrapper from `processor.rs:580`.
  *(verify: dropping AgentManager cancels all in-flight sub-agents; no leaked
  Arc<SessionProcessor>.)*

- [ ] **R-6** — Break the `TeamManager`↔`SessionProcessor` `Arc` cycle by holding
  a `Weak<SessionProcessor>` on one side. *(verify: `Arc::strong_count` drops to 0
  after team teardown.)*

- [ ] **R-5** — Add `impl Drop for TeamManager` that sets every `poll_cancel` and
  `cancel` in `handles`, deregisters notifiers, and aborts stored poll/agent
  `JoinHandle`s. Store both handles in `TeammateHandle`. *(verify: abandoned
  team's poll loops stop; no filesystem polling after drop.)*

- [ ] **R-14** — Store the orchestrator mailbox-loop `JoinHandle` in `AgentEntry`
  and abort it in `unregister`. *(verify: re-registration does not accumulate
  loops.)*

- [ ] **R-15** — Add `impl Drop` for the MCP client that kills child processes
  (`kill_on_drop` or explicit `start_kill` + `wait`). Handle the `Arc` shared
  case with `Weak` or an explicit `shutdown()` method. *(verify: no orphaned
  MCP server processes after session end.)*

- [ ] **R-16** — Call `child.detach()` before dropping the browser `Child` handle
  in `launch.rs`, or store the handle and kill it on `BrowserTool` drop. Prefer
  the latter for deterministic cleanup. *(verify: no zombie Chrome processes after
  browser tool use.)*

### Phase 4 — Teammate and orchestrator task handles

**Goal:** make all fire-and-forget spawns abortable on teardown.

- [ ] **R-18** — Store the team mailbox poll-loop `JoinHandle` in `TeammateHandle`
  and abort it in `shutdown_teammate` / `Drop`.
- [ ] **R-19** — Store the teammate agent-loop `JoinHandle` in `TeammateHandle`.
- [ ] **R-20** — Store the `start_job_async` `JoinHandle` in `JobEntry`; abort on
  shutdown; wrap body to guarantee metrics are updated on panic.

### Phase 5 — Lower-priority caches and verification

- [ ] **R-24** — Remove `cancel_flags` entries on normal teammate completion.
- [ ] **R-25–R-28** — Add periodic sweeps or per-entry eviction to the low-
  cardinality caches if long-running server mode becomes a supported use case.
- [ ] **R-29** — Once R-4/R-6 are fixed, `SessionManager::drop` is safe; add a WAL
  flush there if desired.
- [ ] **Cross-cutting** — Audit all remaining `tokio::spawn` call sites
  (~60 total) and classify each as: (a) must-join, (b) must-abort-on-drop, or
  (c) intentional fire-and-forget. Wrap (b) in `AbortOnDrop`. Add a clippy lint or
  code review checklist to prevent new detached spawns without explicit lifetime
  management.

---

## Verification Strategy

1. **Reproduction harness** — a test script that spawns N sub-agents in sequence,
   each making a failing LLM call (mock provider returning 429), then measures CPU
   and RSS before and after. Before the fix, CPU and RSS rise with N; after, both
   stay flat.

2. **Long-session soak test** — run a 60-minute TUI session with periodic
   sub-agent spawns, team create/destroy cycles, and codeindex enabled. Measure
   RSS and CPU every 5 minutes. Acceptance: RSS plateaus, CPU stays <5% when idle.

3. **Per-finding unit tests** — each Phase 1–3 fix gets a focused test:
   - R-1: assert no `AgentNotice` events after a failed `chat()` returns.
   - R-2: assert `tasks.len()` returns toward 0 after sub-agents complete.
   - R-7: assert codeindex worker thread sleeps when idle (no 20 Hz wakeups).
   - R-4/R-5: assert `Arc::strong_count` of the processor drops to 1 after
     `AgentManager`/`TeamManager` drop.

4. **Process-level check** — `ps`/`htop` confirms no orphaned child processes
   (MCP servers, Chrome) remain after `ragent` exits.

---

## Why Restarting Clears It

Exiting `ragent` tears down the tokio runtime (cancelling all detached tasks)
and the process (releasing all memory and OS handles). On restart, the empty
process begins clean. This confirms every finding above is an **in-process
accumulation** problem — nothing is fundamentally broken in a single request;
the leaks compound over the session lifetime.

---

## Appendix — Investigation Method

Five parallel `explore` sub-agents investigated: (1) background tasks &
sub-agent spawning, (2) file watchers, timers & event bus, (3) streaming, SSE &
HTTP server, (4) memory accumulation & unbounded data structures, (5) MCP client,
sub-agent runtime, teams & `Arc` cycles. Full findings are in
`log/subagents/wait-batch-1787708992977.md`. Key source locations were
re-verified by direct read of `loop_steps.rs`, `task/mod.rs`, `session/mod.rs`,
and `codeindex/worker.rs`.