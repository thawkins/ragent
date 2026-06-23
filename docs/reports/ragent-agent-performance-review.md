# ragent-agent Performance Review

**Reviewer:** swarm-s1
**Date:** 2026-06-22
**Scope:** `crates/ragent-agent/` — all source files, with emphasis on the primary agent loop (`session/processor.rs`) and sub-agent paths (`task/mod.rs`).

---

## Summary

The `ragent-agent` crate is the orchestration heart of the application. It contains the main agent loop (`SessionProcessor::process_user_message`), sub-agent spawning (`TaskManager`), tool registry, storage layer, hooks, permissions, compression pipeline, and prompt construction. The crate is large (~4,140 lines in processor.rs alone) and has already received significant performance work (AgentPerf spec, system prompt caching, history version caching, storage_op offloading, stream buffering). However, numerous performance issues remain across allocation patterns, lock contention, redundant work, blocking I/O on async paths, and data-structure inefficiencies.

Issues are categorized by severity: **High** (hot path, significant impact), **Medium** (moderate impact or less frequent), **Low** (minor or edge-case).

---

## High Severity

### H-1: `Config::load()` called multiple times per `process_user_message`

**File:** `crates/ragent-agent/src/session/processor.rs`, lines ~904–986
**Description:** `crate::Config::load()` is called at least 3 times within a single `process_user_message` invocation:
1. Line ~905: `let cfg = crate::Config::load().ok();` (for generic_openai base_url)
2. Line ~922: `let cfg = crate::Config::load().ok();` (for azure_foundry base_url)
3. Line ~974: `if let Ok(cfg) = crate::Config::load()` (for provider options)
4. Line ~1043: `let session_config = crate::Config::load().unwrap_or_default();`

`Config::load()` reads and parses a JSON file from disk on every call. Even though it may have internal caching at the config crate level, the repeated calls add overhead and redundant parsing. The config should be loaded **once** at the top of `process_user_message` and reused.
**Suggested Fix:** Load config once at the start of `process_user_message` into a local variable and pass it to all downstream code paths. Consider caching the parsed `Config` in the `SessionProcessor` itself.

---

### H-2: `system_prompt.clone()` on every LLM request attempt

**File:** `crates/ragent-agent/src/session/processor.rs`, line ~1711
**Description:** Inside the retry loop, each attempt builds a fresh `ChatRequest` and clones the entire system prompt string (`system: Some(system_prompt.clone())`). The system prompt can be very large (often 5,000–20,000 characters with tool references, file tree, README, agents.md, etc.). This clone happens on every retry attempt and on every step of the agent loop.
**Suggested Fix:** Wrap `system_prompt` in `Arc<str>` or `Arc<String>` and pass `Arc::clone()` (which is just an atomic increment) instead of cloning the string contents. The `ChatRequest` already uses `Arc` for `messages` and `tools`, so `system` should follow the same pattern.

---

### H-3: `chat_messages.clone()` on every LLM retry attempt

**File:** `crates/ragent-agent/src/session/processor.rs`, line ~1706
**Description:** `messages: Arc::new(chat_messages.clone())` — On every retry attempt, the entire chat message vector (which grows with each step) is cloned into a new `Vec` and then wrapped in `Arc`. This is O(n) in message count on every retry. The first attempt also clones unnecessarily since `chat_messages` is owned and could be moved.
**Suggested Fix:** Keep `chat_messages` as an `Arc<Vec<ChatMessage>>` throughout the loop. On retry, `Arc::clone()` is O(1). Only clone the underlying `Vec` when the loop actually needs to mutate it (after tool results are appended). Alternatively, only clone on retry (when buffers are reset) and pass by reference on the first attempt.

---

### H-4: `agent.options.clone()` and `agent.thinking.clone()` on every step

**File:** `crates/ragent-agent/src/session/processor.rs`, lines ~1712, 1722
**Description:** `options: agent.options.clone()` and `thinking: agent.thinking.clone()` are cloned for every `ChatRequest` construction, which happens on every step of the agent loop. `agent.options` is a `HashMap<String, serde_json::Value>` which can contain multiple entries, and each clone allocates.
**Suggested Fix:** Store `agent.options` as `Arc<HashMap<String, Value>>` in `AgentInfo` so it can be cheaply cloned. Alternatively, since the same agent is used for all steps of a session, compute the request options once before the loop and reuse the clone.

---

