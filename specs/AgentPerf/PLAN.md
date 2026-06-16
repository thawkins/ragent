# AgentPerf — Implementation Plan

## Architecture

The agent action loop is the orchestration function `SessionProcessor::process_user_message` in `crates/ragent-agent/src/session/processor.rs`.  Its current profiler scopes (visible in `processor.rs`) and the existing `SystemPromptCache` / `PromptContextCache` give us a clear map of the hot path.  This plan organises the work into six layers, executed in roughly the order listed:

1. **Measurement first** — ship a hermetic Criterion benchmark suite and a baseline report before any optimisation lands.  Without numbers, we cannot prove a change is a win.
2. **Per-scope caching** — push the existing caches (`SystemPromptCache`, `PromptContextCache`, message-history version) to be the default path inside the loop, not a best-effort fallback.
3. **Allocation / clone reduction** — replace `String::clone()` and per-step `Vec<ChatMessage>` allocations with `Arc<str>` / `Arc<Vec<...>>` and stack buffers for small deltas.
4. **Async-runtime hygiene** — move storage I/O off the async runtime via `spawn_blocking`; ban cross-`.await` `MutexGuard` holds; enforce per-step and per-stream budgets.
5. **Tool-call parallelism** — execute independent tool calls concurrently and feed results back in order.
6. **Observability & config** — `agent_perf` config block, `PerfScopeEvent` stream, `GET /perf/snapshot` HTTP endpoint, and a `/perf` TUI panel.

### File Layout

```
crates/ragent-bench/
├── benches/
│   └── agent_loop.rs                # NEW: hermetic Criterion suite
└── src/
    ├── mock_llm_client.rs           # NEW: deterministic MockLlmClient
    └── lib.rs                       # re-export mock client

crates/ragent-agent/src/
├── session/
│   ├── processor.rs                 # EDIT: use caches, spawn_blocking, parallel tools
│   ├── cache.rs                     # EDIT: extend SystemPromptCache with new fields
│   └── mod.rs                       # re-export PerfSnapshot
├── agent/
│   └── mod.rs                       # EDIT: PromptContextCache TTL + invalidation
├── event/
│   └── mod.rs                       # EDIT: add PerfScopeEvent
├── orchestrator/
│   └── (parallel-tool execution helper — possibly new module)
├── perf/
│   ├── mod.rs                       # NEW: agent_perf config + scope emitter
│   ├── snapshot.rs                  # NEW: PerfSnapshot type
│   └── config.rs                    # NEW: AgentPerfConfig
└── lib.rs                           # re-export agent_perf

crates/ragent-config/src/
└── config.rs                        # EDIT: add AgentPerfConfig to Config

crates/ragent-server/src/
├── routes/perf.rs                   # NEW: GET /perf/snapshot
└── lib.rs                           # wire new route

crates/ragent-tui/src/
├── app/state.rs                     # EDIT: subscribe to PerfScopeEvent
└── layout_perf.rs                   # NEW: /perf TUI panel

docs/reports/
├── agent_loop_perf_baseline.md      # NEW: pre-optimisation baseline
├── agent_loop_perf_after_t003.md    # NEW: post-cache numbers
├── agent_loop_perf_after_t007.md    # NEW: post-allocation numbers
├── agent_loop_perf_after_t011.md    # NEW: post-async-hygiene numbers
├── agent_loop_perf_after_t014.md    # NEW: post-tool-parallelism numbers
└── agent_loop_perf_final.md         # NEW: roll-up report
```

### Key Design Decisions

