# APERFPLAN — Agent & Team Performance Improvement Plan

**Created:** 2026-06-22
**Source reviews:**
- `docs/reports/ragent-agent-performance-review.md` (swarm-s1, 2026-06-22)
- `docs/swarm-s2-ragent-team-performance-review.md` (swarm-s2, 2026-06-22)
**Related spec:** `specs/AgentPerf/` (implemented — AgentPerf v1)

---

## 1. Executive Summary

Two independent performance reviews of the `ragent-agent` and `ragent-team` crates
identified **53 distinct issues** (10 high, 18 medium, 15 low in `ragent-agent`;
plus 18 issues across 10 themes in `ragent-team`). The findings converge on four
pervasive anti-patterns:

1. **Blocking synchronous I/O on async executor threads.** Both crates perform
   `std::fs` reads/writes, `serde_json` serialization, `std::process::Command`
   spawns, and SQLite queries directly inside `async fn` bodies. Under concurrent
   teammate activity or multi-step agent loops, this starves the tokio runtime,
   stalls other futures, and inflates latency unpredictably. This is the
   **single highest-leverage fix** — wrapping all file and DB I/O in
   `tokio::task::spawn_blocking`.

2. **Redundant loads / N+1 patterns.** `Config::load()` is called 3–4× per
   `process_user_message`. `TeamStore::load()` is called 5× per `team_task_claim`.
   `ToolRegistry::definitions()` re-sorts on every uncached call. `Mailbox::push`
   re-reads and re-serializes the entire mailbox file for every single message.
   These patterns multiply disk I/O and CPU work by a factor of 3–10× with no
   functional benefit.

3. **Excessive cloning of large data on the hot path.** `system_prompt`,
   `chat_messages`, `agent.options`, `session_config`, and `assistant_parts` are
   deep-cloned on every step of the agent loop and on every tool call
   construction. Wrapping these in `Arc` would convert O(n) heap allocations
   into O(1) atomic reference-count increments.

4. **Unbounded data structures and O(n²) algorithms.** `Storage::get_messages`
   loads ALL messages with no pagination. `TaskList::completed_ids()` rebuilds
   a `Vec<String>` on every `is_claimable` call, yielding O(T²) task-claim
   complexity. Mailbox files grow without pruning. Long sessions and large
   teams degrade super-linearly.

The plan below organizes these findings into **31 actionable tasks** across three
phases, prioritized by severity and expected impact. Each task is self-contained
so an engineer can pick it up without additional context.

---

## 2. Improvement Tasks

### Task Index

| ID | Title | Phase | Severity | Effort | Crate |
|----|-------|-------|----------|--------|-------|
| PERF-001 | Load Config once per process_user_message | 1 | High | S | agent |
| PERF-002 | Store session_id as Arc\<str\> for event publishing | 1 | High | S | agent |
| PERF-003 | Cache tool_names Vec for ToolsSent event | 1 | High | S | agent |
| PERF-004 | Cache format_version existence in Storage | 1 | High | S | agent |
| PERF-005 | Remove duplicate TeamTaskCompleted event | 1 | High | S | team |
| PERF-006 | Wrap system_prompt in Arc\<str\> | 2 | High | M | agent |
| PERF-007 | Wrap chat_messages in Arc\<Vec\> | 2 | High | M | agent |
| PERF-008 | Store agent.options as Arc\<HashMap\> | 2 | High | M | agent |
| PERF-009 | Pre-compute ToolContext template per process_user_message | 2 | High | M | agent |
| PERF-010 | Add has_assistant_messages() to Storage | 2 | High | M | agent |
| PERF-011 | Cache create_builtin_agents() in OnceLock | 2 | Medium | S | agent |
| PERF-012 | Add version counter to ToolRegistry for cache invalidation | 2 | Medium | S | agent |
| PERF-013 | Route sub-agent tool references through SystemPromptCache | 2 | Medium | S | agent |
| PERF-014 | Eliminate estimate_request_bytes JSON serialization | 2 | Medium | S | agent |
| PERF-015 | Fix tool_result_content_for_llm fast-path allocation | 1 | Medium | S | agent |
| PERF-016 | Wrap all team file I/O in spawn_blocking | 2 | High | L | team |
| PERF-017 | Eliminate redundant TeamStore loads in team_task_claim | 2 | High | M | team |
| PERF-018 | Eliminate redundant TeamStore loads in team_task_complete | 2 | High | S | team |
| PERF-019 | Cache team_dir path to avoid directory walk per tool call | 2 | High | S | team |
| PERF-020 | Batch mailbox acknowledgements in team_read_messages | 3 | Medium | M | team |
| PERF-021 | Concurrent broadcast in team_broadcast | 3 | Medium | M | team |
| PERF-022 | Switch mailbox to append-only JSONL format | 3 | High | L | team |
| PERF-023 | In-memory TaskList cache with write-through persistence | 3 | High | L | team |
| PERF-024 | Narrow spawn_lock scope in TeamManager | 3 | Medium | M | team |
| PERF-025 | Replace handles RwLock\<HashMap\> with DashMap | 3 | Medium | M | team |
| PERF-026 | Fix TaskList::completed_ids O(T²) claim path | 2 | Medium | S | team |
| PERF-027 | Cache is_plan_pending in memory | 2 | Medium | S | team |
| PERF-028 | Offload build_system_prompt_with_storage I/O to spawn_blocking | 3 | High | L | agent |
| PERF-029 | Use Arc\<str\>/Arc\<Value\> in history_to_chat_messages | 3 | Medium | L | agent |
| PERF-030 | Reuse event bus in extracted tool adapters | 3 | Low | M | agent |
| PERF-031 | Use ahash/FxHash for non-cryptographic cache keys | 3 | Low | S | agent |

---

### Phase 1: Quick Wins (S effort, immediate impact)

#### PERF-001 — Load Config once per process_user_message

| Field | Value |
|-------|-------|
| **Severity** | High |
| **Effort** | S |
| **Crate** | ragent-agent |
| **Affected files** | `crates/ragent-agent/src/session/processor.rs` (lines ~904–1043) |
| **Source** | Agent review H-1 |

**Description:** `Config::load()` is called at least 3–4 times within a single
`process_user_message` invocation (lines ~905, ~922, ~974, ~1043). Each call reads
and parses a JSON file from disk. The config should be loaded once at the top of
the function and reused.

**Expected impact:** Eliminates 2–3 redundant disk reads + JSON parses per user
turn. Reduces TTFT by the I/O latency of 2–3 config file reads.

**Implementation approach:**
1. At the start of `process_user_message`, call `let cfg = crate::Config::load().unwrap_or_default();` once.
2. Replace all subsequent `crate::Config::load()` calls within the function with references to `cfg`.
3. For code paths that need `session_config` (line ~1043), reuse the same `cfg` variable.
4. Consider caching the parsed `Config` as `Arc<Config>` on `SessionProcessor` itself, refreshed on a TTL.

**Verification:** Confirm no behavior change via existing tests. Add a test that
asserts `Config::load()` is called at most once per `process_user_message` (use a
counter or mock).

---

#### PERF-002 — Store session_id as Arc\<str\> for event publishing

| Field | Value |
|-------|-------|
| **Severity** | High |
| **Effort** | S |
| **Crate** | ragent-agent |
| **Affected files** | `crates/ragent-agent/src/session/processor.rs` (lines ~1529, ~1574, ~1844, ~1920, ~2603, ~2641) |
| **Source** | Agent review H-9 |