### H-5: `ToolContext` construction clones many Arcs and strings per tool call

**File:** `crates/ragent-agent/src/session/processor.rs`, lines ~2260–2277
**Description:** For **every** tool call in every step, a new `ToolContext` is constructed that clones:
- `session_id.to_string()` (String allocation)
- `working_dir.clone()` (PathBuf clone)
- `self.event_bus.clone()` (Arc clone — cheap)
- `self.session_manager.storage().clone()` (Arc clone — cheap)
- `self.task_manager.get().cloned()` (Option<Arc> clone — cheap)
- `model_ref.clone()` (ModelRef clone with two Strings)
- `team_context_for_session.clone()` (Option<Arc<TeamContext>> — cheap)
- `session_config.clone()` wrapped in `Arc::new()` (clones the entire Config struct!)

The `session_config.clone()` is the most expensive — `Config` contains HashMaps, Vecs, and nested structures. This happens for every single tool invocation.
**Suggested Fix:** Pre-compute a `ToolContext` template once per `process_user_message` call and clone only the fields that change per tool call. Store `session_config` as `Arc<Config>` once and pass the Arc clone.

---

### H-6: `build_system_prompt_with_storage` performs synchronous file I/O on the async path

**File:** `crates/ragent-agent/src/agent/mod.rs`, lines ~1850–2033
**Description:** `build_system_prompt_with_storage` performs multiple synchronous filesystem reads:
- `collect_agents_md_content(working_dir)` — recursive directory walk using `ignore::WalkBuilder` (line ~1729)
- `read_git_status(working_dir)` — spawns 3 synchronous `std::process::Command` processes (lines ~1401–1452)
- `read_readme(working_dir)` — synchronous `std::fs::read_to_string` (line ~1457)
- `load_all_blocks(&block_storage, &wd)` — reads memory block files from disk
- `load_legacy_memory()` — reads MEMORY.md files
- `std::fs::read_to_string(&project_analysis)` — reads PROJECT_ANALYSIS.md
- `sqlite_storage.list_memories()` �� SQLite query (blocking)

While `collect_prompt_context` (called earlier in `process_user_message` at line ~1083) does cache these results, `build_system_prompt_with_storage` **re-reads** `agents_md`, `git_status`, and `readme` from disk when they are passed as `None`. The caller at line ~1087 does pass the cached values, so the re-read is avoided — but the function itself still does synchronous SQLite queries and file reads for memory blocks, which run on the async executor thread.
**Suggested Fix:** Wrap all file I/O and SQLite queries in `spawn_blocking` or accept pre-loaded data. The memory block loading and structured memory queries should be offloaded.

---

### H-7: `Storage::get_messages` loads ALL messages with no pagination

**File:** `crates/ragent-agent/src/storage/mod.rs`, lines ~744–783
**Description:** `get_messages` executes `SELECT ... FROM messages WHERE session_id = ?1 ORDER BY created_at ASC` and loads **all** messages for the session into memory, deserializing each message's `parts` JSON column. For long sessions with hundreds of messages, this is O(n) in message count and O(n * parts_size) in deserialization. It is called:
- In `process_user_message` (line ~1053) to check for prior assistant messages — only needs a count, not full content
- In `process_user_message` (line ~1288) to load history for the LLM call — needs full content
- In `run_init_exchange` (line ~2953) — only checks for existence of assistant messages

The first and third calls only need to know **if** an assistant message exists, yet they load and deserialize all messages.
**Suggested Fix:** Add a `has_assistant_messages(session_id)` method that runs `SELECT 1 FROM messages WHERE session_id = ?1 AND role = 'assistant' LIMIT 1`. For the full history load, consider lazy loading or only loading the most recent N messages needed for the context window.

---

### H-8: `Storage::get_session` queries `pragma_table_info` on every call

**File:** `crates/ragent-agent/src/storage/mod.rs`, lines ~490–546
**Description:** Every call to `get_session` runs `SELECT COUNT(*) FROM pragma_table_info('sessions') WHERE name='format_version'` to check if the column exists. This metadata query is executed on **every** session lookup, adding a round-trip to SQLite for no reason after the initial migration. The same pattern appears in `list_sessions` (line ~568).
**Suggested Fix:** Cache the `has_format_version` flag after the first query, or check it once during `migrate()` and store it in an `AtomicBool` on the `Storage` struct.

---

### H-9: Event publishing allocates strings for every event on the hot path

