# PERFPLAN.md — Performance Remediation Plan

Status: **Draft — awaiting prioritisation approval**
Created: 2026-09-14
Owner: ragent team
Baseline version: v1.0.101

---

## 1. Purpose

This plan turns the findings of a full-workspace performance audit (all 16 crates,
~430k LOC) into an ordered, verifiable remediation backlog. It continues the
existing `PERF-NNN` identifier convention already used in the codebase
(`PERF-001`..`PERF-031` are shipped); new tasks start at **PERF-032**.

Each task carries a file/line citation, an impact rating, an effort estimate, and
an explicit acceptance test. A task is only "done" when its acceptance test
passes, not when the code "looks fixed".

---

## 2. Audit method and coverage

11 parallel read-only audit agents (10 subsystem sweeps + 2 verification passes
for the unverified crates). Findings were ranked by impact in four classes:

- **High** — hot path, per-turn / per-frame / per-event; measurable CPU, latency,
  allocation, or correctness cost.
- **Medium** — per-call or per-page; compounds with volume.
- **Low** — bounded or cold-path; fix opportunistically.
- **Unverified** — pattern suspected but not line-confirmed; needs a reading pass
  before it is treated as a defect (see Appendix A).

Confirmed high-value hot paths (ranked):

| # | Path | Location |
|---|------|----------|
| 1 | Full transcript deep-clone every agent turn | `session/loop_steps.rs:760,772`; `session/cache.rs:551,566` |
| 2 | Per-frame clone of every rendered transcript line | `ragent-tui/src/layout.rs:5458` |
| 3 | Per-token O(n^2) re-render/re-wrap of the streaming message | `ragent-tui/src/layout.rs:5425` |
| 4 | Blocking `std::fs` JSONL append on the async gather loop | `ragent-research/src/gather_log.rs:126` |
| 5 | Full `Event` clone on every publish (per-token) | `ragent-types/src/event/mod.rs:1277` |
| 6 | `redact_secrets` collect+sort+`to_string()` on every SSE event | `ragent-types/src/sanitize.rs:136,143` |
| 7 | Every git command executed **twice** | `ragent-tools-vcs/src/git/mod.rs:67` |
| 8 | `reqwest::Client::new()` per tool call / per device-flow poll | `ragent-tools-vcs/src/**/client.rs`, `github/auth.rs:107` |
| 9 | ~12 regexes recompiled per HTML page parse | `masterfetch/metadata.rs:186-305` |
| 10 | Full-indexed-files table load on every stale check | `ragent-codeindex/src/store.rs:487` |

---

## 3. Priority legend

| Priority | Meaning | Target |
|----------|---------|--------|
| `P0` | Critical — correctness/duplication or a per-token/per-request cost | next release |
| `P1` | High — dominant hot-path cost, user-visible latency | M1-M3 |
| `P2` | Medium — compounding per-call cost | M4-M6 |
| `P3` | Low — bounded/cold; batch opportunistically | M7 / ongoing |
| `P4` | Backlog — needs benchmarking first | after baseline |

Effort: **S** (< 1 h), **M** (half day), **L** (1-2 days), **XL** (needs design).

---

## 4. Milestones

| Milestone | Theme | Exit criteria |
|-----------|-------|---------------|
| **M1** | Agent per-turn hot path | Per-turn allocation count and `build_turn_chat_messages` time reduced; `cargo bench -p ragent-agent` shows no regression and an improvement on the turn-loop bench |
| **M2** | TUI render loop | Idle CPU at rest and per-frame allocations both measurably lower on a 500-message transcript; `cargo bench -p ragent-tui` green with improved `bench_history` / `bench_panels` |
| **M3** | Async runtime hygiene | No blocking `std::fs` on async workers in the audited sites; gather-log write throughput improved; SSE per-event allocations removed |
| **M4** | Network & resource reuse | No `reqwest::Client` constructed per call; git subprocess spawned once per command; provider token reads cached |
| **M5** | Regex hoisting | Zero `Regex::new` calls on any per-page / per-candidate / per-plan path; all hoisted to `OnceLock`/`LazyLock` |
| **M6** | Data layer | Storage read path no longer serialised behind writes; snapshot expand no longer clones file bytes; codeindex stale-diff is O(changed) not O(total) |
| **M7** | Guardrails | CI perf gate + benchmark baselines landed; `PERF-NNN` docs updated; FxHashMap policy documented |

