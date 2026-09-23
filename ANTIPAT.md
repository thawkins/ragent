# ragent-agent anti-pattern / code-quality audit

Scope: `crates/ragent-agent/src/` (134 files, 55,199 LOC) plus its dependency
surface. Six parallel audit agents ran against this crate: standards
conformance, duplication, logging hygiene, security/secrets, test quality, and
dependency freshness.

Method note: five of the six agents were `explore` agents whose summaries were
truncated by the orchestration layer and whose full reports never reached disk
(the `log/subagents/<task-id>.md` writer did not fire for that batch — see
"Process findings" at the end). Their surviving bullets are reproduced below
with their evidence re-verified directly against the tree at the cited lines.
The dependency audit (`general-18c31aca`) is reproduced from
`target/temp/audit-agent6-deps.md`.

Severity: HIGH = correctness/security defect shipping today, MEDIUM =
maintainability or dependency risk, LOW = polish.

---

## 1. Standards conformance

- **[HIGH] Emoji and non-ASCII glyphs in production string literals (16 emoji
  sites, project rule forbids them in comments *and* identifiers; these are
  user-facing tool output).** `crates/ragent-agent/src/tool/team_status.rs:95-103`
  (`"🔄"`, `"📋"`, `"🔒"`, `"🛑"`, `"🚀"`, `"❌"`), `tool/team_wait.rs:319-326`,
  `tool/team_task_list.rs:66-68`, `tool/list_agents.rs:257-261`, `tool/cron.rs:132,208,214,284,352,358,428`,
  `tool/team_create.rs:527`, `tool/team_idle.rs:77`, `tool/team_spawn.rs:271,287`,
  `tool/team_task_claim.rs:207`, `tool/wait_agents.rs:316,336,372,378,382`,
  `tool/initiative.rs:559`, `tool/spec_read.rs:96`, `agent/mod.rs:1841` (`" ✅ LOADED"`),
  `goal/mod.rs:88,90`. Replace with plain ASCII status words or the allowed
  ASCII box glyphs (`-`, `|`, `+`).
- **[MEDIUM] Non-ASCII em-dash (`—`, U+2014) at 500+ sites across 113 of 134
  source files.** Concentrated in `session/processor.rs` (68), `agent/mod.rs`
  (107), `team/manager.rs` (69), `session/loop_steps.rs` (44),
  `compaction/runner.rs` (37), `research_adapter.rs` (25),
  `session/prompt_builders.rs` (24). Both in comments and in user-facing
  strings. Mechanical to fix but touching 113 files; decide whether the rule is
  "no emoji" or "ASCII only" and encode that decision in a lint.
- **[MEDIUM] 203 `unwrap()`/`expect()` call sites in non-test source.** The
  project rule ("no `.unwrap()` on user-facing paths") is not enforced by
  clippy (`unwrap_used = "allow"`). No HIGH-severity user-facing path was found
  in a spot check, but 203 is too many to review ad hoc; it needs either a
  deny-level lint with targeted `#[allow]`s or a documented allowlist.
- **[LOW] Wildcard imports: 0.** Rule satisfied.
- **[LOW] `unsafe`: 0 in this crate.** Rule satisfied.
- **[LOW] `println!`/`eprintln!`/`print!` in production code: 0.** Every hit is
  inside a doc-comment or a string literal that *documents* the rule
  (`agent/mod.rs:766,1474`, `mcp/mod.rs:947`, `session/archive.rs:406`,
  `session/processor.rs:1569`, `skill/mod.rs:379`). `tracing` is used
  consistently. Rule satisfied.
- **[LOW] `TODO` markers: 4 total, 2 real** —
  `agent/mod.rs:469` (`Replace Value with typed agent option structs`) and
  `tool/mod.rs:108` (`Replace Value with a typed ToolMetadata struct`). Both are
  deliberate, scoped, and adjacent to the code they describe. Acceptable.
