# ragent-team Performance Review

**Reviewer:** swarm-s2 (swarm task `s2`)  
**Scope:** Every source file in `crates/ragent-team/` — `lib.rs`, `tool.rs`, `tools/mod.rs`, `team/mod.rs`, `team/config.rs`, `team/mailbox.rs`, `team/store.rs`, `team/task.rs`, `team/manager.rs`, `team/swarm.rs`, `team/classify.rs`, and all 20 tool files under `tools/`.

---

## Summary

The `ragent-team` crate implements multi-agent coordination via a **file-backed store** model: `config.json`, `tasks.json`, and per-agent `mailbox/{agent_id}.json` are read/written with `flock`-based locking and atomic rename. This is a pragmatic choice for crash-safety and cross-process consistency, but it creates several **recurring performance patterns** that scale poorly as team sizes and message volumes grow. Below are 18 findings grouped by theme.

---

## 1. Blocking File I/O on Async Paths (HIGH — pervasive)

### Issue 1.1 — `Mailbox::push`, `read_all`, `drain_unread`, `peek_unread`, `mark_read` use synchronous `std::fs` inside async tool calls

| | |
|---|---|
| **File / Lines** | `crates/ragent-team/src/team/mailbox.rs` — `push` (304–327), `read_all` (243–269), `peek_unread` (341–380), `drain_unread` (381–422), `mark_read` (437–471) |
| **Severity** | **High** |
| **Description** | Every mailbox operation performs synchronous `fs::read_to_string`, `serde_json::from_str`, `serde_json::to_string_pretty`, `fs::write`, and `fs::rename` **inside async tool `execute()` methods**. The tokio runtime thread is blocked for the duration of each I/O call. Under concurrent teammate activity (each teammate polls its mailbox every 500 ms and the lead calls `team_read_messages`), multiple tokio worker threads can be simultaneously stalled on disk I/O, starving other async tasks. |
| **Fix** | Wrap the entire read-modify-write cycle in `tokio::task::spawn_blocking` so the async executor is not blocked. Alternatively, migrate to `tokio::fs` for file reads/writes and perform deserialization on the blocking pool. The highest-leverage change is to move `Mailbox::push` and `Mailbox::drain_unread` to `spawn_blocking` since they hold an exclusive `flock` **and** do full-file serialization. |

### Issue 1.2 — `TaskStore` mutating operations (`claim_next`, `claim_specific`, `complete`, `add_task`, `pre_assign_task`, `update_task`, `remove_task`) all use synchronous `std::fs` inside async tool calls

| | |
|---|---|
| **File / Lines** | `crates/ragent-team/src/team/task.rs` — lines 349–397, 398–476, 484–550, 559–582, 585–629, 632–668, 671–696 |
| **Severity** | **High** |
| **Description** | Every task mutation does: acquire exclusive `flock` → `fs::read_to_string` → `serde_json::from_str` → mutate → `serde_json::to_string_pretty` → `fs::write` → `fs::sync_all` → `fs::rename`. All synchronous, all inside `async fn execute()` of the team tools. This is the **N+1 bottleneck** for task operations — each `team_task_claim` and `team_task_complete` round-trips the entire `tasks.json` through serialize/deserialize while holding a file lock. |
| **Fix** | Same as 1.1: `spawn_blocking` for the read-modify-write cycle. Additionally, the repeated `fs::read_to_string` + `serde_json::from_str` on every single mutation means the entire task list is deserialized on every operation; see Issue 4.1 for caching opportunities. |

### Issue 1.3 — `TeamStore::load`, `TeamStore::save`, and directory discovery (`find_team_dir`, `find_project_teams_dir`) are synchronous and called on every tool invocation

