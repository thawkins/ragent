# Performance Plan: Agent Scheduler Optimization

> **Project**: ragent - AI Coding Agent Performance Optimization
> **Created**: 2025-01
> **Status**: Draft - Ready for Implementation

## Executive Summary

This plan outlines milestones and tasks for making the ragent agent scheduler run faster. Analysis reveals several bottlenecks in the current architecture:

1. **Single-threaded agent loop** - Tool calls execute sequentially despite being spawned as futures
2. **Synchronous database operations** - Storage operations block tokio threads via `spawn_blocking`
3. **Repeated system prompt construction** - Built on every message turn, not cached
4. **Inefficient memory cloning** - Multiple clones of large data structures (messages, history)
5. **No batching for tool results** - Individual events per tool result
6. **Suboptimal context window management** - History compaction happens late in the pipeline

---

## Architecture Overview

```
┌────────────────────────────────────────────────────────���────────────────────┐
│                         AGENT ACTION LOOP FLOW                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  User Input ──► Store Message ──► Build System Prompt ──► Compact History   │
│                        │                                                      │
│                        ▼                                                      │
│           ┌─────────────────────────┐                                        │
│           │  AGENT LOOP (per turn)  │                                        │
│           │  ┌─────────────────────┐ │                                        │
│           │  │  1. Call LLM        │ │  ← Network wait (async)              │
│           │  │  2. Stream Response │ │  ← Token-by-token processing         │
│           │  │  3. Parse ToolCalls │ │                                        │
│           │  │  4. Execute Tools   │ │  ← Parallel execution (bounded)      │
│           │  │  5. Collect Results │ │  ← Sequential processing             │
│           │  │  6. Update History  │ │  ← DB write (spawn_blocking)         │
│           │  └─────────────────────┘ │                                        │
│           └─────────────────────────┘                                        │
│                        │                                                      │
│                        ▼                                                      │
│              Persist Final Message ──► Publish Events ──► Done              │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Milestone 1: Tool Execution Parallelism (Foundation)

**Goal**: Maximize CPU utilization during tool execution phase
**Expected Impact**: 40-60% reduction in multi-tool call latency
**Priority**: P0 - Critical

### M1.1: Current State Analysis
**Status**: ✅ Complete

The current tool execution already uses `tokio::spawn` for each tool call:
- Tool calls are spawned as separate tasks (line 1156 in processor.rs)
- Results collected via `futures::future::join_all` (line 1377)
- Tool semaphore bounds concurrency to 5 (`MAX_CONCURRENT_TOOLS` in resource.rs)

However, issues identified:
- Tool result processing happens sequentially after `join_all`
- Each result triggers separate event bus publications
- No streaming of tool results back to LLM

### M1.2: Tool Result Streaming
**Task**: Implement streaming tool results for faster LLM pipelining

**Implementation**:
```rust
// Instead of waiting for all tools to complete:
let results = futures::future::join_all(futures).await;

