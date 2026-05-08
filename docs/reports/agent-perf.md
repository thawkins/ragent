# Performance Remediation Plan: ragent-agent Crate

**Document Version:** 1.0  
**Last Updated:** 2025-01-18  
**Synthesized From:**
- s1: Agent Loop Performance Review (agent_loop_performance_review.md)
- s2: Async Runtime Analysis (async_runtime_analysis_ragent_agent.md)
- s3: Algorithmic Complexity Analysis (algorithmic_complexity_analysis.md)
- s4: Concurrency Analysis (concurrency_analysis_ragent_agent.md)
- s5: Memory Allocation Analysis (ragent-agent-memory-analysis.md)

---

## 1. Executive Summary

### Overview

The ragent-agent crate contains **27+ identified performance issues** across five categories: Async Runtime, Memory Allocation, Algorithmic Complexity, Concurrency, and Agent Loop Efficiency. Under high load or long-running sessions, these issues compound to cause:

- **Blocking I/O in async contexts** causing runtime thread starvation
- **O(n²) message compaction** creating exponential slowdown with history growth
- **Repeated glob pattern compilation** wasting 500ms-2s per session
- **Lock contention hotspots** reducing throughput under concurrent load
- **Excessive cloning** in retry loops creating GC pressure

### Top 3 Critical Issues

| Priority | Issue | Location | Impact |
|----------|-------|----------|--------|
| **P0** | O(n²) Message Compaction Algorithm | `src/session/processor.rs:2575-2590` | With 1000 messages, causes ~250,000 element moves |
| **P0** | Blocking File I/O in Async Context | `src/session/processor.rs:2780` | Blocks runtime thread for large image files |
| **P0** | Repeated Glob Pattern Compilation | `src/session/processor.rs:316-341` | 1000 pattern compilations per 100 tool calls |

### Estimated Performance Impact

| Metric | Current | After Fixes | Improvement |
|--------|---------|-------------|-------------|
| Session processing latency | Baseline | -600ms to -3s | 20-40% reduction |
| Memory allocation rate | Baseline | -30% | Significant GC pressure reduction |
| Concurrent session throughput | Baseline | +20-40% | Better lock-free patterns |
| Long session stability | Degrades with history | Flat | O(n) vs O(n²) scaling |

---

## 2. Performance Categories

### 2.1 Async Runtime Issues

**Severity Distribution:** 2 HIGH, 1 MEDIUM, 1 LOW

#### 2.1.1 HIGH: Blocking File I/O in Image Processing

**Location:** `crates/ragent-agent/src/session/processor.rs:2780`

**Current Code:**
```rust
// In parts_to_chat_content() function:
match std::fs::read(path) {  // BLOCKING!
    Ok(bytes) => {
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        Some(ContentPart::ImageUrl {
            url: format!("data:{mime_type};base64,{b64}"),
        })
    }
    // ...
}
```

**Problem:** Direct `std::fs::read()` call inside an async function blocks the runtime thread while reading potentially large image files.

**Remediation:**
```rust
// Wrap in spawn_blocking:
let path_clone = path.clone();
let bytes = tokio::task::spawn_blocking(move || std::fs::read(&path_clone)).await?;
let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
```

---

#### 2.1.2 HIGH: Blocking File I/O in Tool Execution

**Locations:**
- `tool/bash.rs:716` - Writing script files
- `tool/pdf_read.rs:105` - Reading potentially large PDF files
- `tool/memory_write.rs:560` - Reading memory files
- `tool/glob.rs:131` - `std::fs::read_dir(dir)`
- `tool/list.rs:102` - `std::fs::read_dir(dir)`

**Remediation:** Wrap all blocking I/O in `tokio::task::spawn_blocking` or use `tokio::fs` where available:
```rust
// Before:
std::fs::read_to_string(&path)?

// After:
tokio::task::spawn_blocking(move || std::fs::read_to_string(&path))
    .await?
    .map_err(|e| e.into())
```

---

#### 2.1.3 MEDIUM: Unbounded Channel in EventBus

**Location:** `crates/ragent-agent/src/event/mod.rs`

**Problem:** Broadcast channel capacity of 16 may cause message lagging under burst load.

**Remediation:**
```rust
// Consider:
let (tx, _rx) = broadcast::channel(1024); // Increase capacity
// Or implement backpressure monitoring
```

---

### 2.2 Memory Allocation Issues

**Severity Distribution:** 8 HIGH, 8 MEDIUM, 5 LOW

