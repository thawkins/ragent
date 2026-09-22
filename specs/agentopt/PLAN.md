# Implementation Plan: Optimise `ragent-agent` Crate Performance

This plan is the companion to [`AGENTSPEC.md`](AGENTSPEC.md). Each task maps
back to one or more requirements and targets a single, measurable change with
minimal blast radius.

## Summary

| Item | Value |
|------|-------|
| Spec ID | `agentopt` |
| Primary crate | `crates/ragent-agent` |
| Secondary crates | `crates/ragent-config` (permission checker canonicalisation), `crates/ragent-tools-*` (shared context), `cragent-storage` (tag batching) |
| No new dependencies | ✅ unless a crate already transitively uses `dashmap` / `parking_lot` |
| Public API changes | Minimal; document any `Arc` / shared-reference changes |

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Reuse `reqwest::Client` for URL references | FR-007 | S | High | completed | — |
| T-002 | Reuse `reqwest::Client` for HTTP MCP transport | FR-007 | S | High | completed | — |
| T-003 | Cache fuzzy reference project file list | FR-009 | M | High | completed | — |
| T-004 | Reduce fuzzy matching allocations | FR-009, FR-019 | S | Medium | completed | T-003 |
| T-005 | Return `Arc<AgentInfo>` from agent resolution | FR-005, FR-013 | M | High | completed | — |
| T-006 | Reference-count built-in agent prompt strings | FR-013, FR-018 | S | Medium | completed | T-005 |
| T-007 | Avoid full `AgentInfo` clone in `resolve_agent` / `load_all_agents` | FR-005, FR-013 | S | High | completed | T-005 |
| T-008 | Avoid `definitions()` clone in tool registry | FR-014 | M | High | completed | — |
| T-009 | Share session `ToolContext` across tool calls | FR-004 | M | High | completed | T-008 |
| T-010 | Batch storage output/status writes in `BackgroundTaskService` | FR-008 | M | High | completed | — |
| T-011 | Consolidate `BackgroundTaskService` multi-lock state | FR-015 | M | Medium | completed | T-010 |
| T-012 | Avoid full `TaskEntry` clones in `AgentManager` read paths | FR-006, FR-016 | M | High | completed | — |
| T-013 | Replace `RwLock<HashMap>` with `DashMap` in `AgentManager` | FR-016 | M | Medium | completed | T-012 |
| T-014 | Cache canonical paths in permission checker | FR-017 | S | Medium | completed | — |
| T-015 | Hoist hardwired tool approvals above I/O in `check_permission_with_prompt` | FR-004, FR-017 | S | Medium | completed | — |
| T-016 | Batch memory tag fetches for visualisation | FR-010 | S | High | completed | — |
| T-017 | Single-pass skill/template substitution | FR-011 | M | Medium | completed | — |
| T-018 | Single-pass goal-evaluation context builder | FR-012 | S | Medium | completed | — |
| T-019 | Pre-size snapshot diff buffers | FR-004 | S | Low | completed | — |
| T-020 | Add regression/criterion benchmarks for hot paths | FR-002 | M | Medium | completed | — |
| T-021 | Run full test suite and clippy after all changes | FR-002 | S | Critical | completed | All |
| T-022 | Update `AGENTSPEC.md` status and `CHANGELOG.md` entry | FR-003 | S | Low | completed | T-021 |
## Task Details

### T-001 — Reuse `reqwest::Client` for URL references

In `crates/ragent-agent/src/reference/resolve.rs` (lines ~126–131),
`resolve_url` constructs a fresh `reqwest::Client` for every `@https://...`
reference.

**Change:**
- Add a `static CLIENT: OnceLock<reqwest::Client>` (or `LazyLock`) in the
  module.
- Use the shared client for all head and body fetches.
- Keep the existing timeout and redirect policy defaults in the shared client.

**Verification:**
- Existing reference tests in `crates/ragent-agent/tests/` still pass.
- New regression test or micro-benchmark demonstrates that multiple URL
  references use the same client instance.

---

### T-002 — Reuse `reqwest::Client` for HTTP MCP transport

In `crates/ragent-agent/src/mcp/http.rs` (lines ~102–110), a new client is built
per HTTP MCP server connection.

**Change:**
- Store one `reqwest::Client` per `HttpMcpClient` (or a shared static) and clone
  it as needed.
- Preserve current timeout/redirect settings.

**Verification:**
- MCP HTTP tests pass; add an assertion that the client field is reused across
  `post_once` calls.

---

### T-003 — Cache fuzzy reference project file list

In `crates/ragent-agent/src/reference/fuzzy.rs` (lines ~44–49),
`collect_project_files` walks the whole project tree on every fuzzy reference.