- **[LOW] Clippy on `ragent-agent --all-targets` is not clean: 2 warnings**
  (`session/text_toolcalls.rs:51` and `:69`,
  `clippy::redundant_pub_crate` — "`pub(crate)` function inside private
  module", surfaced twice more via the `lib test` target). Zero-warnings policy
  is stated in AGENTS.md; these are the only two in the crate.

## 2. Duplication

- **[MEDIUM] Byte-identical private `truncate` helpers in two sibling modules.**
  `tool/conversation_search.rs:332` and `tool/session_search.rs:251` are the same
  nine-line function (`Truncate a string to max bytes without breaking UTF-8
  boundaries`). The crate has at least eleven other truncation helpers with
  different semantics (`task/mod.rs:1197` `truncate_str` char-count + ellipsis;
  `session/history.rs:250` `truncate_at_char_boundary`; `session/verification.rs:35`
  `truncate_head_tail`; `agent/mod.rs:95` `truncate_lines`; `compaction/serializer.rs:167`
  `pub fn truncate`; `loop_state.rs:417` `truncate_chars`). Two identical
  copies is the actionable part: fold into one shared helper.
- **[MEDIUM] 13 truncation helpers with three different unit semantics** (byte
  count, char count, line count) and inconsistent naming
  (`truncate`, `truncate_str`, `truncate_lines`, `truncate_chars`,
  `truncate_at_char_boundary`, `truncate_content`, `truncate_preview`,
  `truncate_head_tail`). Every call site has to read the body to know what the
  limit means. Naming should say the unit (`truncate_bytes`, `truncate_chars`).
- **[LOW] Orchestrator-level duplication across crates.** `fn truncate` also
  exists in `ragent-plugins/src/harness.rs:515`, `ragent-plugins/src/control.rs:431`
  and `ragent-research/src/cli.rs:828`. Out of scope for this audit but the same
  helper should probably live in `ragent-types`.
- **[LOW] No duplicated symbol implementations detected** among the 169-tool
  registry, the 20 team tools, or the event/message types in this crate.

## 3. Logging hygiene

- **[LOW] No violations.** All logging goes through `tracing`; no raw
  `println!`/`eprintln!` in production paths (see §1). Structured fields are
  used on the hot paths (`session/processor.rs:3944` timing breakdown).
- **[MEDIUM] Two process-lifetime caches never bound their memory.**
  `agent/mod.rs:58` `static PROMPT_CONTEXT_CACHE: OnceLock<Mutex<HashMap<String,
  PromptContextCache>>>` — keyed by canonicalised working dir *and* two atomic
  flags; one entry per distinct project root touched, held for the process
  lifetime. `agent/mod.rs:2184` `static ENTRY_TOKENS: OnceLock<Mutex<HashMap<i64,
  (u64, usize)>>>` — one entry per memory row id, and the key space is the
  memory store's primary key, so it grows with the store. Neither has an eviction
  path. In a long-lived server process (or a TUI session that opens many
  projects) these are unbounded.
- **[LOW] Cache invalidation for `PROMPT_CONTEXT_CACHE` is time-based only**
  (30 s TTL, `agent/mod.rs:257`), so a stale README/git-status can be rendered
  for up to 30 s after a file change. Acceptable by design; noted so it is not
  mistaken for a bug.
- **[LOW] `session/text_toolcalls.rs` uses a `String` scan buffer and is called
  per streamed chunk.** No logging issue, but it is the only clippy-warned file
  (see §1), so it has had less recent attention than its neighbours.

## 4. Security / secrets

- **[LOW] No hardcoded credentials.** Zero matches for `sk-…`, `ghp_…`,
  `AKIA…` or equivalent in `crates/ragent-agent/src/`. The only "token"-shaped
  hits are the words `token`/`tokens` in type and field names
  (`token_tracker`, `context_window`, `estimate_text_tokens`).
- **[LOW] Credential material is read from config/env, not stored in this
  crate.** `agent/mod.rs`, `event/mod.rs`, `hooks/mod.rs`, `mcp/mod.rs`,
  `session/permissions.rs` reference `api_key`/`secret`/`token` only as
  pass-through field names.
- **[LOW] Permission enforcement is centralised** in
  `session/permissions.rs` (12 non-ASCII lines, no logic smells found in this
  pass); the 7-layer bash security model lives in `ragent-tools-core`, outside
  this scope.
- **[LOW] No `unsafe` and no FFI in this crate**, so there is no memory-safety
  surface to review.

## 5. Test quality

- **[HIGH] 32 inline `#[cfg(test)]` modules in `crates/ragent-agent/src/`
  violate the stated rule ("All tests MUST be located in the `tests/` directory
  inside each crate. Do not add new inline `#[cfg(test)]` modules to library
  source files.").** Workspace-wide the count is 133 files. Confirmed inline
  modules include `mcp/discovery.rs`, `mcp/http.rs`, `orchestrator/mod.rs`,
  `reference/parse.rs`, `reference/resolve.rs`, `session/archive.rs`,
  `session/verification.rs`, `session/loop_capture.rs`, `skill/*` (7 files),
  `task/mod.rs`, `tool/team_memory_read.rs`, `tool/team_memory_write.rs`,
  `tool/new_agent.rs`, `perf/mod.rs`, `compaction/estimator.rs`. 87 external
  test files exist, so the migration pattern is established — these are
  stragglers.
- **[MEDIUM] Test suite size vs source size.** 87 external test files against
  134 source files is reasonable coverage by count, but the inline modules
  above are only exercisable through the private module tree, which is exactly
  why they were never migrated. Migrating them needs the `#[path]`-shim
  strategy already documented in AGENTS.md.
- **[MEDIUM] Test-name convention is partly violated.**
  `task/mod.rs` (inline) carries `test_truncate_str_long`,
  `test_truncate_str_multibyte_boundary_safe`, `test_sanitize_for_id_basic`,
  `test_task_entry_serialization`; other crates have names like
  `truncate_short_string_passes_through` (`ragent-research/src/cli.rs:1617`).
  The declared convention is `test_<component>_<scenario>`.
- **[LOW] No flaky/sleep-based tests found** in the audited module list.
- **[LOW] `seed_completed_for_test`, `set_pending_background_for_test`**
  (`task/mod.rs`, `#[doc(hidden)]`) are deliberate test seams on a public type.
  They work, but they widen the public API for test-only paths; a
  `#[cfg(any(test, feature = "test-helpers"))]` gate would be tidier.

## 6. Dependency freshness (from `target/temp/audit-agent6-deps.md`)

`cargo audit`: **0 vulnerabilities** (advisory DB 1266 entries), warnings only.
`Cargo.lock` is present at the workspace root and git-tracked. `cargo outdated`
cannot run on this workspace — pre-existing `unicode-width` pin conflict
(`ragent-tui` pins `^0.2.2`, ratatui 0.29 requires `=0.2.0`) — so freshness was
checked against the crates.io API directly.

- **[MEDIUM] RUSTSEC-2026-0002 + RUSTSEC-2026-0253 — unsound `IterMut`
  (Stacked Borrows violation) and use-after-free in `LruCache::pop()`.**
  `lru@0.12.5` → `0.18.5`. Workspace root pins `lru = "0.12"`; ragent-agent
  pulls it transitively via `ragent-codeindex`, `ragent-tools-core`,
  `ragent-tools-extended`.
- **[MEDIUM] RUSTSEC-2026-0253 — same use-after-free in the second in-tree
  `lru` copy.** `lru@0.16.4` → `0.18.5`, transitive via
  `printpdf 0.9.1 → azul-layout`. Two different vulnerable copies are in the
  graph, so a single bump is not enough.
- **[MEDIUM] RUSTSEC-2025-0052 — discontinued runtime.** `async-std@1.13.2`,
  unmaintained.
- **[MEDIUM] Deprecated crate used as a *direct* dependency of
  `ragent-agent`.** `serde_yaml@0.9.34+deprecated`; upstream recommends
  `serde_yml` or `yaml-rust2`. This one is in scope for this crate.
- **[LOW] Major/minor lag.** `rmcp 1.8.0 → 3.4.0` (three majors, MCP client),
  `rusqlite 0.32.1 → 0.40.2`, `similar 2.7.0 → 3.2.0`, `dirs 6.0.0 → 7.0.0`,
  `sha2 0.10.9 → 0.11.0`, `base64 0.22.1 → 0.23.1`. Minor lag: `reqwest
  0.13.4 → 0.13.5`, `futures 0.3.33 → 0.3.34`, `uuid 1.24.0 → 1.26.1`,
  `thiserror 2.0.19 → 2.0.20`, `async-trait 0.1.91 → 0.1.92`,
  `flate2 1.1.9 → 1.1.10`.
- **[LOW] Unmaintained transitives with no maintained upgrade path** (RUSTSEC
  2024-0384 / 2026-0192 / 2024-0436 / 2025-0119 / 2024-0370):
  `instant@0.1.13` (via tantivy's `measure_time`), `ttf-parser@0.25.1` (via the
  vendored `pdf-extract`/`lopdf`), `paste@1.0.15`, `number_prefix@0.4.0`,
  `proc-macro-error@1.0.4` (via `printpdf → allsorts → ouroboros`).
- **[LOW] Yanked package in the lockfile:** `chacha20@0.10.1`, transitive via
  the rand/crypto chain.
- **[LOW] Tooling.** No `=x.y` exact pins in `crates/ragent-agent/Cargo.toml`,
  no unused features detected, and two vendored `[patch.crates-io]` overrides
  (`pdf-extract`, `html2text`) are intentional bug-fix forks, not drift.

---

## Process findings (not anti-patterns in the crate, but in this session)

- **[HIGH] `persist_task_output` resolved `log/subagents/` against the *parent*
  session's directory instead of the child session's.** Reports from background
  agents spawned by a parent whose session row pointed at the repo root landed
  outside the crate that ran them, and reports from agents whose parent session
  row was absent silently fell back to a different root. Fixed in
  `crates/ragent-agent/src/task/mod.rs` (`spawn_background_mode` now derives
  `report_dir` from `child_session.directory`, falling back to the
  caller-supplied `working_dir` only when the stored directory is empty).
  Verified by `cargo test -p ragent-agent --test test_spawn_detached` (5/5 pass).
- **[HIGH] Five of six audit agents' full reports were unrecoverable.**
  `wait_agents` truncated each summary and the per-task report files were never
  written, so the complete finding lists were lost. Root cause is the same
  defect as above plus the fact that the write is best-effort (`Option<PathBuf>`
  with no warning on failure). Consider making a failed persistence emit a
  `tracing::warn!` and surfacing `report_status` in the `wait_agents` output so
  a silent loss is visible.
- **[INFO] The persistence path works.** A live sub-agent run wrote
  `crates/ragent-agent/log/subagents/` reports under this fix, and
  `target/temp/spawn_verify_written.md` contains `# SPAWN-VERIFY OK`,
  confirming detached-run file writes reach disk.

## Suggested order of work

1. Fix the two clippy warnings in `session/text_toolcalls.rs` (`redundant_pub_crate`) — 5 minutes, restores the zero-warnings policy.
2. Make `persist_task_output` failures log a warning instead of silently returning `None` — closes the observability hole that lost this session's audit reports.
3. Replace the 16 emoji literals with ASCII status words.
4. Fold the two identical `truncate` copies into one helper; rename the remaining truncation helpers to state their unit.
5. Bump `lru` (both copies), drop `serde_yaml` from `ragent-agent`'s direct deps, and plan the `rmcp`/`rusqlite` majors.
6. Migrate the 32 inline `#[cfg(test)]` modules into `tests/` using the documented `#[path]` shim strategy.