// Stream results as they complete:
while let Some((tool_id, result)) = results_stream.next().await {
    // Immediately add to chat history and publish event
    // LLM can start processing earlier results while others finish
}
```

**Files to modify**:
- `crates/ragent-core/src/session/processor.rs` (lines 1113-1427)

**Acceptance Criteria**:
- [ ] Tool results streamed as they complete
- [ ] No change in final behavior (ordering preserved)
- [ ] Measurable reduction in latency for multi-tool scenarios

### M1.3: Optimize Tool Semaphore
**Task**: Tune `MAX_CONCURRENT_TOOLS` based on workload profiling

**Current**: Hardcoded to 5
**Investigation**: Profile actual tool execution patterns

**Implementation**:
- Make `MAX_CONCURRENT_TOOLS` configurable via `ragent.json`
- Default based on CPU cores: `num_cpus::get().min(8)`

**Files to modify**:
- `crates/ragent-core/src/resource.rs` (lines 67-92)
- `crates/ragent-core/src/config/mod.rs`

---

## Milestone 2: Database & Storage Optimization

**Goal**: Eliminate blocking I/O from hot path
**Expected Impact**: 20-30% reduction in per-turn latency
**Priority**: P0 - Critical

### M2.1: Async Storage Layer
**Task**: Convert storage operations from `spawn_blocking` to true async

**Current State**:
```rust
// processor.rs lines 266-277
async fn storage_op<F, T>(&self, f: F) -> Result<T>
where F: FnOnce(&Storage) -> Result<T> + Send + 'static,
{
    let storage = self.session_manager.storage().clone();
    tokio::task::spawn_blocking(move || f(&storage))  // Blocks thread!
        .await
        .map_err(|e| anyhow::anyhow!("storage task panicked: {e}"))?
}
```

**Implementation Options**:

**Option A**: Use `sqlx` for async SQLite (breaking change)
- Replace rusqlite with sqlx
- Full async/await support
- Connection pooling built-in

**Option B**: Implement storage write buffering
- Accumulate writes in memory
- Flush asynchronously in batches
- Risk: Data loss on crash

**Recommendation**: Option A for correctness, Option B for speed (configurable)

**Files to modify**:
- `crates/ragent-core/src/storage/mod.rs` (94.8 KB - major refactor)
- `crates/ragent-core/src/session/processor.rs` (remove `storage_op` wrapper)

### M2.2: Message Persistence Optimization
**Task**: Batch message writes and use WAL mode

**Current**: Each message triggers separate DB write
**Optimized**: 
- Use SQLite WAL mode for better concurrency
- Batch multiple message updates in single transaction
- Keep recent messages in memory cache

**Implementation**:
```rust
// Add to Storage
pub async fn batch_write_messages(&self, msgs: &[Message]) -> Result<()> {
    let mut tx = self.conn.begin_transaction()?;
    for msg in msgs {
        self.insert_message_tx(&mut tx, msg)?;
    }
    tx.commit()?;
    Ok(())
}
```

**Files to modify**:
- `crates/ragent-core/src/storage/mod.rs`

### M2.3: Session State Caching
**Task**: Cache session metadata in memory to reduce DB lookups

**Current**: `get_session()` hits database every time
**Optimized**: Use `DashMap` for in-memory session cache with TTL

**Files to modify**:
- `crates/ragent-core/src/session/mod.rs`

---

## Milestone 3: System Prompt & Context Optimization

**Goal**: Reduce redundant computation in prompt building
**Expected Impact**: 15-25% reduction in per-turn overhead
**Priority**: P1 - High

### M3.1: System Prompt Caching
**Task**: Cache system prompt components with invalidation

**Current State**: Rebuilds entire system prompt on every turn (processor.rs lines 439-608)

**Components**:
1. Agent base prompt - Changes only on agent switch
2. Tool reference section - Changes only on tool registration
3. LSP guidance - Changes only on LSP connection state change
4. Codeindex guidance - Changes only on index state change
5. Team context - Changes only on team membership change

**Implementation**:
```rust
pub struct SystemPromptCache {
    agent_prompt: Cached<String>,
    tool_reference: Cached<Vec<ToolDefinition>>,
    lsp_guidance: Cached<String>,
    codeindex_guidance: Cached<String>,
    team_guidance: Cached<String>,
    cache_version: AtomicU64,
}

impl SystemPromptCache {
    pub fn invalidate_tool_cache(&self) {
        self.cache_version.fetch_add(1, Ordering::SeqCst);
    }
    
    pub fn get_full_prompt(&self) -> String {
        // Rebuild only invalidated components
    }
}
```

**Files to modify**:
- `crates/ragent-core/src/agent/mod.rs` (add caching)
- `crates/ragent-core/src/session/processor.rs` (use cache)

### M3.2: Incremental History Management
**Task**: Avoid full history rebuilds on each turn

**Current**: `history_to_chat_messages()` processes entire history every turn
**Optimized**: Maintain running chat message list, append only

**Implementation**:
```rust
pub struct SessionState {
    // Instead of rebuilding from DB every turn:
    cached_chat_messages: Vec<ChatMessage>,
    last_message_count: usize,
}

impl SessionState {
    pub fn get_chat_messages(&mut self, session: &Session) -> &[ChatMessage] {
        let current_count = session.messages.len();
        if current_count > self.last_message_count {
            // Only append new messages
            for msg in &session.messages[self.last_message_count..] {
                self.cached_chat_messages.push(convert_message(msg));
            }
            self.last_message_count = current_count;
        }
        &self.cached_chat_messages
    }
}
```

**Files to modify**:
- `crates/ragent-core/src/session/processor.rs`
- `crates/ragent-core/src/session/mod.rs`

### M3.3: Context Window Pre-Compaction
**Task**: Compact history earlier in pipeline with smarter strategy

**Current**: Compacts after loading full history from DB (processor.rs lines 609-625)
**Optimized**: 
- Keep running token count estimate
- Trigger compaction before history reaches threshold
- Use faster approximation for token counting

**Files to modify**:
- `crates/ragent-core/src/session/processor.rs`
- `crates/ragent-core/src/memory/compact.rs`

---

## Milestone 4: Memory & Allocation Optimization

**Goal**: Reduce GC pressure and memory churn
**Expected Impact**: 10-15% smoother performance, less latency variance
**Priority**: P1 - High

### M4.1: String Deduplication
**Task**: Use string interning for repeated values

**Candidates**:
- Tool names (repeated in every tool call)
- Session IDs (repeated in events)
- Common error messages

**Implementation**:
```rust
use string_interner::StringInterner;

