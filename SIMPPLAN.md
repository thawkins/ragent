# SIMPPLAN.md — Code Quality Remediation Plan

> **Source:** `/simplify` review of `HEAD~3` changes (3 parallel explore agents,
> ~30 findings). This plan covers the **issues identified but NOT applied**
> during the initial pass. Items already fixed in that pass are listed in
> [Appendix A — Applied Fixes](#appendix-a--applied-fixes) for reference.
>
> **Scope:** `crates/ragent-research`, `crates/ragent-agent`, `crates/ragent-tui`,
> `crates/ragent-server`, `crates/ragent-storage`, `crates/ragent-tools-core`,
> `crates/ragent-llm`, `crates/ragent-types`, `src/`.
>
> **Status legend:** `[ ]` pending · `[~]` in progress · `[x]` done

---

## Milestone 1 — Correctness Verification (Priority: High)

Verify that recent refactors did not break existing semantics. These are
investigation tasks first — only apply a fix if a real defect is confirmed.

### Tasks

- [ ] **SIMP-001 — Verify `std::mem::take` does not empty `RunOutcome.sources`**
  - **File:** `crates/ragent-research/src/session.rs` (~line 1358–1362)
  - **Issue:** `std::mem::take(&mut sources)` empties the `sources` vec.
    `RunOutcome.sources` is constructed from `synthesis_sources` (line 1707),
    which IS the moved value, so this is likely correct. However, any code
    path that reads `sources` AFTER the take and BEFORE `RunOutcome`
    construction will see an empty vec.
  - **Action:** Grep for all `sources` references after line 1362 in the
    `run()` method. Confirm every downstream use reads `synthesis_sources`,
    not `sources`. If any reads the now-empty `sources`, that is a bug —
    fix by using `synthesis_sources` or cloning before the take.
  - **Verify:** `grep -n 'sources' crates/ragent-research/src/session.rs` from
    line 1362 to end of `run()`; manual review of each hit.
  - **Acceptance:** No reference to `sources` (the original vec) exists after
    the `std::mem::take` call except the moved-into `synthesis_sources`.

- [ ] **SIMP-002 — Audit balance_score semantics inversion consumers**
  - **File:** `crates/ragent-research/src/corpus_critic.rs` (~line 211–218)
  - **Issue:** `balance_score` semantics flipped: old 100 = one-sided,
    new 100 = balanced. The `overall_score` formula (line 276) uses
    `balance_score * 20` — verify this weight makes sense under the new
    semantics (high balance_score should INCREASE overall, which it now does).
  - **Action:**
    1. Read `CorpusCriticReport` struct and its serde serialization.
    2. Grep all consumers of `balance_score` across the workspace
       (`codeindex_references` for `balance_score`).
    3. Check TUI rendering (`research_progress.rs`), CLI output, and HTTP
       event JSON for any hardcoded interpretation of high = bad.
    4. If any consumer still assumes old semantics, fix it.
  - **Verify:** `codeindex_references(symbol: "balance_score")` across all
    crates.
  - **Acceptance:** Every consumer treats 100 = balanced, 0 = one-sided.
    Document the semantic change in CHANGELOG.md.

- [ ] **SIMP-003 — Verify all `SessionConfig` construction sites migrated**
  - **File:** `crates/ragent-research/src/session.rs`
  - **Issue:** `SessionConfig` was flattened into 7 nested sub-structs
    (`InputConfig`, `WebConfig`, `AnalysisConfig`, `EngineConfig`, etc.).
    Any external caller constructing `SessionConfig` directly will fail
    to compile.
  - **Action:**
    1. `codeindex_references(symbol: "SessionConfig")` to find all
       construction sites.
    2. Check: `crates/ragent-server/src/routes/research.rs`,
       `src/cli.rs`, `crates/ragent-tui/src/app/research.rs`,
       `crates/ragent-tui/src/app/slash.rs`, `examples/`.
    3. Confirm every site uses `build_session_config()` from `run_request.rs`
       or constructs with the nested form.
  - **Verify:** `cargo check` passes (compilation would catch this). If it
    compiles, all sites are migrated.
  - **Acceptance:** `cargo check --workspace` succeeds with zero errors.

- [ ] **SIMP-004 — Verify `ConfigSnapshot` JSON schema change consumers**
  - **File:** `crates/ragent-research/src/session.rs` + `cli.rs`
  - **Issue:** `ConfigSnapshot.from_file: Option<String>` changed to
    `from_files: Vec<String>`. The SSE/JSON `config_snapshot` event now emits
    `from_files` instead of `from_file`.
  - **Action:**
    1. Grep for `from_file` in SSE consumers:
       `crates/ragent-server/src/routes/research.rs`,
       `crates/ragent-tui/src/app/research.rs`,
       `crates/ragent-tui/src/research_progress.rs`.
    2. Check test fixtures that assert on event JSON.
    3. Update any consumer parsing `from_file` to parse `from_files`.
  - **Verify:** `grep -rn 'from_file' crates/ragent-server/ crates/ragent-tui/`
  - **Acceptance:** No consumer references the old `from_file` field name.
    CHANGELOG.md documents the schema change.

- [ ] **SIMP-005 — Verify `derive_title_full` callers updated**
  - **File:** `crates/ragent-research/src/item.rs` (~line 548)
  - **Issue:** Signature changed from `from_file: Option<&str>` to
    `from_files: &[String]`. Any external caller breaks.
  - **Action:** `codeindex_references(symbol: "derive_title_full")`.
    Update any caller still passing `Option<&str>`.
  - **Verify:** `cargo check --workspace` succeeds.
  - **Acceptance:** All callers use the new `&[String]` signature or call
    `derive_title_files` directly.

- [ ] **SIMP-006 — Verify `AnalysisConfig.contradiction` None fallback**
  - **File:** `crates/ragent-research/src/session.rs`
  - **Issue:** `AnalysisConfig.contradiction` is `Option<ContradictionConfig>`
    and defaults to `None`. When `None`, contradiction detection must fall
    back to `ContradictionConfig::default()` (the 6 medical/tech dimensions).
    If the session code doesn't apply this fallback, contradiction detection
    silently runs with zero dimensions (disabled).
  - **Action:**
    1. Find the contradiction-graph step in `run()` (~line 1167).
    2. Verify it does `config.analysis.contradiction.as_ref()
       .unwrap_or(&ContradictionConfig::default())` or similar.
    3. If it doesn't, add the fallback.
  - **Verify:** Read the contradiction invocation block in `session.rs`.
  - **Acceptance:** Contradiction detection runs with default dimensions when
    `contradiction` is `None`.

- [ ] **SIMP-007 — Verify `citation_re()` import in synthesis.rs**
  - **File:** `crates/ragent-research/src/synthesis.rs`
  - **Issue:** `citation_re()` is imported from `cite_checker`. Verify the
    import path is correct and the regex is shared, not duplicated.
  - **Action:** `grep -n 'citation_re' crates/ragent-research/src/` — confirm
    single definition in `cite_checker.rs` and import in `synthesis.rs`.
  - **Acceptance:** Single definition, correct import, no duplication.

---

## Milestone 2 — Missing Test Coverage (Priority: High)

### Tasks

- [ ] **SIMP-008 — Re-add deleted corpus_critic tests with new semantics**
  - **File:** `crates/ragent-research/tests/test_corpus_critic.rs`
  - **Issue:** 5 tests were deleted from `corpus_critic.rs` inline tests with
    no replacement:
    1. `empty_sources_produces_degenerate_report`
    2. `shallow_dimension_penalizes_evidence_score`
    3. `contradiction_lowers_tension_score`
    4. `balance_score_flags_monoculture` (asserted old semantics: score > 80
       for monoculture — now must be score < 20)
    5. `derive_gap_queries_includes_shallow_and_opposing`
  - **Action:**
    1. Read `crates/ragent-research/tests/test_corpus_critic.rs` (the external
       test file) to check if these were moved there.
    2. If any are missing, re-add them to `tests/test_corpus_critic.rs`,
       adjusted for the new balance_score semantics (100 = balanced,
       0 = one-sided). The monoculture test must assert `balance_score < 20`
       and the "dominated" issue is present.
    3. Add a companion test: balanced corpus → `balance_score >= 80` and no
       "dominated" issue.
  - **Verify:** `cargo test -p ragent-research --test test_corpus_critic`
  - **Acceptance:** All 5 test scenarios pass (adapted for new semantics) plus
    the new balanced-corpus test. 0 failures.

---

## Milestone 3 — Error Handling Hardening (Priority: Medium)

Remove `.expect()` / `.unwrap()` on user-facing paths and replace with proper
error propagation.

### Tasks

- [ ] **SIMP-009 — Replace `.expect()` in `TeamManager` after `Weak::upgrade()`**
  - **File:** `crates/ragent-agent/src/team/manager.rs` (~lines 775, 854, 888)
  - **Issue:** Three `.expect("SessionProcessor dropped while TeamManager
    still alive")` calls on the `spawn_teammate_internal` hot path. A
    panicked teammate spawn is user-visible. The `Weak::upgrade()` can
    legitimately fail if the processor is dropped before team cleanup
    (session teardown race).
  - **Action:** Replace each `.expect(...)` with
    `.ok_or_else(|| anyhow::anyhow!("SessionProcessor dropped"))` and
    propagate via `?`. `spawn_teammate_internal` already returns `Result<()>`.
  - **Verify:** `cargo test -p ragent-agent --lib`; `cargo clippy -p ragent-agent`
  - **Acceptance:** No `.expect()` in `team/manager.rs` on `Weak::upgrade()`
    results. `cargo clippy` passes clean.

- [ ] **SIMP-010 — Replace `.expect()` in `remove_session_state`**
  - **File:** `crates/ragent-agent/src/session/mod.rs` (~line 153)
  - **Issue:** `cache.lock().expect("session_state_cache poisoned")` panics
    if the mutex is poisoned (prior lock holder panicked).
  - **Action:** Replace with
    `cache.lock().unwrap_or_else(|e| e.into_inner())` to recover from
    poisoning (access inner data despite the panic), OR return early with
    a `tracing::warn!` if the lock fails.
  - **Verify:** `cargo test -p ragent-agent --lib`
  - **Acceptance:** No `.expect()` on the mutex lock in `remove_session_state`.

- [ ] **SIMP-011 — Replace `.expect()` in `checkpoint_wal()` with `lock_conn!` macro**
  - **File:** `crates/ragent-storage/src/storage.rs` (~line 378)
  - **Issue:** `self.conn.lock().expect("storage conn lock poisoned")` in a
    new public method. Other methods use the `lock_conn!` macro which handles
    poison gracefully.
  - **Action:** Replace with `let conn = lock_conn!(self)?;` matching the
    established pattern used at lines 388 and 418.
  - **Verify:** `cargo test -p ragent-storage --lib`; `cargo clippy -p ragent-storage`
  - **Acceptance:** `checkpoint_wal()` uses `lock_conn!` macro. No raw
    `.expect()` on mutex locks in new code.

---

## Milestone 4 — Concurrency Race Fix (Priority: Medium)

### Tasks

- [ ] **SIMP-012 — Fix `cancel()` double-lock in `bg.rs`**
  - **File:** `crates/ragent-tools-core/src/bg.rs` (~lines 220–260)
  - **Issue:** `cancel()` locks `inner`, drops the lock, then immediately
    re-locks to clone `done_notify`. Between the two lock acquisitions, the
    `waiter_task` could complete and call `done_notify.notify_waiters()`. Since
    `Notify::notify_waiters()` only wakes currently-waiting receivers, if
    `cancel()` hasn't called `notified()` yet, it misses the signal. Mitigated
    by a 10-second deadline fallback, but worst case is a 10s delay.
  - **Action:** Clone `done_notify` in the FIRST lock scope alongside setting
    `cancelled` and calling `start_kill`:
    ```rust
    let done_notify = {
        let mut inner = self.inner.lock().expect("...");
        inner.cancelled = true;
        if let Some(child) = inner.child.as_mut() {
            let _ = child.start_kill();
        }
        inner.cancel_notify.notify_one();
        Arc::clone(&inner.done_notify)
    };
    // Now use done_notify without re-locking
    ```
    Apply the same fix to `wait()` (~lines 267–272) if it has the same pattern.
  - **Verify:** `cargo test -p ragent-tools-core --lib`; manual review of the
    lock scopes.
  - **Acceptance:** `cancel()` acquires the `inner` lock at most once.
    `done_notify` is cloned within the first lock scope. No second lock
    acquisition.

---

## Milestone 5 — Cache & Eviction Strategy (Priority: Low-Medium)

### Tasks

- [ ] **SIMP-013 — Add LRU eviction to LLM client cache**
  - **File:** `crates/ragent-agent/src/session/loop_steps.rs` (~lines 296–301)
  - **Issue:** `MAX_LLM_CLIENTS = 8` cache evicts an arbitrary `HashMap` key
    (effectively random). Can evict the most recently used client, causing
    thrashing when cycling between 9+ models.
  - **Action:** Replace `HashMap` with `IndexMap` and evict the first entry
    (oldest insertion order ≈ FIFO), OR add a `last_accessed: Instant` to the
    cache value and evict the oldest. For 8 entries this is trivially cheap.
  - **Verify:** `cargo test -p ragent-agent --lib`
  - **Acceptance:** Eviction removes the least-recently-used entry, not a
    random one. Comment updated to reflect LRU/FIFO strategy.

- [ ] **SIMP-014 — Fix `read_timestamps` eviction strategy and comment**
  - **File:** `crates/ragent-tools-core/src/read.rs` (~lines 323–334)
  - **Issue:** Eviction takes "front" keys from a `HashMap`, but HashMap
    iteration order is arbitrary — this evicts a random quarter, including
    recently-read files. The comment says "oldest quarter" which is wrong.
    Also, `record_edit_timestamp` in `edit_common.rs` does NOT apply the same
    cap — edits can grow the map beyond 2000.
  - **Action:**
    1. Either use an `LruCache` (the `lru` crate is already a dependency) for
       `read_timestamps`, getting true LRU eviction.
    2. OR at minimum, fix the comment to say "arbitrary quarter (HashMap
       iteration order is not insertion-ordered)".
    3. Apply the same cap logic in `edit_common.rs:record_edit_timestamp`.
  - **Verify:** `cargo test -p ragent-tools-core --lib`
  - **Acceptance:** Eviction is LRU or the comment accurately describes the
    strategy. Both read and edit timestamp recording share the same cap.

