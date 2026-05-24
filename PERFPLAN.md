# ragent Agent Loop Performance Plan (PERFPLAN)

**Document:** PERFPLAN.md  
**Date:** 2025-01-19  
**Target:** ragent agent execution loop (`crates/ragent-agent/src/session/processor.rs`)  
**Baseline:** v0.1.0-alpha.91  
**Goal:** >5% cumulative improvement per milestone, measured end-to-end on representative workloads

---

## Executive Summary

Analysis of the agent execution loop identified **11 performance bottlenecks**, of which **7 are individually capable of >5% improvement** and **4 compound to >20% improvement** on typical multi-step coding workloads. The plan is organised into 4 milestones, each self-contained and independently benchmarkable.

### Performance Baseline

| Workload | Typical Steps | Messages | Avg Step Time |
|----------|--------------|----------|---------------|
| Single-turn Q&A | 1 | 2-4 | ~2-5s |
| File edit + compile | 3-5 | 8-15 | ~15-30s |
| Multi-file refactor | 8-15 | 25-50 | ~60-180s |
| Long session (50+ messages) | 20+ | 100+ | Dominant cost shifts to history compaction |

### Identified Bottlenecks (impact sorted)

| # | Bottleneck | File:Line | Est. Impact | Milestone |
|---|-----------|-----------|-------------|-----------|
| 1 | History re-compaction every step | processor.rs:947 | 10-25% on long sessions | 1 |
| 2 | Tool definitions cloned every step | processor.rs:1055 | 8-15% with many tools | 1 |
| 3 | Serde JSON serialization for metrics | processor.rs:2826 | 5-10% per LLM call | 1 |
| 4 | Stream event individual publishes | processor.rs:1264 | 5-15% on fast models | 2 |
| 5 | Retry loop clones entire chat history | processor.rs:1185 | 3-8% on flaky networks | 2 |
| 6 | Permission dir_lists recompiled every check | processor.rs:327 | 3-5% per tool call | 3 |
| 7 | Interim storage update every step | processor.rs:2168 | 2-5% on I/O-bound hosts | 3 |
| 8 | `chars().count()` on large tool results | processor.rs:2797 | 2-4% per large result | 3 |
| 9 | Ollama stall detection string scans | processor.rs:1455 | 1-3% per step (Ollama) | 4 |
| 10 | Event string cloning (session_id) | event/mod.rs | 1-2% cumulative | 4 |
| 11 | std::sync::Mutex in async path | processor.rs:1643 | <1% but blocks executor | 4 |

---

## Milestone 1: Eliminate Per-Step Allocations & Clones (P0 — Largest Impact)

**Goal:** Remove the three largest per-step overheads to achieve 15-30% end-to-end improvement on multi-step workloads.

**Estimated Effort:** 2-3 days  
**Expected Gain:** 15-30% (depends on workload length)

### Task 1.1: Cache Tool Definitions in SessionProcessor

**Problem:** Every loop step calls `self.tool_registry.definitions()` (processor.rs:1055-1060), which:
1. Acquires a `RwLock` read on the tools HashMap
2. Acquires a second `RwLock` read on the hidden HashSet
3. Iterates ~111 tools, calling `.to_string()` on name/description, `.parameters_schema()` on each
4. Collects into a new `Vec<ToolDefinition>`
5. Sorts the result

Tools do not change during a session — MCP tools are registered once at startup via `set_mcp_client()`.

**Solution:** Cache tool definitions as `Arc<Vec<ToolDefinition>>` in `SessionProcessor`, refreshed only when MCP tools are registered.

| Sub-task | Description | Effort |
|----------|-------------|--------|
| T-1.1.1 | Add `cached_tool_definitions: std::sync::OnceLock<Arc<Vec<ToolDefinition>>>` to `SessionProcessor` | S |
| T-1.1.2 | Populate cache at end of `set_mcp_client()` | S |
| T-1.1.3 | Replace `self.tool_registry.definitions()` with `self.cached_tool_definitions.get().unwrap()` in agent loop | S |
| T-1.1.4 | Add `invalidate_tool_cache()` method for dynamic tool registration | S |
| T-1.1.5 | Write benchmark: `bench_tool_definitions_clone` vs cached | M |

