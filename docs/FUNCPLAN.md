# FUNCPLAN.md — Functional Anti-Pattern Remediation Plan

Status: **Remediated — all 62 scheduled tasks complete (FUNC-001..082)**
Created: 2026-09-14
Owner: ragent team
Baseline version: v1.0.103

---

## 1. Purpose

This plan turns a full-workspace *functional* anti-pattern audit (all 16 crates,
~451k LOC) into an ordered, verifiable remediation backlog.

"Functional" means **behaviour, correctness and robustness** — crashes on valid
input, silently swallowed errors, security controls that fail open, concurrency
hazards, and logic that changes semantics. This is deliberately disjoint from
`PERFPLAN.md`, which covers throughput/latency/allocation. A finding that only
costs CPU belongs in PERFPLAN, not here.

Every task carries a file:line citation, a severity, an effort estimate, and an
explicit acceptance test. A task is **done only when its acceptance test passes**,
not when the code "looks fixed".

Task IDs use the `FUNC-NNN` convention, starting at `FUNC-001`.

---

## 2. Audit method and coverage

12 parallel read-only `explore` agents swept the workspace in two waves
(one area per agent), followed by targeted re-verification of every
Critical/High row by the plan author.

Wave 1: agent core, agent tools + tools-core, LLM + config, TUI, research,
tools-extended, VCS, data/services (storage/codeindex/specs/server/types/team/
telemetry/bench/root).

Wave 2 (gap-filling, because agents time-box): VCS gitlab/pipelines/vcs_provider,
codeindex graph/worker, specs/telemetry/bench, server/types/config/root,
research modules not yet read, extended-tools memory/mail/finance.

**Coverage caveats are recorded in Appendix C.** Areas not read to depth are
listed there as a follow-up worklist; this plan is not a clean bill for them.

### 2.1 Headline totals

Counts below are the **scheduled tasks** in this plan (62 total), not the raw
finding count from the audit sweeps (an audit row can fold into a broader task,
and a task can cover several sibling sites).

| Severity | Tasks | Meaning |
|----------|-------|---------|
| Critical | 6 | Crash/security/exfiltration/hang on plausible input |
| High | 30 | Silent data loss, security gap, or runtime-panic family |
| Medium | 25 | Wrong behaviour in an edge case, or a swallowed error hiding failures |
| Low | 1 | Hygiene/fragility; batch opportunistically |

The audit surfaced roughly 150 raw findings; ~90 are scheduled above, ~15 were
verified false-positive or verified-clean (Appendix A), and the balance are
Low/hygiene sites folded into the milestone tasks or deferred with their area in
Appendix C.

### 2.2 Dominant patterns (ranked by blast radius)

1. **Byte-offset slicing of UTF-8 text** (`&s[..n]` where `n` is a byte length) —
   9+ sites across 6 crates; panics on any non-ASCII input.
2. **`let _ = …` / `.ok()` / `unwrap_or_default()` on a real operation** — the
   single largest class; converts failures into false success or blank data.
3. **Poisoned-lock panics** (`.lock().unwrap()` / `.expect(...)`) — 100+ sites;
   one panic while holding a lock bricks the whole subsystem thereafter.
4. **Sync I/O and blocking locks on async paths** (`std::fs`, `std::process`,
   `std::sync::Mutex<Connection>`) — stalls or deadlocks tokio workers.
5. **Redaction that fails open** — a poisoned secret registry silently disables
   exact-match masking; the regex layer then misses registered secrets.

---

## 3. Severity and priority legend

| Severity | Definition |
|----------|------------|
| `Critical` | Crash, data loss, secret exfiltration, or indefinite hang reachable on plausible input |
| `High` | Silent data loss, a security control that fails open, or a panic family that disables a subsystem |
| `Medium` | Wrong behaviour in an edge case, or a swallowed error that hides real failures |
| `Low` | Hygiene/fragility; no current user-visible impact |

| Priority | Meaning | Target |
|----------|---------|--------|
| `P0` | Release-blocking | next release |
| `P1` | High — user-visible correctness or security | M1 |
| `P2` | Medium - compounding | M2-M4 |
| `P3` | Low — batch opportunistically | M5 / ongoing |

Effort: **S** (< 1 h), **M** (half day), **L** (1-2 days), **XL** (needs design).

---

## 4. Anti-pattern taxonomy (class key used in task tables)

| Class | Name | Examples |
|-------|------|----------|
| `SWALLOW` | Silent error swallowing / false success | `let _ =`, `.ok()`, `unwrap_or_default()`, `if let Ok(_)` |
| `PANIC` | Panic on data-driven input | `unwrap()`, `expect()`, `panic!`, `unreachable!`, `[..n]` byte slicing, integer overflow |
| `CONC` | State / concurrency | poisoned locks, guard across `.await`, TOCTOU, detached tasks, unbounded loops |
| `LOGIC` | Logic / semantic fallback | catch-all `_ =>` changing meaning, destructive defaults, off-by-one, dual parsers |
| `RES` | Resource / API misuse | blocking I/O in async, missing timeouts, leaked handles/temp files |
| `SEC` | Security control defect | fail-open redaction, SSRF, secret on disk, credential logging |

---

## 5. Milestones