- **Measure first, optimise second.**  T-001 and T-002 are non-negotiable prerequisites for every subsequent task.  If a baseline number is not in the report, the corresponding optimisation has not been validated.
- **Caches are advisory, not authoritative.**  `SystemPromptCache` and `PromptContextCache` are best-effort: a cache miss must not break correctness.  We extend them with stronger invalidation (hash of inputs, version counter) rather than replacing the underlying computations.
- **No new runtime dependencies on the hot path.**  Pre-approved new deps are limited to `parking_lot`, `smallstr`, `arcstr`, and `bytes` (see FR-034).  Everything else must be justified and reviewed.
- **Backwards-compatible by design.**  The `SessionProcessor` API, the on-disk session format, and the `EventBus` event order are all invariant.  The work is purely internal to the loop.
- **Hermetic benchmarks.**  All benchmarks run against a `MockLlmClient` that produces a deterministic, pre-canned sequence of `StreamEvent`s.  No network.  No flake.  Same numbers on every run.

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Build a hermetic `MockLlmClient` for agent-loop benchmarks | FR-001, FR-005 | M | Critical | completed | — |
| T-002 | Author `agent_loop` Criterion bench suite (latency, allocations, tool throughput) | FR-001, FR-003 | M | Critical | pending | T-001 |
| T-003 | Capture and publish the pre-optimisation baseline report | FR-003 | S | Critical | pending | T-002 |
| T-004 | Wire `RAGENT_AGENT_PERF=1` env var to per-scope `info` logs | FR-002 | S | High | completed | — |
| T-005 | Make `SystemPromptCache` the default path in `process_user_message` | FR-008, FR-009 | M | Critical | completed | — |
| T-006 | Use `PromptContextCache` in the agent loop to skip AGENTS.md / README re-reads | FR-012 | S | High | completed | — |
| T-007 | Add message-history version cache to skip `history_to_chat_messages` re-runs | FR-006, FR-007 | M | Critical | completed | — |
| T-008 | Replace per-step `String::clone` of tool result content with `Arc<str>` | FR-013 | M | High | completed | — |
| T-009 | Hoist `ChatRequest` message vec to `Arc<Vec<ChatMessage>>` and share with cancel guard | FR-014 | S | High | completed | — |
| T-010 | Use stack / `SmallString` buffers for stream deltas; allocate only on flush | FR-015 | S | High | pending | T-007 |
| T-011 | Pre-compile all per-step regexes via `OnceLock`; remove per-call construction | FR-016 | S | Medium | completed | — |
| T-012 | Audit and remove all `storage_op` calls on the async runtime; ensure `spawn_blocking` | FR-010, FR-011 | M | Critical | completed | — |
| T-013 | Batch final-turn storage writes into a single `spawn_blocking` call | FR-011 | S | High | pending | T-012 |
| T-014 | Enforce per-step wall-clock budget and per-stream stall timeout in the loop | FR-017, FR-018, FR-020 | M | High | completed | — |
| T-015 | Replace any cross-`.await` `MutexGuard` with `parking_lot` / `tokio::sync` | FR-019 | M | High | completed | — |
| T-016 | Implement parallel independent tool calls via `futures::future::join_all` | FR-021, FR-022, FR-023 | L | High | pending | T-008, T-012 |
| T-017 | Skip per-iteration compression when `compressed_this_turn` is `true` | FR-024, FR-025 | S | Medium | completed | — |
| T-018 | Use reported `input_tokens` from `StreamEvent::Usage` for compression decisions | FR-026 | S | Medium | pending | T-017 |
| T-019 | Add `agent_perf` config block and load on startup | FR-027 | S | High | completed | — |
| T-020 | Emit `PerfScopeEvent` on the event bus when `agent_perf.profiling` is `true` | FR-028 | M | Medium | pending | T-019 |
| T-021 | Expose `GET /perf/snapshot` HTTP endpoint | FR-029 | S | Medium | pending | T-020 |
| T-022 | Add `/perf` TUI panel that subscribes to `PerfScopeEvent` | FR-002, FR-028 | M | Low | pending | T-020 |
| T-023 | Publish post-cache report (`agent_loop_perf_after_t003`-style) | FR-003 | S | High | pending | T-005, T-006, T-007, T-003 |
| T-024 | Publish post-allocation report | FR-003 | S | High | pending | T-008, T-009, T-010, T-011, T-023 |
| T-025 | Publish post-async-hygiene report | FR-003 | S | High | pending | T-012, T-013, T-014, T-015, T-024 |
| T-026 | Publish post-tool-parallelism report and final roll-up | FR-003 | S | High | pending | T-016, T-025 |
| T-027 | Update `SPEC.md` with agent-loop performance section | — | S | Medium | pending | T-026 |
| T-028 | Update `QUICKSTART.md` with `/perf` TUI command and `agent_perf` config | — | S | Low | pending | T-022, T-019 |
| T-029 | Add regression CI job: `cargo bench -p ragent-bench --bench agent_loop` and fail on >5% regression | FR-004 | M | Medium | pending | T-002, T-026 |
## Task Details

### T-001 — Build a Hermetic `MockLlmClient` for Agent-Loop Benchmarks (M, Critical)

- Create `crates/ragent-bench/src/mock_llm_client.rs`.
- Implement `MockLlmClient` that satisfies `ragent_llm::LlmClient` and yields a deterministic, pre-canned sequence of `StreamEvent`s.
- Pre-canned scenarios:
  1. `simple_text_reply` — emits a 50-token `TextDelta` stream and `Finish { reason: Stop }`.
  2. `single_tool_call` — emits a `ToolCallStart`/`Delta`/`End` for a `read` tool, then `Finish { reason: ToolUse }`.
  3. `multi_step_loop` — emits 3 sequential tool calls followed by a final text reply (8 `StreamEvent`s total).
  4. `large_history` — emits a `Finish` after a configurable delay, used to stress the history-loading path.