**Validation:** `cargo bench` showing >50% reduction in `loop.step.setup` profiler time.

---

### Task 1.2: Incremental History Compaction

**Problem:** `compact_history_with_atomic_tool_calls()` (processor.rs:2645-2766) is called every single step with the full message history:
- Iterates ALL messages to build a tool_call_indices HashMap (O(n) allocation)
- Computes prefix sums (O(n))
- Scans for protected atomic groups (O(n))
- Searches for best cut point (O(n))
- Returns `.to_vec()` clone of suffix

For a 50-message session, this is ~200+ allocations per step, called 10-20 times.

**Solution:** Maintain running state across steps:
1. Track cumulative token count incrementally
2. Only trigger compaction when count exceeds threshold
3. Cache the "last valid cut point" to avoid re-scanning from scratch
4. Use a sliding window approach: when compaction runs, record which messages were dropped; next step only considers new messages

| Sub-task | Description | Effort |
|----------|-------------|--------|
| T-1.2.1 | Add `compaction_state` struct to SessionProcessor with running token count and last cut index | S |
| T-1.2.2 | Modify agent loop to update running count incrementally after each step | M |
| T-1.2.3 | Only call `compact_history_with_atomic_tool_calls()` when `running_tokens > threshold * 0.8` | S |
| T-1.2.4 | Reset compaction state on session switch/clear | S |
| T-1.2.5 | Write benchmark: compact every step vs incremental | M |
| T-1.2.6 | Ensure atomic tool call pairs are still never split | S |

**Validation:** Profile showing `history.compact` scope drops from N× per run to 1× or 2×.

---

### Task 1.3: Eliminate Serde Serialization in `chat_request_payload_bytes`

**Problem:** `chat_request_payload_bytes()` (processor.rs:2826-2830) serializes the entire `ChatRequest` to JSON via `serde_json::to_vec()`, only to count bytes and immediately discard the result:

```rust
fn chat_request_payload_bytes(request: &ChatRequest) -> u64 {
    serde_json::to_vec(request)
        .map(|payload| payload.len() as u64)
        .unwrap_or(0)
}
```

Called twice per step (init exchange + main request). For large requests (50+ messages, 100+ tools), this is pure CPU overhead.

**Solution:** Replace with an approximate byte-counting function that sums:
- `messages.len() * avg_message_bytes`
- `tools.len() * avg_tool_definition_bytes`
- System prompt length

Or compute incrementally as messages are appended.

| Sub-task | Description | Effort |
|----------|-------------|--------|
| T-1.3.1 | Implement `estimate_request_bytes()` using pre-calculated per-message and per-tool averages | S |
| T-1.3.2 | Replace `chat_request_payload_bytes` calls in processor.rs | S |
| T-1.3.3 | Validate estimate is within 20% of actual for typical workloads | S |
| T-1.3.4 | Benchmark: serde serialization vs estimation | S |

**Validation:** `cargo bench` showing `RequestStarted` event no longer appears in hot paths.

---

## Milestone 2: Stream Processing & Retry Optimisations (P1)

**Goal:** Reduce per-token and per-retry overhead to achieve 5-15% additional improvement.

**Estimated Effort:** 2-3 days  
**Expected Gain:** 5-15%

### Task 2.1: Batch Stream Event Publishing

**Problem:** Every `TextDelta`, `ReasoningDelta`, `ToolCallDelta`, etc. triggers `event_bus.publish()` (processor.rs:1264-1382). Each publish:
1. Clones `session_id.to_string()`
2. Clones the delta text
3. Sends through a broadcast channel