| Milestone | Theme | Exit criteria |
|-----------|-------|---------------|
| **M0** | Release-blocking: crash, security, concurrency | All `Critical` + `P0` tasks complete; each has a regression test; `cargo test --workspace` green |
| **M1** | Silent failure → data-loss / false-success | Every `SWALLOW` finding that loses user data or reports false success is fixed and tested |
| **M2** | Panic-surface hardening | No `.lock().unwrap()/.expect()` or reachable `unreachable!` remains on runtime paths |
| **M3** | Async & blocking-I/O hygiene | No `std::fs`/`std::process`/blocking-lock on an async path without `spawn_blocking`/timeout |
| **M4** | Logic, validation & API-contract correctness | `LOGIC` findings resolved; each has an assertion-level test |
| **M5** | Guardrails & prevention | Lint gate + invariant tests land; recurring patterns cannot silently return |

Milestones are ordered by user-visible return. **M0 is the release gate**;
M1-M5 are the follow-up train.

---

## 6. M0 — Release-blocking (crash / security / concurrency)

### 6.1 Correctness crashes (PANIC)

| ID | Sev | Pri | Class | Location | Defect | Required fix | Effort | Acceptance |
|----|-----|-----|-------|----------|--------|--------------|--------|------------|
| FUNC-001 | Critical | P0 | PANIC | `crates/ragent-llm/src/providers/openrouter.rs:892-897` | `&error_body[..MAX_ERR_LEN]` slices an HTTP error body at fixed byte 4096; multibyte UTF-8 → mid-codepoint panic | Clamp to a char boundary (`floor_char_boundary`/`char_indices`) before slicing | S | Unit test feeds a 5 KB body of multibyte chars; no panic, prefix preserved |
| FUNC-002 | Critical | P0 | PANIC | `crates/ragent-agent/src/memory/extract.rs:667`, `:741` | `&content[..content.len().min(60)]` / `..min(80)` cut at byte offsets | Cut on a char boundary (reuse `ragent_types::strutil::truncate_bytes_no_ellipsis`) | S | Test with CJK content > 60/80 bytes passes |
| FUNC-003 | Medium | P0 | LOGIC | `crates/ragent-tools-extended/src/office_common.rs:97-105`; `libreoffice_common.rs:77-84` | The `boundary` used for the final slice is a raw byte offset (`max_body`) or a `rfind('\n')` result, not a char boundary — the truncation length is wrong for non-ASCII, newline-free text (may drop a char or split the notice from the body) | Clamp `boundary` with `is_char_boundary` after the `rfind` fallback; factor one shared helper | S | Test with non-ASCII, newline-free body > limit: output length ≤ budget and ends on a char boundary |
| FUNC-004 | High | P0 | PANIC | `crates/ragent-tools-extended/src/finance/providers/yahoo.rs:30,40` | `Instant::now().checked_sub(..).unwrap()` / `checked_sub(elapsed).unwrap()` in throttle logic | Use `saturating_sub`/`unwrap_or_default` on the `Duration` | S | Throttle test with clock at epoch boundary passes |
| FUNC-005 | High | P0 | PANIC | `crates/ragent-llm/src/providers/router_config.rs:213`, `:239` | `panic!("weight_by_index: index {index} out of range")` on a classifier/config-supplied index | Return clamped value or `Result`; never `panic!` on data-derived index | S | Test with index ≥ 15 returns error/default, no panic |

### 6.2 Security controls (SEC)

| ID | Sev | Pri | Class | Location | Defect | Required fix | Effort | Acceptance |
|----|-----|-----|-------|----------|--------|--------------|--------|------------|
| FUNC-006 | Critical | P0 | SEC | `crates/ragent-types/src/sanitize.rs:180-183` | Poisoned `SECRET_REGISTRY` read yields `registry_empty = true`, so the **exact-match redaction layer is skipped** — registered secrets leak | On poison, consult the registry anyway (`into_inner`) or fail closed (assume non-empty) | S | Test poisons the lock, registers a secret, asserts it is still redacted |
| FUNC-007 | High | P0 | SEC | `crates/ragent-types/src/sanitize.rs:16-43` | `SECRET_PATTERN` is case-sensitive and assignment-only; bypassed by `API_KEY=`, `"token": "…"`, `token: …` | Add case-insensitive assignment group + quoted JSON/YAML form | M | Tests for `API_KEY=`, `"token": "x"`, `token: x` all redact |
| FUNC-008 | High | P0 | SEC | `crates/ragent-types/src/sanitize.rs:90-116` | `unregister_secret`/`clear_secret_registry`/`seed_secrets` use `if let Ok(..)` — poison makes them silent no-ops (clear can appear to succeed; seed can silently fail) | Recover via `PoisonError::into_inner` like `registry_write` already does | S | Poison test asserts clear/seed still operate |
| FUNC-009 | Critical | P0 | SEC | `crates/ragent-tools-vcs/src/gitlab/client.rs:397-442` | `readme_url` from the API response is fetched with `PRIVATE-TOKEN` attached to **whatever host it names** → SSRF + PAT exfiltration | Validate the raw URL host equals the configured instance host before sending any token | M | Test with a foreign `readme_url` host asserts the token is not sent |
| FUNC-010 | High | P0 | SEC | `crates/ragent-tools-vcs/src/gitlab/client.rs:124-127` | `resp.text().await.unwrap_or_default()` on a 2xx, then `if empty { Ok(Null) }` — a body-read failure is reported as **success** | Distinguish read failure from a genuine empty body (`resp.text().await?`) | S | Test forces a body-read error; result is `Err`, not `Ok(Null)` |
| FUNC-011 | High | P0 | SEC | `crates/ragent-tools-vcs/src/gitlab/auth.rs:106` | `get_provider_auth(..).ok().flatten()` collapses a DB error into "not configured" | Only `Ok(None)` means "no token"; propagate the read error | S | Test with DB error returns an error, not "not configured" |
| FUNC-012 | High | P0 | SEC | `crates/ragent-tools-vcs/src/github/auth.rs:63-69` | Token written by `std::fs::write` then chmod `0o600` → world-readable race window; `path.parent().unwrap()` panics | Create with `OpenOptions::mode(0o600)` (unix) / temp + atomic rename; replace `.unwrap()` | M | Unix test asserts the file is never group/world-readable; no-panic on odd home path |
| FUNC-013 | High | P0 | SEC | `crates/ragent-storage/src/storage.rs:1528`; `:244` | `seed_secret_registry` drops unreadable key rows silently (keys escape redaction); `deobfuscate_key_v1` maps invalid UTF-8 to `""` (corrupt key looks absent) | Count+`warn!` skipped rows; distinguish corrupt vs absent | M | Test with one unreadable row asserts a warning/counter and that the key is not silently absent |