- [ ] **SIMP-015 — Add retry backoff to `tasks_cache_dirty` on SQLite error**
  - **File:** `crates/ragent-tui/src/layout.rs` (~lines 3288–3293)
  - **Issue:** On SQLite error, `tasks_cache_dirty = true` causes retry on
    every frame redraw (30–60 FPS). A persistent error (corrupted DB, locked
    WAL) floods `tracing::warn!` and wastes CPU.
  - **Action:** Add a `tasks_cache_retry_count: u32` field to `App`. On error,
    increment it and only set `dirty = true` if `retry_count < 3`. Reset to 0
    on success. Alternatively, use time-based debounce: only retry if the last
    attempt was > 5 seconds ago.
  - **Verify:** `cargo test -p ragent-tui`
  - **Acceptance:** A persistent SQLite error does not cause per-frame retry.
    Retry is capped or debounced.

---

## Milestone 6 — SSE / HTTP API Improvements (Priority: Low)

### Tasks

- [ ] **SIMP-016 — Send diagnostic event on broadcast overflow in SSE stream**
  - **File:** `crates/ragent-server/src/routes/research.rs` (~lines 531–544)
  - **Issue:** `BroadcastStream::Err` (buffer overflow) sends an empty SSE
    event (`Event::default()`) — the client has no idea events were missed.
  - **Action:** On `Err(_)`, send a diagnostic event:
    ```rust
    Err(_) => Ok(axum::response::sse::Event::default()
        .event("research")
        .data(r#"{"type":"lag_warning","message":"Some events were missed due to buffer overflow"}"#)),
    ```
  - **Verify:** `cargo test -p ragent-server --test test_research_routes`
  - **Acceptance:** Broadcast overflow produces a visible `lag_warning` event,
    not an empty SSE frame.