**File:** `crates/ragent-agent/src/session/processor.rs`, multiple locations
**Description:** The agent loop publishes events with `.to_string()` allocations on every iteration:
- Line ~1529: `session_id.to_string()` for AgentError event
- Line ~1574: `session_id.to_string()` for ToolsSent event (plus `tool_names.clone()`)
- Line ~1844: `session_id.to_string()` for TextDelta events
- Line ~1920: `tc.id.clone()`, `tc.name.clone()`, `tc.args_json.clone()` for ToolCallArgs
- Line ~2603: Multiple `.clone()` calls for ToolCallEnd
- Line ~2641: Multiple `.clone()` calls for ToolResult

`session_id` is a `&str` that gets `.to_string()` called dozens of times per agent loop step. Each call allocates a new `String`.
**Suggested Fix:** Store `session_id` as an `Arc<str>` or `String` once at the start of `process_user_message` and clone the Arc (O(1)) instead of allocating a new String each time. The `EventBus` publish path already takes owned strings, so the goal is to avoid repeated heap allocations from `&str.to_string()`.

---

### H-10: `tool_definitions.iter().map(|t| t.name.clone()).collect()` on every step

**File:** `crates/ragent-agent/src/session/processor.rs`, line ~1573
**Description:** On every step of the agent loop, the tool names are collected into a `Vec<String>` for the `ToolsSent` event: `let tool_names: Vec<String> = tool_definitions.iter().map(|t| t.name.clone()).collect();`. With ~111 tools, this allocates 111 Strings on every step. This event is published even when tools haven't changed.
**Suggested Fix:** Cache the tool name list alongside `cached_tool_definitions` and only rebuild when the cache is invalidated. Or skip the `ToolsSent` event entirely when the tool set hasn't changed since the previous step.

---

## Medium Severity

### M-1: `resolve_agent()` calls `create_builtin_agents()` on every invocation

**File:** `crates/ragent-agent/src/agent/mod.rs`, line ~1244
**Description:** `resolve_agent` calls `create_builtin_agents()` which constructs a `Vec<AgentInfo>` with ~15 agents, each containing `String` allocations for name, description, prompt (some prompts are very long — 500+ chars), and permission rulesets. This happens on every agent resolution, which occurs at the start of every `process_user_message` and every sub-agent spawn.
**Suggested Fix:** Cache the builtin agents list in a `static OnceLock<Vec<AgentInfo>>` and search the cached list. The agent definitions are static and never change at runtime.

---

### M-2: `hash_tool_registry` iterates all tools and hashes descriptions on every cache check

**File:** `crates/ragent-agent/src/session/cache.rs`, lines ~358–366
**Description:** `SystemPromptCache::hash_tool_registry` calls `registry.definitions()` (which acquires a read lock, filters, maps, sorts, and returns a `Vec<ToolDefinition>` with cloned names/descriptions/parameters) and then hashes each tool's name and description. This is called on every `get_tool_reference` check. With ~111 tools, each with a description, this is O(n) string hashing on every system prompt cache lookup.
**Suggested Fix:** Cache the hash on the `ToolRegistry` itself (increment on register) and expose it as a method, so the cache check is O(1). Alternatively, use a cheaper change-detection mechanism like a version counter on the registry.

---

### M-3: `build_detailed_tool_reference_section` called for every sub-agent without caching

**File:** `crates/ragent-agent/src/session/processor.rs`, line ~1111
**Description:** When `is_subagent` is true, `build_detailed_tool_reference_section(&self.tool_registry)` is called directly without going through the `SystemPromptCache`. This function iterates all tool definitions, formats each parameter schema, and builds a large string. It runs on every sub-agent message processing.
**Suggested Fix:** Route sub-agent tool references through the same `SystemPromptCache` mechanism (or a separate sub-agent cache entry) so the expensive string building only happens once per tool-registry change.

---

### M-4: `PendingToolCall` cloned for every tool execution spawn

**File:** `crates/ragent-agent/src/session/processor.rs`, line ~2278
**Description:** `let tc_clone = tc.clone();` clones the entire `PendingToolCall` (id, name, args_json — three Strings) for every tool call that is spawned as a future. The clone is needed because the original `tc` is borrowed from the `tool_calls` Vec. With multiple tool calls per step, this adds up.
**Suggested Fix:** Use `Arc<PendingToolCall>` or restructure to avoid the clone by consuming from the Vec.