### 6.3 Concurrency hazards (CONC)

| ID | Sev | Pri | Class | Location | Defect | Required fix | Effort | Acceptance |
|----|-----|-----|-------|----------|--------|--------------|--------|------------|
| FUNC-014 | Critical | P0 | CONC | `crates/ragent-tools-core/src/bash.rs:1108-1112`, `:1210-1211`, `:1611-1633` | `killpg(pgid)` can fire with `pgid = 0` → signals **ragent's own process group**; `capture.pgid.set(pid)` failure is swallowed; child not reaped on the timeout path | Treat pgid `0`/unset as "not armed" and refuse to kill; arm atomically at spawn; `wait()` the child after kill | M | Test: timeout before pgid is recorded must not signal the test process; child is reaped |
| FUNC-015 | Critical | P0 | CONC | `crates/ragent-tui/src/app/slash.rs:6968`, `:7015` | Bare `Handle::block_on(...)` on the UI runtime from a handler **outside** `block_in_place` → panic/deadlock; result discarded | Route through the async command path or `tokio::spawn`; never `block_on` the runtime you are on | M | Test drives `/spec activate` through the handler; TUI does not panic/deadlock |
| FUNC-016 | High | P0 | CONC | `crates/ragent-server/src/routes/research.rs:443-455` (and `:632-644`) | In-flight guard is non-atomic: `contains_key` then drop lock then `register` re-locks and `insert`s unconditionally → two concurrent runs orphan an SSE channel, defeating the 409 | Make check+insert atomic under one lock | M | Concurrency test: two same-name POSTs → exactly one 409, one run registered |
| FUNC-017 | High | P0 | CONC | `crates/ragent-research/src/source_vault.rs:156-157` (+ all methods) | `Arc<std::sync::Mutex<Connection>>` blocks a tokio worker on every vault call; can deadlock under a multi-thread executor | Wrap in `spawn_blocking`, or use a connection-per-task design | L | Test: concurrent vault calls from async tasks complete without starving workers |
| FUNC-018 | High | P0 | CONC | `crates/ragent-codeindex/src/worker.rs:191`, `:197-202`, `:306` | `let _ = handle.join()` hides a worker panic (index freezes silently); `queue_reindex`/`queue_full_reindex` drop `SendError`; `full_reindex()` error swallowed | Log/report the join error; surface `SendError` as "index worker not running"; log reindex failure into `WorkerStats` | M | Test: worker panic surfaces a status/log; a queued reindex on a stopped worker is observable |
| FUNC-019 | Medium | P0 | CONC | `crates/ragent-storage/src/storage.rs:539` | `migrate` runs the DDL batch and the `schema_version` write with no enclosing transaction — failure leaves schema/version inconsistent; concurrent opens race | Wrap DDL batch + version upsert in one `transaction()` | M | Test interrupts migration; asserts atomic version/schema |

---

## 7. M1 — Silent failure → data loss / false success