lazy_static! {
    static ref INTERNER: Mutex<StringInterner> = Mutex::new(StringInterner::new());
}

pub fn intern_tool_name(name: &str) -> String {
    INTERNER.lock().unwrap().get_or_intern(name)
}
```

**Files to modify**:
- `crates/ragent-core/src/tool/mod.rs`
- `crates/ragent-core/src/event/mod.rs`

### M4.2: Message Part Pooling
**Task**: Reuse message part allocations

**Current**: New allocations for every `MessagePart::Text`, `MessagePart::ToolCall`
**Optimized**: Use `objpool` or `crossbeam-queue` for frequent allocations

**Files to modify**:
- `crates/ragent-core/src/message/mod.rs`

### M4.3: Event Bus Optimization
**Task**: Reduce event cloning overhead

**Current**: Events are cloned for every subscriber (event/mod.rs line 697-726)
**Optimized**: 
- Use `Arc<Event>` for shared ownership
- Batch rapid-fire events (tool results)
- Implement event filtering at source

**Files to modify**:
- `crates/ragent-core/src/event/mod.rs`

---

## Milestone 5: Async Runtime & Concurrency

**Goal**: Optimize tokio scheduler usage
**Expected Impact**: 10-20% better throughput under load
**Priority**: P2 - Medium

### M5.1: Dedicated Thread Pools
**Task**: Isolate CPU-bound work from async runtime

**Current**: All work runs on tokio threads
**Optimized**:
- CPU-bound tasks (parsing, serialization) on `rayon` pool
- IO-bound tasks on tokio
- Use `tokio::task::spawn_blocking` strategically

**Implementation**:
```rust
// Use rayon for parallel processing
use rayon::prelude::*;

pub fn compact_history_parallel(history: &[Message]) -> Vec<Message> {
    history.par_chunks(100)
        .map(|chunk| compact_chunk(chunk))
        .collect()
}
```

**Files to modify**:
- `crates/ragent-core/src/memory/compact.rs`
- `crates/ragent-core/src/session/processor.rs`

### M5.2: Task Prioritization
**Task**: Implement priority queues for agent tasks

**Implementation**:
- Use `tokio::sync::mpsc::PriorityQueue` (custom)
- Prioritize:
  1. User-facing responses
  2. Tool executions
  3. Background tasks
  4. Maintenance (compaction, indexing)

**Files to modify**:
- `crates/ragent-core/src/task/mod.rs`

### M5.3: Connection Pooling
**Task**: Pool LLM API connections

**Current**: New connection per request
**Optimized**: Use `reqwest` connection pooling with keep-alive

**Files to modify**:
- `crates/ragent-core/src/provider/http_client.rs` (line 1+)
- All provider implementations

---

## Milestone 6: Advanced Optimizations

**Goal**: Cutting-edge optimizations for maximum performance
**Expected Impact**: Additional 10-15% improvement
**Priority**: P3 - Low (Future work)

### M6.1: Streaming Response Optimization
**Task**: Zero-copy streaming for LLM responses

**Implementation**:
- Use `bytes::Bytes` for response chunks
- Avoid String allocations for each token
- Stream directly to event bus

**Files to modify**:
- All provider implementations (anthropic.rs, openai.rs, etc.)

### M6.2: Predictive Tool Execution
**Task**: Pre-fetch tools based on model intent detection

**Implementation**:
- Analyze streaming tokens for tool call patterns
- Pre-validate tool arguments while streaming
- Pre-fetch file contents for likely `read` calls

**Files to modify**:
- `crates/ragent-core/src/session/processor.rs`

### M6.3: Persistent Connection Cache
**Task**: Keep MCP/LSP connections warm

**Implementation**:
- Connection pooling for MCP servers
- Keep LSP servers alive with ping messages
- Reuse connections across sessions

**Files to modify**:
- `crates/ragent-core/src/mcp/mod.rs`
- `crates/ragent-core/src/lsp/client.rs`

---

## Implementation Roadmap

### Phase 1: Quick Wins (Weeks 1-2)
1. ✅ M1.2: Tool Result Streaming
2. ✅ M2.2: Message Persistence Optimization (WAL mode)
3. ✅ M3.1: System Prompt Caching (partial)

### Phase 2: Core Optimizations (Weeks 3-5)
4. ⬜ M2.1: Async Storage Layer
5. ⬜ M3.2: Incremental History Management
6. ⬜ M3.3: Context Window Pre-Compaction

### Phase 3: Polish & Scale (Weeks 6-8)
7. ⬜ M4.1: String Deduplication
8. ⬜ M4.3: Event Bus Optimization
9. ⬜ M5.1: Dedicated Thread Pools

### Phase 4: Advanced (Ongoing)
10. ⬜ M6.1: Streaming Response Optimization
11. ⬜ M6.2: Predictive Tool Execution
12. ⬜ M6.3: Persistent Connection Cache

---

## Benchmarking Strategy

### Key Metrics

| Metric | Current Baseline | Target | Measurement |
|--------|------------------|--------|-------------|
| Multi-tool latency (5 tools) | ~800ms | <400ms | Criterion bench |
| Per-turn overhead | ~150ms | <50ms | Criterion bench |
| Memory per message | ~2KB | <1KB | heaptrack |
| DB write latency | ~20ms | <5ms | tracing spans |
| Event propagation | ~1ms | <0.5ms | custom timing |

### Benchmark Scenarios

1. **Multi-tool workload**: 5 concurrent `read` tool calls
2. **Long session**: 100-message history processing
3. **Team coordination**: 4 teammate spawn + wait
4. **Mixed workload**: bash + read + edit + grep in sequence

### Benchmark Commands

```bash
# Run all benchmarks
cargo bench -p ragent-core
cargo bench -p ragent-code