For fast models (Claude, GPT-4o) streaming thousands of tokens, this is thousands of allocations and channel sends per second.

**Solution:** Buffer deltas and publish in batches:
- Accumulate `TextDelta` text in a local buffer
- Flush every N milliseconds (e.g., 50ms) or every N characters (e.g., 100)
- Publish single batched event instead of many small ones

| Sub-task | Description | Effort |
|----------|-------------|--------|
| T-2.1.1 | Add `StreamBuffer` struct that accumulates text/reasoning/tool deltas | M |
| T-2.1.2 | Buffer text deltas and flush on timer (50ms) or size threshold (256 chars) | M |
| T-2.1.3 | Keep ToolCallStart/End events immediate (not batched) for correct sequencing | S |
| T-2.1.4 | Benchmark: individual vs batched publishing | S |

**Validation:** `cargo bench` showing reduced `loop.llm.handle.text_delta` time; TUI still renders smoothly.

---

### Task 2.2: Avoid Cloning `chat_messages` on Retry

**Problem:** On every retry attempt (processor.rs:1183-1196), the entire `chat_messages` vector is cloned:

```rust
let attempt_request = ChatRequest {
    messages: chat_messages.clone(),  // <-- clones ALL messages
    tools: (*tool_definitions).clone(),  // <-- clones ALL tool definitions
    // ...
};
```

`chat_messages` grows with every step. After 10 steps, it may contain 30+ `ChatMessage` structs with large text content. Retry on transient network errors is common.

**Solution:** Use `Arc<Vec<ChatMessage>>` for the request; providers that need ownership can clone only if necessary.

| Sub-task | Description | Effort |
|----------|-------------|--------|
| T-2.2.1 | Change `ChatRequest.messages` to `Arc<Vec<ChatMessage>>` or `Arc<[ChatMessage]>` | M |
| T-2.2.2 | Update all provider implementations to accept `Arc` or clone internally | L |
| T-2.2.3 | Same treatment for `tools` field | M |
| T-2.2.4 | Benchmark: clone vs Arc on retry | S |

**Validation:** Retry attempts no longer appear as allocation spikes in heap profiler.

---

## Milestone 3: Permission Checks & Storage Optimisations (P1)

**Goal:** Reduce per-tool-call overhead and unnecessary I/O.

**Estimated Effort:** 2-3 days  
**Expected Gain:** 5-10%

### Task 3.1: Cache Compiled Permission Patterns

**Problem:** `check_permission_with_prompt()` (processor.rs:327-341) calls `get_compiled_denylist()` and `get_compiled_allowlist()` on every permission check. These functions re-compile glob patterns from config every time:

```rust
let denylist = get_compiled_denylist();  // recompiles every call
let allowlist = get_compiled_allowlist();  // recompiles every call
```

A typical multi-step coding session may invoke 20-50 tools, each triggering 1-10 permission checks.

**Solution:** Cache compiled glob sets in a `OnceLock` or `RwLock`, invalidated only on config reload.

| Sub-task | Description | Effort |
|----------|-------------|--------|
| T-3.1.1 | Add `static COMPILED_DENYLIST: OnceLock<globset::GlobSet>` to `ragent-config/src/dir_lists.rs` | S |
| T-3.1.2 | Add `static COMPILED_ALLOWLIST: OnceLock<globset::GlobSet>` | S |
| T-3.1.3 | Add `invalidate_dir_lists_cache()` called on config reload | S |
| T-3.1.4 | Benchmark: compile-every-call vs cached | S |

**Validation:** `tool.permission:*` profiler scopes show <1ms instead of 5-20ms.

---

### Task 3.2: Skip Unchanged Interim Storage Updates

**Problem:** Every step fires an async storage update (processor.rs:2168-2174):

```rust
let mut interim = Message::new(session_id, Role::Assistant, assistant_parts.clone());
interim.id = assistant_msg_id.clone();
let _ = self.storage_op(move |s| s.update_message(&interim)).await;
```

