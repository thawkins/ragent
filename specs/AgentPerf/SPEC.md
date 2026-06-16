---
status: implemented
audit:
  - { time: 1781616896, from: "none", to: "draft", actor: "system" }
  - { time: 1781798400, from: "draft", to: "in_progress", actor: "system" }
  - { time: 1781798401, from: "in_progress", to: "implemented", actor: "system" }
---
# AgentPerf — Agent Action Loop Performance Review & Optimisation

## Overview

The ragent agent action loop — implemented in `crates/ragent-agent/src/session/processor.rs` and orchestrated by `SessionProcessor::process_user_message` — is the central, latency-sensitive hot path of the entire system.  A single user turn can spend **seconds to tens of seconds** inside this loop, broken down across: history loading, context-window compression, system-prompt construction, tool-reference formatting, LLM streaming, tool execution, and storage I/O.  Many sub-steps in this pipeline are observed to do redundant work, to allocate aggressively, or to block the async runtime on synchronous I/O that has a faster async equivalent.

This specification audits the current agent loop, ranks the highest-leverage optimisation opportunities, and defines a concrete set of requirements to **measure, reduce, and stabilise** per-turn latency and CPU usage.  The work is additive: existing functionality (streaming, tool calls, permissions, memory, compression, agents) MUST keep working unchanged — the goal is to make the same path faster, not to change its behaviour.

The specification is the result of a survey of the loop's profiler scopes (see `crates/ragent-agent/src/session/profiler.rs` and `processor.rs`) and the existing per-component caches (`session::cache::SystemPromptCache`, `ReadToolCache`, etc.).  It does not introduce new architectural concepts; it tightens the existing pipeline.

## Goals

1. **Reduce time-to-first-token (TTFT)** for the average user turn on representative models (claude-sonnet-4, gpt-4o, qwen3-8b).
2. **Reduce per-step overhead** inside the tool-call loop so more tool calls can happen per second of wall time.
3. **Reduce allocations and clones** on the hot path; favour `Cow<'_, str>`, `Arc<str>`, and small-string optimisation over `String::clone()`.
4. **Eliminate blocking I/O on the async runtime** (storage writes, file reads, JSON parsing) by moving them to `tokio::task::spawn_blocking` or by caching the results.
5. **Ship a reproducible benchmark suite** that measures agent-loop latency, allocations, and tool-call throughput, and gates future regressions.

## Non-Goals

- New features (model routing, planning modes, additional tools).  This work is a pure performance pass.
- Replacing the underlying provider crates (Anthropic, OpenAI, Foundry, etc.).
- Rewriting the LLM client streaming logic — the existing `StreamBuffer` in `processor.rs` is already a good optimisation and is preserved.
- Changing the public API of `SessionProcessor` or the on-disk session format.

## Requirements

### Profiling, Measurement, and Benchmarking

**FR-001** (Ubiquitous) The system shall provide a reproducible Criterion benchmark suite under `crates/ragent-bench/benches/` that measures agent-loop latency, allocations, and tool-call throughput against a fixed mock `LlmClient`.

**FR-002** (Event-driven) When the `RAGENT_AGENT_PERF=1` environment variable is set, the system shall enable detailed per-scope timing logs (the existing `AgentLoopProfiler` scopes) at `info` level for every `process_user_message` call.

**FR-003** (Ubiquitous) The system shall publish a `docs/reports/agent_loop_perf_baseline.md` report that captures, on a reference machine, baseline numbers for the benchmark suite, including: median first-token latency, median step latency, allocations per step, and tool-call throughput.  Subsequent optimisation tasks (T-003 onward) MUST be evaluated against this baseline.

**FR-004** (Optional) Where the user runs `cargo bench -p ragent-bench --bench agent_loop` on a workstation with a stable clock, the system shall emit a comparison report in `target/criterion/agent_loop/change/` showing per-scenario deltas.

**FR-005** (Unwanted) The system shall not introduce a benchmark that runs against a live network LLM provider; all agent-loop benchmarks MUST be hermetic and use a `MockLlmClient` returning deterministic, pre-canned `StreamEvent` sequences.