**Description:** `session_id` is a `&str` that gets `.to_string()` called dozens
of times per agent loop step for event publishing. Each call allocates a new
`String`.

**Expected impact:** Eliminates ~10–20 heap allocations per agent step.

**Implementation approach:**
1. At the start of `process_user_message`, convert `session_id` to an owned `String` or `Arc<str>`: `let session_id: Arc<str> = Arc::from(session_id);`
2. Replace all `session_id.to_string()` calls with `session_id.clone()` (Arc clone is O(1)).
3. Update event construction sites to use `(*session_id).to_string()` only where an owned `String` is strictly required by the event type — or better, change event fields to accept `Arc<str>`.

**Verification:** Existing tests should pass unchanged. Benchmark allocation count
before/after using the `agent_loop` criterion bench.

---

#### PERF-003 — Cache tool_names Vec for ToolsSent event

| Field | Value |
|-------|-------|
| **Severity** | High |
| **Effort** | S |
| **Crate** | ragent-agent |
| **Affected files** | `crates/ragent-agent/src/session/processor.rs` (line ~1573) |
| **Source** | Agent review H-10 |

**Description:** On every step of the agent loop, tool names are collected into a
`Vec<String>` for the `ToolsSent` event: `tool_definitions.iter().map(|t| t.name.clone()).collect()`.
With ~111 tools, this allocates 111 Strings on every step, even when tools haven't changed.

**Expected impact:** Eliminates 111 String allocations per step (after first step).

**Implementation approach:**
1. Cache the tool name list alongside `cached_tool_definitions` in the step loop.
2. Only rebuild when the tool registry version changes (see PERF-012).
3. Alternatively, skip the `ToolsSent` event entirely when the tool set hasn't changed since the previous step.

**Verification:** Confirm event still fires on first step and when tools change.

---

#### PERF-004 — Cache format_version existence in Storage

| Field | Value |
|-------|-------|
| **Severity** | High |
| **Effort** | S |
| **Crate** | ragent-agent |
| **Affected files** | `crates/ragent-agent/src/storage/mod.rs` (lines ~490–546, ~568) |
| **Source** | Agent review H-8 |

**Description:** Every call to `get_session` and `list_sessions` runs
`SELECT COUNT(*) FROM pragma_table_info('sessions') WHERE name='format_version'`
to check if the column exists. This metadata query executes on every session lookup.

**Expected impact:** Eliminates one SQLite round-trip per session lookup.

**Implementation approach:**
1. Add an `AtomicBool has_format_version` field to the `Storage` struct.
2. Initialize it to `false` on construction.
3. In `get_session`/`list_sessions`, check the AtomicBool first. If `true`, skip the pragma query. If `false`, run the query once, set the AtomicBool, and cache the result.
4. Alternatively, check once during `migrate()` and set the flag there.

**Verification:** Existing storage tests should pass. Add a test that verifies
the pragma query is not executed after the first call.

---

#### PERF-005 — Remove duplicate TeamTaskCompleted event

| Field | Value |
|-------|-------|
| **Severity** | High (correctness + performance) |
| **Effort** | S |
| **Crate** | ragent-team |
| **Affected files** | `crates/ragent-team/src/tools/team_task_complete.rs` (lines 172–195) |
| **Source** | Team review Issue 9.1 / Issue 3.2 |

**Description:** The `Event::TeamTaskCompleted` event is published **twice** —
lines 172–181 and 183–195 are near-identical duplicate blocks, each loading
`TeamStore` from disk to get `lead_session_id`. This is a copy-paste bug that
doubles event bus load and disk I/O for every task completion.

**Expected impact:** Halves the event bus load and disk I/O for task completion.
Fixes a correctness bug (duplicate events).

**Implementation approach:**
1. Delete the duplicate block at lines 183–195.
2. Get `lead_session_id` once from the in-memory `TeamManager` rather than loading `TeamStore`.
3. Keep a single event publish.

**Verification:** Add a test that asserts `TeamTaskCompleted` is published exactly
once per `team_task_complete` call. Check event bus subscriber counts.

---

#### PERF-015 — Fix tool_result_content_for_llm fast-path allocation

| Field | Value |
|-------|-------|
| **Severity** | Medium |
| **Effort** | S |
| **Crate** | ragent-agent |
| **Affected files** | `crates/ragent-agent/src/session/processor.rs` (lines ~3385–3399) |
| **Source** | Agent review M-10 |

**Description:** In the fast path (content under threshold), the function does
`return Arc::from(content.to_string());` — the intermediate `String` allocation
is unnecessary.

**Expected impact:** Eliminates one String allocation per tool result in the
common (non-truncated) path.

**Implementation approach:**
1. Replace `Arc::from(content.to_string())` with `Arc::from(content)` where `content: &str`.
2. `Arc::<str>::from(&str)` does a single allocation (no intermediate String).

**Verification:** Unit test confirming `tool_result_content_for_llm` returns
correct content for short and long inputs.

---

### Phase 2: Medium Complexity (M effort, significant impact)

#### PERF-006 — Wrap system_prompt in Arc\<str\>

| Field | Value |
|-------|-------|
| **Severity** | High |
| **Effort** | M |
| **Crate** | ragent-agent |
| **Affected files** | `crates/ragent-agent/src/session/processor.rs` (line ~1711), `crates/ragent-llm/src/lib.rs` (ChatRequest struct) |
| **Source** | Agent review H-2 |

**Description:** Inside the retry loop, each attempt builds a fresh `ChatRequest`
and clones the entire system prompt string (`system: Some(system_prompt.clone())`).
The system prompt can be 5,000–20,000 characters. This clone happens on every
retry attempt and on every step.

**Expected impact:** Converts O(n) string clone to O(1) Arc clone on every step
and retry.

**Implementation approach:**
1. Change `ChatRequest.system` from `Option<String>` to `Option<Arc<str>>`.
2. Build `system_prompt` once as `Arc<str>` at the top of `process_user_message`.
3. Use `system: Some(Arc::clone(&system_prompt))` in `ChatRequest` construction.
4. Update all `ChatRequest` construction sites and the LLM provider `chat()` methods to accept `Arc<str>`.
5. Provider methods can deref with `&*system` where they need `&str`.

**Risk:** Changing `ChatRequest.system` type touches all provider implementations.
Ensure all 11+ providers compile and tests pass.

**Verification:** `cargo test -p ragent-llm` and `cargo test -p ragent-agent`.
Run `agent_loop` bench to measure allocation reduction.

---

#### PERF-007 — Wrap chat_messages in Arc\<Vec\>

| Field | Value |
|-------|-------|
| **Severity** | High |
| **Effort** | M |
| **Crate** | ragent-agent |
| **Affected files** | `crates/ragent-agent/src/session/processor.rs` (line ~1706), `crates/ragent-llm/src/lib.rs` (ChatRequest struct) |
| **Source** | Agent review H-3 |

**Description:** On every retry attempt, `messages: Arc::new(chat_messages.clone())`
clones the entire chat message vector (which grows with each step) into a new `Vec`
and then wraps in `Arc`. This is O(n) in message count on every retry.

**Expected impact:** Converts O(n) Vec clone to O(1) Arc clone on retries.

**Implementation approach:**
1. Keep `chat_messages` as `Arc<Vec<ChatMessage>>` throughout the loop.
2. On the first attempt, pass `Arc::clone(&chat_messages)`.
3. On retry (when buffers are reset), only then clone the underlying Vec if mutation is needed.
4. `ChatRequest.messages` is already `Arc<Vec<ChatMessage>>`, so this aligns with the existing type.