This runs `spawn_blocking` even when `assistant_parts` hasn't changed since the last update (e.g., during waiting for LLM response).

**Solution:** Track a hash/dirty flag; only update when content has actually changed.

| Sub-task | Description | Effort |
|----------|-------------|--------|
| T-3.2.1 | Add `last_interim_hash: Option<u64>` to step-local state | S |
| T-3.2.2 | Compute hash of `assistant_parts` (e.g., via `ahash` or content-based) | S |
| T-3.2.3 | Skip `storage_op` when hash unchanged | S |
| T-3.2.4 | Ensure placeholder is still created on first step | S |

**Validation:** Steps where no tool calls execute show zero `storage.assistant_interim.update` time.

---

### Task 3.3: Replace `chars().count()` with Byte Length in Tool Result Truncation

**Problem:** `tool_result_content_for_llm()` (processor.rs:2796-2824) uses `chars().count()` four times on potentially large content:

```rust
if content.chars().count() <= MAX_TOOL_RESULT_CHARS_FOR_LLM {  // O(n) UTF-8 decode
    return content.to_string();
}
let head = truncate_at_char_boundary(content, TOOL_RESULT_HEAD_CHARS_FOR_LLM);  // calls chars().count()
let tail = trailing_at_char_boundary(content, TOOL_RESULT_TAIL_CHARS_FOR_LLM);  // calls chars().count()
let omitted_chars = content.chars().count().saturating_sub(...);  // calls chars().count()
```

For a 12,000-char tool result, this decodes UTF-8 four times.