Milestones are ordered by user-visible return. M1/M2/M3 are the release-blocking
set; M4-M7 are the follow-up train.

---

## 5. M1 — Agent per-turn hot path

The agent turn loop runs on every model step, so any per-turn clone is multiplied
by steps x turns. This is the highest-leverage area in the workspace.

| Task | Pri | File:line | Anti-pattern | Fix | Effort | Acceptance |
|------|-----|-----------|--------------|-----|--------|------------|
| **PERF-032** | P0 | `session/loop_steps.rs:760,772`; `session/cache.rs:551,566` | Full `Vec<ChatMessage>` deep-cloned every turn — `cached_chat_messages_for_version().map(|c| c.to_vec())` on the cache-hit path, and `store_chat_messages(built.clone(), None)` clones the just-built vector to cache it | Store/hand out `Arc<Vec<ChatMessage>>`; make `cached_chat_messages_for_version` and `store_chat_messages` take/return the `Arc` so both are O(1) refcount ops | M | `cargo test -p ragent-agent` green; bench asserts zero deep copies per turn (add a counter or a clone-tracking unit test) |
| **PERF-033** | P1 | `session/history.rs:117,137,181` | `history_version_of` rescans the whole message slice every turn; `history_to_chat_messages` rebuilds the entire provider-facing vector on any change | Fold the version incrementally on append; convert only newly-appended messages when the version delta is an append | M | Unit test: N appends => O(N) total conversion work, not O(N^2) |
| **PERF-034** | P1 | `session/processor.rs:1744-1750` | Subagent path rebuilds the filtered `tool_definitions` (`filter().cloned().collect()`) over ~111 defs every turn | Compute the subagent-filtered set once and cache it behind the existing tool-cache version counter | S | Bench: subagent turn allocates 0 new tool-definition vectors |
| **PERF-035** | P1 | `session/processor.rs:2300,2718,1808` | `active_loop_specs`/`active_loops` `.get().cloned()` clones `LoopSpec` (`HashSet<String>`) / `LoopTracker` per loop step | Store/return `Arc<LoopSpec>` / `Arc<LoopTracker>`; read fields under the guard | M | `cargo test -p ragent-agent` green; no `LoopSpec` deep clone per step |
| **PERF-036** | P1 | `compaction/runner.rs:174-184`; `compaction/estimator.rs:185-188` | `select()` serialises every message each call; `estimate_request_tokens` re-sums the full history + tools every step (O(n^2) over a session) | Track request-token total incrementally (add on append, subtract on compaction); memoise per-message token cost by message id | M | Compaction bench: per-step estimation is O(1) not O(history) |
| **PERF-037** | P1 | `compaction/convert.rs:60-78` | Backward linear scan of all prior messages/parts to pair each `ToolResult` with its `ToolUse` — O(n*m) | One-pass `HashMap<call_id, (msg_idx, part_idx)>`, then O(1) pairing | S | Unit test with 200 tool calls completes in linear time; pairing identical to before |
| **PERF-038** | P2 | `compaction/runner.rs:223-230,234-239,575,581-584`; `compaction/serializer.rs:135,143,163-171`; `compaction/prompt.rs:97-99` | Duplicate full-history clones; head transcript materialised then truncated; `truncate()` does two full char scans; summary String cloned twice | Partition in place and move the tail; enforce the 60k cap while joining; single-pass `char_indices().nth(max)`; `Arc<str>` the summary | M | Compaction bench: no full-head allocation beyond the retained slice |
| **PERF-039** | P2 | `agent/mod.rs:2188,2202`; prompt assembly `format!` sites | `estimate_text_tokens` recomputed per memory entry on every prompt build; dozens of `format!` concatenations | Cache token counts per entry id and sum incrementally; build the prompt with `write!` into one buffer | M | Prompt-build bench: no per-entry token recompute on unchanged entries |
| **PERF-040** | P3 | `session/processor.rs:1106-1127,1567-1588` | Per-activity-event `spawn_blocking`; per-run event-bus subscriber task that clones every event then filters by session id | Batch activity writes per turn via a bounded mpsc + single writer thread; filter at the bus before dispatch | M | Activity-log writes per turn <= 1 blocking dispatch; no per-turn subscriber task leak |