---

### M-5: `split_bash_command` allocates a `Vec<String>` for every bash permission check

**File:** `crates/ragent-agent/src/session/processor.rs`, lines ~359–415
**Description:** `split_bash_command` parses a command string character by character, building a `Vec<String>` of sub-commands. It allocates a `String` for each sub-command and for the final result. This runs on every bash tool call, even for simple single commands.
**Suggested Fix:** Return an iterator or use `SmallVec<[String; 4]>` since most bash commands have 1–3 sub-commands. For the common single-command case, return a single-element vec without the full char-by-char parse.

---

### M-6: `extract_resource_from_input` allocates a String for every permission check

**File:** `crates/ragent-agent/src/session/processor.rs`, lines ~306–316
**Description:** `extract_resource_from_input` calls `.map(|s| s.to_string())` to convert the matched `&str` into an owned `String`. This allocation happens for every non-bash tool call that has a permission category.
**Suggested Fix:** Return `&str` instead of `String` where possible, or use `Cow<str>` to avoid allocation when the resource is a simple field lookup.

---

### M-7: `check_permission_with_prompt` calls `std::env::current_dir()` on every file:read

**File:** `crates/ragent-agent/src/session/processor.rs`, lines ~487–498
**Description:** For every `file:read` permission check, `std::env::current_dir()` is called and `resource.canonicalize()` is performed. `current_dir()` is a syscall and `canonicalize()` does filesystem I/O (stat, readlink). This runs on every read tool call.
**Suggested Fix:** Cache the working directory at the start of `process_user_message` and pass it to the permission checker. The working directory doesn't change within a single message processing.

---

### M-8: Interim storage update clones `assistant_parts` even with hash check

**File:** `crates/ragent-agent/src/session/processor.rs`, lines ~2856–2859
**Description:** The interim storage update computes a hash of `assistant_parts` to skip writes when unchanged. However, when the hash **does** change (which is every step that has new content), it clones the entire `assistant_parts: Vec<MessagePart>` into a new `Message` and sends it to `storage_op`. Each `MessagePart::ToolCall` contains `Value` (JSON) inputs/outputs which are expensive to clone.
**Suggested Fix:** Consider using `Arc<Vec<MessagePart>>` for `assistant_parts` so the clone is O(1). The storage operation can then `Arc::try_unwrap` or clone only if needed.

---

### M-9: `history_to_chat_messages` clones all message parts and tool call data

**File:** `crates/ragent-agent/src/session/processor.rs`, lines ~3268–3343
**Description:** `history_to_chat_messages` converts internal `Message` history to `ChatMessage` format, cloning every `text`, `tool`, `call_id`, `state.input`, `state.output` along the way. For sessions with many tool calls, each with large JSON inputs/outputs, this is expensive. While there is a version cache (FR-006), it only helps when history hasn't changed — the first conversion per step still pays the full cost.
**Suggested Fix:** Use `Arc<str>` for text content in `ChatContent::Text` and `Arc<Value>` for tool inputs in `ContentPart::ToolUse` so that cloning is O(1). The conversion can then share references to the original data rather than deep-copying.

---

### M-10: `tool_result_content_for_llm` allocates even in the fast (no-truncation) path

**File:** `crates/ragent-agent/src/session/processor.rs`, lines ~3385–3399
**Description:** In the fast path (content under threshold), the function does:
```rust
if content.len() <= MAX_TOOL_RESULT_BYTES_FOR_LLM {
    return Arc::from(content.to_string());
}
```
`content.to_string()` allocates a new `String` from the `&str`, and then `Arc::from(String)` takes ownership. The intermediate `String` allocation is unnecessary — `Arc::from(content)` (from `&str`) would avoid the intermediate allocation.
**Suggested Fix:** Use `Arc::from(content)` directly when the content is under the threshold. `Arc::<str>::from(&str)` does a single allocation.

---

### M-11: `estimate_request_bytes` calls `.to_string()` on JSON `Value`s for size estimation