# Profile specific benchmark
cargo bench -p ragent-core -- orchestrator
cargo bench -p ragent-core -- session_processor

# Memory profiling
cargo build --release
heaptrack ./target/release/ragent
```

---

## Success Criteria

### Performance Targets

| Area | Target | Measurement Method |
|------|--------|-------------------|
| Tool execution | 50% faster multi-tool calls | Benchmark: `test_parallel_tools` |
| Session startup | 30% faster | Benchmark: `test_session_create` |
| Message processing | 40% less per-turn latency | Tracing: `process_message` span |
| Memory usage | 25% reduction in peak RSS | `/usr/bin/time -v` |
| Responsiveness | <100ms p99 event latency | Event bus timing histogram |

### Quality Gates

- [ ] All existing tests pass
- [ ] No new unsafe code (per `deny.toml`)
- [ ] Benchmark regression tests pass (±5% threshold)
- [ ] Memory leaks verified with `valgrind`
- [ ] Load testing: 10 concurrent sessions stable

---

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Async storage refactor complexity | High | Phase 2, incremental migration with feature flags |
| Cache invalidation bugs | High | Comprehensive tests, cache version logging |
| Memory pressure from caching | Medium | Bounded caches with LRU eviction |
| Thread pool contention | Medium | Tune pool sizes based on CPU count |
| Breaking API changes | Low | Internal APIs only, no user-facing changes |

---

## Appendix: Code Hotspots

### Processor Hotspots (lines from processor.rs)

| Lines | Function | Time % | Optimization |
|-------|----------|--------|--------------|
| 609-625 | `compact_history_with_atomic_tool_calls` | 12% | Pre-compaction, faster approximation |
| 1113-1427 | Tool execution loop | 25% | Streaming, better parallelism |
| 266-277 | `storage_op` | 8% | True async storage |
| 439-608 | System prompt building | 10% | Component caching |
| 1500-1523 | Final persistence | 5% | Batch writes |

### Event Bus Hotspots (event/mod.rs)

| Lines | Function | Time % | Optimization |
|-------|----------|--------|--------------|
| 697-726 | `publish` | 15% | Arc<Event>, batching |

### Task Manager Hotspots (task/mod.rs)

| Lines | Function | Time % | Optimization |
|-------|----------|--------|--------------|
| 185-311 | `spawn_sync` | 8% | Async session creation |
| 312-508 | `spawn_background` | 6% | Priority queue |

---

## Related Documents

- `docs/performance/benchmark-guide.md` - Running benchmarks
- `CODEINDEX.md` - Codebase navigation
- `crates/ragent-core/benches/` - Existing benchmarks
- `hugplan.md` - Hardware upgrade considerations

---

## Changelog

| Version | Date | Changes |
|---------|------|---------|
| 0.1.0 | 2025-01 | Initial perfplan creation |

---

*End of Performance Plan*
