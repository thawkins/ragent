# Deep-Refactor Audit

**Audited at**: `3de6683` (Version 1.0.162), 2026-11-06
**Scope**: whole workspace: 19 crates under `crates/` plus the root `ragent` binary.
Five read-only passes over disjoint areas, plus a baseline `cargo clippy --workspace --all-targets`.
**Supersedes**: the open duplication/standards rows of `docs/reports/code-audit.md`
(re-verified below; most are already fixed).

Every count below carries the command that produced it and the commit it was
taken at. Re-derive a count on the branch point of the car that uses it; never
quote the snapshot.

---

## 1. Architecture verdict

**The architecture is sound. The debt is defects at input boundaries, copy-paste
between sibling modules, and god-functions.**

Evidence: the crate graph is a strict layering with no cycles.

```
ragent-types  <-  ragent-config  <-  ragent-storage / ragent-surface / ragent-codeindex
              <-  ragent-llm, ragent-tools-{core,extended,vcs}, ragent-plugins, ragent-connectors
              <-  ragent-research, ragent-specs  <-  ragent-agent  <-  ragent-server, ragent-tui  <-  ragent (bin)
```

(`grep -oE '^ragent-[a-z-]+' crates/*/Cargo.toml` at `3de6683`.)

Two edges look odd but are deliberate:

- `ragent-tui -> ragent-bench`: drives the `/bench` slash command (`crates/ragent-tui/src/app/slash.rs`, `"bench" =>`).
- `ragent-llm -> ragent-storage`: credential storage.

No structural ticket is warranted. Everything below is local.

---

## 2. Defects (report first)

| ID | Defect | Where (at `3de6683`) | Status |
|----|--------|----------------------|--------|
| D-01 | **P0 sandbox escape.** For a path that does not exist yet, the walk dropped every `..`. `ghost/../../x` was checked as `ghost/x`, and `Write` then created the file outside the working directory. Reproduced. | `crates/ragent-tools-core/src/lib.rs` `check_path_within_any_root` and `_cached` | **Done** |
| D-02 | **Data loss.** `edit_global_config` / `set_global_bool_key` treated *any* read error (invalid UTF-8, EACCES) as an empty file. One YOLO toggle replaced the user's whole `ragent.json` with one key, contradicting the doc on `runtime_flag.rs` `persist_to_global`. | `crates/ragent-config/src/config.rs` | **Done** |
| D-03 | A cancelled sub-agent was recorded as `Completed` / `SubagentComplete{success:true}`. The cancel path returns `Ok`, and both spawn paths treated every `Ok` as success. The sync path also hard-coded `finish_reason: "stop"`. | `crates/ragent-agent/src/task/mod.rs` (both `Ok` arms) | **Done** |
| D-04 | Three copies of the cancelled-turn ending. Two used `?` on `update_message`, so a storage error skipped `MessageEnd` and left the UI with an open message. | `crates/ragent-agent/src/session/processor.rs` `process_user_message` (cancel, interrupt, watchdog) | **Done** |
| D-05 | `is_cancel_error` doc claimed a typed check. `type_name_of_val` on `&dyn Error` never contains "Cancelled", so only the string match ever ran. | `crates/ragent-agent/src/task/mod.rs` | **Done** |
| D-06 | `/swarm` panic: the log line byte-sliced user text at 80. | `crates/ragent-tui/src/app/slash.rs` (`Swarm: decomposing`) | **Done** |
| D-07 | `split_authored_sections` used offsets from `body.to_lowercase()` to slice `body`, which panics on e.g. U+1E9E. It also matched `PLAN.md` inside `TESTPLAN.md`. Reachable from model output. | `crates/ragent-tools-extended/src/archdoc/author_split.rs` | **Done** |
| D-08 | Router `<alias> mode` modifier had no word boundary: "Deep models are hard" was forced to the Reasoning tier. | `crates/ragent-llm/src/providers/router_modifiers.rs` | **Done** |
| D-09 | `Retry-After: 0` had no floor, so one 429 used up every retry instantly. The shared `guard::clamp_retry_after` had zero production callers. | `crates/ragent-llm/src/providers/http_client.rs` `parse_retry_after` | **Done** |
| D-10 | `truncate_at_char_boundary` kept `max_chars - 1` characters. | `crates/ragent-research/src/session/topic.rs` | **Done** |
| D-11 | Telemetry redaction flags `qwen3:latest` / `llama3:latest` model names as `key:secret`. | `crates/ragent-telemetry/src/sensitive.rs` | Blocked: owner decision. Any narrowing of the rule can let a real `user:secret` through; over-redaction is the safe failure. |
| D-12 | Meta-refresh detection reads the *first* `<meta>` tag, not the `http-equiv="refresh"` one. | `crates/ragent-tools-extended/src/masterfetch/envelope.rs` `find_meta_refresh_content` | **Done** |
| D-13 | Web UI served from `env!("CARGO_MANIFEST_DIR")/src/static` at runtime, so any installed binary returns 404. The comment calls it "embedded". | `crates/ragent-server/src/routes/mod.rs` | **Done** |
| D-14 | `team_task_create`: `next_task_id()` reads the list without the lock, so two concurrent creates collide and the loser gets "use a different id". `let _ = remove_task(..)` swallows the rollback. | `crates/ragent-agent/src/tool/team_task_create.rs` | **Done** |
| D-15 | Router doc says the modifier is stripped before forwarding (FR-020). It is not: `remaining_prompt` feeds only the classifier. | `router_modifiers.rs` module doc vs `router_client.rs` `route` | Blocked: owner decision (implement FR-020 stripping, or correct the doc) |
| D-16 | `build_feedback_log` checks length in bytes but cuts by chars, so a 30-char CJK note gets a false `...`. | `crates/ragent-specs/src/commands.rs` | **Done** |
| D-17 | Router-prompt truncation checks `chars().count() > 80` but cuts at byte 80. | `crates/ragent-tui/src/app/event_handler.rs` (second `truncate` loop) | **Done** |
| D-18 | `extract_snippet` lowercases the whole remaining body at every char index: O(n^2) on 256 KiB bodies. | `crates/ragent-research/src/locus.rs` | Approved |