- [ ] **SIMP-017 — Send explicit terminal SSE event on research completion**
  - **File:** `crates/ragent-server/src/routes/research.rs` (~lines 326–549)
  - **Issue:** When the background research run completes, the spawned task
    drops the `tx` sender and the stream ends. The SSE client sees the
    connection close with no explicit "done" event.
  - **Action:** In the spawned task, after `session.run()` completes, send a
    final terminal `SessionEvent` (or a synthetic completion event) through
    `tx` before removing it from the registry:
    ```rust
    let _ = tx.send(SessionEvent::Done { /* ... */ });
    runs.remove(&name_clone);
    ```
  - **Verify:** `cargo test -p ragent-server --test test_research_routes`
  - **Acceptance:** SSE stream emits an explicit terminal event before the
    connection closes.

- [ ] **SIMP-018 — Reduce `to_run_request()` field-sync boilerplate**
  - **File:** `crates/ragent-server/src/routes/research.rs` (~lines 212–242)
  - **Issue:** `CreateResearchRequest::to_run_request()` manually clones 26
    fields. If either struct gains a field, this method must be updated or the
    field is silently dropped. No compile-time sync guarantee.
  - **Action:** Consider deriving `Into<ResearchRunRequest>` for
    `CreateResearchRequest` via a shared type, or deserialize the HTTP body
    directly into `ResearchRunRequest` with `#[serde(default)]` attributes.
    If the wrapper is kept for HTTP-specific serde attrs, add a test that
    asserts field count parity.
  - **Verify:** `cargo test -p ragent-server`
  - **Acceptance:** Either the manual clone is eliminated, or a compile-time
    / test-time guarantee ensures the two structs stay in sync.