**Risk:** If any code path mutates `chat_messages` between attempts, it must use
`Arc::make_mut` to get a mutable reference (COW semantics).

**Verification:** Existing agent loop tests. Verify retry path still works correctly.

---

#### PERF-008 — Store agent.options as Arc\<HashMap\>

| Field | Value |
|-------|-------|
| **Severity** | High |
| **Effort** | M |
| **Crate** | ragent-agent |
| **Affected files** | `crates/ragent-agent/src/agent/mod.rs` (AgentInfo struct, line ~438), `crates/ragent-agent/src/session/processor.rs` (lines ~1712, ~1722) |
| **Source** | Agent review H-4 |

**Description:** `agent.options.clone()` and `agent.thinking.clone()` are cloned
for every `ChatRequest` construction on every step. `agent.options` is a
`HashMap<String, serde_json::Value>` which allocates per entry on clone.

**Expected impact:** Converts per-step HashMap clone to O(1) Arc clone.

**Implementation approach:**
1. Change `AgentInfo.options` from `HashMap<String, Value>` to `Arc<HashMap<String, Value>>`.
2. In `ChatRequest` construction, use `options: Arc::clone(&agent.options)` instead of `agent.options.clone()`.
3. For `thinking`, it's already an `Option<ThinkingConfig>` which is cheap to clone (enum + small struct), so it's lower priority but can follow the same Arc pattern if desired.

**Risk:** Any code that mutates `agent.options` after construction needs `Arc::make_mut`.

**Verification:** `cargo test -p ragent-agent`. Check that custom agents with
options still work correctly.

---

#### PERF-009 — Pre-compute ToolContext template per process_user_message

| Field | Value |
|-------|-------|
| **Severity** | High |
| **Effort** | M |
| **Crate** | ragent-agent |
| **Affected files** | `crates/ragent-agent/src/session/processor.rs` (lines ~2260–2277) |
| **Source** | Agent review H-5 |

**Description:** For every tool call in every step, a new `ToolContext` is
constructed that clones `session_id.to_string()`, `working_dir.clone()`,
`model_ref.clone()`, and — most expensively — `session_config.clone()` wrapped
in `Arc::new()` (clones the entire `Config` struct with HashMaps, Vecs, nested
structures).

**Expected impact:** Eliminates full `Config` struct clone per tool call.

**Implementation approach:**
1. Pre-compute a `ToolContext` template once per `process_user_message` call with all shared fields.
2. For each tool call, clone only the fields that vary (if any) — most fields are constant within a single `process_user_message`.
3. Store `session_config` as `Arc<Config>` once and pass `Arc::clone()` per tool call instead of cloning the Config.
4. If `session_config` is already `Arc<Config>` from PERF-001, this becomes trivial.

**Verification:** Existing tool tests. Verify that tool execution still receives
correct config.

---

#### PERF-010 — Add has_assistant_messages() to Storage

| Field | Value |
|-------|-------|
| **Severity** | High |
| **Effort** | M |
| **Crate** | ragent-agent |
| **Affected files** | `crates/ragent-agent/src/storage/mod.rs` (lines ~744–783), `crates/ragent-agent/src/session/processor.rs` (lines ~1053, ~1288, ~2953) |
| **Source** | Agent review H-7 |

**Description:** `get_messages` loads ALL messages for the session, deserializing
each message's `parts` JSON column. It's called in 3 places, two of which only
need to know if an assistant message exists.

**Expected impact:** For existence checks, reduces from O(n * parts_size)
deserialization to a single `SELECT 1 ... LIMIT 1` query.

**Implementation approach:**
1. Add `pub fn has_assistant_messages(&self, session_id: &str) -> Result<bool>` to `Storage`.
2. SQL: `SELECT 1 FROM messages WHERE session_id = ?1 AND role = 'assistant' LIMIT 1`.
3. Replace the two existence-check call sites (processor.rs ~1053, ~2953) with `has_assistant_messages()`.
4. Keep the full `get_messages` call only for the history-loading path (~1288).
5. Optionally, for the history path, add a `get_recent_messages(session_id, limit)` for lazy loading.

**Verification:** Storage unit tests. Verify `run_init_exchange` still detects
prior assistant messages correctly.

---

#### PERF-011 — Cache create_builtin_agents() in OnceLock

| Field | Value |
|-------|-------|
| **Severity** | Medium |
| **Effort** | S |
| **Crate** | ragent-agent |
| **Affected files** | `crates/ragent-agent/src/agent/mod.rs` (line ~1244, lines ~507–1025) |
| **Source** | Agent review M-1, L-4 |

**Description:** `resolve_agent()` calls `create_builtin_agents()` which
constructs a `Vec<AgentInfo>` with ~15 agents, each containing String allocations
for name, description, and long prompt strings. This happens on every agent
resolution (every `process_user_message` and every sub-agent spawn).

**Expected impact:** Eliminates ~15 AgentInfo constructions (with 500+ char prompts)
on every agent resolution.

**Implementation approach:**
1. Add `static BUILTIN_AGENTS: OnceLock<Vec<AgentInfo>> = OnceLock::new();`
2. In `resolve_agent`, use `BUILTIN_AGENTS.get_or_init(|| create_builtin_agents())`.
3. Search the cached list instead of constructing a new one each time.
4. Agent definitions are static and never change at runtime, so this is safe.

**Verification:** Agent resolution tests. Verify custom agents still override
builtins correctly.

---

#### PERF-012 — Add version counter to ToolRegistry for cache invalidation

| Field | Value |
|-------|-------|
| **Severity** | Medium |
| **Effort** | S |
| **Crate** | ragent-agent |
| **Affected files** | `crates/ragent-agent/src/tool/mod.rs` (ToolRegistry struct, `definitions()` ~1119–1133, `register()`), `crates/ragent-agent/src/session/cache.rs` (lines ~358–366) |
| **Source** | Agent review M-2, L-7 |

**Description:** `hash_tool_registry` iterates all ~111 tools and hashes their
names and descriptions on every cache check. `definitions()` re-sorts on every
uncached call. Both are O(n) operations repeated on every step.

**Expected impact:** Converts O(n) hashing per cache check to O(1) version
comparison. Eliminates O(n log n) sort per uncached `definitions()` call.

**Implementation approach:**
1. Add `version: AtomicU64` to `ToolRegistry`.
2. In `register()`, increment `version` after adding a tool.
3. Expose `pub fn version(&self) -> u64` on `ToolRegistry`.
4. In `SystemPromptCache`, replace `hash_tool_registry` with a version comparison: store the registry version alongside the cached result and invalidate when the version changes.
5. Maintain a sorted `Vec<ToolDefinition>` cache inside `ToolRegistry` that is invalidated (set to `None`) on `register()`. `definitions()` returns the cached sorted vec.

**Verification:** Tool registry tests. Verify cache invalidation when tools are
registered at runtime (e.g., MCP tools).

---

#### PERF-013 — Route sub-agent tool references through SystemPromptCache

| Field | Value |
|-------|-------|
| **Severity** | Medium |
| **Effort** | S |
| **Crate** | ragent-agent |
| **Affected files** | `crates/ragent-agent/src/session/processor.rs` (line ~1111) |
| **Source** | Agent review M-3 |