### Per-Scope Hot-Path Optimisation

**FR-006** (Ubiquitous) The system shall cache the result of `history_to_chat_messages(&history)` per (session_id, history_version) so that the LLM call is not re-prepared on every iteration of the tool-call loop when the history has not changed.

**FR-007** (Event-driven) When the agent loop completes a step that did not mutate the conversation history (e.g. a no-op tool call), the system shall reuse the previous `ChatRequest`'s serialised form, skipping the JSON serialisation step in `build_chat_request`.

**FR-008** (Ubiquitous) The system shall construct the system prompt using `SystemPromptCache` (`session::cache::SystemPromptCache`) so that the static components (base prompt, tool reference, codeindex guidance, team guidance) are computed at most once per `process_user_message` call, not once per step.

**FR-009** (State-driven) While the tool registry, codeindex state, and team context hashes are unchanged, the system shall reuse the previously computed `tool_reference`, `codeindex_guidance`, and `team_guidance` strings without re-iterating the `ToolRegistry`.

**FR-010** (Ubiquitous) The system shall move storage I/O (message creation, message update, history load) off the async runtime by using `tokio::task::spawn_blocking` via the existing `storage_op` helper, so the executor is never blocked on SQLite writes.

**FR-011** (Event-driven) When the agent loop ends (normal stop, error, cancel, max-steps), the system shall batch the final storage writes (assistant-message update, tool-result messages) into a single `spawn_blocking` call.

**FR-012** (Ubiquitous) The system shall avoid re-parsing the `AGENTS.md`, `README.md`, and git-context files for every user turn; the `PromptContextCache` (`agent::PromptContextCache`) is the single source of truth, and the agent loop MUST consult it before touching the filesystem.

### Allocation & Clone Reduction

**FR-013** (Ubiquitous) The system shall avoid `String::clone()` of tool result content on the hot path; tool result content is passed as `&str` slices where possible and as `Arc<str>` when crossing thread or task boundaries.

**FR-014** (Ubiquitous) The system shall replace per-step `Vec<ChatMessage>` allocation with an `Arc<Vec<ChatMessage>>` that is shared between the `ChatRequest` and the cancellation guard.

**FR-015** (Event-driven) When a `StreamEvent::TextDelta` or `StreamEvent::ReasoningDelta` is received, the system shall buffer the delta into a stack-allocated `[u8; 512]` or `SmallString` buffer, and only allocate a `String` when the delta is flushed to the event bus.

**FR-016** (Ubiquitous) The system shall replace `regex::Regex` constructed per call with pre-compiled `OnceLock<Regex>` / `OnceLock<RegexSet>` patterns (the existing `STALL_PATTERN_SET` in `processor.rs` is the template for this pattern).

### Cancellation, Backpressure, and Stall Detection

**FR-017** (Event-driven) When the user cancels the agent loop via `cancel_flag`, the system shall abort the current LLM stream within 100 ms and return control to the TUI without performing any further `storage_op` writes beyond the partial-message save.

**FR-018** (State-driven) While an LLM stream is being consumed, the system shall enforce a per-step wall-clock budget (default 300 s) and a per-stream "no delta" stall timeout (default 60 s); when either is exceeded, the system shall emit a stall event and trigger the existing stall-recovery path.

**FR-019** (Ubiquitous) The system shall not hold a `MutexGuard` across an `.await` point on the agent hot path; the `SystemPromptCache` `Mutex` is acceptable for short critical sections, but session-level state MUST be `parking_lot::Mutex` or a `tokio::sync::RwLock` if it must be held across `.await`.

**FR-020** (Unwanted) The system shall not introduce a new `tokio::time::sleep` call on the agent hot path that blocks the loop for more than 5 ms; any longer sleep MUST be replaced with `tokio::select!` over cancellation and a tick.

### Tool Execution Parallelism

**FR-021** (Optional) Where multiple tool calls in a single assistant turn are independent (no shared state, no shared file path), the system shall execute them in parallel using `futures::future::join_all` and report the results back to the LLM in the original order.