#### 2.2.1 HIGH: System Prompt Clone in Retry Loop

**Location:** `crates/ragent-agent/src/session/processor.rs:1118`

**Current Code:**
```rust
let attempt_request = ChatRequest {
    model: model_ref.model_id.clone(),
    messages: chat_messages.clone(),  // HIGH: cloned every retry
    tools: (*tool_definitions).clone(), // HIGH: cloned every retry
    system: Some(system_prompt.clone()), // MEDIUM: cloned every retry
    // ...
};
```

**Remediation:**
```rust
// Pre-clone once before retry loop
let chat_messages = Arc::new(chat_messages);
let tool_definitions = Arc::new(tool_definitions);

// In retry loop - cheap Arc clones
let attempt_request = ChatRequest {
    messages: (*chat_messages).clone(),
    tools: (*tool_definitions).clone(),
    // ...
};
```

---

#### 2.2.2 HIGH: Large Enum Variants Causing Wasted Space

**Location:** `crates/ragent-agent/src/message/mod.rs:107-140`

**Current Code:**
```rust
pub enum MessagePart {
    Text { text: String },                    // ~24 bytes
    ToolCall { tool: String, call_id: String, state: ToolCallState }, // ~72 bytes
    Reasoning { text: String },              // ~24 bytes
    Image { mime_type: String, path: PathBuf }, // ~56 bytes (largest)
}
```

**Remediation:**
```rust
pub enum MessagePart {
    Text { text: String },
    ToolCall { tool: String, call_id: String, state: ToolCallState },
    Reasoning { text: String },
    Image(Box<ImageData>),  // Box the large variant
}

pub struct ImageData {
    mime_type: String,
    path: PathBuf,
}
```

**Impact:** ~30% memory reduction for message storage.

---

#### 2.2.3 HIGH: Heap Allocations in Hot Loops

**Location:** `crates/ragent-agent/src/session/processor.rs:1988-2000`

**Current Code:**
```rust
chat_messages.push(ChatMessage {
    role: "assistant".to_string(),  // Allocates
    content: ChatContent::Parts(assistant_content_parts),
});
chat_messages.push(ChatMessage {
    role: "user".to_string(),  // Allocates
    content: ChatContent::Parts(tool_result_parts),
});
```

**Remediation:**
```rust
// Use static strings or Arc<str>
const ROLE_ASSISTANT: &str = "assistant";
const ROLE_USER: &str = "user";

chat_messages.push(ChatMessage {
    role: ROLE_ASSISTANT.into(), // Or Arc<str>
    // ...
});
```

---

#### 2.2.4 MEDIUM: Tool Definitions Cloned on Every Request

**Location:** `crates/ragent-agent/src/session/processor.rs:986-988`

**Current Code:**
```rust
let tool_definitions: std::sync::Arc<Vec<ToolDefinition>> = std::sync::Arc::new(if max_steps <= 1 {
    Vec::new()
} else {
    self.tool_registry.definitions() // Creates new Vec each call
});
```

**Remediation:** Cache the `Arc<Vec<ToolDefinition>>` in the processor and only regenerate when tools change.

---

### 2.3 Algorithmic Complexity Issues

**Severity Distribution:** 3 Critical, 6 High, 5 Medium

#### 2.3.1 CRITICAL: O(n²) Message Compaction Algorithm

**Location:** `src/session/processor.rs:2530-2614`

**Current Code:**
```rust
while current_tokens > max_tokens && trimmed.len() > 2 {
    let to_remove = 0; // Try removing the oldest message
    let removed_tokens = estimate_tokens(&trimmed[to_remove]);
    trimmed.remove(to_remove);  // O(n) operation!
    current_tokens -= removed_tokens;
}
```

**Problem:** Each `trimmed.remove(0)` call shifts all remaining n elements, creating O(n²) total complexity.

**Remediation:**
```rust
// Option 1: Use swap_remove for O(1) amortized removal
if let Some(msg) = trimmed.swap_remove(to_remove) {
    current_tokens -= estimate_tokens(&msg);
}

// Option 2: Use VecDeque for O(1) removal from front
let mut trimmed: VecDeque<Message> = messages.iter().cloned().collect();
while current_tokens > max_tokens && trimmed.len() > 2 {
    if let Some(msg) = trimmed.pop_front() {  // O(1)
        current_tokens -= estimate_tokens(&msg);
    }
}
let trimmed: Vec<Message> = trimmed.into_iter().collect();
```

---