| ID | Sev | Pri | Class | Location | Defect | Required fix | Effort | Acceptance |
|----|-----|-----|-------|----------|--------|--------------|--------|------------|
| FUNC-020 | High | P1 | SWALLOW | `crates/ragent-storage/src/storage.rs:1182`; `:4088` | `from_str(parts_json).unwrap_or_default()` turns a corrupt message row into an **empty** message with no log | `warn!` with the message id; skip explicitly or error | S | Test with corrupt `parts` asserts a warning and no silent blank turn |
| FUNC-021 | High | P1 | SWALLOW | `crates/ragent-storage/src/storage.rs:2804` | `memory_forget` discards the DELETE result → reports deletion that did not happen | Propagate `?` and return the changed count | S | Test: failed delete returns an error/false |
| FUNC-022 | Medium | P1 | SWALLOW | `crates/ragent-storage/src/storage.rs:4211`, `:4566`, `:4224` | Corrupt milestones/`blocked_by` JSON silently becomes empty; bad `progress` clamps to 0 | Log with the row id; do not coerce to empty; validate progress | S | Tests with corrupt JSON assert a warning and preserved rest-of-set |
| FUNC-023 | Medium | P1 | SWALLOW | `crates/ragent-storage/src/storage.rs:3092`, `:3219` | Malformed embedding blobs are silently skipped on both search paths | Shared helper logging+counting skips; surface the count | S | Test with a corrupt blob asserts a warning/skip count |
| FUNC-024 | High | P1 | SWALLOW | `crates/ragent-specs/src/commands.rs:1056` | `PlanParser::parse(plan_md).ok()?` — a PLAN.md **parse error** is indistinguishable from "no tasks" | Propagate the error (or `error!` before mapping) | S | Test with malformed PLAN.md yields an error, not `None` |
| FUNC-025 | High | P1 | SWALLOW | `crates/ragent-specs/src/io.rs:61-64` | `if let Ok(file) … { let _ = sync_all() }` then still renames → silently voids the documented crash-durability guarantee | `File::open(..).await?.sync_all().await?` | S | Test asserts sync failure aborts the write |
| FUNC-026 | Medium | P1 | LOGIC | `crates/ragent-specs/src/plan_parser.rs:453-456`; `io.rs:377` | Malformed task rows are `warn!`+`continue`d (tasks silently lost); a second, unvalidated parser in `io.rs` can disagree | Fail the parse / return warnings; delete the duplicate parser | M | Test: malformed row is surfaced; both parsers agree on a fixture |
| FUNC-027 | High | P1 | SWALLOW | `crates/ragent-tui/src/input.rs:1305`, `:1533`; `app/models.rs:501`; `app/swarm.rs:174`; `app/cron.rs:616` | `let _ = storage.set_provider_auth(..)` / `set_setting` / `store.save()` / `set_cron_event_enabled` — credential, model, swarm and cron state silently not persisted while the UI reports success | Handle `Err`, surface a status/toast, keep dialogs open on failure | M | Test: storage failure is visible and state is not reported as saved |
| FUNC-028 | High | P1 | SWALLOW | `crates/ragent-research/src/manager.rs:744` | `read_to_string(RESEARCH.md).unwrap_or_default()` → an unreadable file yields a blank index entry that is silently **unsearchable** | Propagate `?` or `warn!`+skip | S | Test: unreadable RESEARCH.md logs and skips, no blank entry |
| FUNC-029 | High | P1 | SWALLOW | `crates/ragent-research/src/engine.rs:340`; `analysis.rs:653` | `synthesize(..).await.ok()` drops a synthesis error; malformed LLM JSON `.ok()` hides "corrupt" vs "absent" | Match the result; emit an event/log | S | Test: synthesis failure surfaces |
| FUNC-030 | Medium | P1 | SWALLOW | `crates/ragent-research/src/web_gatherer.rs:1968`+`:1992-1999`; `:2410-2428` | Partial sub-query search failure is invisible unless *all* sub-queries fail; a vault-store failure still counts the source as captured | Emit a partial-failure event / `incomplete_coverage` flag; surface vault-store failures | M | Test: one failing sub-query surfaces a partial-failure signal |
| FUNC-031 | Medium | P1 | SWALLOW | `crates/ragent-codeindex/src/lib.rs:954`; `graph/{export,mod,traverse}.rs` | Self-heal `let _ = store.upsert_file(..)` silently fails; missing file ids map to `""` in graph output | Log/handle the write; skip or mark missing paths instead of `""` | M | Test: self-heal failure is logged; graph nodes have no blank `source_file` |
| FUNC-032 | High | P1 | SWALLOW | `crates/ragent-llm/src/providers/anthropic.rs:485`; `openai.rs:492`; indices at `anthropic.rs:493`,`:510`,`:541`,`openai.rs:527` | Malformed `data:` JSON dropped via `Err(_) => continue` (lost tool-call deltas); missing tool index defaults to `0`, merging distinct parallel calls | `warn!` the payload; require `index` (skip/error if absent) | M | Tests: corrupt frame logs; two parallel calls with missing index are not merged |
| FUNC-033 | High | P1 | LOGIC | `crates/ragent-llm/src/providers/anthropic.rs:465`; `openai.rs:474`; `gemini.rs:599` (all providers) | `String::from_utf8_lossy(&chunk)` decodes **each TCP chunk** independently → multibyte chars split across chunks are corrupted | Carry an incomplete-tail byte buffer / decode per complete line | L | Test splits a CJK/emoji payload across chunk boundaries; text is intact |
| FUNC-034 | Medium | P1 | SWALLOW | `crates/ragent-llm/src/providers/http_client.rs:176`, `:199`, `:219` | Retry/4xx/429 error-body reads use `unwrap_or_default()` → empty diagnostics | Include the read error in the message | S | Test forces a body-read error; message names it |
| FUNC-035 | Medium | P1 | SWALLOW | `crates/ragent-tools-extended/src/masterfetch/search/{wikipedia.rs:644-653,openalex.rs:248-249,exa.rs:299}`; `masterfetch/pdf.rs:89`; `cache.rs:366` | Engine/transport/parse errors collapse to `None` → a dead engine looks like a zero-result engine; corrupt PDF/cache metadata silently degraded | Return `Result`; map to `EngineReport::error` / treat as cache miss | M | Tests: a failing engine surfaces an error/`blocked` entry; corrupt cache metadata re-fetches |
| FUNC-036 | Medium | P1 | SWALLOW | `crates/ragent-agent/src/session/verification.rs:199-200`; `session/history.rs:237` | Drain-thread `join().unwrap_or_default()` hides a panicking thread (verification looks clean); missing tool result becomes `""` | Match join result; emit an explicit "(no output)" marker | S | Test: panicking drain surfaces a failure marker |
| FUNC-037 | High | P1 | SWALLOW | `crates/ragent-server/src/routes/mod.rs:388-389`; `routes/research.rs:747`, `:508-512`, `:696-700` | Agent-resolution error silently downgraded to `general`; related-search failure returned as `[]`; terminal-failure event send ignored | Log errors; never conflate failure with empty; observe the terminal send | M | Tests: bad agent config is logged; search failure is distinguishable; failure event delivered |
| FUNC-038 | High | P1 | LOGIC | `crates/ragent-tools-vcs/src/github/github_prs.rs:349` | `input["method"].as_str().unwrap_or("merge")` — an unrecognised method silently performs a real **merge** | Reject values outside `{merge,squash,rebase}` | S | Test: unknown method returns an error, no merge attempted |