**Description:** When `is_subagent` is true, `build_detailed_tool_reference_section`
is called directly without going through the `SystemPromptCache`. This function
iterates all tool definitions, formats each parameter schema, and builds a large
string on every sub-agent message processing.

**Expected impact:** Eliminates per-message tool reference string building for
sub-agents.

**Implementation approach:**
1. Add a separate cache key/entry in `SystemPromptCache` for sub-agent tool references (or reuse the main entry if the tool set is the same).
2. Call `SystemPromptCache::get_tool_reference` for sub-agents too, passing `is_subagent = true`.
3. Only rebuild when the tool registry version changes (see PERF-012).

**Verification:** Sub-agent tests. Verify sub-agents receive correct tool references.

---

#### PERF-014 — Eliminate estimate_request_bytes JSON serialization

| Field | Value |
|-------|-------|
| **Severity** | Medium |
| **Effort** | S |
| **Crate** | ragent-agent |
| **Affected files** | `crates/ragent-agent/src/session/processor.rs` (lines ~3432–3468) |
| **Source** | Agent review M-11 |

**Description:** `estimate_request_bytes` serializes every `ContentPart::ToolUse`
and every `ToolDefinition.parameters` to string via `.to_string()` just to get
`.len()`. With ~111 tools, this serializes ~111 JSON schemas into strings on every
step, then discards them.

**Expected impact:** Eliminates ~111 JSON serializations per step.