---

## 3. Infra / gates

| ID | Finding | Evidence | Status |
|----|---------|----------|--------|
| I-01 | **Done** for the CI gate (`cargo clippy --workspace -D warnings ...` from `ci.yml` now exits 0): 16 `Duration` sites and one unknown lint. Remaining: `--all-targets` test-code warnings. Baseline clippy on the current stable toolchain is not clean. 143 warning sites in total: 72 `redundant_pub_crate`, 37 `Duration` unit, 22 `float_cmp`, 3 unknown lint `clippy::assert_is_empty`, others. | `cargo clippy --workspace --all-targets` at `3de6683` (log: `target/temp/clippy_base.log`, `grep -E '^warning: ' \| grep -v generated \| sort \| uniq -c`) | Approved |
| I-02 | The comment in `ci.yml` says `rust-toolchain.toml` "pins a dated nightly". The file says `channel = "stable"`. | `.github/workflows/ci.yml` clippy and fmt jobs | **Done** |
| I-03 | `tower` is a normal dependency of `ragent-server` but only `tests/` use it. | `grep -rn '\btower::' crates/ragent-server/src` returns no results | **Done** (the `tower-http` `fs` feature stays: pinned by `test_web_ui.rs` `test_serve_dir_feature_enabled`, see B-06) |
| I-04 | `deny.toml` has a stale `ring` license clarify (0.16 text; the lockfile has 0.17), and a comment names `webpki-roots`, which is not in the lockfile. | `deny.toml` | Approved |

---

## 4. Safe backlog (behavior-preserving, frozen tests)