### 7.1 M1 completion checklist (the silent-failure classes)

The M1 goal is not "fix these tasks" but "no failure is silently converted to
success or blank data on a path that reaches the user". Before M1 can close:

1. Every `let _ = <Result>` on a runtime path either handles the error or carries
   a `debug!`/`warn!` with a one-line justification comment.
2. Every `unwrap_or_default()` on a `Result` is either replaced with explicit
   handling or annotated as intentional (with `Option`, not `Result`).
3. Every catch-all `_ =>` in an enum dispatch returns a *neutral* value or errors;
   none silently selects a destructive default (merge, overwrite, delete).
4. Each fixed row in the tables above has a regression test that traces to its
   FUNC ID (see FUNC-081).

---

## 8. M2 — Panic-surface hardening

| ID | Sev | Pri | Class | Location | Defect | Required fix | Effort | Acceptance |
|----|-----|-----|-------|----------|--------|--------------|--------|------------|
| FUNC-040 | High | P2 | PANIC | `crates/ragent-tools-extended/src/lib.rs:372-405`; `masterfetch/robots.rs:918-1010`; `masterfetch/cache.rs:324-586`; `finance/throttle.rs:38-62`; `finance/providers/paid.rs:54` | `.expect("… lock poisoned")` on `RwLock`/`Mutex` — one panic bricks the tool registry / cache / robots / finance surfaces for the process lifetime | `unwrap_or_else(PoisonError::into_inner)` | M | Grep test asserts no `.expect("…poisoned")` remains; poison test passes |
| FUNC-041 | High | P2 | PANIC | `crates/ragent-agent/src/team/manager.rs:1384,1413,1637`; `session/mod.rs:154,169`; `background/mod.rs` (13 sites) | `.lock().unwrap()/.expect("… poisoned")` — a poisoned watchdog/cache/background lock panics unrelated callers | Recover via `into_inner()`; centralise a lock helper | M | Poison tests for each subsystem pass |
| FUNC-042 | Medium | P2 | PANIC | `crates/ragent-research/src/` (~68 `.lock().unwrap()` sites; runtime ones in `session.rs`, `manager.rs`) | Poisoned research locks panic the gather/session path | Recover via `into_inner()` on runtime paths (`session.rs:1825` is the reference pattern) | M | Grep gate: no `.lock().unwrap()` in non-test research code |
| FUNC-043 | Medium | P2 | PANIC | `ragent-agent`: `history.rs:757`, `reference/resolve.rs:245`, `compaction/runner.rs:204,252`, `team/task.rs:672`; `ragent-storage`: `storage.rs:1730`; `ragent-tools-core`: `bash.rs:862,1076`; `ragent-llm`: `ollama_cloud.rs:428`, `ollama.rs:334`; `ragent-tui`: `input_handler.rs:719`, `input.rs:2108-2158`, `research.rs:255`; `ragent-tools-extended`: `gmail.rs:407` | `unreachable!()`/`expect`/`unwrap` on invariant-dependent paths — a refactor or unexpected variant becomes a process panic | Return `Result`/`bail!`/default; no `unreachable!` on data-driven dispatch | M | Each site has a test that exercises the previously-"unreachable" branch |
| FUNC-044 | Medium | P2 | PANIC | `crates/ragent-storage/src/storage.rs:3799,3951,4832` | Positional `row.get(7)/get(11).unwrap_or(0/0.0)` — a schema shift silently zeroes ranks / misreads state | Bind by column name and propagate the get error | S | Test asserts a renamed/added column surfaces an error |
| FUNC-045 | Medium | P2 | PANIC | `crates/ragent-codeindex/src/worker.rs:174`; `search.rs:474`; `graph/edges.rs:299,519`; `graph/communities.rs:193`; `store.rs:939,970,998` | `.expect("failed to spawn index worker thread")` aborts the process on thread exhaustion; `.expect("writer initialised above")`; `unwrap_or_default()` collapses NULL vs empty | Return `Result` from spawn; `ok_or_else` for writers; document NULL contract | M | Spawn-failure test returns `Err`; no blank graph fields |