**Implementation approach:**
1. Cache the serialized size of tool definitions (they don't change between steps). Store a `total_tool_bytes: usize` alongside `cached_tool_definitions`.
2. For `ContentPart::ToolUse` inputs, use `serde_json::to_string(&input).len()` only for the actual tool call inputs (typically 1–5 per step), not for all tool definitions.
3. Alternatively, use a cheaper size estimate: `serde_json::to_string(&input).map(|s| s.len()).unwrap_or(0)` for inputs, and a cached sum for tool schemas.

**Verification:** Existing `estimate_request_bytes` tests. Run `agent_loop` bench
to measure improvement.

---

#### PERF-016 — Wrap all team file I/O in spawn_blocking

| Field | Value |
|-------|-------|
| **Severity** | High |
| **Effort** | L |
| **Crate** | ragent-team |
| **Affected files** | `crates/ragent-team/src/team/mailbox.rs` (push, read_all, peek_unread, drain_unread, mark_read), `crates/ragent-team/src/team/task.rs` (all mutating methods 349–696), `crates/ragent-team/src/team/store.rs` (load, save, find_team_dir) |
| **Source** | Team review Issues 1.1, 1.2, 1.3 |

**Description:** Every mailbox operation, task mutation, and config load/save
performs synchronous `std::fs` reads/writes, `serde_json` serialization, and
`flock`-based locking inside `async fn execute()` methods. Under concurrent
teammate activity, multiple tokio worker threads are simultaneously stalled on
disk I/O.

**Expected impact:** Frees tokio worker threads during team operations. Prevents
async runtime starvation under concurrent teammate activity. This is the single
highest-leverage change for team crate async health.

**Implementation approach:**
1. Wrap the entire read-modify-write cycle in `tokio::task::spawn_blocking` for each async method.
2. For `Mailbox::push`, `Mailbox::drain_unread`, `TaskStore::claim_next`, `TaskStore::complete`, `TeamStore::load`, `TeamStore::save` — create blocking wrappers:
   ```rust
   pub async fn push_blocking(&self, msg: MailboxMessage) -> Result<()> {
       let path = self.path.clone();
       tokio::task::spawn_blocking(move || {
           let mbox = Mailbox::open(&path)?;
           mbox.push(msg)
       }).await?
   }
   ```
3. Update all tool `execute()` methods to call the `_blocking` variants.
4. Alternatively, migrate to `tokio::fs` for file reads/writes and perform deserialization on the blocking pool.

**Risk:** `spawn_blocking` has overhead (~10µs per spawn). For very fast
operations, the overhead may exceed the benefit. Benchmark to confirm net
positive. The flock-based locking cannot use async locks, so `spawn_blocking` is
the correct approach.

**Verification:** Integration tests with concurrent teammate activity. Monitor
tokio runtime metrics (if available) for thread starvation.

---

#### PERF-017 — Eliminate redundant TeamStore loads in team_task_claim

| Field | Value |
|-------|-------|
| **Severity** | High |
| **Effort** | M |
| **Crate** | ragent-team |
| **Affected files** | `crates/ragent-team/src/tools/team_task_claim.rs` (lines 65–88, 98–100, 110–115, 206–222) |
| **Source** | Team review Issue 3.1 |

**Description:** A single `team_task_claim` call performs 5 file operations: (1)
debug `TaskStore::read()`, (2) `TaskStore::claim_next/claim_specific` (full
read-modify-write), (3) `TeamStore::load()` for `lead_session_id`, (4)
`TeamStore::load()` + `save()` to update `current_task_id`.

**Expected impact:** Reduces from 5 file operations to ~2 per claim.

**Implementation approach:**
1. Remove the debug `store.read()` at lines 65–88 or gate it behind `tracing::enabled!(Level::DEBUG)`.
2. Get `lead_session_id` from the in-memory `TeamManager` or `ToolContext` rather than loading `config.json`.
3. Combine the `current_task_id` update with the claim write (single write to `tasks.json` + single write to `config.json`).

**Verification:** `team_task_claim` tests. Verify claim still works and
`current_task_id` is updated.

---

#### PERF-018 — Eliminate redundant TeamStore loads in team_task_complete

| Field | Value |
|-------|-------|
| **Severity** | High |
| **Effort** | S |
| **Crate** | ragent-team |
| **Affected files** | `crates/ragent-team/src/tools/team_task_complete.rs` (lines 79–103, 172–204) |
| **Source** | Team review Issue 3.2 (partially addressed by PERF-005) |

**Description:** After removing the duplicate event (PERF-005),
`team_task_complete` still loads `TeamStore` 3+ times: debug read, lead_sid load,
current_task_id clear.

**Expected impact:** Reduces from 3+ file operations to ~1 per completion.

**Implementation approach:**
1. Remove/gate the debug `store.read()` at lines 79–103.
2. Get `lead_session_id` from in-memory `TeamManager` or `ToolContext`.
3. Consolidate `current_task_id` clear into a single config write.

**Verification:** `team_task_complete` tests.

---

#### PERF-019 — Cache team_dir path to avoid directory walk per tool call

| Field | Value |
|-------|-------|
| **Severity** | High |
| **Effort** | S |
| **Crate** | ragent-team |
| **Affected files** | `crates/ragent-team/src/team/store.rs` (find_team_dir 52–68, find_project_teams_dir 33–45), all tool files that call `find_team_dir` |
| **Source** | Team review Issue 1.3 |

**Description:** `find_team_dir` walks up the directory tree calling
`candidate.is_dir()` (a `stat()` syscall per level) on every parent. This is
called from nearly every team tool's `execute()` method, often multiple times
per call.

**Expected impact:** Eliminates directory-walk syscalls per tool call after first
resolution.

**Implementation approach:**
1. Cache the resolved `team_dir: PathBuf` in `ToolContext` or on `TeamManager`.
2. All tool `execute()` methods use the cached path instead of calling `find_team_dir`.
3. If the team directory may change (team renamed/moved), invalidate on team lifecycle events.

**Verification:** Team tool tests. Verify tools still find the correct team
directory.

---

#### PERF-026 — Fix TaskList::completed_ids O(T²) claim path

| Field | Value |
|-------|-------|
| **Severity** | Medium |
| **Effort** | S |
| **Crate** | ragent-team |
| **Affected files** | `crates/ragent-team/src/team/task.rs` (`completed_ids()`, `is_claimable()`, `next_claimable()`) |
| **Source** | Team review Issues 7.1, 7.2 |

**Description:** `completed_ids()` creates a new `Vec<String>` and scans all tasks
on every `next_claimable` / `is_claimable` call. `is_claimable` then uses
`completed_ids.contains(dep)` (linear scan on a Vec). For T tasks with D
dependencies, this is O(T * D) per claim check, and O(T²) for a full
`next_claimable` scan.

**Expected impact:** Reduces task-claim path from O(T²) to O(T) or better.

**Implementation approach:**
1. Cache `completed_ids` as a `HashSet<String>` on `TaskList`, invalidated when task status changes.
2. `is_claimable` checks dependencies against the HashSet (O(1) per dep).
3. Alternatively, maintain a `completed: HashSet<String>` alongside the tasks Vec, updated on `complete()`.

**Verification:** Task store tests with many tasks and dependencies.

---

#### PERF-027 — Cache is_plan_pending in memory

| Field | Value |
|-------|-------|
| **Severity** | Medium |
| **Effort** | S |
| **Crate** | ragent-team |
| **Affected files** | `crates/ragent-team/src/team/manager.rs` (`is_plan_pending` 1283–1292), `crates/ragent-agent/src/session/processor.rs` (call sites) |
| **Source** | Team review Issue 6.3 |

**Description:** `is_plan_pending` loads `TeamStore` from disk on every call. It's
used by the session processor to gate write/bash tools, meaning it's called on
every tool invocation for teammates in plan-pending mode.

**Expected impact:** Eliminates disk read per tool call for plan-pending checks.

**Implementation approach:**
1. Cache `is_plan_pending` result on `TeamManager` with a short TTL (e.g., 2 seconds).
2. Invalidate when `team_submit_plan` or `team_approve_plan` is called.
3. Alternatively, store a `plan_pending: AtomicBool` on the `TeammateHandle` and check it directly.

**Verification:** Plan approval flow tests.

---

### Phase 3: Deep Optimizations (L effort, architectural changes)

#### PERF-020 — Batch mailbox acknowledgements in team_read_messages

| Field | Value |
|-------|-------|
| **Severity** | Medium |
| **Effort** | M |
| **Crate** | ragent-team |
| **Affected files** | `crates/ragent-team/src/tools/team_read_messages.rs` (lines 137–145), `crates/ragent-team/src/team/mailbox.rs` |
| **Source** | Team review Issue 10.1 |

**Description:** For each unread message, `mailbox.acknowledge(&m.message_id)`
is called, which does a full lock → read → deserialize → mark → serialize → write
cycle. For N unread messages, this is N full read-modify-write cycles.

**Expected impact:** Reduces from N file operations to 1 per `team_read_messages` call.

**Implementation approach:**
1. Add `Mailbox::mark_all_read(&self, message_ids: &[String]) -> Result<()>` that does a single lock → read → deserialize → mark all → serialize → write → unlock.
2. Update `team_read_messages` to collect all message IDs and call `mark_all_read` once.

**Verification:** `team_read_messages` tests with multiple messages.

---

#### PERF-021 — Concurrent broadcast in team_broadcast

| Field | Value |
|-------|-------|
| **Severity** | Medium |
| **Effort** | M |
| **Crate** | ragent-team |
| **Affected files** | `crates/ragent-team/src/tools/team_broadcast.rs` (lines 80–96) |
| **Source** | Team review Issue 10.2 |

**Description:** The broadcast loop iterates active members sequentially, calling
`Mailbox::open` + `Mailbox::push` for each. For T teammates, this is T sequential
file lock acquisitions + T full file rewrites.

**Expected impact:** Reduces broadcast time from O(T) sequential to O(1) parallel
(wall-clock).

**Implementation approach:**
1. Use `futures::future::join_all` or `tokio::task::spawn` per teammate to push concurrently.
2. Since each mailbox is a separate file with a separate lock, there's no contention.
3. Collect results and report partial failures.

**Verification:** Broadcast tests with multiple teammates.

---

#### PERF-022 — Switch mailbox to append-only JSONL format

| Field | Value |
|-------|-------|
| **Severity** | High |
| **Effort** | L |
| **Crate** | ragent-team |
| **Affected files** | `crates/ragent-team/src/team/mailbox.rs` (entire file) |
| **Source** | Team review Issue 4.1 |

**Description:** `Mailbox::push` re-reads and re-serializes the entire mailbox
file for every single message. As mailboxes grow (messages are never pruned), this
degrades linearly to O(N) per send.

**Expected impact:** Converts `push` from O(N) full-file rewrite to O(1) append.
Enables streaming reads. Reduces memory usage.

**Implementation approach:**
1. Change mailbox file format from a single JSON array to newline-delimited JSON (JSONL) — one `MailboxMessage` per line.
2. `push` appends a single line to the file (no read-modify-write needed).
3. `read_all` / `peek_unread` / `drain_unread` read lines incrementally (can use `BufReader::lines()`).
4. `mark_read` updates a single line — this requires read-modify-write, but only for the specific message (can use line-offset indexing or a separate `read_status.json` index file).
5. Add a migration path for existing JSON-format mailbox files: detect format on open (first char is `[` → old format, `{` → JSONL) and convert.
6. Optionally prune read messages older than a threshold during `drain_unread`.

**Risk:** Changing file format requires a migration. Old mailbox files must be
read correctly. Add format detection (first non-whitespace char: `[` = array,
`{` = JSONL). This is the largest single change in the plan.

**Verification:** Mailbox round-trip tests. Migration test (read old format,
write new format, read back). Concurrency test (concurrent push from multiple
teammates).

---

#### PERF-023 — In-memory TaskList cache with write-through persistence

| Field | Value |
|-------|-------|
| **Severity** | High |
| **Effort** | L |
| **Crate** | ragent-team |
| **Affected files** | `crates/ragent-team/src/team/manager.rs`, `crates/ragent-team/src/team/task.rs` |
| **Source** | Team review Issue 4.2, Architectural Recommendation 1 |

**Description:** `TaskStore` re-deserializes the entire `tasks.json` on every
mutation (claim, complete, add, update, remove). For teams with many tasks, this
is wasteful repeated work.

**Expected impact:** Eliminates repeated full-file deserialization. Task
operations become O(1) memory operations + O(1) write-through.

**Implementation approach:**
1. Maintain an in-memory `TaskList` on `TeamManager`, loaded once on team open.
2. All task operations mutate the in-memory list and write-through to disk.
3. Use file-watcher or mtime check to detect external modifications and reload.
4. The `flock` is still acquired for the write-through to maintain cross-process consistency.
5. Alternatively, migrate to SQLite (already a dependency) for individual row updates.

**Risk:** In-memory cache must stay consistent with on-disk state across
processes. The file-watcher must handle race conditions (external write during
in-memory mutation). Use `flock` + mtime comparison on every operation as a
safety net.

**Verification:** Task store tests with concurrent operations. Cross-process
consistency test (two `TeamManager` instances operating on the same team).

---

#### PERF-024 — Narrow spawn_lock scope in TeamManager

| Field | Value |
|-------|-------|
| **Severity** | Medium |
| **Effort** | M |
| **Crate** | ragent-team |
| **Affected files** | `crates/ragent-team/src/team/manager.rs` (spawn_lock line 478, locked at line 596) |
| **Source** | Team review Issue 5.2 |

**Description:** `spawn_teammate_internal` acquires `spawn_lock` at the start and
holds it for the entire spawn process: config load, agent ID allocation, session
creation, system prompt building, memory loading, handle registration, and the
`tokio::spawn` of the agent loop. Only the agent ID allocation + config update
needs serialization.

**Expected impact:** Enables concurrent teammate spawning.

**Implementation approach:**
1. Narrow the lock to only cover: config read → `next_agent_id()` → `add_member()` → `save()`.
2. Release the lock before session creation and agent loop spawning.
3. The handle registration can use a separate short-lived lock or atomic operation.

**Verification:** Spawn tests with concurrent spawn calls.

---

#### PERF-025 — Replace handles RwLock\<HashMap\> with DashMap

| Field | Value |
|-------|-------|
| **Severity** | Medium |
| **Effort** | M |
| **Crate** | ragent-team |
| **Affected files** | `crates/ragent-team/src/team/manager.rs` (handles: Arc\<RwLock\<HashMap\>\> line 474) |
| **Source** | Team review Issue 5.1 |

**Description:** `handles` is accessed via `read().await` or `write().await` in
nearly every manager method. Under contention (many teammates + watchdog
running), this creates lock wait time. DashMap is already used in the orchestrator
per the README.

**Expected impact:** Lock-free concurrent access to teammate handles.

**Implementation approach:**
1. Replace `Arc<RwLock<HashMap<String, TeammateHandle>>>` with `Arc<DashMap<String, TeammateHandle>>`.
2. Update all access sites: `.read().await` → `.get(&id)`, `.write().await` → `.get_mut(&id)`.
3. DashMap is already a dependency (used in ragent-agent).

**Verification:** Manager tests. Concurrent access test.

---

#### PERF-028 — Offload build_system_prompt_with_storage I/O to spawn_blocking

| Field | Value |
|-------|-------|
| **Severity** | High |
| **Effort** | L |
| **Crate** | ragent-agent |
| **Affected files** | `crates/ragent-agent/src/agent/mod.rs` (lines ~1850–2033) |
| **Source** | Agent review H-6 |

**Description:** `build_system_prompt_with_storage` performs multiple synchronous
filesystem reads and SQLite queries on the async path: `collect_agents_md_content`
(recursive directory walk), `read_git_status` (spawns 3 `std::process::Command`
processes), `read_readme`, memory block file reads, `sqlite_storage.list_memories()`.

**Expected impact:** Prevents async runtime blocking during system prompt
construction.

**Implementation approach:**
1. Wrap all file I/O and SQLite queries in `spawn_blocking`.
2. Accept pre-loaded data (agents_md, git_status, readme) as parameters — these are already cached by `PromptContextCache`.
3. Move memory block loading and structured memory queries to `spawn_blocking`.
4. The `collect_agents_md_content` recursive walk should be in `spawn_blocking` (it already is via `collect_prompt_context`, but the standalone call in `build_system_prompt_with_storage` is not).

**Verification:** System prompt tests. Verify prompt content is identical.

---

#### PERF-029 — Use Arc\<str\>/Arc\<Value\> in history_to_chat_messages

| Field | Value |
|-------|-------|
| **Severity** | Medium |
| **Effort** | L |
| **Crate** | ragent-agent |
| **Affected files** | `crates/ragent-agent/src/session/processor.rs` (lines ~3268–3343), `crates/ragent-types/src/event/mod.rs` (ChatContent, ContentPart types) |
| **Source** | Agent review M-9, M-8 |

**Description:** `history_to_chat_messages` clones every `text`, `tool`,
`call_id`, `state.input`, `state.output` during conversion. For sessions with
many tool calls and large JSON inputs/outputs, this is expensive. The version
cache (FR-006) helps when history hasn't changed, but the first conversion per step
pays the full cost.

**Expected impact:** Converts deep clone to O(1) Arc clone for all message
content.

**Implementation approach:**
1. Change `ChatContent::Text(String)` to `ChatContent::Text(Arc<str>)`.
2. Change `ContentPart::ToolUse { input: Value }` to `ContentPart::ToolUse { input: Arc<Value> }`.
3. `history_to_chat_messages` wraps existing data in `Arc` (O(1)) instead of cloning.
4. Provider implementations deref with `&*input` where they need `&Value`.
5. Also wrap `assistant_parts` in `Arc<Vec<MessagePart>>` for the interim storage update (M-8).

**Risk:** Changing `ChatContent` and `ContentPart` types touches all provider
implementations and serialization code. This is a cross-crate type change.

**Verification:** `cargo test` across all crates. LLM provider tests. Serialization
round-trip tests.

---

#### PERF-030 — Reuse event bus in extracted tool adapters

| Field | Value |
|-------|-------|
| **Severity** | Low |
| **Effort** | M |
| **Crate** | ragent-agent |
| **Affected files** | `crates/ragent-agent/src/tool/mod.rs` (lines ~511–540 for ExtractedCoreToolAdapter, ~840 for ExtractedExtendedToolAdapter) |
| **Source** | Agent review L-8 |

**Description:** Every execution of an extracted tool creates a new `EventBus`
(`Arc::new(EventBus::new(256))`), spawns two tokio tasks (event forwarders), and
aborts them after execution. This means every single tool call allocates an event
bus, two task stacks, and two channel subscriptions.

**Expected impact:** Eliminates per-tool-call event bus allocation and task spawning.

**Implementation approach:**
1. Create the adapter's event bus once (lazily, stored on the adapter struct) and reuse it across calls.
2. The forwarder tasks can be long-lived (spawn once, not per call) — they forward events from the adapter bus to the session bus.
3. For tools that don't emit events, skip the forwarder entirely.

**Verification:** Tool execution tests. Verify events still propagate correctly.

---

#### PERF-031 — Use ahash/FxHash for non-cryptographic cache keys

| Field | Value |
|-------|-------|
| **Severity** | Low |
| **Effort** | S |
| **Crate** | ragent-agent |
| **Affected files** | `crates/ragent-agent/src/session/processor.rs` (line ~3258), `crates/ragent-agent/src/session/cache.rs` (line ~171) |
| **Source** | Agent review L-1 |

**Description:** `DefaultHasher` (SipHash-1-3) is used for cache keying. While
cryptographically resistant, it is slower than alternatives like `FxHash` or
`AHash` for non-adversarial cache keying.

**Expected impact:** 2–5× faster hashing for cache keys.

**Implementation approach:**
1. Add `ahash` or `rustc-hash` as a dependency.
2. Replace `DefaultHasher` with `AHasher` (from `ahash`) or `FxHasher` (from `rustc-hash`) in cache key construction.
3. These are non-cryptographic hashers optimized for speed on short keys.

**Verification:** Cache tests. Verify cache invalidation still works correctly.

---

## 3. Milestones

### Phase 1: Quick Wins (1–2 weeks)

**Target outcome:** Eliminate the most impactful low-effort issues. Measurable
reduction in per-turn allocations and disk I/O without architectural changes.

**Tasks:** PERF-001, PERF-002, PERF-003, PERF-004, PERF-005, PERF-015

**Expected improvements:**
- 2–3 fewer disk reads per `process_user_message`
- ~131 fewer String allocations per agent step (111 tool names + ~20 session_id)
- 1 fewer SQLite round-trip per session lookup
- 1 fewer String allocation per tool result
- Duplicate event bug fixed (correctness)

**Exit criteria:** All 6 tasks merged, tests passing, benchmark suite shows
measurable improvement in allocation count.

---

### Phase 2: Medium Complexity (2–4 weeks)

**Target outcome:** Eliminate hot-path cloning of large data, reduce team crate
disk I/O by 60–80%, fix O(n²) algorithms.

**Tasks:** PERF-006, PERF-007, PERF-008, PERF-009, PERF-010, PERF-011, PERF-012,
PERF-013, PERF-014, PERF-016, PERF-017, PERF-018, PERF-019, PERF-026, PERF-027

**Expected improvements:**
- system_prompt, chat_messages, agent.options, session_config clones → O(1) Arc clones
- Storage existence checks → single SQL query instead of full message load
- Builtin agents cached in OnceLock (eliminates ~15 AgentInfo constructions per resolution)
- Tool registry cache invalidation → O(1) version check instead of O(n) hashing
- All team file I/O moved to `spawn_blocking` (async runtime no longer blocked)
- team_task_claim: 5 → ~2 file operations
- team_task_complete: 3+ → ~1 file operations
- team_dir cached (no directory walk per tool call)
- Task claim path: O(T²) → O(T)
- is_plan_pending: disk read → in-memory check

**Exit criteria:** All 15 tasks merged, full test suite passing, no regressions
in agent loop or team coordination behavior. Benchmark suite shows 40%+ reduction
in per-step allocation count and 50%+ reduction in team operation latency.

---

### Phase 3: Deep Optimizations (3–6 weeks)

**Target outcome:** Architectural improvements for scaling. Mailbox format change,
in-memory caching, concurrent operations, type-level Arc adoption.

**Tasks:** PERF-020, PERF-021, PERF-022, PERF-023, PERF-024, PERF-025, PERF-028,
PERF-029, PERF-030, PERF-031

**Expected improvements:**
- Mailbox push: O(N) → O(1) (append-only JSONL)
- Task operations: O(disk) → O(memory) with write-through
- Broadcast: O(T) sequential → O(1) parallel wall-clock
- Concurrent teammate spawning (spawn_lock narrowed)
- Lock-free handle access (DashMap)
- System prompt I/O offloaded from async runtime
- History conversion: deep clone → Arc clone (cross-crate type change)
- Tool adapter event bus reused (no per-call allocation)
- Faster cache key hashing (2–5×)

**Exit criteria:** All 10 tasks merged, full test suite passing including
migration tests for mailbox format. Benchmark suite shows 60%+ reduction in
per-turn latency for large sessions/teams. No correctness regressions.

---

## 4. Benchmarking Strategy

### 4.1 Existing Benchmarks

The project already has a Criterion benchmark suite at
`crates/ragent-agent/benches/agent_loop.rs` measuring:
- `estimate_request_bytes` (small/medium/large)
- `serde_json::to_vec` payload size (small/medium/large)
- `compress_history` (with compression feature)
- `compiled_dir_lists`
- `tool_result_truncation`

**Action:** Run these benchmarks before and after each phase to establish baselines
and measure deltas.

```bash
# Establish baseline
cargo bench -p ragent-agent --bench agent_loop -- --save-baseline pre-phase1

# After Phase 1
cargo bench -p ragent-agent --bench agent_loop -- --baseline pre-phase1

# After Phase 2
cargo bench -p ragent-agent --bench agent_loop -- --baseline pre-phase1
```

### 4.2 New Benchmarks Needed

| Benchmark | Measures | Crate | Priority |
|-----------|----------|-------|----------|
| `bench_config_load` | Config::load() call count + latency | agent | High |
| `bench_tool_context_construction` | ToolContext allocation per call | agent | High |
| `bench_history_conversion` | history_to_chat_messages with N messages | agent | Medium |
| `bench_team_task_claim` | team_task_claim latency (cold + warm cache) | team | High |
| `bench_team_mailbox_push` | Mailbox::push with N existing messages | team | High |
| `bench_team_broadcast` | team_broadcast with T teammates | team | Medium |

### 4.3 Metrics to Track

**Per-turn metrics (agent loop):**
- Time-to-first-token (TTFT)
- Per-step latency (median, p95, p99)
- Allocations per step (use `dhat` or `jemalloc` stats)
- Disk reads per turn (count)
- Disk writes per turn (count)
- JSON serialization calls per turn (count)
- Tool-definition caching hit rate

**Team coordination metrics:**
- `team_task_claim` latency (cold cache / warm cache)
- `team_task_complete` latency
- `team_read_messages` latency (N messages)
- `team_broadcast` latency (T teammates)
- Mailbox push latency (N existing messages)
- Concurrent teammate operation throughput

**System-level metrics:**
- Tokio runtime thread starvation (blocked time per worker thread)
- Peak memory usage per session
- Peak memory usage per team

### 4.4 Benchmark Protocol

1. **Before any optimization:** Run full benchmark suite, save as baseline.
   Capture `docs/reports/agent_loop_perf_baseline.md` (per FR-003 of AgentPerf spec).
2. **After each task:** Run affected benchmarks, compare to baseline.
3. **After each phase:** Run full benchmark suite, generate comparison report in
   `target/criterion/agent_loop/change/`.
4. **Regression gating:** If any benchmark regresses by >10%, investigate before
   proceeding.

### 4.5 Profiling

- Set `RAGENT_AGENT_PERF=1` to enable per-scope timing logs (per FR-002 of AgentPerf spec).
- Use `cargo flamegraph` for CPU profiling of the agent loop.
- Use `dhat` for heap profiling to measure allocation reductions.
- Use `tokio-console` to identify async task stalls and runtime starvation.

---

## 5. Risk & Rollback

### 5.1 High-Risk Changes

| Task | Risk | Mitigation | Rollback |
|------|------|------------|----------|
| PERF-006 (Arc\<str\> for system) | Changes `ChatRequest.system` type — touches all 11+ provider implementations | Comprehensive provider test suite; incremental migration with `From<String>` impls | Revert to `Option<String>`; all provider code already handles `&str` via deref |
| PERF-007 (Arc\<Vec\> for messages) | COW semantics needed if messages mutate between attempts | Use `Arc::make_mut` for mutation paths; verify no code mutates between attempts | Revert to `Vec<ChatMessage>` clone |
| PERF-008 (Arc\<HashMap\> for options) | Any post-construction mutation needs `Arc::make_mut` | Audit all `agent.options` mutation sites; most are read-only after construction | Revert to `HashMap<String, Value>` |
| PERF-016 (spawn_blocking for all team I/O) | `spawn_blocking` has ~10µs overhead per spawn; for very fast ops, overhead may exceed benefit | Benchmark net positive before merging; use `try_io` pattern for fast paths | Revert to synchronous calls |
| PERF-022 (JSONL mailbox) | File format change requires migration; old files must still be read | Format detection on open (first char: `[` = old, `{` = new); auto-migrate on first write; keep old reader as fallback | Revert to JSON array format; JSONL files can be read by joining lines and parsing as array |
| PERF-023 (in-memory TaskList cache) | Cross-process consistency; external writes not seen | Use mtime check + flock on every operation; file-watcher for invalidation | Revert to read-on-every-operation |
| PERF-029 (Arc types in ChatContent/ContentPart) | Cross-crate type change — touches ragent-types, ragent-llm, ragent-agent, all providers | Incremental: add Arc variants alongside String, migrate callers, then remove String variants | Revert to owned types |

### 5.2 Medium-Risk Changes

| Task | Risk | Mitigation |
|------|------|------------|
| PERF-010 (has_assistant_messages) | SQL query change; must match existing schema | Test against real SQLite DB with existing sessions |
| PERF-012 (ToolRegistry version) | Version counter must increment on ALL registration paths (including MCP, custom agents) | Audit `register()` call sites; add test that registers a tool and checks version bump |
| PERF-024 (narrow spawn_lock) | Concurrent spawns may race on agent ID allocation | Lock covers only the ID allocation + config update; session creation is outside lock |
| PERF-025 (DashMap for handles) | DashMap iteration order is non-deterministic; any code depending on order will break | Audit all `handles.iter()` sites; use sorted iteration where order matters |

### 5.3 Low-Risk Changes

Tasks PERF-001, PERF-002, PERF-003, PERF-004, PERF-005, PERF-011, PERF-013,
PERF-014, PERF-015, PERF-019, PERF-026, PERF-027, PERF-031 are low-risk:
they are localized changes with clear before/after behavior equivalence and
can be reverted individually without affecting other tasks.

### 5.4 Rollback Strategy

- Each task should be a separate commit (or small PR) for easy revert.
- Phase 1 tasks are independently revertible.
- Phase 2 tasks are mostly independent, except PERF-006/007/008/009 form a related
  set (Arc adoption) — revert as a group if issues arise.
- Phase 3 tasks have dependencies: PERF-022 (JSONL) must be reverted before
  PERF-023 (in-memory cache) if the cache relies on the new format. PERF-029
  (Arc types) is a cross-crate change that must be reverted atomically.
- Always run the full test suite after reverting any task.

### 5.5 Correctness Verification

For every task:
1. **Unit tests:** All existing tests must pass without modification.
2. **Integration tests:** Agent loop tests, team coordination tests, provider tests.
3. **Behavioral equivalence:** The agent's observable behavior (LLM calls, tool
   results, event emissions, session storage) must be identical before and after.