- All responses must be byte-identical between runs (use a fixed RNG seed and a `Cow<'static, str>` for the canned text).

### T-002 — Author `agent_loop` Criterion Bench Suite (M, Critical)

- Create `crates/ragent-bench/benches/agent_loop.rs`.
- Scenarios:
  1. `bench_simple_text_reply` — first-token latency, full-stream latency, allocations.
  2. `bench_single_tool_call` — step latency, allocations, `spawn_blocking` wait time.
  3. `bench_multi_step_loop` — total turn latency, allocations, tool-call throughput.
  4. `bench_large_history` — history-load latency, system-prompt construction latency.
  5. `bench_parallel_tool_calls` — measures end-to-end latency with 4 parallel tool calls.
- Use `criterion::BenchmarkGroup` with `Throughput::Elements` for tool calls and `Throughput::Bytes` for token streams.
- Configure `criterion` to emit `change` reports so `git diff` can be used to compare runs.

### T-003 — Capture and Publish the Pre-Optimisation Baseline Report (S, Critical)

- Run `cargo bench -p ragent-bench --bench agent_loop` on a reference machine (document the machine in the report).
- Save raw results to `docs/reports/agent_loop_perf_baseline.md` with: machine info, Rust version, commit SHA, date, all five scenarios' median / p99 / allocations.
- Also save Criterion's HTML report under `target/criterion/agent_loop/baseline/` so it can be re-opened later.

### T-004 — Wire `RAGENT_AGENT_PERF=1` Env Var to Per-Scope Logs (S, High)

- In `crates/ragent-agent/src/session/processor.rs`, gate the existing `tracing::info!` calls inside `profiler.scope(...)` blocks on a new `agent_perf::is_profiling_enabled()` helper.
- The helper reads `RAGENT_AGENT_PERF` first, then the `agent_perf.profiling` config field.
- Add 3 unit tests in `crates/ragent-agent/tests/test_agent_perf_env.rs` covering: env var set, config set, both unset, and a runtime toggle.

### T-005 — Make `SystemPromptCache` the Default Path (M, Critical)

- Audit `process_user_message` for the four system-prompt component calls (base prompt, tool reference, codeindex guidance, team guidance) and ensure each consults the cache before recomputing.
- Add a new `AgentInfo`-keyed cache entry for the per-agent base prompt so that subsequent turns for the same agent skip the template expansion.
- Add a unit test that asserts the second call within a session is a cache hit for each component.

### T-006 — Use `PromptContextCache` to Skip AGENTS.md / README Re-Reads (S, High)

- Verify that every call to `read_agents_md`, `read_readme`, and `git_context` in `process_user_message` goes through `PromptContextCache`.
- Add a TTL (default 60 s) to the cache so long-running sessions do not serve stale content forever; expose a `refresh_prompt_context()` helper for the `/refresh-context` TUI command.
- Add a benchmark that measures the cost of a cache hit vs a cache miss.

### T-007 — Add Message-History Version Cache (M, Critical)

- Compute a `history_version: u64` from `(message_count, last_message_id, last_modified_unix_ms)`; the loop's `storage_op` already returns the message list, so the version is a cheap derivative.
- Wrap `history_to_chat_messages(&history).await` in `if cached_version == current_version { return cached; }`.
- Add a `Bench-Compare` scenario to the agent_loop bench that exercises the cache hit path.

### T-008 — Replace `String::clone` of Tool Result Content with `Arc<str>` (M, High)

- Audit all call sites of `tool_result_content_for_llm` in `processor.rs`.
- Change the function signature to return `Arc<str>` and update the truncation logic to operate on `&str` slices.
- Add a `cargo clippy::pedantic` check that fails on any new `String::clone()` of tool result content.

### T-009 — Hoist `ChatRequest` Message Vec to `Arc<Vec<ChatMessage>>` (S, High)

- Introduce a `SharedChatRequest` newtype that owns the `Arc<Vec<ChatMessage>>` and the `Arc<HashMap<String, ToolDefinition>>`.
- Update `build_chat_request` to return `SharedChatRequest`; update the cancel guard to hold a `Weak<SharedChatRequest>`.

### T-010 — Use Stack / `SmallString` Buffers for Stream Deltas (S, High)