| | |
|---|---|
| **File / Lines** | `crates/ragent-team/src/team/store.rs` — `load` (190–203), `save` (216–256), `find_team_dir` (52–68), `find_project_teams_dir` (33–45) |
| **Severity** | **High** |
| **Description** | `TeamStore::load` acquires a shared `flock`, reads `config.json`, deserializes it, and drops the lock — synchronously. `save` clones the config, stamps timestamps, serializes, writes a temp file, `sync_all`, and renames — synchronously. These are called from nearly every tool's `execute()` method (often multiple times per call — see Issue 3.1). The `find_project_teams_dir` walks up the directory tree calling `candidate.is_dir()` on every parent, which is a `stat()` syscall per level. |
| **Fix** | (a) Move `load`/`save` into `spawn_blocking`. (b) Cache the resolved `team_dir` path in `ToolContext` (or on `TeamManager`) so the directory walk is not repeated per tool call. (c) Consider an in-memory config cache with file-watcher invalidation instead of full re-read on every operation. |

---

## 2. Redundant Config Loads / N+1 Patterns (HIGH)

### Issue 3.1 — `team_task_claim` loads `TeamStore` 3–4 times per invocation

| | |
|---|---|
| **File / Lines** | `crates/ragent-team/src/tools/team_task_claim.rs` — lines 65–88 (debug read), 98–100 (lead_sid load), 110–115 (current_task_id update load+save), 206–222 (duplicate for `claim_next` path) |
| **Severity** | **High** |
| **Description** | A single `team_task_claim` call performs: (1) `TaskStore::read()` for debug logging, (2) `TaskStore::claim_next/claim_specific` which itself does a full read-modify-write of `tasks.json`, (3) `TeamStore::load()` to get `lead_session_id`, (4) `TeamStore::load()` + `save()` to update `current_task_id`. That is **5 file operations** (2 task reads, 1 task write, 2 config reads, 1 config write) for a single claim. The `lead_session_id` is available from `ctx` or the team manager and does not need a disk read. |
| **Fix** | (a) Remove the debug `store.read()` at lines 65–88 or gate it behind `tracing::enabled!(Level::DEBUG)`. (b) Get `lead_session_id` from the in-memory `TeamManager` or `ToolContext` rather than loading `config.json`. (c) Combine the `current_task_id` update with the claim write (single write to `tasks.json` + single write to `config.json`). |

### Issue 3.2 — `team_task_complete` publishes `TeamTaskCompleted` event **twice** and loads `TeamStore` 3+ times

| | |
|---|---|
| **File / Lines** | `crates/ragent-team/src/tools/team_task_complete.rs` — lines 79–103 (debug read), 172–181 (first event publish + lead_sid load), 183–195 (**duplicate** event publish + lead_sid load), 199–204 (current_task_id clear load+save) |
| **Severity** | **High** |
| **Description** | The `TeamTaskCompleted` event is published **twice** — lines 176–181 and 190–195 are near-identical duplicates, each loading `TeamStore` from disk to get `lead_session_id`. This is both a correctness bug (duplicate events) and a performance issue (redundant disk reads + event bus publishes). Additionally, `run_team_hook` at line 145 does yet another `TeamStore::load` internally (see Issue 3.4). |
| **Fix** | Remove the duplicate event publish block (lines 183–195). Get `lead_session_id` once from the team manager. Consolidate the `current_task_id` clear into the same config write if possible. |

### Issue 3.3 — `team_shutdown_ack` loads and saves `TeamStore` 3 times

| | |
|---|---|
| **File / Lines** | `crates/ragent-team/src/tools/team_shutdown_ack.rs` — lines 56–63 (mark Stopped load+save), 68–74 (read correlation_id load), 86–92 (clear correlation_id load+save) |
| **Severity** | **Medium** |
| **Description** | The tool loads `config.json` three times: once to mark Stopped, once to read `shutdown_request_id`, once to clear `shutdown_request_id`. Each load acquires a shared `flock` and deserializes the full config. |
| **Fix** | Do a single `TeamStore::load`, mutate the member (set Stopped, clear `current_task_id`, read `shutdown_request_id`, clear it), then one `save`. The `correlation_id` can be extracted from the in-memory config before saving. |

### Issue 3.4 — `run_team_hook` loads `TeamStore` on every hook invocation