**Change:**
- Add a `Mutex<HashMap<PathBuf, CachedFileList>>` keyed by `working_dir`.
- Cache entry contains `mtime`, `files: Vec<PathBuf>`, and a small TTL.
- Invalidate on directory mtime change or after TTL expiry.

**Verification:**
- Test that a second reference for the same directory returns the cached list
  and that editing a file (touching directory mtime) invalidates it.

---

### T-004 — Reduce fuzzy matching allocations

In `crates/ragent-agent/src/reference/fuzzy.rs` (lines ~111–172),
`fuzzy_match` materialises several lowercased strings per candidate.

**Change:**
- Lowercase the query once.
- Compare paths using `eq_ignore_ascii_case` or iterator-based Unicode case
  folding without allocating intermediate strings.
- Return references/slices instead of cloning `PathBuf`s where possible.

**Verification:**
- Fuzzy reference tests pass; add a micro-benchmark showing fewer allocations
  for 10k candidates if feasible.

---

### T-005 — Return `Arc<AgentInfo>` from agent resolution

In `crates/ragent-agent/src/agent/mod.rs` (lines ~1324–1371), `resolve_agent`
finds a built-in and calls `.cloned()` on a struct containing the full prompt.

**Change:**
- Store built-in agents as `Vec<Arc<AgentInfo>>`.
- Change `resolve_agent` to return `Option<Arc<AgentInfo>>`.
- Update all call sites to handle the reference-counted return.

**Verification:**
- All agent resolution tests pass.
- A new test asserts the same prompt string is not physically copied on each
  call (same pointer or shallow clone check).

---

### T-006 — Reference-count built-in agent prompt strings

Inside `AgentInfo`, change `prompt: Option<String>` to `prompt: Option<Arc<str>>`
(or keep `String` if callers need mutation, but ensure composition copies only
when necessary).

**Change:**
- Update the `AgentInfo` struct definition and constructors.
- Adjust prompt composition in the session processor to borrow or clone the
  `Arc<str>` instead of copying the string text.

**Verification:**
- Existing agent/prompt tests pass.
- No regression in custom-agent loading.

---

### T-007 — Avoid full `AgentInfo` clone in `resolve_agent` / `load_all_agents`

`load_all_agents` (lines ~1425–1451) calls `builtin_agents().to_vec()`, copying
all built-ins again.

**Change:**
- Return `Vec<Arc<AgentInfo>>` from `load_all_agents` and related helpers.
- Merge custom agents into the same shared representation.

**Verification:**
- `/agents` and agent selection tests pass.

---

### T-008 — Avoid `definitions()` clone in tool registry

In `crates/ragent-agent/src/tool/mod.rs` (lines ~1337+), `definitions()` clones
the entire tool definition vector.

**Change:**
- Add a `definitions_arc()` method returning `Arc<[ToolDef]>`.
- Use it in the session processor and anywhere else that only needs a read-only
  snapshot.

**Verification:**
- Tool listing and dispatch tests pass.
- Benchmark or test confirms no per-LLM-call clone of definitions.

---

### T-009 — Share session `ToolContext` across tool calls

Tool adapters rebuild `ragent_tools_core::ToolContext` and related context
structs on every `execute()` (lines ~520–524, ~1055–1063, ~1160–1164).

**Change:**
- Introduce `SharedToolContext` holding `Arc`s of the constant session fields.
- Build it once per session and pass a cheap reference to each tool adapter.
- Keep mutable fields (e.g. read timestamps) behind interior mutability only if
  actually required.

**Verification:**
- All tool execution tests pass.
- No change in public tool trait signatures unless unavoidable.

---

### T-010 — Batch storage output/status writes in `BackgroundTaskService`

In `crates/ragent-agent/src/background/mod.rs`, `flush_task` writes output and
status in two separate storage calls every 2 seconds even when unchanged.

**Change:**
- Track `last_flush_hash` or `last_flush_bytes` per task.
- Skip storage writes when stdout/stderr/progress are unchanged.
- Combine output and status into one `Storage::write_async` call when a flush
  is required.

**Verification:**
- Background-task tests pass.
- A test asserts no storage writes occur during an idle 2-second interval.

---

### T-011 — Consolidate `BackgroundTaskService` multi-lock state

`BackgroundTaskService` holds three independent `Mutex<HashMap>` fields
(lines ~133–194, ~261–287, ~300–347).

**Change:**
- Consolidate related maps into one `Mutex<Inner>` (or `RwLock<Inner>`)
  structure to remove multi-lock acquisition.
- Consider `DashMap` if the crate already depends on it.

**Verification:**
- Concurrent background-task access tests pass with no deadlocks.

---

### T-012 — Avoid full `TaskEntry` clones in `AgentManager` read paths

In `crates/ragent-agent/src/task/mod.rs`, `list_agents`,
`running_background_count`, and `drain_completed` clone entire `TaskEntry`
records, including large result strings.