---

## Milestone 7 — Code Deduplication (Priority: Low)

### Tasks

- [ ] **SIMP-019 — Extract shared INSERT helper in `activity_log.rs`**
  - **File:** `crates/ragent-storage/src/activity_log.rs` (~lines 351, 406,
    1017, 1082, 1132, 1398, 1472)
  - **Issue:** The exact same `INSERT INTO activity_events (...)` SQL string
    with the same `params![]` binding pattern is duplicated 7 times across
    `append`, `append_new`, `branch_from_checkpoint`, `expire_run`,
    `archive_run`, `resume_run`, and `append_mutation_rejected_locked`.
  - **Action:** Extract a private helper:
    ```rust
    fn insert_event_locked(conn: &Connection, event: &ActivityEvent) -> Result<()> {
        let kind_json = serde_json::to_string(&event.kind)
            .context("Failed to serialise event kind")?;
        conn.execute(
            "INSERT INTO activity_events
                (run_id, seq, id, schema_version, timestamp, kind)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                event.run_id.as_str(),
                event.seq as i64,
                event.id.as_str(),
                event.schema_version as i64,
                event.timestamp.to_rfc3339(),
                kind_json,
            ],
        )?;
        Ok(())
    }
    ```
    Each of the 7 call sites becomes a one-liner.
  - **Verify:** `cargo test -p ragent-storage`; all activity_log tests pass.
  - **Acceptance:** Single INSERT helper. 7 call sites reduced to one-liners.
    All existing tests pass.