| | |
|---|---|
| **File / Lines** | `crates/ragent-team/src/team/manager.rs` — `run_team_hook` (350–374) |
| **Severity** | **Medium** |
| **Description** | `run_team_hook` calls `TeamStore::load(team_dir)` to find the hook configuration, then spawns a subprocess. This means every `team_task_create`, `team_task_complete`, and `team_idle` call does an extra full config read+deserialize just to check if a hook is configured (usually none are). |
| **Fix** | Cache hook configuration in `TeamManager` or pass the already-loaded `TeamStore` as a parameter. Hooks are rarely configured, so the 99th-percentile case is a wasted deserialization. |

### Issue 3.5 — `team_spawn` calls `find_team_dir` multiple times and loads `TeamStore` redundantly

| | |
|---|---|
| **File / Lines** | `crates/ragent-team/src/tools/team_spawn.rs` — lines 262–263 (task pre-assign), 291–292 (memory scope persist) |
| **Severity** | **Medium** |
| **Description** | After `spawn_teammate` returns (which itself does 2 config loads inside `spawn_teammate_internal`), the tool calls `find_team_dir` again (directory walk), opens a `TaskStore`, then calls `find_team_dir` **again** and loads `TeamStore` to persist `memory_scope`. The `spawn_teammate_internal` method already loaded and saved the config; the memory scope could have been set there. |
| **Fix** | Pass `memory_scope` into `spawn_teammate_internal` so it is persisted in the single config write that already happens. Avoid the second `find_team_dir` call by caching the `team_dir` from the first lookup. |

---

## 3. Repeated Full-File Serialization / Deserialization (MEDIUM–HIGH)

### Issue 4.1 — `Mailbox::push` re-reads and re-serializes the entire mailbox file on every single message

| | |
|---|---|
| **File / Lines** | `crates/ragent-team/src/team/mailbox.rs` — `push` (304–327) |
| **Severity** | **High** |
| **Description** | `push` does: acquire exclusive lock → read entire file → deserialize to `Vec<MailboxMessage>` → push one message → serialize the full vec → write the full file → release lock. For a mailbox with N messages, this is O(N) deserialization + O(N) serialization per message send. As mailboxes grow (messages are never pruned — `drain_unread` marks them read but keeps them in the file), this degrades linearly. |
| **Fix** | (a) Use an append-only format (e.g., JSONL — one message per line) so `push` only writes a single line, not the entire file. (b) Prune read messages periodically (e.g., in `drain_unread`, after marking all read, optionally remove messages older than a threshold). (c) At minimum, batch multiple pushes into a single write if several messages are queued at once. |

### Issue 4.2 — `TaskStore` re-deserializes the entire `tasks.json` on every mutation

| | |
|---|---|
| **File / Lines** | `crates/ragent-team/src/team/task.rs` — all mutating methods (349–696) |
| **Severity** | **Medium** |
| **Description** | Each of `claim_next`, `claim_specific`, `complete`, `add_task`, `pre_assign_task`, `update_task`, `remove_task` reads the full `tasks.json`, deserializes it, mutates one task, re-serializes the entire list, and writes it back. For a team with 50+ tasks this is wasteful but not catastrophic; the main concern is that it happens on the async path (Issue 1.2). |
| **Fix** | An in-memory `TaskList` cache on `TeamManager` with write-through to disk would eliminate repeated deserialization. Alternatively, use a key-value store (SQLite, sled) instead of a single JSON file so individual tasks can be updated without rewriting the full list. |

### Issue 4.3 — `TeamStore::save` clones the entire config on every write

| | |
|---|---|
| **File / Lines** | `crates/ragent-team/src/team/store.rs` — `save` (216–256), specifically line 221 |
| **Severity** | **Low** |
| **Description** | `save` does `let mut config = self.config.clone()` to avoid mutating `self` while stamping `schema_version` and `updated_at`. This clones the entire `TeamConfig` including the `members: Vec<TeamMember>` vector (which contains strings, `Option<String>`s, `DateTime`s, etc.). For teams with many members, this is an unnecessary heap allocation per save. |
| **Fix** | Stamp `updated_at` and `schema_version` on `self.config` directly (as `save` takes `&self` but could be changed to `&mut self`), or use a temporary borrow. Alternatively, stamp the fields on `self.config` before cloning only the timestamp values. |