**Change:**
- Return lightweight summary structs from read-heavy paths.
- Clone the full `result` only when the caller explicitly needs it.

**Verification:**
- Sub-agent and wait-agent tests pass.
- Add a test with a large synthetic result and assert no full clones during
  `list_agents` / `running_background_count`.

---

### T-013 — Replace `RwLock<HashMap>` with `DashMap` in `AgentManager`

`AgentManager` uses `tokio::sync::RwLock<HashMap<String, TaskEntry>>`.

**Change:**
- Replace with `DashMap<String, Arc<TaskEntry>>` plus an `AtomicUsize` for
  running count.
- Ensure `waiter_count` semantics remain correct.

**Verification:**
- Concurrent sub-agent tests pass.
- `cargo clippy` clean.

---

### T-014 — Cache canonical paths in permission checker

`check_permission_with_prompt` calls `canonicalize()` on every file read
(lines ~221–232 in session permissions).

**Change:**
- Add a step-scoped cache mapping raw resource strings to canonical `PathBuf`.
- Clear or reset the cache between agent turns.

**Verification:**
- Permission tests pass.
- Test that repeated file reads in the same turn use the cache.

---

### T-015 — Hoist hardwired tool approvals above I/O

Hardwired allow/deny checks should run before canonicalisation and globset
compilation.

**Change:**
- In `check_permission_with_prompt`, short-circuit `codeindex`, `task`,
  `ask_user`, `team`, and similar hardwired permissions before any path I/O.

**Verification:**
- Existing permission tests pass.
- New test asserts that a hardwired tool returns without canonicalising.

---

### T-016 — Batch memory tag fetches for visualisation

`memory/visualisation.rs` calls `storage.get_memory_tags(mem.id)` once per row.

**Change:**
- Add `storage.get_memory_tags_for_ids(ids: &[i64])` returning a map of id → tags.
- Call it once before the visualisation loops.

**Verification:**
- Memory visualisation tests pass.
- A test asserts exactly one tag query for N rows.

---

### T-017 — Single-pass skill/template substitution

`template/mod.rs` and `skill/args.rs` repeatedly scan and replace placeholders.

**Change:**
- Implement a single-pass scanner that finds placeholders and writes the output
  once.
- Pre-size the output buffer based on the input length plus expected
  substitution sizes.

**Verification:**
- Skill/template tests pass.
- Add a benchmark or unit test for multiple substitutions.

---

### T-018 — Single-pass goal-evaluation context builder

`goal/mod.rs` builds a `Vec<String>` then joins it.

**Change:**
- Pre-size a `String` buffer and append formatted messages directly.
- Avoid intermediate collections.

**Verification:**
- Goal/autopilot tests pass.

---

### T-019 — Pre-size snapshot diff buffers

`snapshot/mod.rs` builds diffs in un-sized strings.

**Change:**
- Estimate output capacity from input sizes and call
  `String::with_capacity` before building.

**Verification:**
- Snapshot tests pass.

---

### T-020 — Add regression/criterion benchmarks for hot paths

Add small Criterion benchmarks covering:
- agent resolution,
- tool definitions serialisation,
- fuzzy reference matching,
- permission check for file reads,
- skill substitution.

**Verification:**
- `cargo bench -p ragent-agent` runs and produces numbers.

---

### T-021 — Run full test suite and clippy after all changes

Before declaring the work done:

1. `cargo test -p ragent-agent` passes.
2. `cargo clippy -p ragent-agent -- -D warnings` passes.
3. `cargo fmt -p ragent-agent` is clean.

---

### T-022 — Update `AGENTSPEC.md` status and `CHANGELOG.md` entry

- Transition `AGENTSPEC.md` frontmatter status to `in_progress` after T-001–T-003
  land, and to `implemented` after T-021 passes.
- Add a `CHANGELOG.md` entry under the current version describing the
  ragent-agent performance improvements.

## Definition of Done

- All tasks listed above are marked `completed` in this plan.
- `cargo test -p ragent-agent` passes without warnings.
- `cargo clippy -p ragent-agent` passes.
- `cargo fmt -p ragent-agent` produces no diff.
- Hot-path benchmarks (T-020) show measurable improvement or at minimum no
  regression.
- `AGENTSPEC.md` status is `implemented` and `CHANGELOG.md` is updated.
- No user-visible commands or output strings changed except as required by the
  optimisations.

## Notes

- Keep each PR/task small; do not combine T-005, T-012, and T-013 in one mega
  change because they touch different locking patterns.
- If a task requires a breaking public API change, update this plan with a
  dedicated "API impact" note and notify reviewers.
- The review findings files in `log/subagents/` may be referenced for additional
  low-impact cleanups after the high/medium tasks are complete.