- [ ] **SIMP-020 — Extract shared lifecycle-event helper for expire/archive**
  - **File:** `crates/ragent-storage/src/activity_log.rs` (~lines 1067–1103,
    1118–1155)
  - **Issue:** `archive_run` is almost identical to `expire_run` — both insert
    a `Lifecycle { event: "expired: {reason}" }` event, then
    `DELETE FROM activity_events WHERE run_id = ?1`. The lifecycle-insert block
    is copy-pasted verbatim.
  - **Action:** Extract a private
    `fn append_lifecycle_locked(&self, conn, run_id, event_str) -> Result<()>`
    and call it from both methods. Or have `archive_run` call `expire_run`
    internally after exporting.
  - **Verify:** `cargo test -p ragent-storage`
  - **Acceptance:** Lifecycle-insert block appears once. Both `expire_run` and
    `archive_run` call the shared helper.

---

## Milestone 8 — Minor Polish (Priority: Low)

### Tasks

- [ ] **SIMP-021 — Expand `is_stopword_lc` list**
  - **File:** `crates/ragent-research/src/verify.rs`
  - **Issue:** The stopword list has 26 entries but misses common function
    words: "with", "into", "upon", "than", "also", "from", "have", "been",
    "their", "there", "where", "which", "while".
  - **Action:** Add the missing common stopwords to the list. Keep it
    conservative — only add words that are clearly function words (not
    domain terms that could affect keyword relevance scoring).
  - **Verify:** `cargo test -p ragent-research --lib`
  - **Acceptance:** Stopword list includes common English function words.
    Existing verifier tests still pass.