#### 2.3.2 CRITICAL: Linear Permission Rule Evaluation

**Location:** `src/permission/mod.rs:298-307`

**Current Code:**
```rust
pub fn check(&self, permission: &str, resource: &str) -> PermissionAction {
    let mut action = PermissionAction::Ask;
    for rule in &self.ruleset {  // O(rules) for every check
        if rule.matches(permission, resource) {
            action = rule.action.clone();
        }
    }
    action
}
```

**Problem:** With 100 permission rules, every tool call requires 100 string comparisons and glob matches.

**Remediation:**
```rust
// Index rules by permission type
pub struct PermissionChecker {
    rules_by_permission: HashMap<String, Vec<PermissionRule>>,
    // ...
}

pub fn check(&self, permission: &str, resource: &str) -> PermissionAction {
    if let Some(rules) = self.rules_by_permission.get(permission) {
        for rule in rules {  // Only check relevant rules
            if rule.matches(permission, resource) {
                return rule.action.clone();
            }
        }
    }
    PermissionAction::Ask
}
```

---

#### 2.3.3 HIGH: Repeated Glob Pattern Compilation

**Location:** `crates/ragent-agent/src/session/processor.rs:316-341`

**Current Code:**
```rust
for pattern in get_denylist() {  // Called every tool execution
    if let Ok(glob) = globset::Glob::new(&pattern) {  // Compiled here
        if glob.compile_matcher().is_match(resource) {
            return Ok(PermissionAction::Deny);
        }
    }
}
```

**Remediation:**
```rust
// In dir_lists.rs - store compiled patterns
use globset::{GlobSet, GlobSetBuilder};

pub struct CompiledDirLists {
    allowlist: GlobSet,
    denylist: GlobSet,
}

// Compile once when loading config
pub fn load_from_config() {
    let allow_set = compile_patterns(&allowlist);
    let deny_set = compile_patterns(&denylist);
    // Store CompiledDirLists instead of raw strings
}

// In processor.rs - use is_match() directly
if compiled_lists.denylist.is_match(resource) {
    return Ok(PermissionAction::Deny);
}
```

---

#### 2.3.4 HIGH: File Read Tool with Multiple Line Iterations

**Location:** `src/tool/read.rs:320-570`

**Problem:** Same file content iterated ~10 times for different processing stages.

**Remediation:** Refactor to single-pass line processing or cache parsed results between stages.

---

### 2.4 Concurrency Issues

**Severity Distribution:** 4 Medium, 8 Low

#### 2.4.1 MEDIUM: SessionProcessor Heavy RwLock Usage

**Location:** `session/processor.rs:422-440`

**Current Code:**
```rust
pub permission_checker: Arc<tokio::sync::RwLock<PermissionChecker>>,
pub event_bus: Arc<EventBus>,
pub mcp_client: std::sync::OnceLock<Arc<tokio::sync::RwLock<crate::mcp::McpClient>>>,
// ... 7 Arc-wrapped shared resources
```

**Problem:** Creates a "lock sandwich" pattern where multiple locks may need to be acquired during a single operation.

**Remediation:**
```rust
// Replace hot-path RwLock<HashMap> with DashMap
use dashmap::DashMap;

pub struct SessionProcessor {
    permission_checker: Arc<DashMap<String, PermissionAction>>,  // Lock-free
    // ...
}
```

---

#### 2.4.2 MEDIUM: EventBus RwLock<HashMap> Step Counter

**Location:** `event/mod.rs:506-514`

**Current Code:**
```rust
#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<Event>,
    steps: Arc<RwLock<HashMap<String, u64>>>,  // Write lock on every event
}
```

**Remediation:**
```rust
use dashmap::DashMap;
use std::sync::atomic::AtomicU64;

pub struct EventBus {
    sender: broadcast::Sender<Event>,
    steps: Arc<DashMap<String, AtomicU64>>,  // Lock-free concurrent access
}
```

---

#### 2.4.3 MEDIUM: Storage Mutex<Connection> Blocking

**Location:** `storage/mod.rs:206, 243, 266`

**Current Code:**
```rust
pub struct Storage {
    conn: Mutex<Connection>,  // All DB ops serialize through single Mutex
}
```

**Remediation:** Consider connection pooling or WAL mode with concurrent reads.

---

#### 2.4.4 MEDIUM: AgentLoopProfiler RwLock on Hot Path

**Location:** `session/profiler.rs:54-60, 133-145`