**FR-022** (State-driven) While tool calls are executing in parallel, the system shall hold the permission-checked allow-set in an `Arc<PermissionDecision>` and pass clones to each parallel branch so that the inner `check_permission` calls do not contend on the global checker lock.

**FR-023** (Unwanted) The system shall not run more than `max_concurrent_tools` (default: `min(4, num_cpus)`) tool calls in parallel within a single turn, to avoid overwhelming the storage layer or the permission system.

### Compression and Context Window Management

**FR-024** (Ubiquitous) The system shall skip the per-iteration Headroom compression check when `compressed_this_turn` is already `true`, as implemented in the existing hysteresis logic in `processor.rs`.

**FR-025** (Event-driven) When the local token estimate (from `session::cache::SessionCache::estimated_token_count`) is below the configured `auto_threshold` for the model, the system shall skip the full `compress_history` call and reuse the previously compressed history.

**FR-026** (Optional) Where the model's reported `input_tokens` from `StreamEvent::Usage` is available, the system shall use that value as the source of truth for compression decisions, bypassing the local Headroom estimate.

### Configuration & Observability

**FR-027** (Ubiquitous) The system shall accept the following configuration under the top-level `agent_perf` key in `ragent.json`:

| Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | boolean | `true` | Master switch for the performance subsystem |
| `profiling` | boolean | `false` | Enable detailed per-scope timing logs at `info` level |
| `step_budget_secs` | u64 | `300` | Maximum wall-clock seconds per agent step |
| `stall_timeout_secs` | u64 | `60` | Maximum seconds without a stream delta before stall recovery fires |
| `max_concurrent_tools` | u32 | `min(4, num_cpus)` | Maximum parallel tool calls per turn |
| `parallel_independent_tools` | boolean | `true` | Execute independent tool calls in parallel |

**FR-028** (Event-driven) When `agent_perf.profiling` is `true`, the system shall emit a `PerfScopeEvent { scope, duration_us }` for every `profiler.scope(...)` invocation, regardless of whether the global `RAGENT_AGENT_PERF` env var is set.

**FR-029** (Ubiquitous) The system shall expose the latest `AgentLoopProfiler::snapshot()` result via a `GET /perf/snapshot` HTTP endpoint on the ragent server, returning a JSON object with per-scope `count`, `total_ms`, `avg_ms`, `max_ms`, and `last_ms` fields.

### Backward Compatibility and Safety

**FR-030** (Ubiquitous) The system shall keep the existing `SessionProcessor::process_message` and `SessionProcessor::process_user_message` signatures unchanged; all performance work happens inside the existing functions.

**FR-031** (Ubiquitous) The system shall keep the on-disk session format (`Storage` schema) byte-compatible with the current implementation; the new caches are in-memory only.

**FR-032** (Unwanted) The system shall not change the order of events published on the `EventBus` from the agent loop; consumers (TUI, HTTP SSE) MUST see the same event sequence they see today.

**FR-033** (Ubiquitous) The system shall keep the existing `StreamBuffer` (text/reasoning delta coalescing) intact, and the per-step event-flush thresholds (`STREAM_BUFFER_SIZE_THRESHOLD`, `STREAM_BUFFER_FLUSH_MS`) shall remain the canonical tuning knobs unless the baseline report identifies them as suboptimal.

**FR-034** (Ubiquitous) The system shall not introduce a new dependency on the agent hot path without a documented justification; preferred new dependencies are limited to `parking_lot`, `smallstr`, `arcstr`, and `bytes`.

## Configuration Example

```jsonc
{
  "agent_perf": {
    "enabled": true,
    "profiling": true,
    "step_budget_secs": 300,
    "stall_timeout_secs": 60,
    "max_concurrent_tools": 4,
    "parallel_independent_tools": true
  }
}
```

## Out of Scope

- Speculative / parallel LLM calls (running multiple providers in parallel and picking the first to finish) — separate spec.
- Provider-side optimisations (HTTP/2 multiplexing, connection pooling) — handled in the `ragent-llm` crate.
- Caching of LLM responses at the protocol level — semantically risky and a separate spec.
- Profile-guided optimisation (PGO) of the release binary — could be added later but is not part of this pass.