**File:** `crates/ragent-agent/src/session/processor.rs`, lines ~3432–3468
**Description:** `estimate_request_bytes` (called on every LLM request to publish `RequestStarted`) serializes every `ContentPart::ToolUse { input }` and every `ToolDefinition { parameters }` to string via `.to_string()` just to get the `.len()`. This is full JSON serialization of every tool's schema on every step. With ~111 tools, this serializes ~111 JSON schemas into strings just to measure their length, then discards the strings.
**Suggested Fix:** Use `serde_json::to_string(&input).len()` only when needed, or better, cache the serialized size of tool definitions (they don't change between steps). For `ContentPart` inputs, use a cheaper size estimate based on `Value`'s internal representation.

---

### M-12: Profiler scope creation overhead even when profiling is disabled

**File:** `crates/ragent-agent/src/session/profiler.rs` and `crates/ragent-agent/src/session/processor.rs`
**Description:** The agent loop is littered with `let _scope = profiler.scope("...")` calls (50+ occurrences in `process_user_message`). Each `scope()` call creates a `ProfileScope` object that checks `is_enabled()` and potentially starts a timer. While the profiler is disabled by default, the scope objects are still constructed and dropped, adding overhead to every step. When profiling IS enabled, the `scope()` method acquires a `RwLock` write lock on the stats HashMap for every scope enter and exit.
**Suggested Fix:** Make `scope()` return a no-op `ZST` when profiling is disabled (compile-time or runtime check with early return). Use a lock-free concurrent map (e.g., `DashMap`) instead of `RwLock<HashMap>` for the stats when profiling is enabled.

---

### M-13: `resolve_team_context_for_session` scans all team directories on cache miss

**File:** `crates/ragent-agent/src/session/processor.rs`, lines ~3214–3246
**Description:** On a cache miss (every 5 seconds per session), `resolve_team_context_for_session` calls `TeamStore::list_teams(working_dir)` which lists all team directories, then loads each team store from disk and checks session membership. This is O(teams) filesystem operations.
**Suggested Fix:** The 5-second TTL cache helps, but the miss path is still expensive. Consider maintaining an in-memory index of session_id → team_name that is updated when teams are created/joined/left, making the lookup O(1).

---

### M-14: `SessionState` stored behind `std::sync::Mutex` — potential contention

**File:** `crates/ragent-agent/src/session/mod.rs` and `session/processor.rs`, line ~1374
**Description:** The per-session state cache (`SessionState`) is accessed via `self.session_manager.session_state_cache(session_id)` which returns a `Mutex` guard. The mutex is locked on every step to check the history version cache and to store updated chat messages. While the lock is not held across `.await` (confirmed by the code dropping the guard before awaiting), it still serializes access. For concurrent sub-agent sessions sharing a processor, this could cause contention.
**Suggested Fix:** Use `parking_lot::Mutex` (faster than `std::sync::Mutex`) or use per-session `RwLock` if reads are more frequent than writes.

---

## Low Severity

### L-1: `DefaultHasher` used for cache keying — poor distribution

**File:** `crates/ragent-agent/src/session/processor.rs`, line ~3258 and `session/cache.rs`, line ~171
**Description:** `std::collections::hash_map::DefaultHasher` (which is SipHash-1-3) is used for `history_version_of` and `SystemPromptCache` hashing. While SipHash is cryptographically resistant, it is slower than alternatives like `FxHash` or `AHash` for non-adversarial cache keying.
**Suggested Fix:** Use `ahash::AHasher` or `rustc-hash::FxHasher` for non-cryptographic hash purposes. These are 2–5x faster for short keys.

---

### L-2: `PooledString` in `message/pool.rs` is thread-local but not used in the hot path

**File:** `crates/ragent-agent/src/message/pool.rs`
**Description:** A thread-local string pool (`PooledString`) exists but doesn't appear to be used in the main agent loop or message construction path. The pool exists but provides no benefit if the hot path doesn't use it.
**Suggested Fix:** Either integrate `PooledString` into the message construction path (e.g., for `text_buffer` and `reasoning_buffer` in the stream processing) or remove the unused pool to reduce code complexity.

---

### L-3: `build_file_tree` uses recursive directory reads synchronously

**File:** `crates/ragent-agent/src/agent/mod.rs`, lines ~320–371
**Description:** `build_file_tree` recursively reads directories and sorts entries synchronously. It is called via `spawn_blocking` in `collect_prompt_context` (line ~265), which is correct, but the function itself allocates many `String` and `PathBuf` objects during traversal. The sort by `file_name` on every directory level adds overhead.
**Suggested Fix:** Consider using a single pre-allocated `String` buffer with `write!` instead of pushing to a `Vec<String>` and joining. Skip the sort for directories with many entries where order doesn't matter (e.g., `node_modules` is already filtered, but `src/` subdirs could be numerous).

---

### L-4: `create_builtin_agents()` allocates long prompt strings on every call

**File:** `crates/ragent-agent/src/agent/mod.rs`, lines ~507–1025
**Description:** `create_builtin_agents()` constructs ~15 `AgentInfo` structs with `.to_string()` on multi-line string literals. Some prompts are 500+ characters. These allocations happen every time `resolve_agent` is called (see M-1).
**Suggested Fix:** Use `&'static str` for the prompt fields in a static table, or cache the result in a `OnceLock` (see M-1 fix).

---

### L-5: `detect_incomplete_file_task` allocates `to_lowercase()` on every no-tool step

**File:** `crates/ragent-agent/src/session/processor.rs`, lines ~3684–3743
**Description:** When the model produces no tool calls, `detect_incomplete_file_task` is called which does `user_text.to_lowercase()` (allocating a new String) and `split_whitespace().any(...)` with per-word allocation. This runs on every no-tool step.
**Suggested Fix:** Use `user_text.to_ascii_lowercase()` (faster, no Unicode case folding) or check for the keywords with `contains()` on the original string (case-insensitive matching via `eq_ignore_ascii_case`).

---

### L-6: `stall_pattern_set().is_match(&text_buffer)` runs regex on full text buffer

**File:** `crates/ragent-agent/src/session/processor.rs`, line ~2098
**Description:** The stall detection regex set is matched against the entire `text_buffer` on every no-tool step. The buffer can be large (model output text). `RegexSet::is_match` scans the entire string for any of the 12 patterns.
**Suggested Fix:** Only check the first N characters (e.g., first 500) since stall patterns are planning phrases that appear at the start of the output. Use `stall_pattern_set().is_match(&text_buffer[..text_buffer.len().min(500)])`.

---

### L-7: `ToolRegistry::definitions()` sorts on every call

**File:** `crates/ragent-agent/src/tool/mod.rs`, lines ~1119–1133
**Description:** `definitions()` acquires read locks on both `tools` and `hidden`, filters, maps to `ToolDefinition` (cloning name, description, and parameters_schema), and sorts the result. This is called from `get_cached_tool_definitions` (which caches the result) but also from `hash_tool_registry` and `build_tool_reference_section`. The sort is O(n log n) on every uncached call.
**Suggested Fix:** Maintain a sorted cache of `ToolDefinition` inside `ToolRegistry` that is invalidated on `register()`. This makes `definitions()` O(1) for the common case where no new tools have been registered.

---

### L-8: `ExtractedCoreToolAdapter::execute` spawns two event-forwarder tasks per tool call

**File:** `crates/ragent-agent/src/tool/mod.rs`, lines ~511–540
**Description:** Every execution of an extracted core tool creates a new `EventBus` (`Arc::new(EventBus::new(256))`), spawns two tokio tasks (`spawn_extracted_to_core_forwarder`, `spawn_core_to_extracted_forwarder`), and aborts them after execution. The same pattern exists for `ExtractedExtendedToolAdapter` (line ~840). This means every single tool call that goes through these adapters allocates an event bus, two task stacks, and two channel subscriptions.
**Suggested Fix:** Create the adapter's event bus once (lazily, stored on the adapter) and reuse it across calls. Or, for tools that don't emit events, skip the forwarder entirely. The abort/respawn cycle is wasteful.

---

### L-9: `agent.options` is a `HashMap<String, Value>` — allocation-heavy for small option sets

**File:** `crates/ragent-agent/src/agent/mod.rs`, line ~438
**Description:** `AgentInfo::options` is `HashMap<String, Value>`, but most agents have 0–3 options. `HashMap` has overhead for small sizes (hash table allocation, bucket array).
**Suggested Fix:** Use `Vec<(String, Value)>` or `SmallVec<[(String, Value); 4]>` for the options field. Linear search over 0–3 items is faster than hashing and uses less memory.

---

### L-10: `PermissionChecker` uses `parking_lot::RwLock` but is rarely written to

**File:** `crates/ragent-agent/src/session/processor.rs`, line ~605
**Description:** `permission_checker: Arc<parking_lot::RwLock<PermissionChecker>>` is read-locked on every tool call (line ~523) and write-locked only when "Always" permission is recorded (line ~566). The read lock is cheap but unnecessary — the checker's rules don't change during normal operation. Only `record_always` mutates it.
**Suggested Fix:** Use `ArcSwap<PermissionChecker>` for lock-free reads. The checker is replaced atomically when "Always" is recorded. This eliminates the RwLock read overhead on the hot path.

---

### L-11: `TaskManager` uses `tokio::sync::RwLock` for tasks HashMap — async lock overhead

**File:** `crates/ragent-agent/src/task/mod.rs`, lines ~160–161
**Description:** `tasks: Arc<RwLock<HashMap<String, TaskEntry>>>` and `cancel_flags: Arc<RwLock<HashMap<String, Arc<AtomicBool>>>>` use `tokio::sync::RwLock`. The task map is accessed on every `spawn_sync`, `spawn_background`, `drain_completed`, and `cancel` call. `tokio::sync::RwLock` has higher overhead than `parking_lot::RwLock` because it involves async scheduling.
**Suggested Fix:** Use `parking_lot::RwLock` or `DashMap` for the task map since the operations are quick and don't need to be async-aware. The locks are held for very short durations (insert/get/remove) and don't span `.await` points.

---

### L-12: `spawn_background` clones all fields for the tokio::spawn closure

**File:** `crates/ragent-agent/src/task/mod.rs`, lines ~405–416
**Description:** `spawn_background` clones `parent_sid`, `agent`, `prompt`, `model`, `event_bus`, `tasks`, `cancel_flags`, `processor`, `tid`, `csid`, `working_dir_buf` — 11 clone operations. The `processor` is an `Arc` (cheap), but `prompt` and `agent` are `String` clones.
**Suggested Fix:** Acceptable for background tasks (low frequency), but could use `Arc<str>` for prompt and agent name to reduce per-spawn allocation.

---

### L-13: `uuid::Uuid::new_v4().to_string().split('-').next()` in task ID generation

**File:** `crates/ragent-agent/src/task/mod.rs`, lines ~201–209 and ~351–359
**Description:** Task ID generation does `uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("task")` which allocates a full UUID string (36 chars), then splits it, then takes the first segment. This creates 2 string allocations (the UUID string and the split segment).
**Suggested Fix:** Use `uuid::Uuid::new_v4().simple()` or extract the first 8 bytes directly: `format!("{:08x}", uuid::Uuid::new_v4().as_u128() & 0xFFFFFFFF)`.

---

### L-14: `build_system_prompt_with_storage` does `agent.prompt.as_deref().is_none_or(|p| p.contains("{{...}}"))` multiple times

**File:** `crates/ragent-agent/src/agent/mod.rs`, lines ~1898–1956
**Description:** The function checks `agent.prompt.as_deref().is_none_or(|p| !p.contains("{{WORKING_DIR}}"))`, `!p.contains("{{FILE_TREE}}")`, `!p.contains("{{AGENTS_MD}}")`, `!p.contains("{{GIT_STATUS}}")`, `!p.contains("{{README}}")` — five separate `contains()` scans over the agent prompt string. The prompt can be long (500+ chars).
**Suggested Fix:** Extract `agent.prompt.as_deref()` to a local `Option<&str>` once and check all template variables in a single pass, or pre-compute a set of which template variables are present.

---

### L-15: `parse_hook_configs` clones every `Value` from the raw hooks array

**File:** `crates/ragent-agent/src/hooks/mod.rs`, lines ~98–111
**Description:** `parse_hook_configs` iterates `raw_hooks: &[Value]` and calls `serde_json::from_value::<HookConfig>(value.clone())` — cloning each JSON value. This runs on every `process_user_message` call (line ~1047).
**Suggested Fix:** Use `serde_json::from_value` with a borrowed approach, or cache the parsed hook configs on the `SessionProcessor` since they don't change during a session.

---

## Summary Table

| ID | Severity | File | Description |
|----|----------|------|-------------|
| H-1 | High | processor.rs:904-986 | Config::load() called 3-4 times per message |
| H-2 | High | processor.rs:1711 | system_prompt.clone() on every LLM request |
| H-3 | High | processor.rs:1706 | chat_messages.clone() on every retry attempt |
| H-4 | High | processor.rs:1712,1722 | agent.options/thinking cloned every step |
| H-5 | High | processor.rs:2260-2277 | ToolContext construction clones Config per tool call |
| H-6 | High | agent/mod.rs:1850-2033 | Synchronous file I/O in build_system_prompt |
| H-7 | High | storage/mod.rs:744-783 | get_messages loads ALL messages, no pagination |
| H-8 | High | storage/mod.rs:490-546 | pragma_table_info queried on every get_session |
| H-9 | High | processor.rs (multiple) | session_id.to_string() dozens of times per step |
| H-10 | High | processor.rs:1573 | Tool names cloned into Vec on every step |
| M-1 | Medium | agent/mod.rs:1244 | create_builtin_agents() on every resolve_agent |
| M-2 | Medium | cache.rs:358-366 | hash_tool_registry iterates all tools on every cache check |
| M-3 | Medium | processor.rs:1111 | Sub-agent tool reference not cached |
| M-4 | Medium | processor.rs:2278 | PendingToolCall cloned per tool spawn |
| M-5 | Medium | processor.rs:359-415 | split_bash_command allocates Vec per bash call |
| M-6 | Medium | processor.rs:306-316 | extract_resource_from_input allocates String |
| M-7 | Medium | processor.rs:487-498 | current_dir() + canonicalize() on every file:read |
| M-8 | Medium | processor.rs:2856-2859 | assistant_parts cloned for interim storage |
| M-9 | Medium | processor.rs:3268-3343 | history_to_chat_messages deep-clones all parts |
| M-10 | Medium | processor.rs:3385-3399 | tool_result_content_for_llm allocates in fast path |
| M-11 | Medium | processor.rs:3432-3468 | estimate_request_bytes serializes JSON for size |
| M-12 | Medium | profiler.rs/processor.rs | Profiler scope objects created even when disabled |
| M-13 | Medium | processor.rs:3214-3246 | Team context scan on cache miss |
| M-14 | Medium | session/mod.rs | SessionState behind std::sync::Mutex |
| L-1 | Low | processor.rs:3258 | DefaultHasher used for cache keys |
| L-2 | Low | message/pool.rs | PooledString exists but unused in hot path |
| L-3 | Low | agent/mod.rs:320-371 | build_file_tree allocates many strings |
| L-4 | Low | agent/mod.rs:507-1025 | create_builtin_agents allocates long prompts |
| L-5 | Low | processor.rs:3684-3743 | detect_incomplete_file_task to_lowercase allocation |
| L-6 | Low | processor.rs:2098 | Stall regex matched against full text buffer |
| L-7 | Low | tool/mod.rs:1119-1133 | ToolRegistry::definitions() sorts on every call |
| L-8 | Low | tool/mod.rs:511-540 | Event forwarder tasks spawned per tool call |
| L-9 | Low | agent/mod.rs:438 | HashMap for 0-3 options is allocation-heavy |
| L-10 | Low | processor.rs:605 | RwLock on PermissionChecker for read-heavy path |
| L-11 | Low | task/mod.rs:160-161 | tokio::sync::RwLock for task HashMap |
| L-12 | Low | task/mod.rs:405-416 | spawn_background clones 11 fields |
| L-13 | Low | task/mod.rs:201-209 | UUID string split for task ID |
| L-14 | Low | agent/mod.rs:1898-1956 | Five contains() scans over agent prompt |
| L-15 | Low | hooks/mod.rs:98-111 | Hook configs re-parsed on every message |

---

## Top Recommendations (Priority Order)

1. **H-1 + H-5:** Load `Config` once per `process_user_message` and store as `Arc<Config>` on the processor. Eliminates repeated disk reads and per-tool-call Config cloning.
2. **H-2 + H-3 + H-4:** Wrap `system_prompt`, `chat_messages`, and `agent.options` in `Arc` so clones are O(1). These are the highest-frequency large-data clones on the hot path.
3. **H-7 + H-8:** Add `has_assistant_messages()` to Storage (avoids full message load for simple checks) and cache `format_version` existence after migration (avoids pragma query on every session lookup).
4. **H-9:** Store `session_id` as an owned `String` or `Arc<str>` once per `process_user_message` and clone the Arc instead of calling `.to_string()` on every event publish.
5. **M-1:** Cache `create_builtin_agents()` in a `OnceLock`. This eliminates ~15 `AgentInfo` constructions (with long prompt strings) on every agent resolution.
6. **M-2 + L-7:** Add a version counter to `ToolRegistry` that bumps on `register()`. Use it for cache invalidation instead of hashing all tool definitions.
7. **L-8:** Refactor the extracted tool adapters to reuse their event bus instead of creating and destroying one per execution.