**Current Code:**
```rust
pub fn record_duration(&self, label: &str, duration: Duration) {
    if let Ok(mut stats) = self.stats.write() {  // Write lock on hot path
        let entry = stats.entry(label.to_string()).or_default();
        entry.count += 1;
        // ...
    }
}
```

**Remediation:** Use thread-local aggregation or a lock-free histogram (e.g., `metrics` crate).

---

### 2.5 Redundant Work Patterns

#### 2.5.1 Config Loading on Every Message

**Location:** `src/session/processor.rs:698-700`

**Current Code:**
```rust
let session_config = {
    let _scope = profiler.scope("config.load");
    crate::Config::load().unwrap_or_default()  // Parses JSON on EVERY message!
};
```

**Remediation:** Cache config and watch for file changes using `notify` crate.

---

#### 2.5.2 Skill Registry Reloaded Every Message

**Location:** `src/session/processor.rs:726-730`

**Current Code:**
```rust
let skill_registry = {
    let _scope = profiler.scope("skills.load_registry");
    crate::skill::SkillRegistry::load(&working_dir, &skill_dirs)  // O(n) file operations
};
```

**Remediation:** Cache registry with mtime-based invalidation.

---

#### 2.5.3 Context Collection on Every Message

**Location:** `src/session/processor.rs:731-734`

**Current Code:**
```rust
let (git_status, readme, agents_md, file_tree) = {
    let _scope = profiler.scope("prompt.collect_context");
    crate::agent::collect_prompt_context(&working_dir).await  // File operations!
};
```

**Remediation:** Cache context with file watcher invalidation.

---

## 3. Prioritized Remediation Roadmap

### Phase 1: Immediate (Week 1) — Critical Fixes

| Task | File | Lines | Effort | Expected Impact |
|------|------|-------|--------|-----------------|
| Fix O(n²) message compaction | `session/processor.rs` | 2575-2590 | 4h | Eliminates exponential slowdown |
| Add spawn_blocking to image I/O | `session/processor.rs` | 2780 | 2h | Prevents runtime blocking |
| Cache compiled glob patterns | `config/dir_lists.rs` | New | 6h | Saves 500ms-2s per session |
| Index permission rules | `permission/mod.rs` | 298-307 | 4h | O(n) → O(1) rule lookup |

**Total Phase 1 Effort:** ~2 days  
**Expected Improvement:** 30-50% latency reduction for long sessions

---

### Phase 2: Medium Priority (Weeks 2-3) — Memory & Concurrency

| Task | File | Lines | Effort | Expected Impact |
|------|------|-------|--------|-----------------|
| Box large MessagePart variants | `message/mod.rs` | 107-140 | 4h | 30% memory reduction |
| Arc-wrap chat_messages in retry | `session/processor.rs` | 1118 | 2h | Reduce GC pressure |
| Replace RwLock<HashMap> with DashMap | `event/mod.rs`, `orchestrator/registry.rs` | Multiple | 8h | 20-40% contention reduction |
| Cache tool_definitions | `session/processor.rs` | 986-988 | 2h | ~5-10KB per request |
| Fix blocking Mutex in async contexts | `agent/mod.rs`, `internal_llm/mod.rs` | Multiple | 4h | Prevents thread starvation |
| Pre-size vectors in hot loops | `session/processor.rs`, `agent/mod.rs` | Multiple | 2h | Minor allocation reduction |

**Total Phase 2 Effort:** ~4 days  
**Expected Improvement:** 20-30% memory reduction, improved concurrency

---

### Phase 3: Low Priority (Weeks 4-6) — Polish & Monitoring

| Task | File | Lines | Effort | Expected Impact |
|------|------|-------|--------|-----------------|
| Cache config loading | `session/processor.rs` | 698-700 | 4h | Remove per-message file ops |
| Cache skill registry | `session/processor.rs` | 726-730 | 4h | Remove per-message file ops |
| Cache prompt context | `session/processor.rs` | 731-734 | 6h | Remove per-message file ops |
| Implement LRU eviction | `predictive.rs` | 104-116 | 4h | Better cache hit rates |
| Add lock contention metrics | Various | New | 8h | Ongoing monitoring |
| Add #[inline] to hot functions | Various | Multiple | 2h | Minor speedup |

**Total Phase 3 Effort:** ~1 week  
**Expected Improvement:** Reduced file I/O, better observability

---

## 4. Specific Code Refactoring Examples

### 4.1 Before/After: O(n²) Message Compaction