- [ ] **SIMP-022 — Clarify `wal_autocheckpoint` comment with page unit**
  - **File:** `crates/ragent-storage/src/storage.rs` (~lines 362–364)
  - **Issue:** Comment says "500 is more aggressive" but doesn't mention
    that `wal_autocheckpoint` is in pages (500 pages × 4 KB = ~2 MB), not
    bytes.
  - **Action:** Update comment to: `// The default is 1000 pages; 500 is more
    aggressive (500 pages ≈ 2 MB checkpoint threshold) for a desktop agent
    that may run for hours.`
  - **Verify:** Visual review.
  - **Acceptance:** Comment clarifies the unit is pages and the approximate
    byte equivalent.

- [ ] **SIMP-023 — Simplify `show_research` repeated `if q.full` blocks**
  - **File:** `crates/ragent-server/src/routes/research.rs` (~lines 392–407)
  - **Issue:** `if q.full { ... } else { None/Vec::new() }` is repeated 4 times
    in a struct literal for `topic`, `queries`, `output_format`, `model`.
  - **Action:** Build the optional fields conditionally before the struct
    literal:
    ```rust
    let (topic, queries, output_format, model) = if q.full {
        (Some(item.topic.clone()), item.queries.clone(),
         item.output_format.clone(), item.model.clone())
    } else {
        (None, Vec::new(), None, None)
    };
    ```
  - **Verify:** `cargo test -p ragent-server --test test_research_routes`
  - **Acceptance:** `q.full` checked once, not 4 times. Test output unchanged.

- [ ] **SIMP-024 — Verify `config_path: None` doesn't drop needed info**
  - **File:** `crates/ragent-agent/src/session/mod.rs` (~lines 224–227)
  - **Issue:** Old code derived `config_path` from `Config::load()` +
    `RAGENT_CONFIG` env var. New code hardcodes `None`. If any downstream
    consumer of `Session.config_path` depends on it being populated, info
    is silently dropped.
  - **Action:**
    1. `codeindex_references(symbol: "config_path")` to find all consumers.
    2. If no consumer reads `config_path` (display, reload, logging), the
       `None` is safe — consider removing the field entirely.
    3. If a consumer exists, restore the derivation.
  - **Acceptance:** Either `config_path` is populated correctly, or the field
    is removed if unused.