---

## 4. Lock Contention on Shared State (MEDIUM)

### Issue 5.1 — `TeamManager::handles` uses `RwLock<HashMap>` with frequent read+write cycles

| | |
|---|---|
| **File / Lines** | `crates/ragent-team/src/team/manager.rs` — `handles: Arc<RwLock<HashMap<String, TeammateHandle>>>` (line 474), used in `spawn_teammate_internal` (718), `suspend_teammate` (936–940), `resume_teammate` (976–982), `shutdown_teammate` (1031–1038), `shutdown_all` (1096–1098), `start_poll_loop` (implicit via clone), `start_watchdog` (1145–1157), `record_progress` (1257–1261), `is_plan_pending` (1283–1292) |
| **Severity** | **Medium** |
| **Description** | `handles` is accessed via `read().await` or `write().await` in nearly every manager method. The `spawn_lock` Mutex (line 478) serializes all spawns, but many other operations independently take `handles.read().await`. The watchdog (line 1120) takes `handles.read().await` on every tick, and the poll loops reference `handles` indirectly. Under contention (many teammates + watchdog running), this creates lock wait time. |
| **Fix** | Consider `DashMap` (already used elsewhere in the codebase per the README) for lock-free concurrent access. Alternatively, use `Arc<RwLock<HashMap>>` with fine-grained locking — the `spawn_lock` could be replaced with per-agent locks if spawn operations are independent. |

### Issue 5.2 — `spawn_lock` Mutex serializes all teammate spawns globally

| | |
|---|---|
| **File / Lines** | `crates/ragent-team/src/team/manager.rs` — `spawn_lock: Arc<Mutex<()>>` (line 478), locked at line 596 |
| **Severity** | **Medium** |
| **Description** | `spawn_teammate_internal` acquires `spawn_lock` at the very start (line 596) and holds it for the **entire** spawn process: config load, agent ID allocation, session creation, system prompt building, memory loading, handle registration, and the `tokio::spawn` of the agent loop. This means only one teammate can be spawned at a time, and all other spawn calls block. The lock is held unnecessarily long — the only part that needs serialization is the agent ID allocation + config update. |
| **Fix** | Narrow the lock scope: only hold `spawn_lock` for the config read + `next_agent_id()` + `add_member()` + `save()` portion (lines 616–631). Release it before session creation and agent loop spawning. |

### Issue 5.3 — `last_progress` uses `std::sync::Mutex` (blocking) inside async context

| | |
|---|---|
| **File / Lines** | `crates/ragent-team/src/team/manager.rs` — `TeammateHandle.last_progress: Arc<std::sync::Mutex<std::time::Instant>>` (line 395), locked in `start_watchdog` (1150), `record_progress` (1260) |
| **Severity** | **Low** |
| **Description** | The watchdog loop acquires `h.last_progress.lock().unwrap()` (line 1150) inside a `tokio::spawn` async block. `std::sync::Mutex` is a blocking mutex — if the lock is held by `record_progress` on another thread, the watchdog task will block. In practice this is unlikely to cause significant contention (the critical section is a single `Instant` assignment), but it violates async hygiene. |
| **Fix** | Use `AtomicU64` storing the monotonic `Instant` as nanos, or use `tokio::sync::Mutex` if the critical section is non-trivial. |

---

## 5. Polling & Busy-Wait Patterns (MEDIUM)

### Issue 6.1 — `reconcile_spawning_members` retries 10 times with 100ms sleep even when there is nothing to reconcile