**Before:**
```rust
// O(n²) - DO NOT USE
while current_tokens > max_tokens && trimmed.len() > 2 {
    let removed_tokens = estimate_tokens(&trimmed[0]);
    trimmed.remove(0);  // O(n) - shifts all elements!
    current_tokens -= removed_tokens;
}
```

**After:**
```rust
// O(n) - RECOMMENDED
let mut trimmed: VecDeque<Message> = messages.iter().cloned().collect();
while current_tokens > max_tokens && trimmed.len() > 2 {
    if let Some(msg) = trimmed.pop_front() {  // O(1)
        current_tokens -= estimate_tokens(&msg);
    }
}
let trimmed: Vec<Message> = trimmed.into_iter().collect();
```

---

### 4.2 Before/After: Glob Pattern Caching

**Before:**
```rust
// Compiles patterns on every check
for pattern in get_denylist() {
    if let Ok(glob) = globset::Glob::new(&pattern) {  // Expensive!
        if glob.compile_matcher().is_match(resource) {
            return Ok(PermissionAction::Deny);
        }
    }
}
```

**After:**
```rust
// Compile once, reuse everywhere
pub struct CompiledDirLists {
    allowlist: GlobSet,
    denylist: GlobSet,
}

impl CompiledDirLists {
    pub fn is_allowed(&self, resource: &str) -> bool {
        self.allowlist.is_match(resource)  // No recompilation!
    }
}
```

---

### 4.3 Before/After: RwLock to DashMap Migration

**Before:**
```rust
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;

pub struct EventBus {
    steps: Arc<RwLock<HashMap<String, u64>>>,
}

impl EventBus {
    pub async fn increment_step(&self, key: &str) {
        let mut steps = self.steps.write().await;  // Blocks writers
        *steps.entry(key.to_string()).or_insert(0) += 1;
    }
}
```

**After:**
```rust
use std::sync::Arc;
use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct EventBus {
    steps: Arc<DashMap<String, AtomicU64>>,  // Lock-free!
}

impl EventBus {
    pub fn increment_step(&self, key: &str) {
        self.steps
            .entry(key.to_string())
            .or_default()
            .fetch_add(1, Ordering::Relaxed);  // No await needed!
    }
}
```

---

### 4.4 Before/After: Blocking I/O in Async Context

**Before:**
```rust
async fn process_image(path: &Path) -> Option<ContentPart> {
    match std::fs::read(path) {  // BLOCKS!
        Ok(bytes) => { /* ... */ }
        Err(_) => None,
    }
}
```

**After:**
```rust
async fn process_image(path: &Path) -> Option<ContentPart> {
    let path = path.to_owned();
    match tokio::task::spawn_blocking(move || std::fs::read(&path))
        .await
        .ok()
        .flatten()
    {
        Ok(bytes) => { /* ... */ }
        Err(_) => None,
    }
}
```

---

## 5. Benchmarking Recommendations

### 5.1 Benchmark Suite

Add the following Criterion benchmarks to validate fixes:

```rust
// benches/session_processing.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_message_compaction(c: &mut Criterion) {
    let messages = generate_test_messages(1000);
    c.bench_function("compact_1000_messages", |b| {
        b.iter(|| compact_history(black_box(&messages)))
    });
}

fn bench_permission_check(c: &mut Criterion) {
    let checker = PermissionChecker::with_test_rules(100);
    c.bench_function("check_100_rules", |b| {
        b.iter(|| checker.check(black_box("file:write"), black_box("/path")))
    });
}

fn bench_glob_matching(c: &mut Criterion) {
    let patterns = vec!["src/**/*.rs", "tests/**/*.rs"];
    c.bench_function("glob_compile_and_match", |b| {
        b.iter(|| check_glob_patterns(black_box(&patterns), black_box("/path")))
    });
}

criterion_group!(benches, bench_message_compaction, bench_permission_check, bench_glob_matching);
criterion_main!(benches);
```

### 5.2 Profiling Tools

| Tool | Command | Use Case |
|------|---------|----------|
| DHAT | `valgrind --tool=dhat target/debug/ragent` | Heap allocation analysis |
| heaptrack | `heaptrack target/debug/ragent` | Memory usage over time |
| cargo-flamegraph | `cargo flamegraph --bin ragent` | CPU profiling |
| perf | `perf record -g ./target/release/ragent` | Linux profiling |
| tokio-console | `tokio-console` | Async runtime introspection |

### 5.3 Load Testing