- Replace the `String` accumulators in the stream consumer with `smallstr::SmallString<[u8; 256]>`.
- Only allocate a `String` when the buffer is flushed to the event bus (i.e. when `StreamBuffer` calls `drain_text` / `drain_reasoning`).
- Add a benchmark that measures the allocation count before and after the change.

### T-011 — Pre-Compile All Per-Step Regexes via `OnceLock` (S, Medium)

- Survey `processor.rs` for any `Regex::new(...)` calls that are not already wrapped in `OnceLock`.
- Move them to module-level `OnceLock<Regex>` statics.
- Add a unit test that asserts the regex is created exactly once across 100 invocations.

### T-012 — Audit and Remove All `storage_op` Calls on the Async Runtime (M, Critical)

- Walk every `self.storage_op(...)` call site in `process_user_message` and verify the closure is `Send + 'static` and does not hold an `await` point internally.
- For any site that does, refactor to use a single `spawn_blocking` call with a struct of inputs.
- Add a test that times `process_user_message` end-to-end on a session with 1000 messages and asserts no individual storage write takes more than 5 ms on the async executor.

### T-013 — Batch Final-Turn Storage Writes (S, High)

- Collect all "update_message" / "create_message" operations for the final turn into a `Vec<StorageOp>` and dispatch them as a single `spawn_blocking` call.
- This is a refactor: the on-disk ordering of writes MUST be preserved.

### T-014 — Enforce Per-Step Wall-Clock Budget and Per-Stream Stall Timeout (M, High)

- Wrap each iteration of the step loop in `tokio::time::timeout(agent_perf.step_budget_secs, ...)`.
- Add a `last_delta_at: Instant` to the stream consumer; when `Instant::now() - last_delta_at > stall_timeout_secs`, emit `Event::StallDetected` and trigger the existing stall-recovery path.
- The stall timeout is checked inside the same `tokio::select!` as cancellation so the loop remains responsive.

### T-015 — Replace Cross-`.await` `MutexGuard` with `parking_lot` / `tokio::sync` (M, High)

- Grep for `.lock().await` and any `let _g = mutex.lock(); ...; something.await;` patterns in `crates/ragent-agent/src/session/`.
- Convert short critical sections to `parking_lot::Mutex`; convert long-held (or `await`-crossing) guards to `tokio::sync::RwLock` or `tokio::sync::Mutex`.
- Add a clippy lint (or `#[deny]` attribute) banning `std::sync::Mutex` in `session/`.

### T-016 — Implement Parallel Independent Tool Calls (L, High)

- Identify the call site where tool calls are dispatched from the assistant message.
- For each tool call, determine if it depends on a previous tool call's output (read after write, or shared file path).  If independent, execute it in parallel.
- Use `futures::future::join_all` to await the parallel set, then write the results back in the original order.
- Respect `max_concurrent_tools` via a `tokio::sync::Semaphore`.

### T-017 — Skip Per-Iteration Compression When `compressed_this_turn` is `true` (S, Medium)

- Verify the existing hysteresis logic in `processor.rs` (the `compressed_this_turn` flag) is in fact taking the fast path; add a tracing event when it does.
- Add a benchmark that measures the cost of a 2-step turn with and without the fast path.

### T-018 — Use Reported `input_tokens` from `StreamEvent::Usage` for Compression Decisions (S, Medium)

- The loop already captures `last_reported_input_tokens` from the most recent `StreamEvent::Usage`.  Wire it into `should_compress_with_reported` so that compression decisions are made on the provider's number, not the local estimate.
- Add a unit test with a mock stream that emits a `Usage` event with a known token count, and assert the compression decision uses it.

### T-019 — Add `agent_perf` Config Block (S, High)

- Add `AgentPerfConfig` to `ragent_config::Config` with the fields described in FR-027.
- Validate the fields (e.g. `max_concurrent_tools >= 1`, `step_budget_secs >= 5`).
- Re-export the config from `crates/ragent-agent/src/perf/config.rs`.

### T-020 — Emit `PerfScopeEvent` on the Event Bus (M, Medium)

- Add `Event::PerfScope { scope: String, duration_us: u64 }` to `ragent-types`.
- In `session::profiler`, when `agent_perf.profiling` is `true`, publish a `PerfScopeEvent` for every scope.
- Add a benchmark that asserts publishing a `PerfScopeEvent` is cheaper than a `tracing::info!` call (the bus path is in-process and lock-free).

### T-021 — Expose `GET /perf/snapshot` HTTP Endpoint (S, Medium)