| | |
|---|---|
| **File / Lines** | `crates/ragent-team/src/team/manager.rs` — `reconcile_spawning_members` (491–575) |
| **Severity** | **Medium** |
| **Description** | The reconciliation loop runs 10 iterations, each loading `TeamStore` from disk and scanning members, with a 100ms sleep between attempts. Even when there are zero spawning members on the first attempt, it breaks early (line 528–530) — but if there are spawning members that fail to spawn, it retries 10 times doing 10 full config loads + 10 spawn attempts. The `to_spawn` collection at line 504–525 allocates a `Vec` of 4-tuples (with cloned strings) on every iteration. |
| **Fix** | (a) Use a `Notify` or file-watcher to wake the reconciliation loop when a new member is written to config, instead of polling. (b) Avoid the Vec allocation — iterate directly. (c) If polling is necessary, back off exponentially after the first few attempts. |

### Issue 6.2 — Mailbox poll loop opens a new `Mailbox` object on every iteration

| | |
|---|---|
| **File / Lines** | `crates/ragent-team/src/team/manager.rs` — `start_poll_loop` (873–923), specifically line 897 |
| **Severity** | **Low** |
| **Description** | Every 500 ms (or on `Notify`), the poll loop calls `Mailbox::open(&team_dir, &agent_id)` which does `fs::create_dir_all` (a syscall) + string formatting for the path. This directory creation is idempotent but still a syscall on every poll. |
| **Fix** | Create the `Mailbox` once before the loop and reuse it. The `Mailbox` struct stores `path`, `team_dir`, and `agent_id` — all immutable — so it can be safely reused. |

### Issue 6.3 — `is_plan_pending` loads `TeamStore` from disk on every call (used by session processor to gate write/bash tools)

| | |
|---|---|
| **File / Lines** | `crates/ragent-team/src/team/manager.rs` — `is_plan_pending` (1283–1292) |
| **Severity** | **Medium** |
| **Description** | `is_plan_pending` is called by the session processor to check if a teammate is in `PlanPending` state before allowing write/bash operations. This means **every tool call** by a plan-pending teammate triggers a full `config.json` read + deserialize + flock. For an active teammate doing many file operations, this is a disk read per tool call. |
| **Fix** | Cache plan status in `TeamManager` or in an in-memory `AtomicBool` on `TeammateHandle`. Update it when `approve_plan` is called. |

---

## 6. Algorithmic Complexity / Inefficient Data Structures (MEDIUM)

### Issue 7.1 — `TaskList::completed_ids()` creates a new `Vec<String>` and scans all tasks on every `next_claimable` / `is_claimable` call

| | |
|---|---|
| **File / Lines** | `crates/ragent-team/src/team/task.rs` — `completed_ids` (210–216), `next_claimable` (220–223), `is_claimable` (139–145) |
| **Severity** | **Medium** |
| **Description** | `next_claimable` calls `self.completed_ids()` (allocates a `Vec<String>`) then iterates all tasks calling `is_claimable` which does `completed_ids.contains(dep)` — a linear scan inside a linear scan. For T tasks with D deps each, this is O(T × T_completed × D) per `claim_next` call. |
| **Fix** | Maintain a `HashSet<String>` of completed task IDs on `TaskList` (or compute it once and reuse). `is_claimable` can check `self.depends_on.iter().all(|dep| done_set.contains(dep))` in O(D) with a HashSet. |

### Issue 7.2 — `Task::is_claimable` uses `completed_ids.contains(dep)` (linear scan) on a `Vec`

| | |
|---|---|
| **File / Lines** | `crates/ragent-team/src/team/task.rs` — `is_claimable` (139–145) |
| **Severity** | **Low** |
| **Description** | `completed_ids.contains(dep)` is O(N) for a `Vec<String>`. Called inside `next_claimable`'s `find()` which is O(T), making the total O(T × N × D). |
| **Fix** | Pass a `HashSet<&str>` or `&HashSet<String>` instead of `&[String]`. |

### Issue 7.3 — `remove_trailing_commas` in `swarm.rs` allocates a `Vec<char>` for the entire JSON string

