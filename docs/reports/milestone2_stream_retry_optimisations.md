# Milestone 2 Completion Report — Stream Processing & Retry Optimisations

**Date:** 2025-01-20
**Spec:** PERFPLAN.md Milestone 2
**Status:** ✅ COMPLETE

---

## Summary

Implemented two performance optimisations targeting per-token and per-retry overhead
in the agent execution loop (`crates/ragent-agent/src/session/processor.rs`):

| Task | Description | Expected Gain |
|------|-------------|---------------|
| T-2.1 | Batch stream event publishing | 5–15% on fast models |
| T-2.2 | Arc for ChatRequest messages/tools | 3–8% on retry |

---

## Task 2.1: Batch Stream Event Publishing

### Problem

Every `TextDelta`, `ReasoningDelta`, etc. triggered `event_bus.publish()`
(processor.rs:1264-1382). Each publish:

1. Cloned `session_id.to_string()`
2. Cloned the delta text
3. Sent through a broadcast channel

For fast models streaming thousands of tokens, this was thousands of allocations
and channel sends per second.

### Solution

Added a `StreamBuffer` struct that accumulates text/reasoning deltas and flushes
them in batches:

- **Size threshold:** 256 characters
- **Time threshold:** 50 ms (`tokio::time::Duration::from_millis(50)`)
- **Immediate events:** `ToolCallStart` / `ToolCallEnd` are forwarded
  immediately so sequencing is preserved.

### Implementation Details

**Location:** `crates/ragent-agent/src/session/processor.rs` (lines ~34-90)

```rust
struct StreamBuffer {
    text: String,
    reasoning: String,
    flush_size: usize,
    flush_interval: Duration,
    last_flush: Instant,
}
```

**Stream loop changes:** The inner stream `match event` block now routes
`TextDelta` and `ReasoningDelta` through `StreamBuffer::push_text()` /
`push_reasoning()`. When a flush triggers, a single batched event is published
instead of many small ones.

**End-of-stream flush:** When `next_event` returns `None`, any remaining buffered
text is drained and published.

**Tool-call barrier:** On `ToolCallStart`, all buffered text is flushed
immediately so the tool call appears after the preceding text in the correct
order.

### Files Modified

- `crates/ragent-agent/src/session/processor.rs` — added `StreamBuffer` struct,
  modified stream event handling loop

---

## Task 2.2: Avoid Cloning `chat_messages` on Retry

### Problem

On every retry attempt (processor.rs:1183-1196), the entire `chat_messages` vector
was cloned:

```rust
let attempt_request = ChatRequest {
    messages: chat_messages.clone(),  // <-- clones ALL messages
    tools: (*tool_definitions).clone(),  // <-- clones ALL tool definitions
    // ...
};
```

`chat_messages` grows with every step. After 10 steps it may contain 30+
`ChatMessage` structs with large text content. Retry on transient network errors
is common.

### Solution

Changed `ChatRequest` fields to use `Arc<Vec<T>>`:

```rust
pub struct ChatRequest {
    pub messages: Arc<Vec<ChatMessage>>,
    pub tools: Arc<Vec<ToolDefinition>>,
    // ...
}
```

On retry, only the `Arc` pointer is cloned (a cheap refcount increment). Providers
that need ownership can clone internally if necessary; most providers only read
from the vectors.

### Files Modified

| File | Change |
|------|--------|
| `crates/ragent-llm/src/llm.rs` | Changed `messages` and `tools` fields to `Arc<Vec<T>>` |
| `crates/ragent-llm/src/providers/anthropic.rs` | Updated iteration over `request.messages` |
| `crates/ragent-llm/src/providers/openai.rs` | Updated iteration over `request.messages` |
| `crates/ragent-llm/src/providers/gemini.rs` | Updated iteration over `request.messages` |
| `crates/ragent-llm/src/providers/huggingface.rs` | Updated iteration + test construction sites |
| `crates/ragent-llm/src/providers/ollama.rs` | Updated iteration over `request.messages` |
| `crates/ragent-llm/src/providers/ollama_cloud.rs` | Updated iteration over `request.messages` |
| `crates/ragent-llm/src/providers/copilot.rs` | Updated test construction sites |
| `crates/ragent-llm/src/providers/thinking.rs` | Updated test construction sites |
| `crates/ragent-llm/tests/test_thinking_adapters.rs` | Updated `Arc::new` for test requests |
| `crates/ragent-agent/src/session/processor.rs` | Updated all `ChatRequest` construction sites |
| `crates/ragent-agent/benches/agent_loop.rs` | Updated benchmark request builders |
| `crates/ragent-bench/src/model.rs` | Updated benchmark request construction |
| `crates/ragent-server/src/routes/mod.rs` | Updated server route request construction |
| `crates/ragent-tui/src/app.rs` | Updated TUI request construction |
| `Cargo.toml` | Enabled `serde` `"rc"` feature for `Arc` serialization |
| `crates/ragent-llm/Cargo.toml` | Switched `serde` dep to workspace (`features = ["derive", "rc"]`) |

### Serde Compatibility

The workspace `Cargo.toml` was updated to include `"rc"` in the `serde` feature
set (`features = ["derive", "rc"]`). This enables `Serialize`/`Deserialize`
impls for `Arc<T>` so that `ChatRequest` can still derive those traits.

---

## Validation

### Build

```bash
$ cargo check
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.92s
```

### Tests

```bash
$ cargo test -p ragent-agent --lib
    Finished test profile [unoptimized + debuginfo] target(s) in 2m 43s
    Running unittests src/lib.rs
    ...
test result: ok. 332 passed; 0 failed; 0 ignored

$ cargo test -p ragent-llm
    Finished test profile [unoptimized + debuginfo] target(s) in 1m 15s
    ...
test result: ok. 53 passed; 0 failed; 0 ignored (lib)
test result: ok. 25 passed; 0 failed; 0 ignored (doc)

$ cargo test
    Finished test profile [unoptimized + debuginfo] target(s) in 6m 08s
    ...
test result: ok.
```

### Benchmarks

```bash
$ cargo check --bench agent_loop -p ragent-agent
    Finished dev profile [unoptimized + debuginfo] target(s) in 1m 23s
```

---

## Expected Performance Impact

| Metric | Before | After |
|--------|--------|-------|
| Per-token event publishes | 1 per delta | 1 per 256 chars or 50 ms |
| Retry message clone | Deep clone of `Vec<ChatMessage>` | `Arc` refcount bump |
| Retry tool clone | Deep clone of `Vec<ToolDefinition>` | `Arc` refcount bump |

**Cumulative estimated gain:** 5–15% additional improvement on top of
Milestone 1, depending on model speed and retry frequency.

---

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| `Arc` changes break provider trait object safety | `ChatRequest` still owns the `Arc`; providers receive `ChatRequest` by value |
| Batch publishing delays TUI updates | 50 ms max delay is below human perception threshold |
| Tool-call sequencing broken by batching | `ToolCallStart` triggers an explicit flush before the event is published |
| `Arc` serde requires `"rc"` feature | Added `"rc"` to workspace `serde` features; verified all tests pass |

---

## Next Steps

- Proceed to **Milestone 3** (Permission Checks & Storage Optimisations):
  - T-3.1: Cache compiled permission patterns
  - T-3.2: Skip unchanged interim storage updates
  - T-3.3: Replace `chars().count()` with byte length in tool result truncation