---

## 6. M2 — TUI render loop

The render loop is the most visible cost: it runs on every keystroke, every
streamed token, and on an idle safety redraw.

| Task | Pri | File:line | Anti-pattern | Fix | Effort | Acceptance |
|------|-----|-----------|--------------|-----|--------|------------|
| **PERF-041** | P0 | `ragent-tui/src/layout.rs:5462-5472` | **Every frame** rebuilds `message_content_lines` by cloning every wrapped `String` of the entire transcript, unconditionally, only for selection-copy | Rebuild only when a selection is active, or store `Arc<str>`/indices and materialise lazily at copy time | S | `bench_panels`: per-frame allocations drop to ~0 at idle on a 500-message transcript |
| **PERF-042** | P1 | `layout.rs:5425-5445` | Streaming bumps `edit_seq` per token, so the **whole** last message is re-parsed, re-wrapped and re-stringified each token — O(n^2) over a reply | Append only the new tail + re-wrap only the last line; or throttle re-render to one per frame (~16 ms) | L | Streaming a 5k-token reply no longer shows superlinear CPU; `bench_markdown` guards the wrap helper |
| **PERF-043** | P1 | `layout.rs:5426-5435` | Linear staleness scan over **all** messages every frame (`edit_seq` compare from index 0) | Track a `message_cache_dirty_from` watermark alongside `message_cache_width`; scan only from the first dirty group | S | Idle frame does O(1) staleness work |
| **PERF-044** | P1 | `lib.rs:820-830` | `needs_redraw || elapsed >= IDLE_REDRAW_INTERVAL_MS` still runs the full render on idle ticks even when nothing changed | Make the safety redraw a no-op when `!app.needs_redraw` and no countdown/animation is active | S | Idle CPU at rest reduced; no lost countdown/spinner updates |
| **PERF-045** | P1 | `lib.rs:750-820` | ~15 `poll_*`/`refresh_*` calls run on **every** wake | Gate each behind a coarse per-job due-check, or a single due-scheduler | M | Wake path executes only due jobs; bench/`Instant` counter confirms skips |
| **PERF-046** | P2 | `layout.rs:5549-5622`; `layout.rs:2199-2229`; `layout.rs:5521` | Input re-wrapped + cursor re-measured every frame; visible window cloned out of the cache each frame | Cache wrapped input + height keyed on `(input_version, inner_width)`; return borrowed `&[Line]` for the visible window | M | Per-frame input work is O(1) when input/width unchanged |
| **PERF-047** | P2 | `widgets/message_widget.rs:3330-3334` | `format!("{}{}", " ".repeat(indent), line)` allocates twice per line — compounded by PERF-042 | Emit indent as a separate `Span::raw(INDENT)` + `Span::raw(line)` | S | No `repeat`/`format!` in `to_lines`; output identical |
| **PERF-048** | P3 | `layout.rs:3348-3350,3499-3501`; `layout_statusbar.rs:281-294,394-426`; `layout.rs:270-292,5482-5487` | `wrapped_lines = lines.clone()` duplicates the document; status-bar spans measured 4x per frame; button/title labels reallocated ~6x per frame | Reuse the `Vec`/`Arc<[Line]>`; measure each span set once; cache labels/titles keyed on their inputs | M | Memory/research viewers hold one copy of rendered lines |

---

## 7. M3 — Async runtime hygiene

Blocking the tokio worker stalls unrelated tasks; these are small, contained
fixes with outsized latency effects.