**Solution:** Use byte length (`content.len()`) for the threshold check (safe: we're truncating anyway, a few bytes off is fine). Only use `chars().count()` for the actual truncation boundary.

| Sub-task | Description | Effort |
|----------|-------------|--------|
| T-3.3.1 | Replace threshold check with `content.len() <= MAX_TOOL_RESULT_BYTES_FOR_LLM` | S |
| T-3.3.2 | Keep char-aware truncation for actual boundary, but only once | S |
| T-3.3.3 | Benchmark: `chars().count()` vs `len()` on 12K char string | S |

**Validation:** 3× reduction in `tool_result_content_for_llm` time for large outputs.

---

## Milestone 4: Micro-Optimisations & Polish (P2)

**Goal:** Clean up remaining hot-path allocations and lock contention for cumulative 2-5% gain.

**Estimated Effort:** 2 days  
**Expected Gain:** 2-5%

### Task 4.1: Pre-Compile Ollama Stall Detection Patterns

**Problem:** Every step with no tool calls scans text_buffer against 12 literal strings (processor.rs:1455-1466):

```rust
text_buffer.contains("Let me")
    || text_buffer.contains("I'll")
    || text_buffer.contains("I will")
    || text_buffer.contains("I'm going to")
    // ... 8 more
```

**Solution:** Use a compiled `regex::RegexSet` or `aho_corasick::AhoCorasick` automaton, built once.

| Sub-task | Description | Effort |
|----------|-------------|--------|
| T-4.1.1 | Build `RegexSet` or `AhoCorasick` with stall patterns at startup | S |
| T-4.1.2 | Replace 12 `.contains()` calls with single automaton search | S |

---

### Task 4.2: Use `Arc<str>` for Session IDs in Events

**Problem:** Every `event_bus.publish()` clones `session_id.to_string()` (Event enum fields all use `String`). Session IDs are typically 10-36 chars and immutable.

**Solution:** Change `session_id: String` to `session_id: Arc<str>` in all `Event` variants. This is a breaking change across the crate but has zero runtime impact on correctness.

| Sub-task | Description | Effort |
|----------|-------------|--------|
| T-4.2.1 | Change `session_id: String` to `session_id: Arc<str>` in `Event` enum | M |
| T-4.2.2 | Update all publish sites to use `Arc::from(session_id)` | M |
| T-4.2.3 | Update all subscribe/consume sites | M |
| T-4.2.4 | Verify TUI and HTTP server still compile and display correctly | S |

---

### Task 4.3: Replace `std::sync::Mutex` with `tokio::sync::RwLock` for `active_spec`

**Problem:** `self.active_spec.lock().unwrap()` (processor.rs:1643) is called in an async context. `std::sync::Mutex` blocks the async executor thread.

**Solution:** Change `active_spec: std::sync::Mutex<Option<String>>` to `tokio::sync::RwLock<Option<String>>`.

| Sub-task | Description | Effort |
|----------|-------------|--------|
| T-4.3.1 | Change field type to `tokio::sync::RwLock` | S |
| T-4.3.2 | Update all `.lock().unwrap()` to `.read().await` / `.write().await` | S |

---

## Measurement & Benchmarking

### Benchmark Harness

Add a Criterion benchmark in `crates/ragent-agent/benches/agent_loop.rs`:

```rust
// Benchmark: 10-step agent loop with 5 tools per step
// Measures wall-clock time for process_message() on a mock provider
```

### Key Metrics

| Metric | How to Measure | Target Improvement |
|--------|---------------|-------------------|
| Step time | `agent_loop_profiler` scope `loop.step.total` | -15% per milestone |
| Allocations | `dhat` heap profiler | -30% allocations |
| Lock contention | `parking_lot` contention counters | Near zero |
| LLM wait % | `cumulative_model_wait_ms / total_ms` | Keep >85% (we're not the bottleneck) |

### Regression Testing

After each milestone:
1. Run full test suite: `cargo test -p ragent-agent`
2. Run benchmark: `cargo bench -p ragent-agent`
3. Profile: `cargo flamegraph --bench agent_loop`
4. Compare against baseline results stored in `target/criterion/`

---

## Implementation Order

Recommended order within each milestone:

**Milestone 1:**
1. T-1.1 (cache tool definitions) — easiest, highest impact
2. T-1.3 (serde elimination) — no API changes
3. T-1.2 (incremental compaction) — largest impact, most complex

**Milestone 2:**
1. T-2.1 (batch stream events) — visible UX improvement
2. T-2.2 (Arc for messages) — requires provider API changes

**Milestone 3:**
1. T-3.1 (cache dir_lists) — isolated to config crate
2. T-3.3 (chars().count()) — trivial
3. T-3.2 (skip interim updates) — requires careful testing

**Milestone 4:**
1. T-4.1 (compiled patterns) — trivial
2. T-4.3 (tokio RwLock) — trivial
3. T-4.2 (Arc<str>) — most invasive, save for last

---

## Risks & Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Arc changes break provider trait object safety | Medium | High | Keep `ChatRequest` fields as `Vec` for trait, convert internally |
| Incremental compaction misses atomic pairs | Low | Critical | Extensive property-based tests for compaction correctness |
| Batch publishing delays TUI updates | Medium | Medium | Keep 50ms max delay; users won't perceive |
| Config caching becomes stale | Low | High | Invalidate on every config reload; add tests |
| Arc<str> changes break serialization | Low | Medium | serde implements `Serialize` for `Arc<str>` |

---

## Summary

| Milestone | Tasks | Est. Effort | Expected Gain |
|-----------|-------|-------------|---------------|
| 1: Eliminate per-step allocations | 3 | 2-3 days | 15-30% |
| 2: Stream & retry optimisation | 2 | 2-3 days | 5-15% |
| 3: Permission & storage | 3 | 2-3 days | 5-10% |
| 4: Micro-optimisations | 3 | 2 days | 2-5% |
| **Total** | **11** | **8-11 days** | **27-60%** |

The **conservative cumulative estimate is 25-35%** improvement on typical multi-step coding workloads, with **up to 50% on very long sessions** (where history compaction dominates). Single-turn workloads see smaller gains (~5-10%) since they don't exercise the loop multiple times.
