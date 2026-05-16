# Concurrency Audit Report: Concurrent Agent Execution

**Date:** 2025-01-17
**Scope:** Full workspace review of ragent codebase
**Focus:** Issues impacting concurrent agent execution (multiple sessions, teams, sub-agents)

---

## Executive Summary

The ragent codebase has **8 concurrency issues** that impact concurrent agent execution, ranging from **CRITICAL** to **LOW** severity. The most severe issues are:

1. **Async blocking in HTTP server** — Synchronous SQLite calls block the Tokio async runtime
2. **TOCTOU races in team task/mailbox stores** — Unlocked reads observe truncated/corrupted files
3. **Event loss under load** — Broadcast channel silently drops events when buffer is full

All issues are fixable with targeted changes. No fundamental architecture flaws were found.

---

## Issue Matrix

| # | Severity | Component | Issue | Impact |
|---|----------|-----------|-------|--------|
| 1 | 🔴 **CRITICAL** | `ragent-server` | Sync SQLite in async handlers blocks runtime | All HTTP requests stall during DB ops |
| 2 | 🔴 **CRITICAL** | `ragent-team` | `Mailbox::read_all()` no lock → corrupted reads | Messages lost or garbled |
| 3 | 🟡 **HIGH** | `ragent-team` | `TaskStore::read()` no lock → stale data | Task list empty during updates |
| 4 | 🟡 **HIGH** | `ragent-team` | Mailbox write is non-atomic (truncate-before-write) | Crash leaves mailbox empty |
| 5 | 🟡 **HIGH** | `ragent-types` | Event bus drops events silently (256 buffer) | TUI/SSE misses events under load |
| 6 | 🟡 **HIGH** | `ragent-agent` | Permission checker `RwLock` write blocks all reads | All sessions pause on rule update |
| 7 | 🟢 **MEDIUM** | `ragent-types` | `RwLock` panic poisoning on step map | Thread panic kills event bus |
| 8 | 🟢 **LOW** | `ragent-types` | `intern.rs` is dead code with global `Mutex` | Unused overhead |

---

## 1. 🔴 CRITICAL: Async Blocking in ragent-server Routes

### Description
All HTTP handlers in `ragent-server/src/routes/mod.rs` call synchronous `Storage` methods directly inside `async fn` without wrapping in `tokio::task::spawn_blocking`. This blocks the Tokio async runtime threads, causing all concurrent HTTP requests to stall.

### Affected Code

```rust
// crates/ragent-server/src/routes/mod.rs:219
async fn list_sessions(State(state): State<Arc<AppState>>) -> Result<...> {
    let sessions = state.storage.list_sessions(); // BLOCKS async thread!
    ...
}
```

Full list of call sites:

| Line | Handler | Blocking Call |
|------|---------|---------------|
| 219 | `list_sessions` | `state.storage.list_sessions()` |
| 262 | `create_session` | `state.storage.create_session(...)` |
| 285 | `get_session` | `state.storage.get_session(...)` |
| 299 | `archive_session` | `state.storage.archive_session(...)` |
| 309 | `get_messages` | `state.storage.get_messages(...)` |
| 402 | `abort_session` | `state.storage.get_session(...)` |
| 404 | `abort_session` | `state.storage.archive_session(...)` |
| 600 | `spawn_task` | `state.storage.get_session(...)` |
| 910 | `verify_session_exists` | `state.storage.get_session(...)` |

### Impact
- Under concurrent load (multiple agents via HTTP), all requests serialize on the DB mutex
- The async runtime thread pool is blocked, so even non-DB work stalls
- Server throughput drops to essentially single-threaded

### Correct Pattern Already Exists
`ragent-agent/src/session/processor.rs:498-520` has `storage_op()` which wraps calls correctly:

```rust
async fn storage_op<F, T>(&self, f: F) -> Result<T>
where
    F: FnOnce(&Storage) -> Result<T> + Send + 'static,
    T: Send + 'static,
{
    let storage = self.session_manager.storage().clone();
    tokio::task::spawn_blocking(move || f(&storage)).await?
}
```