---

## 9. M3 — Async & blocking-I/O hygiene

| ID | Sev | Pri | Class | Location | Defect | Required fix | Effort | Acceptance |
|----|-----|-----|-------|----------|--------|--------------|--------|------------|
| FUNC-050 | High | P2 | RES | `crates/ragent-tools-vcs/src/git/mod.rs:66-80`; `github/client.rs:272-277`; `github/github_prs.rs:258-267` | `std::process::Command::output()` with **no timeout** executed inline in `async fn execute` — a hung git blocks a tokio worker | Run via `spawn_blocking` (or `tokio::process` + `timeout`) with kill-on-timeout | M | Test: a sleeping git shim is killed at the timeout; worker not stalled |
| FUNC-051 | Medium | P2 | RES | `crates/ragent-research/src/gather_log.rs:29,96-104,169-193` | `std::fs::create_dir_all`/`OpenOptions::open` run sync on async paths; the `MutexGuard` is held across `write_all`+flush | `spawn_blocking` for open/write; keep the critical section minimal | M | Test: log write under a slow sink does not block the runtime |
| FUNC-052 | Medium | P2 | RES | `crates/ragent-research/src/session.rs:3539`; `cluster.rs:173-492`; `source_vault.rs:205-247,494-498` | `std::fs::read_to_string`/`read_dir`/`write`/`rename` on paths reachable from async code | `tokio::fs` or `spawn_blocking`; surface temp-cleanup failure | M | Async-path test shows no blocking call on the runtime thread |
| FUNC-053 | Medium | P2 | CONC | `crates/ragent-tools-vcs/src/gitlab/client.rs:258-341`, `:104-105`; `gitlab/gitlab_pipelines.rs:269` | Recursive tree fetch has no entry/request cap; 429 `bail!`s with no `Retry-After`/backoff; jobs list has no pagination (>100 jobs silently truncated) | Add a max-entry/request budget; honour `Retry-After` with bounded retries; follow pagination | M | Tests: deep tree is bounded; 429 retries then fails; >100 jobs are paged |
| FUNC-054 | Medium | P2 | RES | `crates/ragent-agent/src/session/verification.rs:161-169` | Drain threads are only joined on timeout/error branches → thread leak per verification run | Join both handles on every exit path | S | Test asserts join on the success path |

---

## 10. M4 — Logic, validation & API-contract correctness

| ID | Sev | Pri | Class | Location | Defect | Required fix | Effort | Acceptance |
|----|-----|-----|-------|----------|--------|--------------|--------|------------|
| FUNC-060 | High | P2 | LOGIC | `crates/ragent-tui/src/input.rs:322` | `.get(selected_index).cloned().unwrap_or_default()` submits an **empty-string answer** to the agent on a stale index | Clamp the index; handle out-of-range explicitly | S | Test: out-of-range selection yields no blank answer |
| FUNC-061 | High | P2 | LOGIC | `crates/ragent-tools-core/src/move_file.rs:62-70`; `copy_file.rs:66`; `append_file.rs:66-75` | `move_file` creates parents then renames (leaving dirs on failure) and silently clobbers an existing dst; `copy_file` self-copy corrupts; append never flushes | Rename-first + EXDEV fallback; refuse to clobber unless forced; bail on self-copy; `flush()` after append | M | Tests: failed move leaves no dirs; self-copy errors; append is flushed |
| FUNC-062 | Medium | P2 | LOGIC | `crates/ragent-tools-vcs/src/github/client.rs:157,174,191` | `post`/`put`/`patch` hardcode `https://api.github.com{path}`, ignoring `self.base_url` (which `get` honours) — GitHub Enterprise/mock routing broken | Build the URL the same way `get` does | S | Test: `with_base_url` routes a write to the configured host |
| FUNC-063 | Medium | P2 | LOGIC | `crates/ragent-tools-vcs/src/github/github_issues.rs:471-480`; `github_prs.rs:71-73`; `gitlab/client.rs:404` | Hand-rolled `urlencoded` emits `%{:02X}` per `char` (wrong for non-ASCII); `base` branch interpolated unencoded; gitlab `file_path` encodes only `/` | Percent-encode UTF-8 bytes; encode `base`; reuse proper path encoding | M | Tests: label with `é`/`€`, branch with `&`, path with `#` all encode correctly |
| FUNC-064 | High | P2 | LOGIC | `crates/ragent-codeindex/src/store.rs:704-730` | `parent_id` is resolved only from first-pass `id_map`; a symbol whose parent is itself a child resolves to `None` — nesting silently lost/non-deterministic | Two-phase: insert all with NULL parent, then one `UPDATE … SET parent_id` pass | M | Test: a 3-level nesting resolves every parent correctly |
| FUNC-065 | Medium | P2 | LOGIC | `crates/ragent-research/src/verify.rs:156-164`, `:179-182,219`; `contradiction.rs:426,439` | Empty analysis returns `passed: true` ("no findings"); uncited findings `continue` before `checked` so it can PASS while listing citation failures; catch-all `_ => "positive"/"negative"` mislabels unknown dimensions | `passed:false`/`NotVerified` on empty; count uncited findings; return neutral "unknown" on catch-all | M | Tests: empty analysis is not "verified"; an uncited finding makes it fail; unknown dimension is neutral |
| FUNC-066 | Medium | P2 | LOGIC | `crates/ragent-server/src/routes/mod.rs:364-369`, `:156-165`; `routes/research.rs:566` | Rate limit off-by-one (61/min enforced vs "60" advertised); `constant_time_eq` early-returns on length mismatch and stops at the shorter slice (not constant-time); dropped SSE sends invisible | Increment after check; compare fixed-length digests; rate-limited debug log on send failure | S | Tests: 61st request 429s; length-mismatch path is timing-equalised; drop is logged |
| FUNC-067 | Medium | P2 | LOGIC | `crates/ragent-tools-vcs/src/vcs_provider.rs:118`, `:185` | `input.contains("github.com")` classifies by substring (a GitLab URL containing it misroutes); any dotted first segment is treated as a host (dotted namespaces misroute) | Parse the URL/host and compare equality; require a stronger host signal | M | Tests: `gitlab.com/x/github.com/y` routes to GitLab; dotted namespace stays with the configured host |
| FUNC-068 | Medium | P2 | LOGIC | `crates/ragent-tools-core/src/write.rs:68-74` vs `create.rs:75`, `append_file.rs:58`, `copy_file.rs:57-58`, `move_file.rs:58-59`, `mkdir.rs:50`, `rm.rs:70`; `rm.rs:62-66`; `create.rs:77,92` | Only `write` honours `allowed_roots`; siblings validate against `working_dir` only → valid destinations wrongly rejected. `rm` blocks literal `[`/`*`/`?` filenames; `create` labels Created/Overwrote from a TOCTOU `exists()` probe | One shared root-validation helper; drop the glob character heuristic; derive existence from the open result | M | Tests: each file tool accepts an `allowed_roots` destination; a literal `[` filename is deletable; Created/Overwrote is correct under race |
| FUNC-069 | Medium | P2 | LOGIC | `crates/ragent-tools-vcs/src/gitlab/gitlab_issues.rs:304`; `gitlab_pipelines.rs` (job pagination, see FUNC-053) | Non-numeric assignee ids silently dropped → partial/misleading assignee set | Validate and return an error naming the bad id | S | Test: `assignees=a,b` errors on `b` |