| ID | Candidate | Canonical owner after | Pins | Status |
|----|-----------|-----------------------|------|--------|
| S-01 | Idle-timeout / "stream stalled" block copied into every provider. The texts differ only in their label. | `http_client::next_chunk_or_stall(label, ..)` | Message text `"{Label}: stream stalled - no data received for {N}s"` is asserted by `ragent-agent/tests/test_session_processor.rs` and `ragent-telemetry/tests/test_logs_export.rs`; 120 s by `test_m5_6_provider_consistency.rs`. | Approved |
| S-02 | `EndpointSource` enum and its `tag()` are byte-identical in plugins and connectors. | `ragent-surface` with re-exports | `ragent_plugins::EndpointSource` is used in `test_store_index.rs` | Approved |
| S-03 | `read_capped` chunked-ceiling loop exists 3x as a fn and 2x inline. | `ragent-surface` public fn with crate-local error mapping | `ragent_plugins::read_capped` and `ragent_connectors::read_capped` with their error variants | Approved |
| S-04 | Hand-rolled truncation that matches a `ragent_types::strutil` helper exactly: <br>- `research/supervisor.rs` (2 sites, `truncate_bytes`)<br>- `codeindex/search.rs` (`truncate_bytes_no_ellipsis`)<br>- `tui/layout_teams.rs` `clip` (`truncate_chars_no_ellipsis`)<br>- `tui/event_handler.rs` hook warning (`truncate_bytes`)<br>- `research/cluster.rs` vs `document.rs` (identical bodies) | `strutil` | Hook-warning format asserted in `test_event_swarm_research_deep.rs`; it matches `truncate_bytes` | Approved |
| S-05 | `#[test]` fns declared in production source after the inline-test hook: <br>- `research/cli.rs` (7)<br>- `specs/plan_parser.rs` (4)<br>- `tui/research_progress.rs` (7) | the existing `tests/inline/*_tests.rs` files | `scripts/security-unwrap-baseline.txt` counts drop (shrink-only) | Approved |
| S-06 | **Done** except the five specs/codeindex fns named in their crate `DOCS.md` (`hash_file`, `create_spec`, `search_specs_with_archived`, `resolve_requirements`, `task_id_at`). Dead code with zero references workspace-wide: <br>- agent: `file_ops/wrapper.rs`, `cancel_blocking`, `pre_assign_task_blocking`, `capture_is_rollbackable`, `pending_verdicts_for_test`, `status_history::to_status`, `with_request_timeout`<br>- research: 7 fns<br>- codeindex: 3 fns<br>- specs: 4 fns<br>- tui: `toolchain::probe_version` | (delete) | None in tests; `docs/howtos/toolchain.md` names `probe_version` | Done (partial) |
| S-07 | `HumanFallback` trait with a single impl, plus an injector (`with_fallback`) with zero callers. | inline `LoggingFallback` | none | Rejected: documented human-review extension point; deleting it saves ~20 lines and removes the seam the module doc advertises |
| S-08 | Lying comments: <br>- `bang_command.rs` says chars but counts bytes<br>- `compaction/runner.rs` `cap_chars` is compared against bytes<br>- `take_sse_line` doc says "no allocation"<br>- `cardinality.rs` Clone comment<br>- `connectors/store.rs` `save` doc<br>- `fallback.rs` allow reason<br>- `widgets/mod.rs` / `message_widget.rs` `[red]` markers | the code | `"characters omitted"` is asserted in `test_bang_command.rs`, so keep that wording | Approved |
| S-09 | Monster functions: `process_user_message` (~2.3k lines), `call_llm_step` (~840), `build_system_prompt_with_storage_inner` (~710), `team_create::execute` (~550). | phase helpers in the same module | none on source text | Approved (incremental; D-04 already extracted one phase) |

Line counts for S-09 come from a brace-matching scan at `3de6683`. Re-derive them with
`awk '/fn process_user_message/,/^    }$/' crates/ragent-agent/src/session/processor.rs | wc -l`.

---

## 5. Blocked by pins (need a test-edit-sanctioned ticket)

| ID | Candidate | Pin |
|----|-----------|-----|
| B-01 | `MessageWidget` struct/impl (~470 lines) is dead in the app. The live renderer is `layout.rs` `messages_to_lines`, and the two have drifted (Thinking placeholder, `[red]` parsing, indent). | 19 test refs in 4 TUI test files |
| B-02 | `utils::truncate_with_ellipsis` has no production caller (and uses the U+2026 ellipsis). | `test_shorten_middle.rs`, `test_misc_modules_deep.rs` |
| B-03 | `guard::cap_read`, `http_client::append_stream_chunk_capped`, `StoreCatalog`, and `check_path_within_root(_cached)` have test-only callers. | their tests and doctests |
| B-04 | `source_registry` module: a trait with one impl and no production caller. | `tests/inline/source_registry_tests.rs` |
| B-05 | 709 exact-status-string assertions in the TUI tests (carried over from `code-audit.md`). | the tests themselves |
| B-06 | `tower-http`'s `fs` feature is unused by production code since D-13. | `crates/ragent-server/tests/test_web_ui.rs` `test_serve_dir_feature_enabled` |

---

## 6. Deliberate: do not "fix"

| Item | Why |
|------|-----|
| `compaction/serializer.rs` `truncate` | It appends `"\n[truncated]"` and keeps `max` chars; that is different semantics from `strutil`. |
| `tui/widgets/message_widget.rs` `truncate_str` | It already delegates to `strutil`; the suffix past `max` is documented. |
| `research/comparison.rs` `truncate_cell` | It adds a `trim_end()` that `strutil` does not have. |
| `ragent-tui -> ragent-bench` dependency | Powers `/bench`. |
| `#[allow(dead_code)]` in research `fallback.rs` / `title.rs` / `relevance.rs` | Those files are `#[path]`-included into test binaries. |

## 7. Prior `code-audit.md` rows re-verified (now Superseded)