| | |
|---|---|
| **File / Lines** | `crates/ragent-team/src/team/swarm.rs` — `remove_trailing_commas` (164–182) |
| **Severity** | **Low** |
| **Description** | The function collects the entire input string into a `Vec<char>` (`let chars: Vec<char> = s.chars().collect()`) then iterates char-by-char. For a large decomposition response (multi-KB), this doubles memory usage (the String + the Vec<char>). |
| **Fix** | Iterate over `s.char_indices()` and build the result string without materializing the full char vec. Or use a regex like `r",\s*([}\]])"` to remove trailing commas in a single pass. |

### Issue 7.4 — `strip_explicit_agent_type_hint` collects all whitespace-separated tokens into a `Vec` then joins

| | |
|---|---|
| **File / Lines** | `crates/ragent-team/src/team/classify.rs` — `strip_explicit_agent_type_hint` (166–179), line 174 |
| **Severity** | **Low** |
| **Description** | Line 174: `result.split_whitespace().collect::<Vec<_>>().join(" ")` — splits the entire string into a Vec of &str, then joins them back. This normalizes whitespace but allocates a Vec + String for what is usually a no-op (most descriptions don't have the hint). |
| **Fix** | Only do the whitespace normalization for the portion around the removed hint, or skip it entirely if the original text didn't have the hint (the common path returns `text.to_string()` at line 178). |

---

## 7. Excessive String Cloning and Allocation (LOW–MEDIUM)

### Issue 8.1 — `team_task_claim` and `team_task_complete` clone task fields for metadata that could use references

| | |
|---|---|
| **File / Lines** | `crates/ragent-team/src/tools/team_task_claim.rs` — lines 66–80, 116–134, 202–243; `crates/ragent-team/src/tools/team_task_complete.rs` — lines 80–95, 206–218 |
| **Severity** | **Low** |
| **Description** | The debug logging path (lines 65–88 in `team_task_claim.rs`) maps every task to a `format!("{} ({})", ...)` string and collects into a `Vec<String>` on **every** claim attempt, even in release mode. The `json!({...})` metadata construction also clones `task.id`, `task.title`, `task.description` into the JSON value. |
| **Fix** | Gate the debug logging behind `tracing::enabled!(tracing::Level::DEBUG)`. Use `json!` with references where possible (serde_json accepts `&str` for string values). |

### Issue 8.2 — `build_team_prompt_addition` always allocates the roster string even when the roster is empty

| | |
|---|---|
| **File / Lines** | `crates/ragent-team/src/team/manager.rs` — `build_team_prompt_addition` (142–193) |
| **Severity** | **Low** |
| **Description** | When `teammate_roster` is empty, the function allocates a `"none yet".to_string()`. When non-empty, it maps each `(name, id)` to `format!("{name} ({id})")`, collects into a Vec, and joins. The entire function returns a `String` via `format!` which allocates the full prompt. This is called once per spawn, so impact is low. |
| **Fix** | Minor — use `write!` into a `String::with_capacity(...)` to avoid intermediate allocations. Not high priority. |

### Issue 8.3 — `publish_message_event` serializes `MessageType` to JSON on every message

| | |
|---|---|
| **File / Lines** | `crates/ragent-team/src/team/manager.rs` — `publish_message_event` (1329–1372), lines 1338–1341 |
| **Severity** | **Low** |
| **Description** | For every mailbox message drained, `publish_message_event` calls `serde_json::to_value(&msg.message_type)` to convert the `MessageType` enum to a snake_case string. This allocates a `serde_json::Value` (which is an enum with heap-allocated internals) just to extract a string. |
| **Fix** | Add a `MessageType::as_str()` method (like `TaskStatus` and `MemberStatus` already have) that returns `&'static str` directly. Use that instead of serializing to JSON. |

---

## 8. Redundant Work / Code Duplication (MEDIUM — also correctness)

### Issue 9.1 — Duplicate `TeamTaskCompleted` event publication in `team_task_complete`

| | |
|---|---|
| **File / Lines** | `crates/ragent-team/src/tools/team_task_complete.rs` — lines 172–181 and 183–195 |
| **Severity** | **Medium** (correctness + performance) |
| **Description** | The `Event::TeamTaskCompleted` event is published **twice** with identical content. Lines 172–181 and 183–195 are near-identical code blocks (the second even has the same comment). Each loads `TeamStore` from disk to get `lead_session_id`. This doubles the event bus load and disk I/O for every task completion. |
| **Fix** | Delete the duplicate block (lines 183–195). This is likely a copy-paste error from the M5-T7 milestone. |

### Issue 9.2 — `team_create` creates a full `ToolRegistry` with all tools for blueprint seeding

| | |
|---|---|
| **File / Lines** | `crates/ragent-team/src/tools/team_create.rs` — lines 177, 263 |
| **Severity** | **Low** |
| **Description** | `crate::tool::create_default_registry()` is called to instantiate the full tool registry (all ~111 tools) just to look up and execute one seed tool. This happens twice (once for task-seed.json, once for spawn-prompts.json). The registry construction allocates all tool Arcs and the internal HashMap. |
| **Fix** | Create a minimal registry with only the tools referenced in seed files, or pass the existing `ToolRegistry` from the session context. |

### Issue 9.3 — `Config::default()` is constructed twice in `spawn_teammate_internal`

| | |
|---|---|
| **File / Lines** | `crates/ragent-team/src/team/manager.rs` — lines 600, 668 |
| **Severity** | **Low** |
| **Description** | `Config::default()` is called at line 600 (to validate agent type) and again at line 668 (to resolve the agent). `Config::default()` may involve loading config files from disk. |
| **Fix** | Construct `Config::default()` once and reuse. Or better, pass the lead's `Config` through the `TeamManager`. |

---

## 9. Missing Batching / Caching (MEDIUM)

### Issue 10.1 — `team_read_messages` acknowledges each message individually with a separate file lock + write cycle

| | |
|---|---|
| **File / Lines** | `crates/ragent-team/src/tools/team_read_messages.rs` — lines 137–145 |
| **Severity** | **Medium** |
| **Description** | For each unread message, `mailbox.acknowledge(&m.message_id)` is called, which does: acquire exclusive lock → read full file → deserialize → find message → mark read → serialize full file → write → release lock. For N unread messages, this is N full read-modify-write cycles of the mailbox file, each acquiring an exclusive `flock`. |
| **Fix** | Add a `Mailbox::mark_all_read(&self, message_ids: &[String]) -> Result<()>` that does a single lock → read → deserialize → mark all → serialize → write → unlock. Or use `drain_unread` which does this in one pass (but changes semantics from peek+ack to drain). |

### Issue 10.2 — `team_broadcast` opens a new `Mailbox` and pushes individually to each teammate (sequential, not concurrent)

| | |
|---|---|
| **File / Lines** | `crates/ragent-team/src/tools/team_broadcast.rs` — lines 80–96 |
| **Severity** | **Medium** |
| **Description** | The broadcast loop iterates `active` members sequentially, calling `Mailbox::open` + `Mailbox::push` for each. Each `push` does a full read-modify-write of that agent's mailbox file (Issue 4.1). For T teammates, this is T sequential file lock acquisitions + T full file rewrites. |
| **Fix** | (a) Use `tokio::task::spawn` or `futures::join_all` to push concurrently. (b) Since each mailbox is a separate file with a separate lock, there's no contention — they can be written in parallel. (c) Consider a broadcast mailbox file that all teammates read from, rather than duplicating the message into each individual mailbox. |

---

## 10. Miscellaneous

### Issue 11.1 — `load_memory_block` reads the entire `MEMORY.md` into memory and then truncates line-by-line

| | |
|---|---|
| **File / Lines** | `crates/ragent-team/src/team/manager.rs` — `load_memory_block` (223–270) |
| **Severity** | **Low** |
| **Description** | The function reads the full file via `std::fs::read_to_string`, then iterates lines with `.take(MEMORY_MAX_LINES).take_while(...)` collecting into a `Vec<&str>` and joining. For a 25 KB file this is fine, but the `collect::<Vec<_>>().join("\n")` pattern allocates a Vec + String. The file is read synchronously on the async spawn path. |
| **Fix** | Minor — use `spawn_blocking` for the file read (consistent with Issue 1.1). The truncation logic could use a `String::with_capacity` and `push_str` instead of collect+join. |

### Issue 11.2 — `adopt_orphaned_tasks` calls `update_task` in a loop (N lock+read+write cycles)

| | |
|---|---|
| **File / Lines** | `crates/ragent-team/src/team/manager.rs` — `adopt_orphaned_tasks` (1232–1252) |
| **Severity** | **Low** |
| **Description** | For each orphaned task, `task_store.update_task(&tid, ...)` is called, which does a full read-modify-write of `tasks.json`. If K tasks are orphaned, that's K separate lock+read+serialize+write cycles. This only runs during leader recovery, so impact is low. |
| **Fix** | Batch the update: read the list once, update all orphaned tasks, write once. |

### Issue 11.3 — `resolve_agent_id` in `team_message.rs` loads `TeamStore` and linearly scans members

| | |
|---|---|
| **File / Lines** | `crates/ragent-team/src/tools/team_message.rs` — `resolve_agent_id` (134–155) |
| **Severity** | **Low** |
| **Description** | `resolve_agent_id` loads `config.json`, then linearly scans `config.members` comparing both `agent_id` and `name`. Called by `team_message`, `team_assign_task`, `team_shutdown_teammate`, `team_approve_plan`. For teams with many members, the linear scan is O(N). |
| **Fix** | (a) Build a `HashMap<String, String>` from name→agent_id when `TeamStore` is loaded and cache it. (b) Or add an index on `TeamConfig` — `member_by_name` already does a linear scan (config.rs line 398). |

---

## Priority Ranking

| Priority | Issue | Impact |
|----------|-------|--------|
| **P0** | 1.1, 1.2, 1.3 | Blocking I/O on async paths starves the tokio runtime |
| **P0** | 3.2 (Issue 9.1) | Duplicate `TeamTaskCompleted` event — correctness bug + 2× disk I/O |
| **P1** | 3.1 | `team_task_claim` does 5 file operations per claim |
| **P1** | 4.1 | `Mailbox::push` rewrites entire file per message; unbounded growth |
| **P1** | 6.3 | `is_plan_pending` disk read per tool call |
| **P2** | 5.2 | `spawn_lock` held too long |
| **P2** | 10.1 | Per-message acknowledge does N full file rewrite cycles |
| **P2** | 10.2 | Broadcast is sequential, not concurrent |
| **P2** | 7.1 | `completed_ids()` O(T²) in claim path |
| **P3** | 3.3, 3.4, 3.5, 4.2, 4.3, 5.1, 5.3, 6.1, 6.2, 7.2–7.4, 8.1–8.3, 9.2–9.3, 11.1–11.3 | Lower-frequency or lower-cost inefficiencies |

---

## Architectural Recommendations

1. **In-memory state cache with write-through persistence**: The most impactful change would be to maintain an in-memory copy of `TeamConfig`, `TaskList`, and each `Mailbox` on `TeamManager`, updated on writes and invalidated on external changes (via file watcher or mtime check). This eliminates the repeated full-file deserialize on every operation.

2. **JSONL mailbox format**: Switching mailbox files to newline-delimited JSON (one `MailboxMessage` per line) makes `push` O(1) append-only instead of O(N) full rewrite, and enables streaming reads.

3. **`spawn_blocking` wrapper**: All `TeamStore::load`, `TeamStore::save`, `TaskStore::*`, and `Mailbox::*` methods should be wrapped in `tokio::task::spawn_blocking` when called from async contexts. This is the single highest-leverage change for async runtime health.

4. **`DashMap` for `handles`**: Replace `RwLock<HashMap<String, TeammateHandle>>` with `DashMap` for lock-free concurrent access, matching the pattern already used in the orchestrator.

5. **Batch task operations**: `adopt_orphaned_tasks` and `team_read_messages` acknowledge should batch their updates into a single read-modify-write cycle instead of one per item.