| Task | Pri | File:line | Anti-pattern | Fix | Effort | Acceptance |
|------|-----|-----------|--------------|-----|--------|------------|
| **PERF-049** | P0 | `ragent-research/src/gather_log.rs:126-135` | Every JSONL record does open + 2x `write_all` + `flush` — blocking `std::fs` on the async gather loop, hundreds of records per sweep | Hold a `Mutex<BufWriter<File>>` opened once; flush only at summary/drop | S | Gather-log throughput bench: no open/flush per record |
| **PERF-050** | P1 | `gather_log.rs:106-123` | `json!` value tree + clone of every detail key/value + `to_rfc3339` String per record | `#[derive(Serialize)]` borrowed-field record struct; write the timestamp in place | S | Same JSONL bytes; zero per-record `Value` allocations |
| **PERF-051** | P1 | `ragent-agent/src/reference/fuzzy.rs:76,82,129` (via `resolve.rs:210`) | Recursive `std::fs` walk to depth 10 000 inside `async fn resolve_fuzzy` on every `@fuzzy` reference | `tokio::task::spawn_blocking` the walk; `DashMap`/`FxHashMap` the project-file cache (`fuzzy.rs:34`) | M | No blocking fs walk on the executor; `@fuzzy` latency unchanged or better |
| **PERF-052** | P1 | `ragent-tools-core/src/glob.rs:91,142,161` | Whole-tree recursive `read_dir`/`is_dir` walk inside `async fn execute` with no `spawn_blocking` | Wrap `collect_matches(...)` in `spawn_blocking` | S | `glob` over a large tree no longer pins a worker thread |
| **PERF-053** | P2 | `ragent-tools-core/src/read.rs:201,359` | Blocking `std::fs::metadata` in async `execute`; `cached_read` already fetched the same mtime asynchronously and discards it | Return/pass the async `mtime` into `record_read_timestamp`; drop the duplicate stat | S | One metadata syscall per read, async |
| **PERF-054** | P1 | `ragent-types/src/event/mod.rs:1277` | `self.sender.send(event.clone())` clones the entire event (large `TextDelta`/`ToolResult` payloads) per publish, on the per-token path | Move the event into `send`; log from the `Err(SendError(ev))` value | S | `publish` performs no payload clone; event-bus bench improved |
| **PERF-055** | P1 | `ragent-types/src/sanitize.rs:136,143,147` | `redact_secrets` does `msg.to_string()`, collect+sort the registry, per-secret `replace`, then `replace_all().into_owned()` — 2-3 full copies per event even with no secret | Add `redact_secrets_cow(&str) -> Cow<str>`: early-return `Borrowed` when registry empty and `SECRET_PATTERN` does not match; keep the registry pre-sorted on insert | M | SSE bench: redaction of a non-secret payload allocates nothing |
| **PERF-056** | P2 | `ragent-server/src/routes/mod.rs:405,511` | `let session_id = id.clone();` inside the SSE `filter_map` runs per streamed event per client; `redacted == *text` compare after full redaction | Clone `id` once before `BroadcastStream`; call the new `Cow` redactor; count `Lagged` drops | M | One id clone per connection, not per event |

---

## 8. M4 — Network & resource reuse

| Task | Pri | File:line | Anti-pattern | Fix | Effort | Acceptance |
|------|-----|-----------|--------------|-----|--------|------------|
| **PERF-057** | P0 | `ragent-tools-vcs/src/git/mod.rs:67-87` | `run_git_or_error` runs **every** git command twice (once for output, once to read exit status). For mutating subcommands this risks double side-effects | Have `run_git` return the `Output`; derive stdout/stderr/status from one spawn | S | Integration test asserts each git tool spawns exactly one process |
| **PERF-058** | P1 | `ragent-tools-vcs/src/github/client.rs:99,109,121`; `gitlab/client.rs:34,44`; `gitlab_pipelines.rs:468`; `github/auth.rs:72,107`; `gitlab/auth.rs:165` | `reqwest::Client::new()` per tool call and per device-flow poll iteration — new TLS pool + handshake each time | Process-wide `static OnceLock<reqwest::Client>`; clone the handle into each client struct | M | No `Client::new` outside the OnceLock initialiser in these crates |
| **PERF-059** | P2 | `github/auth.rs:9-26`; `gitlab/client.rs:28`; `gitlab_pipelines.rs:472` | Token read from disk / credential store decrypted on every request, re-loaded twice in one flow | Cache the resolved token in storage/auth; invalidate on save/delete | M | Token read once per process lifetime (or on explicit refresh) |
| **PERF-060** | P1 | `ragent-llm/src/providers/copilot.rs:856` | `reqwest::Client::new()` per device-flow poll call, bypassing the cached streaming client | Use `create_http_client()` | S | Device-flow poll reuses the shared client |
| **PERF-061** | P1 | `ragent-llm/src/providers/azure_foundry.rs:220-238` | Retry clones the whole serialised request body up to 5x | `body_bytes.into()` a `Bytes`, then refcount-only clone per attempt | S | Retry path performs no payload copy (`Bytes` clone) |
| **PERF-062** | P2 | `ragent-llm/src/providers/ollama_cloud.rs:524-528,721` | Body serialised twice per call (once to a String only for an 800-char log); full `to_string` per completed stream message for a log | Serialise once; slice the bytes for the log; gate the preview behind `tracing::enabled!(DEBUG)` | S | One serialisation per request; no log allocation when debug off |
| **PERF-063** | P3 | 12 streaming providers (see Appendix B) | SSE accumulation buffers start at `String::new()` and thrash the allocator on long streams | `String::with_capacity(8 * 1024)` per line buffer | S | No realloc cliff on long streams |