### Remediation
Wrap every synchronous storage call in `tokio::task::spawn_blocking` (see Remediation Plan §1).

---

## 2. 🔴 CRITICAL: Mailbox::read_all() Unsynchronized

### Description
`Mailbox::read_all()` in `ragent-team/src/team/mailbox.rs` reads the mailbox JSON file without acquiring any advisory lock, while all write operations (`push`, `drain_unread`, `mark_read`) acquire an exclusive `flock`.

### Affected Code

```rust
// crates/ragent-team/src/team/mailbox.rs:157-167
pub fn read_all(&self) -> Result<Vec<MailboxMessage>> {
    if !self.path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(&self.path)  // NO LOCK!
        .with_context(|| format!("read {}", self.path.display()))?;
    ...
}
```

### Impact
- Concurrent `read_all()` can observe the file after `set_len(0)` but before `write_all()`, seeing an **empty file**
- Can observe **partial/corrupt JSON** mid-write
- Messages appear lost or garbled to teammates

### Remediation
Acquire a shared (`lock_shared`) or exclusive (`lock_exclusive`) advisory lock before reading (see Remediation Plan §2).

---

## 3. 🟡 HIGH: TaskStore::read() Unsynchronized (TOCTOU)

### Description
`TaskStore::read()` in `ragent-team/src/team/task.rs` reads `tasks.json` without acquiring the advisory `flock`, while all mutating operations (`claim_next`, `add_task`, `complete`, etc.) acquire an exclusive lock.

### Affected Code

```rust
// crates/ragent-team/src/team/task.rs:154-165
pub fn read(&self) -> Result<TaskList> {
    if !self.path.exists() {
        return Ok(TaskList::default());
    }
    let raw = fs::read_to_string(&self.path)  // NO LOCK!
        .with_context(|| format!("read {}", self.path.display()))?;
    ...
}
```

### Impact
- `team_task_list` tool returns empty/stale task lists during concurrent updates
- TOCTOU between `path.exists()` check and `read_to_string()` call
- Same empty-file race as mailbox issue

### Remediation
Acquire shared lock in `read()` (see Remediation Plan §2).

---

## 4. 🟡 HIGH: Mailbox Write is Non-Atomic

### Description
`mailbox.rs::write_locked()` truncates the file to zero bytes before writing new content. If the process crashes or is killed between `set_len(0)` and `write_all()`, the mailbox is left empty.

### Affected Code

```rust
// crates/ragent-team/src/team/mailbox.rs:169-176
fn write_locked(file: &mut File, messages: &[MailboxMessage]) -> Result<()> {
    let json = serde_json::to_string_pretty(messages)?;
    file.set_len(0)?;        // ← TRUNCATED HERE
    file.seek(SeekFrom::Start(0))?;
    file.write_all(json.as_bytes())?;  // ← WRITTEN HERE
    file.flush()?;
    Ok(())
}
```

### Impact
- Crash during mailbox update → total message loss for that mailbox
- Same pattern exists in `task.rs::write_locked()`

### Remediation
Write to temp file, then `fs::rename()` atomically (see Remediation Plan §3).

---

## 5. 🟡 HIGH: Event Bus Silently Drops Events

### Description
`EventBus::publish()` uses `tokio::sync::broadcast` with a fixed-size buffer (256). When the buffer is full, oldest events are silently overwritten. Slow subscribers (TUI, SSE) receive `Lagged` errors but events are lost.

### Affected Code

```rust
// crates/ragent-types/src/event/mod.rs:704-731
pub fn publish(&self, event: Event) {
    if self.sender.send(event.clone()).is_err() {
        // Only catches SendError (zero subscribers), NOT overflow
        // ... warning log ...
    }
}
```

### Impact
- Under concurrent agent load, TUI may miss tool call events, permission requests, or stream chunks
- SSE consumers may see gaps in event streams
- Events disappear without publisher knowing