```bash
# Concurrent session test
wrk -t4 -c100 -d30s --latency http://localhost:9100/health

# Memory stress test
# Run 1000 iterations of agent loop with large message history
cargo test --release --test agent_stress -- --nocapture
```

### 5.4 Validation Checklist

- [ ] Message compaction scales linearly (O(n)) with history size
- [ ] No blocking `std::fs` calls in async contexts
- [ ] Glob patterns compiled once per config load
- [ ] Permission checks use O(1) indexed lookup
- [ ] Memory usage flat during long sessions
- [ ] Lock contention <5% under concurrent load
- [ ] No cloning in retry loops

---

## 6. Dependencies to Add

```toml
# Cargo.toml additions
[dependencies]
dashmap = "6.0"           # Lock-free concurrent HashMap
# Already present: globset
globset = "0.4"

[dev-dependencies]
criterion = "0.5"       # Benchmarking framework
heaptrack = "0.14"        # Memory profiling (optional)

[[bench]]
name = "session_processing"
harness = false
```

---

## 7. Risk Assessment

| Risk | Mitigation |
|------|------------|
| DashMap migration may introduce subtle bugs | Keep RwLock fallback behind feature flag |
| VecDeque changes message ordering | Thorough unit tests, gradual rollout |
| spawn_blocking adds overhead | Only for I/O >1ms, profile before/after |
| Caching may serve stale data | Implement file watcher invalidation |

---

## 8. Success Metrics

| Metric | Baseline | Target | Measurement |
|--------|----------|--------|-------------|
| Session latency (p99) | TBD | -30% | Criterion benchmarks |
| Memory allocation rate | TBD | -30% | DHAT/heaptrack |
| Concurrent session throughput | TBD | +30% | Load testing |
| Lock contention time | TBD | <5% | tokio-console |
| Message compaction time | O(n²) | O(n) | Custom benchmark |

---

## 9. Appendix: Complete File/Line Reference

### Async Runtime Issues
| File | Lines | Issue |
|------|-------|-------|
| `session/processor.rs` | 2780 | Blocking file read for images |
| `tool/bash.rs` | 716 | Blocking script file write |
| `tool/pdf_read.rs` | 105 | Blocking PDF read |
| `tool/memory_write.rs` | 560 | Blocking memory file read |
| `tool/glob.rs` | 131 | Blocking read_dir |
| `tool/list.rs` | 102 | Blocking read_dir |
| `event/mod.rs` | 156 | Broadcast channel capacity |

### Memory Issues
| File | Lines | Issue |
|------|-------|-------|
| `session/processor.rs` | 1118 | chat_messages.clone() in retry |
| `session/processor.rs` | 986 | tool_definitions.clone() |
| `session/processor.rs` | 577 | user_msg.clone() |
| `message/mod.rs` | 107-140 | Large MessagePart variants |
| `memory/compact.rs` | 38-69 | Large DedupResult variants |
| `skill/invoke.rs` | 166-170 | format_skill_message clone |

### Algorithmic Issues
| File | Lines | Issue |
|------|-------|-------|
| `session/processor.rs` | 2575-2590 | O(n²) message compaction |
| `session/processor.rs` | 298-307 | Linear permission scan |
| `session/processor.rs` | 316-341 | Repeated glob compilation |
| `session/processor.rs` | 698-700 | Config reloaded per message |
| `session/processor.rs` | 726-730 | Skills reloaded per message |
| `tool/read.rs` | 320-570 | 10+ line iterations |
| `tool/memory_search.rs` | 200-280 | Multiple result scans |

### Concurrency Issues
| File | Lines | Issue |
|------|-------|-------|
| `session/processor.rs` | 422-440 | Multiple RwLock fields |
| `event/mod.rs` | 506-514 | RwLock<HashMap> step counter |
| `storage/mod.rs` | 206, 243, 266 | Mutex<Connection> blocking |
| `orchestrator/registry.rs` | 58, 129-145 | RwLock across iteration |
| `task/mod.rs` | 152-163 | Double RwLock pattern |
| `team/manager.rs` | 25, 48-60 | Mixed sync primitives |
| `session/profiler.rs` | 54-60, 133-145 | RwLock on hot path |
| `session/cache.rs` | 104-140 | Multiple Mutex fields |
| `agent/mod.rs` | 37-43 | std::sync::Mutex in async |
| `internal_llm/mod.rs` | 366-367, 378-412 | Blocking Mutex |

---

*End of Performance Remediation Plan*