---

## 9. M5 — Regex hoisting

A single HTML page parse recompiles ~12 regexes today. Every per-page /
per-candidate regex must become a `OnceLock`/`LazyLock` static.

| Task | Pri | File:line | Anti-pattern | Fix | Effort | Acceptance |
|------|-----|-----------|--------------|-----|--------|------------|
| **PERF-064** | P1 | `masterfetch/metadata.rs:186,188,192,194,233,245,246,247,249,273,274,305` | 12 regexes compiled per page parse | Hoist each to `static RE: OnceLock<Regex>` | M | Zero `Regex::new` in the page-parse path; `mf_fetch`/crawl bench improved |
| **PERF-065** | P1 | `ragent-research/src/web_date.rs:54,70,101,120,129,145,155` | 7 regexes compiled per invocation | Hoist to `OnceLock` statics (pattern already used in `title.rs`, `analysis/parser.rs`) | S | Zero per-call compiles in `web_date` |
| **PERF-066** | P2 | `ragent-research/src/clarify.rs:58`; `planner.rs:285`; `cluster.rs:289`; `session/topic.rs:444`; `document.rs:2115` | Inline `Regex::new` on per-call paths | Hoist to `OnceLock` statics | S | No per-call compiles on plan/clarify/cluster paths |
| **PERF-067** | P2 | `ragent-agent/src/template/mod.rs:156` | `Regex::new` executed **inside a loop** | Hoist to `OnceLock` | S | Regex compiled once per process |
| **PERF-068** | P2 | `ragent-research/src/web_gatherer/relevance.rs:20-44,103-175` | Per candidate: 3x `to_lowercase` + `format!` haystack + `query.to_lowercase()` recomputed + `normalize_query_terms` rebuilt + `Vec<String>` per term | Precompute lowercase query + normalised terms once per sub-query; reuse a scratch buffer; SmallVec/callback for variants | M | Relevance bench: per-candidate allocations drop from ~8 to <= 2 |

---

## 10. M6 — Data layer