---

## 11. M5 — Guardrails & prevention

| ID | Sev | Pri | Class | Location | Defect | Required fix | Effort | Acceptance |
|----|-----|-----|-------|----------|--------|--------------|--------|------------|
| FUNC-080 | High | P3 | GUARD | workspace `clippy.toml` / crate lints | The recurring classes have no compiler gate, so they silently return | Add `clippy::disallowed_methods` for `std::sync::{Mutex,RwLock}::lock().unwrap()/expect` and (where feasible) `std::fs` on async crates; run in CI | M | CI fails on a seeded violation |
| FUNC-081 | High | P3 | GUARD | tests across crates | No regression tests pin the invariants fixed here | Add tests: redaction fail-closed (FUNC-006), pgid-zero guard (FUNC-014), research-run TOCTOU (FUNC-016), URL-encode table (FUNC-063), nesting resolution (FUNC-064), no-`block_on` handler (FUNC-015) | M | All new tests green; each traces to a FUNC ID |
| FUNC-082 | Low | P3 | GUARD | `crates/ragent-research/src/search_budget.rs:51-67`; `web_gatherer.rs:1979`; `gather_log.rs:15-22`; `server/routes/mod.rs:156-165` | Contract/doc drift: `try_acquire` increments on rejection (doc says it does not); unlimited budget reported as `0`; log loses records on hard kill; "constant-time" claim is inaccurate | Fix behaviour or the doc; make the budget field `Option`/sentinel; flush policy; correct the comment | S | Docs match behaviour; tests assert the documented semantics |

---

## 12. Verification protocol (every task)