4. **Concurrency tests:** For team crate changes, run concurrent teammate
   scenarios to verify no deadlocks or race conditions are introduced.
5. **Migration tests:** For PERF-022 (mailbox format), test reading old-format
   files and verify auto-migration.

---

## 6. Dependency Graph

```
Phase 1 (independent):
  PERF-001 ──┐
  PERF-002 ──┤
  PERF-003 ──┤  (all can be done in parallel)
  PERF-004 ──┤
  PERF-005 ──┤
  PERF-015 ──┘

Phase 2:
  PERF-001 ──► PERF-009 (ToolContext uses cached Config)
  PERF-012 ──► PERF-003 (tool_names cache uses registry version)
  PERF-012 ──► PERF-013 (sub-agent cache uses registry version)
  PERF-016 ──► PERF-017, PERF-018 (redundant loads less impactful if I/O is async)
  PERF-019 ──► PERF-017, PERF-018 (cached team_dir needed before eliminating loads)
  PERF-005 ──► PERF-018 (duplicate event removed first, then remaining loads)

Phase 3:
  PERF-022 ──► PERF-023 (JSONL mailbox before in-memory task cache)
  PERF-016 ──► PERF-022 (spawn_blocking before format change)
  PERF-016 ──► PERF-023 (spawn_blocking before in-memory cache)
  PERF-006 ──► PERF-029 (Arc<str> in ChatRequest before Arc in ChatContent)
```