### Remediation
Increase buffer size, add overflow logging, and/or switch to `mpsc` per-subscriber (see Remediation Plan §4).

---

## 6. 🟡 HIGH: Permission Checker Write Lock Contention

### Description
`SessionProcessor` holds `Arc<tokio::sync::RwLock<PermissionChecker>>`. All sessions share one checker. Read-heavy usage is fine, but write locks (e.g., when user grants "Always Allow" via TUI) block all permission checks across all sessions.

### Affected Code

```rust
// crates/ragent-agent/src/session/processor.rs:422
permission_checker: Arc<tokio::sync::RwLock<PermissionChecker>>,
```

### Impact
- Momentary pause in ALL sessions when any session updates permission rules
- Under high concurrency, this creates jitter

### Remediation
Replace with `Arc<RwLock<PermissionChecker>>` using `parking_lot` or `std::sync::RwLock` for faster writes, or use a channel-based update pattern (see Remediation Plan §5).

---

## 7. 🟢 MEDIUM: RwLock Panic Poisoning

### Description
`EventBus::set_step()` and `current_step()` use `.expect("step map poisoned")` on `RwLock` guards. If any thread panics while holding the write lock, all future step operations panic.

### Affected Code

```rust
// crates/ragent-types/src/event/mod.rs:646, 660
let mut map = self.steps.write().expect("step map poisoned");
// ...
self.steps.read().expect("step map poisoned")
```

### Impact
- One thread panicking kills event bus for all sessions
- No recovery path

### Remediation
Use `.unwrap_or_else(|_| ...)` to recover from poisoning, or switch to `parking_lot::RwLock` (which does not poison).

---

## 8. 🟢 LOW: intern.rs is Dead Code

### Description
`crates/ragent-types/src/intern.rs` contains a global `Mutex<StringInterner>` that is never used in production code. Only called from its own unit tests.

### Affected Code

```rust
// crates/ragent-types/src/intern.rs:24-25
static INTERNER: Lazy<Mutex<StringInterner<DefaultBackend>>> =
    Lazy::new(|| Mutex::new(StringInterner::new()));
```

### Impact
- None in production, but adds compilation overhead and dead code

### Remediation
Delete the module (see Remediation Plan §6).

---

## Remediation Plan

### §1. Fix Async Blocking in ragent-server (CRITICAL)

**Files:** `crates/ragent-server/src/routes/mod.rs`

For each handler, wrap storage calls:

```rust
// BEFORE (blocking):
let sessions = state.storage.list_sessions();

// AFTER (non-blocking):
let storage = state.storage.clone();
let sessions = tokio::task::spawn_blocking(move || storage.list_sessions())
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))??;
```

Affected handlers: `list_sessions`, `create_session`, `get_session`, `archive_session`, `get_messages`, `abort_session`, `spawn_task`, `verify_session_exists`.

**Alternative:** Add `read_async`/`write_async` helpers to `Storage` itself, so callers don't need to wrap each call.

**Priority:** P0 (blocks concurrent execution)
**Estimated Effort:** 2 hours

---

### §2. Fix TOCTOU in TaskStore and Mailbox (CRITICAL / HIGH)

**Files:**
- `crates/ragent-team/src/team/task.rs`
- `crates/ragent-team/src/team/mailbox.rs`

For `TaskStore::read()`:
```rust
pub fn read(&self) -> Result<TaskList> {
    let file = fs::OpenOptions::new()
        .read(true)
        .open(&self.path)?;
    file.lock_shared()?;  // or lock_exclusive on platforms without shared locks
    let raw = fs::read_to_string(&self.path)?;
    file.unlock()?;
    ...
}
```

For `Mailbox::read_all()`:
```rust
pub fn read_all(&self) -> Result<Vec<MailboxMessage>> {
    let file = fs::OpenOptions::new()
        .read(true)
        .open(&self.path)?;
    file.lock_shared()?;
    let raw = fs::read_to_string(&self.path)?;
    file.unlock()?;
    ...
}
```