1. `cargo fmt` after every `.rs` edit (mandatory — see AGENTS-RUST.md).
2. `cargo check -p <crate>` then `cargo clippy -p <crate> -- -D warnings`.
3. `cargo test -p <crate>` (the task's acceptance test first, then the crate suite).
4. For panic/UTF-8 tasks: the test **must** use non-ASCII multibyte input.
5. For security tasks: the test must assert the *negative* (secret absent / token not sent).
6. `cargo fmt --check` before marking complete.

A task is not complete until its acceptance test in the tables above passes.

---

## 13. Non-goals

- **No performance work** — allocation/latency findings live in `PERFPLAN.md`.
- **No behaviour change beyond the stated fix**; every task is surgical and
  must keep observable output byte-compatible unless the task explicitly says otherwise.
- **No `unsafe`** beyond the single approved site (`kill_process_group`).
- **No blanket `#[allow]` to silence a class** — each fix handles the case.
- Low-severity hygiene sites with no runtime exposure (e.g. test-only locks,
  `fmt::Write` `let _ =`) are out of scope and recorded as rejected in Appendix A.

---

## Appendix A — Verified clean / rejected findings

Recorded so the audit is not repeated and false positives do not become work items.

| Reported as | Location | Verdict |
|-------------|----------|---------|
| High (byte-slice panic) | `crates/ragent-agent/src/compaction/serializer.rs:171` | **False positive.** `cut` comes from `char_indices().nth(max)` and is a char boundary by construction; doc comment confirms. No action. |
| High (byte-slice panic) | `crates/ragent-tools-extended/src/gmail.rs:488` | **False positive.** `cut` is built from `char_indices`; `cut == 0` only when the string is empty, where `[..0]` is safe. No action. |
| "~80 non-test `unwrap`/`expect` in TUI cron" | `crates/ragent-tui/src/app/cron.rs` | **Correction.** All 80 are inside `#[cfg(test)] mod tests` (starts line 694); production code is unwrap/expect-free. The real cron defect is `let _ = set_cron_event_enabled` (FUNC-027). |
| Poison-panic ("research has 68 `.lock().unwrap()`") | `crates/ragent-research/src/tier_router.rs:52,59` | Test-only observer; no runtime exposure. Excluded from FUNC-042. |
| `let _ = write!/writeln!` | `crates/ragent-research/src/document.rs:1984-2738` | Writer is a `String` via `fmt::Write` (infallible). Benign. |
| Retry/backoff uncapped | `crates/ragent-llm/src/providers/http_client.rs:159` | Verified capped (≤8 s, bounded attempts). No action. |
| `[..n]` slicing in SSE helpers | `http_client.rs:45`, `router_client.rs:492`, `router_classifier.rs:933` | Verified correct (cut at `\n`/boundary-checked). No action. |
| Divide-by-zero in score math | `crates/ragent-research/src/corpus_critic.rs:157,200,216,233` | Verified guarded. No action. |
| `VCS: GIT_ASKPASS=false` | `crates/ragent-tools-vcs/src/git/mod.rs:73` | Low/hygiene: set `GIT_ASKPASS` to empty rather than `false`. Folded into M4 opportunistic work. |

---

## Appendix B — Milestone → task index

| Milestone | Tasks |
|-----------|-------|
| M0 | FUNC-001 .. FUNC-019 |
| M1 | FUNC-020 .. FUNC-038 |
| M2 | FUNC-040 .. FUNC-045 |
| M3 | FUNC-050 .. FUNC-054 |
| M4 | FUNC-060 .. FUNC-069 |
| M5 | FUNC-080 .. FUNC-082 |

---

## Appendix C — Coverage gaps (follow-up audit worklist)

The wave agents time-boxed; these areas were **not** read to the depth required
to claim they are defect-free. They are a reading worklist, not a clean bill.

| Area | Not yet audited |
|------|-----------------|
| `ragent-config/src` | Full crate — parsing, defaulting, error reporting (only grepped) |
| `ragent-server` | `sse.rs` (1178 lines, event mapping); `routes/memory.rs` (`unwrap_or_default` at 190/300/316 unverified) |
| `ragent-tools-extended` | `plot/*`, `codeindex_*.rs` bodies, `channels.rs`, `memory/embedding*`; masterfetch `extractor.rs`, `youtube.rs`, `urlnorm.rs`, `language.rs`, `links.rs`, `security.rs`, `crawl/` |
| `ragent-research` | Deep read of `relevance.rs`, `document.rs` (4414 lines), `session.rs` (6430 lines), `analysis.rs`, `cluster.rs`, `io.rs`, `open_access.rs` beyond grep hits |
| `ragent-tools-vcs` | `gitlab/auth.rs` body, `gitlab_mrs.rs` body (pagination/state mapping) |
| `ragent-telemetry` | `recorder.rs` beyond grep hits |
| `ragent-tui` | `cron.rs` production paths beyond the storage-swallow rows; `layout_*`, `research_progress.rs` |
| `ragent-agent` | Call-site tracing of the two detached `tokio::spawn` sites (`team/manager.rs:916`, `hooks/mod.rs:604`) |
| root `src/` | `main.rs`, `cli.rs`, `panic_hook.rs` (grep-clean for panics, not read) |

A follow-up pass should re-run the same per-area prompts on these targets and
append `FUNC-100+` tasks, repeating the M0 verification protocol for anything
Critical/High that surfaces.

---

## Appendix D — Recurrence prevention

The five dominant patterns all share one property: they fail **silently** and
only under non-obvious input (non-ASCII, a poisoned lock, a second concurrent
request, a slow disk). Prevention therefore targets visibility, not just repair:

1. **UTF-8 slicing** — forbid raw `&s[..n]` in review; route all truncation
   through `ragent_types::strutil` helpers (FUNC-001/002/003 establish the pattern).
2. **Silent swallow** — every `let _ =` on a `Result` must carry a comment
   justifying discard, or a `debug!`/`warn!`; CI gate in FUNC-080.
3. **Poison panics** — one workspace lock helper using `PoisonError::into_inner`
   (FUNC-041), then a grep gate (FUNC-040/042).
4. **Blocking on async** — `spawn_blocking`/`tokio::fs` at the boundary; the
   `await_holding_lock` lint is necessary but not sufficient (FUNC-080).
5. **Fail-open security** — redaction and auth decisions must default to the
   *safe* outcome on ambiguity (FUNC-006/008); tests assert the negative.