---

## 7. Summary Table

| ID | Title | Phase | Severity | Effort | Expected Impact |
|----|-------|-------|----------|--------|-----------------|
| PERF-001 | Load Config once per process_user_message | 1 | High | S | -2-3 disk reads/turn |
| PERF-002 | Store session_id as Arc\<str\> | 1 | High | S | -20 allocs/step |
| PERF-003 | Cache tool_names Vec | 1 | High | S | -111 allocs/step |
| PERF-004 | Cache format_version in Storage | 1 | High | S | -1 SQLite query/lookup |
| PERF-005 | Remove duplicate TeamTaskCompleted event | 1 | High | S | Fix correctness bug, -50% event I/O |
| PERF-015 | Fix tool_result_content_for_llm allocation | 1 | Medium | S | -1 alloc/tool result |
| PERF-006 | Arc\<str\> for system_prompt | 2 | High | M | O(n)→O(1) clone/step |
| PERF-007 | Arc\<Vec\> for chat_messages | 2 | High | M | O(n)→O(1) clone/retry |
| PERF-008 | Arc\<HashMap\> for agent.options | 2 | High | M | O(n)→O(1) clone/step |
| PERF-009 | Pre-compute ToolContext template | 2 | High | M | -1 Config clone/tool call |
| PERF-010 | has_assistant_messages() in Storage | 2 | High | M | O(n)→O(1) existence check |
| PERF-011 | Cache builtin agents in OnceLock | 2 | Medium | S | -15 AgentInfo constructions |
| PERF-012 | ToolRegistry version counter | 2 | Medium | S | O(n)→O(1) cache check |
| PERF-013 | Sub-agent tool reference caching | 2 | Medium | S | -1 string build/sub-agent step |
| PERF-014 | Eliminate estimate_request_bytes serialization | 2 | Medium | S | -111 JSON serializations/step |
| PERF-016 | spawn_blocking for team I/O | 2 | High | L | Frees async runtime |
| PERF-017 | Eliminate redundant loads in team_task_claim | 2 | High | M | 5→2 file ops/claim |
| PERF-018 | Eliminate redundant loads in team_task_complete | 2 | High | S | 3→1 file ops/complete |
| PERF-019 | Cache team_dir path | 2 | High | S | -1 dir walk/tool call |
| PERF-026 | Fix completed_ids O(T²) | 2 | Medium | S | O(T²)→O(T) claim path |
| PERF-027 | Cache is_plan_pending | 2 | Medium | S | -1 disk read/tool call |
| PERF-020 | Batch mailbox acknowledgements | 3 | Medium | M | N→1 file ops/read_messages |
| PERF-021 | Concurrent broadcast | 3 | Medium | M | O(T)→O(1) wall-clock |
| PERF-022 | JSONL mailbox format | 3 | High | L | O(N)→O(1) push |
| PERF-023 | In-memory TaskList cache | 3 | High | L | O(disk)→O(memory) task ops |
| PERF-024 | Narrow spawn_lock scope | 3 | Medium | M | Concurrent spawns |
| PERF-025 | DashMap for handles | 3 | Medium | M | Lock-free access |
| PERF-028 | spawn_blocking for system prompt I/O | 3 | High | L | Frees async runtime |
| PERF-029 | Arc types in ChatContent/ContentPart | 3 | Medium | L | O(n)→O(1) history clone |
| PERF-030 | Reuse tool adapter event bus | 3 | Low | M | -1 EventBus+2 tasks/call |
| PERF-031 | ahash/FxHash for cache keys | 3 | Low | S | 2-5× faster hashing |

---

## 8. References

- **Agent performance review:** `docs/reports/ragent-agent-performance-review.md` (swarm-s1, 2026-06-22)
- **Team performance review:** `docs/swarm-s2-ragent-team-performance-review.md` (swarm-s2, 2026-06-22)
- **AgentPerf spec (v1, implemented):** `specs/AgentPerf/SPEC.md`, `specs/AgentPerf/PLAN.md`
- **Existing benchmark suite:** `crates/ragent-agent/benches/agent_loop.rs`
- **Performance benchmark guide:** `docs/performance/benchmark-guide.md`