- [ ] **SIMP-025 — Remove redundant `notice_handle.0.abort()` or document intent**
  - **File:** `crates/ragent-agent/src/session/loop_steps.rs` (~line 1074)
  - **Issue:** `AbortOnDrop` guard already aborts on scope exit. The explicit
    `notice_handle.0.abort()` is technically redundant. The comment says it's
    for promptness (cancel before stream processing starts).
  - **Action:** Either remove the redundant call (trust the guard) or add a
    one-line comment: `// Explicit abort for promptness; the AbortOnDrop
    // guard is a safety net for scope exit.` (If a comment already exists,
    verify it's sufficient.)
  - **Acceptance:** The intent (promptness vs safety net) is documented or
    the redundancy is removed.

---

## Milestone Summary

| Milestone | Priority | Tasks | Theme |
|-----------|----------|-------|-------|
| M1 | High | SIMP-001 – SIMP-007 | Correctness verification of refactors |
| M2 | High | SIMP-008 | Missing test coverage (corpus_critic) |
| M3 | Medium | SIMP-009 – SIMP-011 | Error handling (.expect removal) |
| M4 | Medium | SIMP-012 | Concurrency race (bg.rs double-lock) |
| M5 | Low-Med | SIMP-013 – SIMP-015 | Cache/eviction strategy |
| M6 | Low | SIMP-016 – SIMP-018 | SSE/HTTP API improvements |
| M7 | Low | SIMP-019 – SIMP-020 | Code deduplication (activity_log) |
| M8 | Low | SIMP-021 – SIMP-025 | Minor polish |

**Total: 25 tasks across 8 milestones.**

---

## Execution Order

1. **M1 (SIMP-001–007)** — Investigation-first. These are mostly "verify and
   fix if broken" tasks. Run them in parallel where possible. SIMP-003 and
   SIMP-005 are likely already satisfied (the code compiles). SIMP-001,
   SIMP-002, SIMP-004, SIMP-006 need manual review.

2. **M2 (SIMP-008)** — Depends on M1/SIMP-002 confirming the new balance
   semantics are intentional. Re-add tests with corrected assertions.

3. **M3 (SIMP-009–011)** — Independent. Can run in parallel. All are
   straightforward `.expect()` → `?` / macro replacements.

4. **M4 (SIMP-012)** — Independent. Single-file fix in bg.rs.

5. **M5 (SIMP-013–015)** — Independent of each other. SIMP-013 and SIMP-014
   are cache-strategy changes; SIMP-015 is TUI-specific.

6. **M6 (SIMP-016–018)** — Independent. All in `ragent-server`. SIMP-018
   is optional (architectural — could skip if the sync risk is acceptable).

7. **M7 (SIMP-019–020)** — SIMP-019 first (INSERT helper), then SIMP-020
   (lifecycle helper, may depend on SIMP-019's helper existing).

8. **M8 (SIMP-021–025)** — All independent, lowest priority. Do last.

---

## Appendix A — Applied Fixes

The following issues were already fixed during the initial `/simplify` pass
and are NOT part of this plan:

| ID | File | Fix Applied |
|----|------|-------------|
| H-2 | `web_gatherer.rs` | `body_preview_of` O(n²) → incremental char count |
| M-2 | `document.rs` | `render_finding_sources` case-insensitivity restored (`to_lowercase`) |
| — | `copilot.rs` | Removed dead `find_gh_cli_token` + `GH_CLI_TOKEN_CACHE` (~85 lines) |
| — | `bg.rs` | `debug!()` → `trace!()` on per-line hot path + added `trace` import |
| — | `event_handler.rs` | Removed dead `"task_delete"` from match arm |
| — | `task/mod.rs` | Removed uncalled `reap_reported()` method |
| — | `activity_log.rs` | Two `unreachable!()` → `anyhow::bail!()` |
| — | `session.rs` | Reformatted mangled comment (merged lines) |

All applied fixes verified with `cargo fmt`, `cargo check`, `cargo clippy`,
and `cargo test` (606 research tests, 59 tools-core tests — all passing,
zero warnings).