**Priority:** P0
**Estimated Effort:** 1 hour

---

### §3. Atomic File Writes for Mailbox and TaskStore (HIGH)

**Files:**
- `crates/ragent-team/src/team/task.rs`
- `crates/ragent-team/src/team/mailbox.rs`

Replace `write_locked` with atomic write-then-rename:

```rust
fn write_atomic(path: &Path, messages: &[MailboxMessage]) -> Result<()> {
    let json = serde_json::to_string_pretty(messages)?;
    let temp_path = path.with_extension("tmp");
    fs::write(&temp_path, json)?;
    fs::rename(&temp_path, path)?;  // Atomic on POSIX and Windows
    Ok(())
}
```

Keep the `flock` around the entire read-modify-rename cycle.

**Priority:** P1
**Estimated Effort:** 1 hour

---

### §4. Fix Event Bus Event Loss (HIGH)

**File:** `crates/ragent-types/src/event/mod.rs`

Option A — Increase buffer and log overflow:
```rust
let (sender, _) = broadcast::channel(1024);  // Was 256
```

Option B — Detect and log when receivers lag:
```rust
pub fn publish(&self, event: Event) {
    match self.sender.send(event.clone()) {
        Ok(n) => { /* n = number of receivers */ }
        Err(_) => {
            tracing::warn!("Event dropped: no subscribers");
        }
    }
}
```

Option C (best long-term) — Switch to per-subscriber `mpsc` channels with a distributor task, so each subscriber has its own backpressure handling.

**Priority:** P1
**Estimated Effort:** 2 hours (Option A+B), 1 day (Option C)

---

### §5. Reduce Permission Checker Lock Contention (HIGH)

**File:** `crates/ragent-agent/src/session/processor.rs`

Option A — Use `parking_lot::RwLock` (faster, no poisoning):
```rust
permission_checker: Arc<parking_lot::RwLock<PermissionChecker>>,
```

Option B — Channel-based updates (no write locks during normal operation):
- Permission checker uses `Arc<ArcSwap<PermissionChecker>>` or `Arc<RwLock<>>` with rule updates sent via a channel
- Normal reads are lock-free

**Priority:** P2
**Estimated Effort:** 2 hours

---

### §6. Remove Dead intern.rs Code (LOW)

**File:** `crates/ragent-types/src/intern.rs`

Delete the file and remove its re-export from `lib.rs`.

**Priority:** P3
**Estimated Effort:** 15 minutes

---

## Verification Checklist

After remediation, verify:

- [ ] `cargo test` passes across all crates
- [ ] `cargo test` in `ragent-server` still passes
- [ ] `cargo test` in `ragent-team` still passes
- [ ] HTTP server handles concurrent requests without blocking (test with `ab` or `wrk`)
- [ ] Team task list shows correct data during concurrent claim/complete operations
- [ ] Mailbox messages are not lost during concurrent read/write
- [ ] Event bus does not drop events under sustained load

---

## Appendix: Call Chain Analysis

### Concurrent Execution Paths

```
User Input → HTTP Server → routes/mod.rs → processor.rs → process_message()
    │
    ├── Single session: sequential processing (OK)
    ├── Multiple sessions via HTTP: compete for Storage Mutex (BLOCKS)
    ├── Teams: TaskStore/ Mailbox file races (CORRUPTS)
    └── Sub-agents: EventBus drops events (LOSES)
```

### Storage Access Patterns

| Component | Sync/Async | Wraps in spawn_blocking? |
|-----------|------------|--------------------------|
| `processor.rs` | Async | ✅ Yes (`storage_op()`) |
| `routes/mod.rs` | Async | ❌ No (CRITICAL) |
| `app.rs` | Async | ❌ No (HIGH) |
| `storage.rs` itself | Sync | N/A (library) |

---

*Report generated by concurrency audit sub-agents. All line numbers reference commit 40e1a50.*