| Task | Pri | File:line | Anti-pattern | Fix | Effort | Acceptance |
|------|-----|-----------|--------------|-----|--------|------------|
| **PERF-069** | P1 | `ragent-storage/src/storage.rs:272` | Single `Mutex<Connection>` serialises every read behind every write; a long FTS warm-up stalls all session reads | WAL + a reader connection (or a small pool: `deadpool-sqlite`); route reads to a reader handle | L | Concurrent read latency during a bulk write is bounded; add a test that reads complete while a write tx is open |
| **PERF-070** | P2 | `ragent-storage/src/snapshot.rs:72,93` | `base.files.clone()` clones every file's bytes per incremental expand; `to_full` re-clones added files | `mem::take`/move the base map and consume `self.added` | M | Snapshot expand bench: no full byte-map clone |
| **PERF-071** | P2 | `ragent-storage/src/activity_log.rs:947,1586`; `storage.rs:838-845,906-916` | Non-cached `prepare` for static SQL; static SQL rebuilt with `.to_string()` per call | `prepare_cached`; hoist static SQL to `const &str` | S | No per-call statement compile/alloc on run-list and session paths |
| **PERF-072** | P1 | `ragent-codeindex/src/store.rs:487` | `get_stale_files` loads the **entire** `indexed_files` table into a `HashMap` on every stale check | SQL diff via `LEFT JOIN` / hash-join, or stream rows | M | Stale check is O(changed) memory; `full_reindex` with 0 changes is cheap |
| **PERF-073** | P1 | `store.rs:689,703` | Leading-wildcard `LIKE '%' || ? || '%'` disables the index — full scan of `symbols` (and a join) per `query_symbols` | Prefix FTS5 or an anchored/prefix match; normalise path prefixes | M | `codeindex_search` p95 latency improved on a large repo |
| **PERF-074** | P2 | `store.rs:206,544,602,815,925,947,977` | `upsert_file` does INSERT + a second `SELECT id`; `apply_diff`/`upsert_*` execute per-row instead of prepared/batched; ref queries sort all rows with no `LIMIT` | `INSERT ... RETURNING id`; `prepare_cached` once per loop; wrap in transactions; push `LIMIT` into SQL | L | Bulk-index bench: queries-per-file reduced; no auto-commit-per-row |
| **PERF-075** | P2 | `ragent-codeindex/src/search.rs:443` | New Tantivy `IndexWriter` constructed per batch | Hold one long-lived writer behind the mutex | M | Writer created once per process |
| **PERF-076** | P2 | `ragent-agent/src/mcp/mod.rs:774-783` | `call_tool_by_name` nested linear scan `servers x tools` per `mcp_tool` invocation | Maintain `HashMap<tool_name, server_id>` rebuilt on `refresh_tools` | S | O(1) tool->server lookup |
| **PERF-077** | P2 | `ragent-research/src/web_gatherer.rs:1678,1867,1880,1896,2093` | Whole `Vec<WebSearchHit>` cloned per cache insert; query `String` cloned per hit; full page body cloned before `spawn_blocking` | `Arc<[WebSearchHit]>` for caches; `Arc<str>` queries; move `page.body` by value | M | Per-page gather allocations drop; cache insert is refcount-only |

---

## 11. M7 — Guardrails & hardening

| Task | Pri | Item | Fix | Effort | Acceptance |
|------|-----|------|-----|--------|------------|
| **PERF-078** | P2 | No CI perf gate | Add `scripts/check-perf-regression.sh` (mirrors `check-bench-regression.sh`), wired into `/rust-hygiene`; run the criterion benches that ship (`ragent-agent`, `ragent-tui`, `ragent-server`, `ragent-codeindex`, `ragent-research`) against checked-in baselines | M | CI fails on a >10% regression in a tracked bench |
| **PERF-079** | P2 | Sparse bench coverage on hot paths | Add criterion benches: agent turn loop (M1 targets), `build_turn_chat_messages`, compaction `select`/`estimate`, gather-log append, event `publish`, `redact_secrets` | L | Each M1/M3 acceptance test has a bench behind it |
| **PERF-080** | P3 | Hasher choice inconsistent | Document and apply a policy: `FxHashMap`/`FxHashSet` for integer/short-non-adversarial keys, `DashMap` for contended concurrent maps; adopt `hashbrown` where `ahash` is not already pulled in (see Appendix C sites) | M | Policy note in `docs/`; audited sites converted |
| **PERF-081** | P3 | `PERF-NNN` docs drift | Extend the perf table in `docs/agentorch.md` with PERF-032..PERF-082 as tasks land; keep code comments referencing the ids | S | `docs/agentorch.md` perf table current |
| **PERF-082** | P3 | Prompt assembly `format!` density | Convert large prompt builders (`agent/mod.rs`, `compaction/prompt.rs`) to `write!` into one buffer | S | No `push_str(&format!(..))` in prompt builders |

---

## 12. Non-goals

- No `unsafe` micro-optimisations (workspace rule).
- No change to observable behaviour or on-disk formats; every task must be
  byte-for-byte output-compatible unless explicitly noted.