- **TUI `cursor_line` / `band_line` copies**: now `layout_table_rows.rs`, used by both files.
- **Marketplace helpers in plugins/connectors**: now `ragent_surface::marketplace_ledger`.
- **SSE buffer guard (append + cap + "SSE buffer exceeded")**: now `http_client::append_chunk_guarded`. Only the stall block (S-01) remains.
- **Seven `ragent-server` `test_state()` copies**: now `tests/support/mod.rs`. One copy remains in `test_openai_surface.rs`.
- **`processor.rs` double logging (fatal loop error, cancel timing)**: both fixed.
- **`task/mod.rs` `truncate_str`**: delegates to `strutil`.
- **`research/cli.rs` truncate**: delegates to `strutil`.

---

## 8. Cleanup train ledger

| ID | Preserved behavior | Validation | Status |
|----|--------------------|-----------|--------|
| D-01 | Paths under the root (and alias/bind-mount roots) accepted; `..` escapes rejected | `cargo nextest run -p ragent-tools-core -E 'test(test_path_traversal::) + test(test_lib_registry::)'` | Done |
| D-02 | Missing/blank file -> `{}`; invalid JSON is an error; atomic write | `cargo nextest run -p ragent-config -E 'test(test_runtime_flag_toggles::) + test(test_config_save::)'` | Done |
| D-03..05 | Cancel path returns `Ok`; suspend flags cleaned; watchdog still `Err` | `cargo nextest run -p ragent-agent` | Done |
| D-06 | Log text `Swarm: decomposing - <first 80 bytes>` | `cargo check -p ragent-tui` (log-only, no seam) | Done |
| D-07 | Marker identity decides the slot; case-insensitive | `-E 'test(test_archdoc_author_split::)'` | Done |
| D-08 | `X mode: / X mode, / X mode ...` still detected | `-p ragent-llm -E 'test(router_modifiers)'` | Done |
| D-09 | Cap 30 s; `http_client::MAX_RETRY_AFTER` name kept | `-E 'test(test_rate_limit_retry::) + test(test_m5_6_provider_consistency::)'` | Done |
| D-10 | Char-based cut, no suffix | `-E 'test(test_session_topic)'` | Done |
| D-12..14, D-16, D-17 | see section 2 | `cargo nextest run --workspace` (15282 passed at `1fe6b91`) | Done |
| D-11, D-15 | see section 2 | - | Blocked: owner decision |
| D-18, I-01 (all-targets), I-04, S-01..05, S-08, S-09 | see sections 3-4 | per row | Approved |
| I-01 (CI gate), I-02, I-03, S-06 (partial) | see sections 3-4 | the `ci.yml` clippy command; `cargo check --workspace --all-targets` | Done |
| S-07 | see section 4 | - | Rejected |
| B-01..05 | see section 5 | - | Blocked: needs a test-edit-sanctioned ticket |

### Prevention decisions

- **D-01**: Guard added (a regression test for `..` through a missing directory, in both the check and the `Write` tool).
- **D-02**: Guard added (an unreadable-file test asserting the bytes are untouched).
- **D-03**: Guard added (`cancelled_sync_task_is_recorded_as_cancelled`).
- **D-06**: No guard justified (log-only `format!`; a source grep would be brittle).
- **Remaining defects**: each car adds a locking test for its failing path.

## 9. Host-dependent test failures found during the drain

The first full run in a cloud container (root, `GITHUB_TOKEN` and `AWS_*` in
the environment, a long checkout path and branch name, docker but no podman)
failed 18 tests that were unrelated to the code under test. They were
pre-existing. Each was fixed by detecting the host, never by weakening an
assertion. The test files edited, all for isolation:

- **Permission-denied tests** (8 TUI tests plus `ragent-agent` `test_rollback_capture`): skip when `crate::permission_bits_enforced()` (`crates/ragent-tui/tests/all.rs`) or an inline probe shows the user writes through `0o500`.
- **Provider detection inherited from the host** (6 TUI tests):
  - `slash_support::enter_project` now clears every provider credential variable.
  - `crate::without_host_providers(&mut app)` disables host-detected providers in the test's own storage.
- **Status-bar tests** (2): pin `app.cwd` and `app.git_branch`.
- **`test_container_backend` missing-image test**: used `podman` while skipping only when no runtime at all was present; it now uses the detected runtime.

## 10. Lessons for the next audit

- `scripts/check_shared_guards.py` only flags `fn truncate_*` *definitions*. Inline step-back loops (`event_handler.rs`, `supervisor.rs`, `search.rs`) slip past it.
- `scripts/check_inline_tests.py` only catches inline `mod` bodies. Free `#[test]` fns placed after a `#[cfg(test)] #[path]` hook (S-05) slip past it.
- Tests that are `#[path]`-included can pin a *bug* within tolerance. `test_session_topic_extra.rs` asserted `>= MAX-1`, which is exactly the D-10 off-by-one.