- Create `crates/ragent-server/src/routes/perf.rs`.
- Return the latest `AgentLoopProfiler::snapshot()` as JSON.
- Document the endpoint in `PROVIDERS.md` or a new `docs/perf-endpoint.md`.

### T-022 — Add `/perf` TUI Panel (M, Low)

- Subscribe to `PerfScopeEvent` in the TUI app state.
- Add a new `DialogType::PerfPanel` that shows a sortable table of per-scope `count`, `total_ms`, `avg_ms`, `max_ms`, `last_ms`.
- Bind the `/perf` slash command to open the panel.

### T-023 — Publish Post-Cache Report (S, High)

- After T-005, T-006, T-007 are merged, re-run the bench suite and publish `docs/reports/agent_loop_perf_after_t003.md` (a typo in the original filename; the real name reflects that T-005/6/7 are the cache tasks).
- Compare against the baseline; assert that no scenario regressed by more than 5%.

### T-024 — Publish Post-Allocation Report (S, High)

- After T-008, T-009, T-010, T-011, publish `docs/reports/agent_loop_perf_after_t007.md`.
- Highlight allocation-count deltas (criterion's `AllocCount` is the source).

### T-025 — Publish Post-Async-Hygiene Report (S, High)

- After T-012, T-013, T-014, T-015, publish `docs/reports/agent_loop_perf_after_t011.md`.
- Highlight p99 latency reductions and any new tail-latency outliers.

### T-026 — Publish Post-Tool-Parallelism Report and Final Roll-Up (S, High)

- After T-016, publish `docs/reports/agent_loop_perf_after_t014.md` and `docs/reports/agent_loop_perf_final.md`.
- The final report must include: before/after numbers for every scenario, a summary table, and a list of trade-offs made along the way (e.g. memory cost of the new caches).

### T-027 — Update `SPEC.md` with Agent-Loop Performance Section (S, Medium)

- Add a new section to `SPEC.md` documenting the agent-loop performance subsystem, the `agent_perf` config block, and the `/perf` TUI command.
- Link to the final perf report.

### T-028 — Update `QUICKSTART.md` with `/perf` and `agent_perf` (S, Low)

- Add a short paragraph and a config snippet to the Quick Start.
- Mention the new TUI command and the HTTP endpoint.

### T-029 — Add Regression CI Job (M, Medium)

- Add a `perf-check` job to `.github/workflows/` (or the existing CI) that runs `cargo bench -p ragent-bench --bench agent_loop -- --bench` against a stored baseline and fails on >5% regression.
- Use `criterion`'s built-in baseline comparison (`--save-baseline` / `--baseline`) so the job is self-contained.
- Document the workflow in `docs/reports/agent_loop_perf_ci.md`.

## Estimated Effort

| Phase | Tasks | Total Effort |
|---|---|---|
| Measurement (T-001 → T-004) | M + M + S + S | ~4 days |
| Caching (T-005 → T-007, T-017, T-018) | M + S + M + S + S | ~4 days |
| Allocation reduction (T-008 → T-011) | M + S + S + S | ~3 days |
| Async hygiene (T-012 → T-015) | M + S + M + M | ~5 days |
| Tool parallelism (T-016) | L | ~4 days |
| Observability & config (T-019 → T-022) | S + M + S + M | ~4 days |
| Reporting (T-023 → T-026) | S + S + S + S | ~2 days |
| Docs & CI (T-027 → T-029) | S + S + M | ~3 days |
| **Total** | | **~29 days** |

## Risks

- **Cache invalidation bugs.**  Stronger caching means more opportunities to serve stale content.  Mitigation: the `PromptContextCache` TTL (T-006), the cache-version counter (T-005), and a `/refresh-context` command for emergency invalidation.
- **Parallel tool calls changing observable ordering.**  Even though results are written back in order, the wall-clock ordering of "started" events will differ.  Mitigation: T-016 documents this; the `EventBus` event order is preserved (FR-032), only the wall-clock interleaving changes.
- **Benchmark flakiness on shared CI machines.**  Criterion mitigates this with warmups and statistical analysis, but a >5% regression threshold is tight.  Mitigation: T-029 uses `--bench` mode (single iteration) and a fixed clock; the threshold can be tuned up if CI is noisy.
- **`spawn_blocking` overhead at low load.**  When the storage is fast, the `spawn_blocking` indirection is pure overhead.  Mitigation: T-012 keeps small in-line writes (e.g. the user-message save) on the async path and only moves large operations off it.
- **Profile-guided regression.**  Optimising one scope can starve another.  Mitigation: every optimisation task must update its own report, and the final report (T-026) explicitly lists any regressions.