- No speculative caching of data that is not proven hot (the audit ranked by
  path frequency; do not add caches for cold paths).
- Low-impact findings (e.g. `estimator.rs:98` `n.to_string().len()`) are fixed
  opportunistically only while touching the surrounding code, never as standalone
  work items.

## 13. Verification protocol (every task)

1. `cargo fmt` after every `.rs` edit (mandatory).
2. `cargo check -p <crate>` then `cargo clippy -p <crate> -- -D warnings`.
3. `cargo test -p <crate>` (targeted test first, then the crate suite).
4. For hot-path tasks: run the relevant criterion bench before and after; record
   the delta in the task's changelog entry.
5. `cargo fmt --check` before marking complete.

A task is not complete until its acceptance test in the table above passes.

---

## Appendix A — Unverified items (must be read before action)

These were pattern-matched but not line-confirmed in the audit; treat as a
reading worklist, not a defect list.

| Area | Suspected pattern |
|------|-------------------|
| `ragent-storage/src/activity_log.rs` | RunId clones in append/validation path |
| `ragent-research/src/verify.rs:135,176` | Default-hashed `usize` keys + per-source `HashSet<String>` word cache |
| `ragent-research/src/local_gatherer.rs:335,656` | Whole candidate-set clone; default-hashed lookup maps |
| `ragent-agent/src/memory/extract.rs` (896 LOC) | Large-entry clones / N+1 storage calls (not body-inspected) |
| `ragent-agent/src/{hooks,skill,background,goal,orchestrator,team,tool}` | `.clone()` density (grep-derived counts only) |
| `ragent-telemetry/src/{recorder,instruments}.rs` | Span-attribute allocation on hot paths (not read) |
| `ragent-specs` discovery/manager | Filesystem-walk caching (not read) |
| `ragent-tools-core/src/{multiedit,replace,edit_log,bash,open,file_info,list}.rs` | Whole-file clones / unbounded output capture (not read) |
| `ragent-tui/src/{input,research_progress,layout_active_agents,layout_teams}.rs` | Per-frame cost (grep sweep clean, not line-audited) |

## Appendix B — SSE buffer sites (PERF-063)

`anthropic.rs:422`, `ollama_cloud.rs:569`, `gemini.rs:550`, `bedrock.rs:585`,
`bedrock.rs:942`, `azure_resource.rs:418`, `copilot.rs:483`, `openai.rs:399`,
`openai_responses.rs:410`, `openrouter.rs:605`, `huggingface.rs:559`,
`ollama.rs:482`.

## Appendix C — Default-hasher sites (PERF-080)

`ragent-agent`: `trigger/runtime.rs:79` (`HashMap<u64,..>`),
`reference/fuzzy.rs:34` (`HashMap<PathBuf,..>`).
`ragent-research`: `search_budget.rs:92,100`, `session.rs:3495,3678,3691`,
`corpus_critic.rs:255`, `manager.rs:240`, `locus.rs:137`,
`contradiction.rs:324,354,395`.
`ragent-tools-extended`: `masterfetch/search/consensus.rs:323`,
`masterfetch/search/engine.rs:609`.

---

## Appendix D — Confirmed-clean areas (no action)

Recorded so the audit is not repeated:

- `ragent-llm`: main chat path reuses a `OnceLock`-cached `reqwest::Client`;
  bodies serialised once; registry lookups O(1); streaming is incremental.
- `ragent-server`: session/memory routes wrap store calls in `spawn_blocking`;
  SSE uses borrowed typed `Serialize` structs.
- `ragent-tui`: no render-time regex; `theme.rs` has no per-cell lookups;
  research/memory markdown parse is `line_cache`-gated; message render is
  `edit_seq`-keyed (the defect is only the per-token case, PERF-042).
- `ragent-tools-core`: `write.rs` async; `grep.rs` uses `spawn_blocking` + parallel
  `WalkBuilder` + streaming search; `glob.rs` has a parallel (`par_iter`) path.
- `ragent-tools-vcs`: no repeated `git rev-parse`; no pagination loop in
  `gitlab_pipelines`.
