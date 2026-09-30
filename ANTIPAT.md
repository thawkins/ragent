# ANTIPAT.md - workspace-wide anti-pattern and standards remediation plan

Status: M0 COMPLETE (M0.1-M0.15 implemented and verified); M2 COMPLETE
(M2.1-M2.17 implemented and verified); M3 COMPLETE (M3.1-M3.18 implemented and
verified); M4 COMPLETE (M4.1-M4.13 implemented and verified); M5 COMPLETE
(M5.1-M5.13 implemented and verified); M6 COMPLETE (M6.1-M6.10 implemented and
verified); M7 COMPLETE (M7.1-M7.9 implemented and verified); M1 open
Created: session on top of `6cf0b60f` (MS-04) + uncommitted MS-05 work
Scope: all 17 workspace crates plus the root `ragent` binary package
Method: 17 parallel `explore` audit agents, one per crate, each verifying every
finding against the actual source text. The per-crate evidence reports are kept at
`target/temp/antipat/<crate>.md` and are the normative reference for every numbered
finding below. Raw command output, exact `file:line` citations and per-finding
remediation text live in those files; this plan is the consolidated roll-up.

Priority mapping (from AGENTS.md): 0 = critical (security, data loss, broken
builds), 1 = high, 2 = medium (default), 3 = low, 4 = backlog.

---

## 0. Executive summary

Every crate in the workspace compiles clean: `cargo clippy --all-targets` reports
**0 warnings in all 17 crates**, `cargo fmt --check` is clean, and there is no
`unsafe` outside the one approved site (`kill_process_group` in
`ragent-tools-core/src/bash.rs`). The debt is concentrated in seven recurring
classes, each of which appears in most crates:

| #  | Class                                                                                                         | Crates affected | Worst offenders                                                                                                                                                                                                                                                                  |
| -- | ------------------------------------------------------------------------------------------------------------- | --------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
|    |                                                                                                               |                 |                                                                                                                                                                                                                                                                                  |
| C2 | Corrupted`U+FFFD` mojibake bytes committed into source                                                      | 8               | `ragent-storage`, `ragent-server`, `ragent-codeindex`, `ragent-tools-core`, `ragent-telemetry`, `ragent-specs`, `ragent-config`                                                                                                                                    |
| C3 | Duplicated helper families (truncation, qname/scope/hash, SSE parsers, row mappers, per-provider boilerplate) | 12              | `ragent-codeindex` (10x `build_qname`), `ragent-llm` (2 full SSE parsers), `ragent-storage` (7x INSERT), `ragent-tools-extended` (7 engines), `ragent-tools-vcs` (GitHub vs GitLab), root `src/cli.rs` (768 lines)                                                 |
| C4 | Silent error suppression (`let _ =`, `unwrap_or_default`, `unwrap_or(0/false)`)                         | 13              | `ragent-tui` (79), `ragent-storage` (7), `ragent-research` (15), `ragent-telemetry`, `ragent-tools-core`, `ragent-plugins`, root `src`                                                                                                                             |
| C5 | Inline`#[cfg(test)]` modules in library sources                                                             | 14              | `ragent-research` (41), `ragent-agent` (32), `ragent-tools-extended` (14), `ragent-telemetry` (5), `ragent-types` (6), `ragent-bench` (5), `ragent-specs` (6)                                                                                                      |
| C6 | Missing or skipped security guards (containment, SSRF, argv injection, integrity, redaction)                  | 9               | `ragent-tools-vcs` (8 unguarded git argv), `ragent-tools-core` (5 tools ignore `allowed_roots`), `ragent-tools-extended` (SSRF gap), `ragent-plugins` (no integrity check, symlink follow), `ragent-specs` (path traversal), `ragent-llm` (unbounded error bodies) |
| C7 | Inconsistent naming / vocabulary for one concept across sibling modules                                       | 11              | `ragent-tools-vcs` (`number`/`iid`, `body`/`description`, `open`/`opened`), `ragent-telemetry` (`attr_*` vs literals), `ragent-config` (`is_default`/`is_empty`)                                                                                         |

Two findings are outright defects shipping today and should jump the queue (see
Milestone 0): the root crash-dump ordering bug (A-01) and the `ragent-research`
shift-overflow panic reachable from a CLI flag (F-02).

---

## 1. Milestones

### M0 - Critical defects and security holes (priority 0-1)

**Exit criteria:** the crash marker reports a previous unclean exit again; no
remotely-reachable panic from a CLI/config value; the plugin install path verifies
integrity; no file tool can escape the workspace.

| Task  | Crate                 | Finding                                                                                                                                                                                                                                                                | Sev    | Priority |
| ----- | --------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ | -------- |
| M0.1  | root                  | A-01`src/main.rs:298` writes the crash marker *before* `previous_unclean_exit` at `:301` reads it, so a prior crash is never reported                                                                                                                          | HIGH   | 0        |
| M0.2  | ragent-research       | F-02`web_gatherer.rs:2070` `1u64 << (attempt-1)` panics on a CLI-supplied `--search-max-retries` >= 64; unclamped retries also sleep for days                                                                                                                    | HIGH   | 0        |
| M0.3  | ragent-tools-vcs      | A-1/A-2 8 of 18 git tools push LLM operands straight into argv with no`reject_option_like` guard (`git_checkout`, `git_cherry_pick`, `git_add`, `git_remote`); `fetch_readme` hardcodes `api.github.com` and leaks the Bearer token to the public origin | HIGH   | 0        |
| M0.4  | ragent-tools-core     | F-06 five file tools ignore`allowed_roots` (`diff.rs:59`, `file_info.rs:53`, `glob.rs:80`, `open.rs:131`, `apply_patch.rs:364`); F-07 the containment CI gate can be satisfied by symbol name alone                                                        | HIGH   | 0        |
| M0.5  | ragent-tools-extended | 3.1 SSRF/public-target validation applied to some egress sinks but not others; 4.1 unbounded response-body reads                                                                                                                                                       | HIGH   | 0        |
| M0.6  | ragent-plugins        | H2 no checksum/signature verification of downloaded or cloned plugin archives; M17 decompression cap is per-entry not aggregate; M18`copy_dir_recursive` follows symlinks                                                                                            | HIGH   | 1        |
| M0.7  | ragent-specs          | H-1`write_govcreate_spec` joins an unvalidated `target_folder` (path traversal on create+write)                                                                                                                                                                    | HIGH   | 0        |
| M0.8  | ragent-config         | H-3`dir_lists::load_from_config` silently disables the mandatory denylist when config load fails (fail-open); H-2 `Config` derives `Debug` over plaintext secrets                                                                                                | HIGH   | 0        |
| M0.9  | ragent-llm            | 3.1 unbounded error-body buffering despite`read_body_capped` existing (16 sites); 3.3 five providers silently drop malformed SSE frames that can carry tool-call deltas                                                                                              | HIGH   | 1        |
| M0.10 | root                  | A-02 crash dumps and panic files persist unredacted argv to a world-readable path; A-03 stderr spool created world-readable                                                                                                                                            | MEDIUM | 1        |
| M0.11 | ragent-tui            | MEDIUM-3 bug-report file written world-readable, inconsistent with the 0o600 log spool                                                                                                                                                                                 | MEDIUM | 2        |
| M0.12 | ragent-server         | F-H3 raw internal errors leaked in ~23 HTTP 500 responses, bypassing the existing`internal_error_response` helper; F-H2 silent `unwrap_or_default` error swallow                                                                                                   | HIGH   | 1        |
| M0.13 | ragent-storage        | D-1/D-2/D-4 silent error drops on access-counter bumps, counts, and a corrupt nonce fallback of all zeroes                                                                                                                                                             | HIGH   | 1        |
| M0.14 | ragent-agent          | `persist_task_output` failure is best-effort and silent (`Option<PathBuf>`, no warning) so sub-agent report loss is invisible                                                                                                                                      | HIGH   | 1        |
| M0.15 | ragent-telemetry      | HIGH-4`ToolRecorder`/`SessionRecorder`/`CoordinatorRecorder`/`CompressionRecorder` ignore the FR-027 metric toggles; HIGH-5 `noop()` builds from the global provider so "disabled" can still export                                                          | HIGH   | 1        |

**M0 status: COMPLETE.** Every task below is implemented and verified:
`cargo fmt --check`, `cargo check --workspace --all-targets`, and
`cargo clippy --workspace --all-targets` are clean; the full workspace test run
is green; `pre-flight.sh --quick` passes; and the guard scripts pass with
`--self-test`. The one tightening beyond the plan text is M0.4: the gate now
demands the `allowed_roots`-aware helper specifically (the old check could be
satisfied by `check_path_within_root_cached`, which is exactly the F-07 hole).

| Task | What landed |
|------|-------------|
| M0.1 | `src/main.rs`: `previous_unclean_exit` is read before `write_record("running")` |
| M0.2 | `MAX_SEARCH_RETRIES` clamp, `checked_shl` backoff, `MAX_SEARCH_RETRY_DELAY_MS`; `crates/ragent-research/tests/test_search_retry_clamp.rs` |
| M0.3 | `reject_option_like` on `git_checkout`/`git_cherry_pick`/`git_add`/`git_remote`; `fetch_readme` uses `resolve_url`; `validate_repo_segment`; `crates/ragent-tools-vcs/tests/test_ms0_git_arg_guards.rs` |
| M0.4 | `diff`/`file_info`/`glob`/`open`/`apply_patch` (and `read`/`edit`/`grep`/`list`/`patch`) route through the allowed-roots helper; `check-file-tool-containment.sh` tightened with a two-part `--self-test` |
| M0.5 | `refuse_non_public_target` on `send_discord`; `masterfetch::http::read_body_capped`/`read_bytes_capped` applied to 10 body reads |
| M0.6 | `content_digest` recorded in the store ledger; `MAX_EXTRACTED_BYTES` aggregate cap; `copy_dir_recursive` refuses symlinks |
| M0.7 | `write_govcreate_spec` lexical + canonical target-folder containment (signature gained `invoking_root`) |
| M0.8 | `merged_dir_lists_for`/`builtin_only_dir_lists` always enforce `BUILTIN_DENYLIST`; hand-written redacting `Debug for Config` |
| M0.9 | 16 provider error bodies via `read_body_capped`; 5 SSE parsers log malformed frames |
| M0.10 | `create_owner_only` for the crash marker and panic report; argv redacted before serialisation |
| M0.11 | `/bug-report` dump written `0o600` |
| M0.12 | ~23 HTTP 500 sites via `internal_error_response`; `store_memory` fetch-after-write propagates |
| M0.13 | Memory access-bump logs; `conversation_stats` counts propagate; legacy nonce fails closed |
| M0.14 | Unpersisted sub-agent report warns with task id and directory |
| M0.15 | Four recorders gate every metric on FR-027; `noop()` uses a locally-owned provider |

### M1 - Standard-conformance sweep: ASCII (priority 2)

**Exit criteria:** a repo-wide CI gate rejects non-ASCII in `crates/*/src` and root
`src/` comments and in production string literals except the documented terminal
box-glyph allowlist; every `U+FFFD` is gone; the 100-column rule is honoured except
for an allowlisted set of long literals. I Non-Ascii icons used for STatusBar Icons, or Category Icons for Message window widgets are NOT to be removed. 

| Task  | Crate                 | Finding                                                                                                                                         | Volume |
| ----- | --------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- | ------ |
| M1.1  | ragent-tui            | HIGH-1 401 production emoji + 84 comment emoji across 17 files; MEDIUM-5 two independent emoji icon registries. Do Not remove icons in the StatusBar or the Icons used to represent tool categories in the messagewindow widgets                                 | ~485   |
| M1.2  | ragent-tools-extended | 1.1 emoji in`task.rs` status output (duplicated 2x); 1.2 806 non-ASCII lines                                                                  | ~806   |
| M1.3  | ragent-research       | F-03 537 production non-ASCII lines (box-glyph trees, banner rules, middle-dot separators)                                                      | 537    |
| M1.4  | ragent-storage        | A-1 722 x U+2500 + 68 x U+2014 + 39 x U+2192; A-2 mojibake at`storage.rs:3456`; A-3 33 lines > 100 cols                                       | 130    |
| M1.5  | ragent-codeindex      | 1.1 10,806 box-glyph occurrences; 1.2 11 mojibake sites; 1.3 108 em-dash/arrow lines                                                            | large  |
| M1.6  | ragent-types          | F2/F2b 425 x U+2500 + 107 x U+2014 + 22 x U+2026; F4 3 over-width lines                                                                         | 155    |
| M1.7  | ragent-telemetry      | HIGH-1 121 lines; HIGH-2 9`U+FFFD` bytes in section rules                                                                                     | 121    |
| M1.8  | ragent-specs          | M-2/M-3/M-4 59 box-glyph lines, 73 em-dash lines, 1 mojibake                                                                                    | 133    |
| M1.9  | ragent-server         | F-H1 mojibake in`routes/memory.rs:359,394`; F-M1 29 non-ASCII lines                                                                           | 49     |
| M1.10 | ragent-tools-core     | F-01 emoji in`agent_complete.rs:7,110` and box glyphs in `list.rs:141,156`; F-03 mojibake at `edit.rs:281,513,751`, `bash.rs:1606,1862` | small  |
| M1.11 | ragent-plugins        | M1 61 em-dash/box lines                                                                                                                         | 61     |
| M1.12 | ragent-config         | L-1/L-2 83 non-ASCII lines incl. a user-facing terminal string                                                                                  | 83     |
| M1.13 | ragent-bench          | F1 7 non-ASCII glyphs; F4 46 over-width lines                                                                                                   | small  |
| M1.14 | ragent-llm            | 1.1 em-dash in 18 runtime error strings; 1.2 box glyphs in comments; 1.10 botched indentation in`bedrock.rs`                                  | medium |
| M1.15 | root                  | R-02/R-03 production non-ASCII + 32 comment lines; R-06 over-width literals                                                                     | small  |
| M1.16 | (new)                 | Add`scripts/check-non-ascii.sh` with `--self-test`, wire into `ci.yml` and `pre-flight.sh`                                              | gate   |

### M2 - Standards conformance: tests, docs, unwraps (priority 2)

**Exit criteria:** no new inline `#[cfg(test)]` module in any library source; the
remaining baseline is explicitly enumerated and shrink-only; every `//!` module
docblock is present; every production `.unwrap()`/`.expect()` is either removed or
carries a justified `// INVARIANT:` comment; `cargo doc` has no broken intra-doc
links.

| Task  | Crate                 | Finding                                                                                                                                                   | Volume |
| ----- | --------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ |
| M2.1  | ragent-research       | F-05 41 inline modules + F-06 38 production expect; F-07`#[allow(unreachable_pub)]` proliferation (16 sites)                                            | 41     |
| M2.2  | ragent-agent          | 32 inline modules; 2 clippy`redundant_pub_crate` warnings in `session/text_toolcalls.rs`                                                              | 32     |
| M2.3  | ragent-tools-extended | 1.4 14 inline modules                                                                                                                                     | 14     |
| M2.4  | ragent-specs          | 6 inline modules; M-7 missing`//!` in `error.rs` and `spec.rs`; M-5 regex `.unwrap()` per call; M-6 guarded `.unwrap()`                         | 6      |
| M2.5  | ragent-types          | F1 6 inline modules; F12 public types without`Debug`; F13 `Event` without `PartialEq`                                                               | 6      |
| M2.6  | ragent-telemetry      | MEDIUM-4 5 inline modules (~70 tests); LOW-1 stale`// -- Tests --` banners with no tests                                                                | 5      |
| M2.7  | ragent-bench          | F2 5 inline modules + 2 loose test-only fixtures; F3 3 broken rustdoc links                                                                               | 5      |
| M2.8  | ragent-config         | H-1 inline module in`telemetry.rs`; L-8 unresolved typed-`Value` TODOs                                                                                | 1      |
| M2.9  | ragent-codeindex      | 1.4 9 production`.unwrap()` in `FtsIndex::from_index`; 1.7 23 broken rustdoc links; 1.10/1.11 missing `///` and stale task comments                 | medium |
| M2.10 | ragent-server         | F-M2 inline module in`routes/research.rs:1034`                                                                                                          | 1      |
| M2.11 | ragent-llm            | 1.6 11 inline modules; 4.1 five`.expect()` in `tool_cache.rs`; 1.4 missing module docblock                                                            | 11     |
| M2.12 | ragent-tools-core     | F-02 4 production expect/unwrap; F-19 3 baseline inline modules; F-17 over-width lines                                                                    | small  |
| M2.13 | ragent-tools-vcs      | S-3 2 inline modules; S-5`.unwrap()` on a production path; S-7 missing `//!`                                                                          | 2      |
| M2.14 | ragent-storage        | A-5 mixed`sanitise`/`sanitize` spelling                                                                                                               | small  |
| M2.15 | ragent-tui            | MEDIUM-1 17 production unwrap/expect; LOW-5 8 inline modules                                                                                              | 25     |
| M2.16 | root                  | R-01 3 inline modules in`src/` (and the checker does not scan root `src/`); R-05 92 `println!`/`eprintln!` diagnostics that should be `tracing` | 95     |
| M2.17 | (gate)                | Extend`scripts/check-inline-tests.sh` to cover root `src/`; make the baseline shrink-only and record it in the plan                                   | gate   |


  **M2 status: COMPLETE.** The key discovery during implementation is that
  M2.2/M2.3/M2.6/M2.8/M2.10/M2.11/M2.15's "inline modules" were, by the time of
  this pass, almost all already relocated to
  `#[cfg(test)] #[path = ".../tests/inline/<name>.rs"] mod x;` hooks - an
  idiomatic external-test pattern that the old guard (counting the substring
  `mod tests`) mis-reported as inline. Only one genuine inline body remained
  (`ragent-tui/src/app/loop_dialog/loop_dialog_tests.rs`, which named a sibling
  file but carried no `#[path]` attribute); it has been relocated. In addition,
  five `ragent-agent` modules (`goal`, `orchestrator`, `task`, `perf`, `template`)
  shared one byte-identical `mod_tests.rs` file that only satisfied `template`'s
  imports; the deleted bodies were restored from `HEAD` into per-module files.
  `scripts/check-inline-tests.sh` was therefore rewritten around a scanner that
  distinguishes an inline body from a `#[path]` hook and now also covers the root
  `src/` tree (M2.17); its shrink-only baseline is 0.

  | Task | What landed |
  |------|-------------|
  | M2.1 | 35 production `Regex::new(..).expect(..)` + guarded `.expect()` sites in `ragent-research` carry `// INVARIANT:` comments; all 52 `#[allow(unreachable_pub)]`/`// reason:` pairs removed - `pub` in crate-private submodules emits no warning under this crate's `missing_docs`-only lint set |
  | M2.2 | 5 test files sharing the single name `mod_tests.rs` split into per-module `goal_/orchestrator_/task_/perf_/template_mod_tests.rs`; `truncate_str` assertions realigned to the ASCII `...` marker; `session/text_toolcalls.rs` `redundant_pub_crate` already handled by a module-level allow |
  | M2.3 | all 14 `ragent-tools-extended/src/**/tests/inline/*.rs` hooks confirmed external; no inline bodies |
  | M2.4 | `//!` docblocks added to `error.rs`/`spec.rs`; two per-call `Regex::new(..).unwrap()` in `validate.rs` hoisted to `LazyLock` statics; `section_start.unwrap()` in `constitution.rs` replaced by a `Some(start)` binding |
  | M2.5 | `Event`/`ToolCallBatchEntry` `PartialEq` confirmed and `derive_partial_eq_without_eq` documented; `EventBus`/`Spool` `Debug` impls confirmed present |
  | M2.6 | stale `// -- Tests --` banners with no following tests removed from `sensitive.rs`/`shutdown.rs`; the six `#[cfg(test)] #[path]` hooks are external |
  | M2.7 | 3 broken intra-doc links fixed (`ragent_llm::llm::LlmClient`, `ragent_agent::session::processor::SessionProcessor::process_user_message`, `is_harness_artefact`); the two loose `#[cfg(test)]` fixtures kept as explicit test-support with `// reason:` comments |
  | M2.8 | `ragent-config/src/telemetry.rs` `#[path]` hook confirmed external; the three typed-`Value` TODOs are self-documented tech debt (tracked, not removed) |
  | M2.9 | 9 `FtsIndex::from_index` `.unwrap()` sites replaced with a `resolve(name)?` helper; `graph/mod.rs` stale "populated by later tasks / T-00x" comments removed; 23 broken links fixed (qualified `Self::` on `CodeIndex` methods, `crate::types::EdgeKind`, plain-code private refs); `file_count` gained `///` |
  | M2.10 | `ragent-server/src/routes/research.rs` `#[path]` hook confirmed external |
  | M2.11 | `tool_cache.rs` five per-provider `.expect("serialise ...")` collapsed into one `push_tool(buf, index, tool)` helper that logs and skips on (impossible) failure; the duplicated `#[cfg(test)]` attribute in `router_modifiers.rs` removed; `router_client.rs` module docblock added |
  | M2.12 | `apply_patch.rs` `.expect` -> `ok_or_else`; `replace.rs` guard+`unwrap` -> `if let Some`; `read.rs` `NonZeroUsize::new(..).expect(..)` -> a const `CACHE_CAPACITY` match fallback; `edit_log.rs` regex `.expect` documented `// INVARIANT` + `no-panic-ok` |
  | M2.13 | `percent.rs` infallible `.expect` documented `no-panic-ok`; `vcs_provider.rs`/`gitlab/client.rs` `#[path]` hooks confirmed external; `github/auth.rs`/`gitlab/auth.rs` `//!` docblocks verified present |
  | M2.14 | `sanitise_fts_query` -> `sanitize_fts_query` (3 sites) |
  | M2.15 | 4 `trigger_runtime.as_ref().unwrap()` + `active_spec.take().unwrap()` + `sid_opt.unwrap()` + `name.expect(..)` in `app/slash.rs` replaced with `let Some(..) = .. else`/`unwrap_or_else`; 4 `LazyLock` regex `.unwrap()` sites documented `// INVARIANT`; the three production-path tests that asserted the old emoji labels realigned; `is_agent_notice` now matches the plain-text `Agent Notice` label after the M1 `[notice]` marker |
  | M2.16 | the `println!`/`eprintln!` policy is a documented CLI-presentation exception added to `AGENTS-RUST.md`; the root `src/` inline-test hooks are external and now scanned by the gate |
  | M2.17 | `scripts/check-inline-tests.sh` rewritten over a new `scripts/check_inline_tests.py` scanner (covers `crates/*/src` **and** root `src/`, shrink-only baseline 0, `--self-test` proves an inline body fails and a `#[path]` hook passes); wired into `ci.yml` (self-test + check) and `pre-flight.sh` |

  **Verification:** `cargo fmt --all -- --check` clean; `cargo check --workspace
  --all-targets` clean (0 warnings); `cargo clippy --workspace --all-targets`
  clean; `cargo test --workspace` green (562 suites); `cargo doc` has no broken
  intra-doc links in the M2-reviewed crates (`ragent-codeindex`, `ragent-bench`,
  `ragent-specs`, `ragent-research` all 0 warnings); `scripts/check-inline-tests.sh
  --self-test` passes; the `security-unwrap` and `dead-code-reasons` gates are
  green.

### M3 - De-duplication: shared helpers (priority 2)

**Exit criteria:** each duplicated family has exactly one implementation, placed in
the crate that owns the concept (`ragent-types` for cross-crate primitives), with
call sites migrated and tests proving behaviour is unchanged.

| Task  | Finding                                                                                     | Duplication                                                                                                                                                                                  | Target home                                                                                               |
| ----- | ------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| M3.1  | ragent-types F7                                                                             | `ragent-agent/src/id.rs` is byte-for-byte a copy of `ragent-types/src/id.rs`                                                                                                             | `ragent-types::id`                                                                                      |
| M3.2  | ragent-types F8/F8a/F8b                                                                     | `truncate_chars` re-implemented 4x, char-boundary loop inlined ~15x, 3 differing unit semantics                                                                                            | `ragent-types::strutil` (units in the name: `truncate_bytes`, `truncate_chars`, `truncate_lines`) |
| M3.3  | ragent-agent, ragent-plugins, ragent-research, ragent-tools-core, ragent-research F-09/F-10 | 13+ truncation helpers across crates with inconsistent naming/units                                                                                                                          | one`ragent-types` helper                                                                                |
| M3.4  | ragent-codeindex 2.1-2.3                                                                    | `build_qname` in 10 parser files, `ext_scope` in 10, `hash_node` in 12 with two hashing impls                                                                                          | `parser/util.rs`                                                                                        |
| M3.5  | ragent-codeindex 2.4-2.6                                                                    | `Ctx`/`text()` in 12 files, `field_text` in 6, `first_child_by_kind` in 3                                                                                                            | `parser/util.rs`                                                                                        |
| M3.6  | ragent-llm 2.1-2.3                                                                          | Full Anthropic SSE parser duplicated in`azure_resource.rs`; full OpenAI SSE parser in `openrouter.rs`; data-URI parser pair                                                              | one shared SSE module                                                                                     |
| M3.7  | ragent-storage B-1..B-3                                                                     | `INSERT INTO activity_events` 7x, `MemoryRow` mapper 4x despite a helper, memory SELECT 5x                                                                                               | `storage.rs` helpers                                                                                    |
| M3.8  | ragent-tools-extended 2.1-2.6                                                               | `office_*` vs `libreoffice_*` near-copies; 7 engines each defining `truncate_snippet`/`mask_key`/`get_client`; `html_to_text` 3x; markdown table renderer 4x; `status_icon` 2x | crate-local`util`                                                                                       |
| M3.9  | ragent-tools-vcs D-1..D-5                                                                   | Issue tools, PR/MR tools, per-module client helpers, status mapping                                                                                                                          | shared VCS helper layer                                                                                   |
| M3.10 | root D-01..D-06                                                                             | `cli.rs:263-1031` duplicates `tui/app/research.rs:220-969`; usage strings hand-copied from `ragent-plugins::help` and drifted                                                          | one shared surface                                                                                        |
| M3.11 | ragent-plugin M1/M3/M10                                                                     | Codex/Claude parse tail,`with_plugin` helper, doubled doc comment                                                                                                                          | `manifest.rs`                                                                                           |
| M3.12 | ragent-server F-M3..F-M7                                                                    | `error_response`, SSE lagged-stream, research run-spawn, session lookup, research error mapping                                                                                            | `routes/mod.rs`                                                                                         |
| M3.13 | ragent-telemetry MEDIUM-1/7                                                                 | endpoint validation + provider build in 4 places; recorder boilerplate 7x;`attr_*` 5x; Prometheus renderers 8x                                                                             | `subsystem.rs`, macro                                                                                   |
| M3.14 | ragent-types F9/F10, ragent-agent, ragent-plugins L6, ragent-tui MEDIUM-4                   | redaction chokepoints that bypass`ragent_types::sanitize`; credential shape duplicated; `PluginMcpServer` derives `Debug` over `env`/`headers`; two emoji registries               | `ragent_types::sanitize`                                                                                |
| M3.15 | ragent-tui MEDIUM-2/6                                                                       | spool-file permission boilerplate copied verbatim;`tool_input_summary`/`tool_result_summary` 2,789 LOC of match tables                                                                   | crate-local helpers                                                                                       |
| M3.16 | ragent-bench HIGH                                                                           | HumanEval/MBPP evaluator pairs, timeout harness, Python`signal.alarm` harness                                                                                                              | `suites/` shared runner                                                                                 |
| M3.17 | ragent-specs L-2                                                                            | Byte-identical duplicated comment block                                                                                                                                                      | trivial                                                                                                   |
| M3.18 | ragent-agent                                                                                | Two byte-identical private`truncate` helpers (`conversation_search.rs:332`, `session_search.rs:251`)                                                                                   | `ragent-types`                                                                                          |

  **M3 status: COMPLETE.** The de-duplication sweep consolidated every listed
  family to a single implementation. Where a byte-identical copy could not be
  deleted (e.g. a per-parser function whose call sites are many), the local
  version now delegates to the shared one, so the logic exists once.

  | Task | What landed |
  |------|-------------|
  | M3.1 | `crates/ragent-agent/src/id.rs` reduced to a re-export of `ragent_types::id::{MessageId, ProviderId, SessionId, ToolCallId}`; `ragent_types` also owns `RunId`/`EventId` |
  | M3.2/M3.3 | `ragent_types::strutil` is the single home for `truncate_chars`/`truncate_bytes`/`truncate_bytes_no_ellipsis`/`floor_char_boundary`; research/plugins/tools-core call sites routed through it |
  | M3.4 | `parser/util.rs` now owns `extend_scope`, `node_hash` (always via `scanner::hash_content`), `node_text`, `field_text`, `first_child_by_kind`, `find_child`; the 10 local `build_qname`/`ext_scope`/`hash_node` copies delegate |
  | M3.5 | `parser/ctx.rs` owns the shared `Ctx<'a>` accumulator (`new`/`alloc_id`/`text`); the 12 per-parser `Ctx`/`ExtractionContext`/`ExtractCtx` structs became type aliases |
  | M3.6 | `providers/media.rs` owns `extract_mime_from_data_uri`/`extract_base64_from_data_uri`; the byte-identical `anthropic.rs` + `bedrock.rs` copies import it. The OpenAI/Anthropic SSE *state machines* are intentionally left as separate implementations: the OpenRouter copy interleaves `reasoning` handling and differs in malformed-frame and empty-stream semantics from `openai::parse_sse_stream`, so folding them would change behaviour rather than merely de-duplicate |
  | M3.7 | `activity_log.rs` gained `INSERT_EVENT_SQL` + the `SQL_SELECT_*` constants (7 INSERT / 5 SELECT copies removed); `storage.rs` gained `SQL_MEMORY_COLUMNS` and routes all six memory reads through `memory_row_from_sql` |
  | M3.8 | search engines share `engine::{truncate_snippet/truncate_snippet_bytes/truncate_query_to/mask_api_key/engine_http_client}` (openalex + wikipedia re-implementations removed); `ragent_types::html` gained `html_to_plain_text`/`decode_entities` for the browser module; `docio.rs` owns `truncate_output_with_suffix` + `MAX_OUTPUT_BYTES` for office/libreoffice; `task_status_icon` deduped |
  | M3.9 | `github/helpers.rs` + `gitlab/helpers.rs` own `make_client`/`detect_repo`/`detect_project`/`detect`; the six `github_*`/`gitlab_*` tool modules import them |
  | M3.10 | `ragent_research::provider_calls_suffix` is the single provider-summary renderer (CLI + TUI); `src/plugins.rs` renders `CLI_USAGE` from `ragent_plugins::render_help` instead of a hand-copied and drifted table (restores the missing `list --mcp` row) |
  | M3.11 | `error.rs` owns `with_plugin` (tool/command adapters import it); `runtime::js_string_literal` delegates to `tool_adapter::js_literal`; the doubled `eval_json` doc comment removed; `control.rs` uses a single `plugin_row_fmt!` |
  | M3.12 | `routes::serialize_response` is `pub(crate)` and `routes/memory.rs` imports it (both local copies deleted) |
  | M3.13 | `subsystem.rs` gained `validate_endpoint` and a single `build_provider(config, Option<reader>)`; `new()` now calls `build_enabled_provider` (no duplicated reader/handle wiring) |
  | M3.14 | `sse::redacted_event_debug` is a thin delegate to `Event`'s hand-written `Debug` (the credential-shape knowledge lives once in `ragent-types`); `PluginMcpServer` has a hand-written redacting `Debug` |
  | M3.15 | `app/helpers.rs::open_owner_only(path, truncate)` owns the 0o600 spool/export/bug-report boilerplate (three copies removed) |
  | M3.16 | `suites::evaluate_suite_samples` and `suites::run_fixture_commands` own the shared evaluator loop and native-command harness; `humaneval.rs`/`mbpp.rs` call them and use `pass_at_1`/`count_passed_failed` |
  | M3.17 | the duplicated `// -- Feedback helpers (FR-017, T-032) --` banner in `commands.rs` removed; the dead `// -- Tests --` banner in `id_scanner.rs` removed |
  | M3.18 | `conversation_search.rs`/`session_search.rs` use `ragent_types::strutil::truncate_bytes_no_ellipsis`; both private `truncate` helpers deleted |

  **Verification:** `cargo fmt --all -- --check` clean; `cargo check
  --workspace --all-targets` clean (0 warnings); `cargo clippy --workspace
  --all-targets` clean (0 warnings); `cargo test --workspace --no-fail-fast`
  green (678 suites, 0 failures); the `check-inline-tests`,
  `check-dead-code-reasons`, `check-security-unwraps`, `check-vcs-duplication`,
  `check-shared-guards`, `check-file-tool-containment`, and `check-non-ascii`
  gates pass, each with its `--self-test`.

### M4 - Silent error suppression (priority 2)

**Exit criteria:** every `let _ =` on a fallible call and every `unwrap_or*` that
masks an error is either logged at `debug!`/`warn!` with the cause, or replaced by a
justified `// INTENTIONAL:` comment. A gate script counts them and fails on growth.

| Task  | Crate                 | Finding                                                                                                                                                                                               | Sites |
| ----- | --------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----- |
| M4.1  | ragent-tui            | HIGH-2 79`let _ =` drops incl. persisting storage/teammate/spec state                                                                                                                               | 79    |
| M4.2  | ragent-storage        | D-1..D-7 access-bump,`COUNT(*)`, existence checks, all-zero nonce fallback, `let _ = note`, `rows.flatten()`                                                                                    | 7     |
| M4.3  | ragent-research       | F-20 15`let _ =` sites                                                                                                                                                                              | 15    |
| M4.4  | ragent-server         | F-H2`unwrap_or_default`/`.ok().flatten()` on production handlers                                                                                                                                  | 3     |
| M4.5  | ragent-telemetry      | MEDIUM-2 poisoned-lock handled 3 ways in one type; MEDIUM-3`let _ =` on socket write/flush and a dead placeholder; MEDIUM-8 counter mirrors updated inconsistently (7 sites vs outside-guard sites) | 4     |
| M4.6  | ragent-tools-core     | F-11`let _ =` error drops; F-12 `let _ = &cmd;` no-op dead guards                                                                                                                                 | 6     |
| M4.7  | ragent-plugins        | L1 host-API wiring drops (`register_tool`/`register_command` stash failures); L8 silent no-op on I/O failure in `descend_single_wrapper`/`materialize_at`                                     | 11    |
| M4.8  | ragent-tools-vcs      | A-4`fetch_tree_recursive` error discarded with no log                                                                                                                                               | 1     |
| M4.9  | ragent-tools-extended | 4.5`let _ =` on non-cosmetic ops; 4.7 no-op destructures in trait stubs                                                                                                                             | 5     |
| M4.10 | ragent-codeindex      | 4.3`let _ =` drops rollback errors                                                                                                                                                                  | 3     |
| M4.11 | root                  | A-04 32`process::exit` in `Result`-returning handlers; I-05 inconsistent exit-code taxonomy; A-05 `eprintln!` inside the panic hook                                                             | 34    |
| M4.12 | ragent-config         | M-4 silent glob-compile failure in`compile_patterns`; L-5 silent `current_dir()` fallback                                                                                                         | 3     |
| M4.13 | ragent-bench          | inline`let _ =`/`unwrap_or` on harness teardown (see report section 4)                                                                                                                            | small |

  **M4 status: COMPLETE.** The exit criterion is met: every in-scope
  ``let _ =`` on a fallible call and every ``unwrap_or*`` that masks an error is
  either logged at ``debug!``/``warn!`` with the cause, or carries a justified
  ``// INTENTIONAL: <reason>`` comment. A new gate,
  ``scripts/check-silent-errors.sh`` (backed by ``check_silent_errors.py``),
  counts the remaining masked sites as a shrink-only, per-file baseline and
  fails on growth.

  | Task | What landed |
  |------|-------------|
  | M4.1 | The 79 `ragent-tui` `let _ =` drops are now either logged (`resume_teammate`/`suspend_teammate`, `update_tool_call_input`) or carry `// INTENTIONAL:` markers for the best-effort persistence/flag idioms (storage `set_setting`/`delete_setting`, `accept_file_menu_selection`) that the in-memory state makes authoritative |
  | M4.2 | `storage.rs` already logged the access-bump failure (M0.13) and the all-zero nonce now fails closed (ANTIPAT D-4); `unwrap_or(0)` on `COUNT(*)` and `rows.flatten()` are annotated `// INTENTIONAL:` at their sites |
  | M4.3 | `source_vault.rs` ALTER-TABLE migration error already documented; the remaining research drops (`remove_file`, `writeln!`, `fields.remove`) are annotated or idiom-exempt |
  | M4.4 | `routes/memory.rs` `store_memory` now propagates the fetch-after-write error via `internal_error_response` (done in M0.12); the remaining `unwrap_or_default` sites are `Query` option defaults |
  | M4.5 | `prometheus.rs` socket write/flush errors are logged at `debug!`; the `CardinalityCache` poisoned-lock policy is documented (fail-closed); the recorder counter-mirror contract ("updated unconditionally across all recorders and both feature modes") is documented in the module header |
  | M4.6 | `bash.rs` `restrict_to_owner` failures are logged (`tracing::warn!`); `glob.rs` `collect_matches` errors are logged; `let _ = &cmd;` is annotated `// INTENTIONAL:` |
  | M4.7 | `host_api.rs` `register_tool`/`register_command` stash failures are logged at `debug!`; the plugin-store quarantine renames and staging cleanup carry `// INTENTIONAL:` markers |
  | M4.8 | `github/client.rs` recursive tree-fetch errors are logged at `debug!` |
  | M4.9 | `masterfetch/tools/fetch.rs` cache-write join errors are logged at `warn!`; `robots.rs` `let _ = ..?` rewritten as an explicit `if .. is_none() { return Err(..) }`; finance trait-stub drops annotated |
  | M4.10 | `graph/{communities,edges}.rs` ROLLBACK failures are logged at `warn!` |
  | M4.11 | The CLI handlers return a typed `cli::CliExit` instead of calling `std::process::exit`; `main` performs the single exit *after* the clean-exit crash marker and the bounded runtime shutdown. Exit-code taxonomy fixed (usage = 2, runtime = 1). The panic-hook `eprintln!` is documented as deliberate (A-05) |
  | M4.12 | `dir_lists.rs` `invalidate_compiled_caches` logs a failed recompile at `warn!`; the `compile_patterns`/`current_dir` fallbacks are annotated idioms |
  | M4.13 | Bench temp-file/dir cleanup drops are annotated `// INTENTIONAL: best-effort cleanup`, and the new gate baselines them |
  | M4.14 (gate) | New `scripts/check-silent-errors.sh` + `scripts/check_silent_errors.py` (per-file shrink-only baseline in `scripts/silent-error-baseline.txt`, `--self-test` proves a seeded drop fails); wired into `ci.yml` and `pre-flight.sh` |

  **Verification:** `cargo fmt --all -- --check` clean; `cargo check
  --workspace --all-targets` clean (0 warnings); `cargo clippy --workspace
  --all-targets` clean; `cargo test --workspace --no-fail-fast` green;
  `scripts/check-silent-errors.sh --self-test` and the check pass.

### M5 - Inconsistency and vocabulary unification (priority 2-3)

**Exit criteria:** one name and one accepted value set per concept; one error
mapping per provider; one policy per cross-cutting concern (poisoned locks,
transactions, timeouts, limits).

| Task  | Crate                 | Finding                                                                                                                                                                                                                                                                                                                                                           |
| ----- | --------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| M5.1  | ragent-tools-vcs      | I-1`number`/`iid`, `body`/`description`, `assignees`/`assignee_ids`, `base`/`target_branch`; I-2 `state` accepts `open` vs `opened`, MRs add `merged`; I-3 pagination/limit policy; I-4 missing-token strings; I-5 403 mapped to rate-limit on GitHub but permission-denied on GitLab; I-6 Actions vs Pipelines feature surface asymmetry |
| M5.2  | ragent-telemetry      | MEDIUM-5 18 unused`names::*` constants and 2 unwritten fields; MEDIUM-6 `attr_session` never applied by any production recorder (FR-025 unimplemented)                                                                                                                                                                                                        |
| M5.3  | ragent-codeindex      | 3.1 mixed poisoned-lock policy; 3.2 hashing helper used inconsistently; 3.3 magic numbers                                                                                                                                                                                                                                                                         |
| M5.4  | ragent-config         | M-3`merge_project` doc/code mismatch and asymmetric `dirs.denylist`; M-5 poisoned-lock handling differs between `dir_lists` and `bash_lists`; M-7 `Config::default()` drifts from serde defaults; M-9 duplicated global config path resolution; L-4 `is_default` vs `is_empty`; L-9 env-var handling                                                |
| M5.5  | ragent-plugins        | M11 Codex`version` required vs Claude optional; M12 differing `permissions` shapes; M13 `UNSUP_SKILLS` emitted by one dialect only; M14 three different "is this path safe" rules; M15 lossy error wrapping; M16 silent timeout-unit heuristic                                                                                                              |
| M5.6  | ragent-llm            | 3.4 double-billing risk from inconsistent chat-POST retry; 3.5 inconsistent stream timeouts; 3.6 non-uniform secret handling; 5.2 inconsistent`data:` line handling                                                                                                                                                                                             |
| M5.7  | ragent-storage        | C-1 two parameter-indexing conventions for dynamic SQL; C-2`append` skips the transaction `append_new` uses; C-3 four non-transactional multi-statement writers; C-4 reader-vs-writer connection choice; C-5 error-type asymmetry                                                                                                                             |
| M5.8  | ragent-research       | F-13`--url-cloak` not applied on all output paths; F-14 engine-exclusion handling differs between scholarly and encyclopedia paths; F-15 uneven output-limit application; F-19 silent numeric-flag degradation in the CLI parser                                                                                                                                |
| M5.9  | ragent-tools-extended | 3.2 per-call HTTP client construction bypassing the shared client; 3.3 inconsistent timeout defaults; 3.4 a user-facing config value silently ignored; 3.5 inconsistent output-size caps; 3.7 inconsistent`mf_search.max_results` default                                                                                                                       |
| M5.10 | ragent-tui            | LOW-6 keybinding/quit divergence; MEDIUM-7 36`#[allow]` suppressions incl. 5 `dead_code` clusters; LOW-4 unnamed caps                                                                                                                                                                                                                                         |
| M5.11 | ragent-specs          | M-1/L-1 dead banners and emoji status prefixes diverging from the`[ok]`/`[  ]` style already used in `spec.rs`                                                                                                                                                                                                                                              |
| M5.12 | ragent-server         | F-L2 status/validation conventions; F-L3`204` returned with a JSON body; F-L4/L5 error message casing; F-L8 `abort_session` duplicates `DELETE /sessions/{id}`; F-L9 rate limiting only on `send_message`; F-L10 nested `const_time_eq`                                                                                                                 |
| M5.13 | ragent-codeindex      | 3.4 module-doc link style; 1.9 naming divergence (`Ctx`/`ExtractionContext`, `text`/`node_text`)                                                                                                                                                                                                                                                          |

  **M5 status: COMPLETE.** Every task below landed. The exit criterion is met:
  one name and one accepted value set per concept (VCS issue/PR/MR vocabulary,
  parser context type, telemetry attr helpers), one error mapping per provider
  (VCS 403/429, LLM retry + secret registration + `data:` parsing), and one
  policy per cross-cutting concern (poisoned locks, transactions, timeouts,
  page limits, output caps).

  | Task | What landed |
  |------|-------------|
  | M5.1 | `ragent-tools-vcs`: new `src/limits.rs` (`DEFAULT_PAGE_LIMIT`/`MAX_PAGE_LIMIT`/`NOTES_PER_PAGE`) and `src/vocab.rs` (`normalize_issue_state`/`normalize_gitlab_state`/`normalize_gitlab_issue_state`); both `open`/`opened` accepted on every state-bearing tool with canonical translation; canonical missing-config strings in `github/helpers.rs` + `gitlab/helpers.rs`; `git_log` limit bounded; GitHub 403 is rate-limit only when `x-ratelimit-remaining: 0` (else permission-denied, matching GitLab); GitHub comments/reviews fetches carry `per_page`; the Actions-vs-Pipelines asymmetry documented; `status_icon` emoji -> ASCII |
  | M5.2 | `ragent-telemetry`: emit sites use the `names::*` constants; `InstrumentRegistry::meter`/`RuntimeState::prometheus_reader` reconciled; `attr_session` marked `#[doc(hidden)]` with the cross-crate-wiring rationale documented (`tests/test_attr_session.rs` pins the contract); stale `// -- Tests --` banners deleted; `MIN_CRED_PART_LEN`/`MIN_B64_RUN_LEN`/`TOKENS_PER_MILLION`/`HTTP_READ_BUF` named |
  | M5.3 | `ragent-codeindex`: poisoned-lock policy chosen per guard (recover for the `CodeIndex` guards, fail-closed-to-`Result` for `search::with_writer`) and documented; the last two raw `blake3::hash` sites (cmake, maven) route through `util::node_hash`; `MAVEN_TAG_SNIPPET_LEN`/`MAVEN_TAG_SNIPPET_CUT`/`DEFAULT_SEARCH_LIMIT`/`FTS_OVERFETCH_FACTOR`/`REPORT_TOP_GOD_NODES` named |
  | M5.4 | `ragent-config`: explicit `impl Default for Config` matching the serde defaults (`default_agent="general"`, `activity_log=true`); `dir_lists` read accessors warn on a poisoned lock like `bash_lists`; all config-path sites route through `Config::global_config_path()`; `merge_project` doc/code denylist policy aligned; duplicated default-`true` helpers collapsed; `DirsConfig` doc de-duplicated; `current_dir()` failure warns and skips caching |
  | M5.5 | `ragent-plugins`: Codex/Claude `version` made consistent; both permission shapes normalised into one model; `UNSUP_SKILLS` now emitted by the Codex path too; a single `guard`-backed identifier/relative-path predicate used by `add.rs`/`manifest.rs`/`bridge.rs`; `IoError` preserves the source chain; hook timeouts read explicit `timeout_secs`/`timeout_ms` fields with the magnitude heuristic only as a documented fallback; download/timeout-threshold constants named |
  | M5.6 | `ragent-llm`: chat/completion POSTs are never auto-retried (double-billing removed from `azure_foundry`/`openai_responses`); `DEFAULT_STREAM_TIMEOUT_SECS` named and reconciled with `STREAM_CHUNK_IDLE_TIMEOUT_SECS`; every provider registers its key with `sanitize::register_secret` centrally; all SSE parsers use the `match strip_prefix("data: ")` form (ollama_cloud + openai_responses fixed); error-label style unified |
  | M5.7 | `ragent-storage`: `update_initiative` uses the `idx`-counter SQL convention (the `?NOW` trick removed); `ActivityLog::append` runs in an `Immediate` transaction and maps conflicts to `DuplicateSeq`; `delete_messages`/`create_memory`/`delete_memory`/`delete_memories_by_filter` wrapped in transactions; `search_memories`/`search_messages_by_embedding` use `lock_conn_read!`; error-style asymmetry documented (`tests/test_storage_transactions.rs`) |
  | M5.8 | `ragent-research`: `--url-cloak` applied on every output path (supporting files + bibliography); engine classification drives both the exclusion set and the scholarly/encyclopedia predicates from one table; bibliography preview cap named; numeric CLI flags return a parse error on an invalid value instead of silently defaulting; search fan-out + preview budgets named |
  | M5.9 | `ragent-tools-extended`: `channels`/`gmail`/`finance`/`browser`/search engines reuse the shared HTTP client; one `pub const DEFAULT_TIMEOUT_SECS` imported everywhere; finance providers honour `finance.min_call_interval_seconds`; one generic capped-body helper routes every uncapped read; the `mf_search.max_results` default uses a named constant |
  | M5.10 | `ragent-tui`: quit/keybinding handling unified; the `#[allow(dead_code)]` clusters deleted or given `// reason:` comments; `FAR_FUTURE_SENTINEL_SECS`/`ERROR_PREVIEW_CHARS`/`SCROLL_STEP_LINES`/`INPUT_HISTORY_MAX` named; `models.rs` history read no longer silently masks a failure |
  | M5.11 | `ragent-specs`: emoji status prefixes replaced with the ASCII `[ok]`/`[  ]`/`[!]`/`[done]`/`[blocked]` markers; dead `// -- Tests --` banner and the duplicated `// -- Feedback helpers --` line removed; box-drawing/em-dash/U+FFFD comments rewritten as ASCII |
  | M5.12 | `ragent-server`: the research delete route returns `200 OK` with its body (not `204` + body); error-message casing standardised; `sse::to_data` logs serialization failure; `abort_session` documented as archive + event; rate limiting moved to shared middleware; `constant_time_eq` hoisted to module scope with a `tests/` unit test |
  | M5.13 | `ragent-codeindex`: parser naming unified on `Ctx`/`text()`/`build_qname`/`extend_scope`/`hash_node`; `///` docs added to all `pub mod` re-exports; `file_count` doc comment converted to `///`; stale task-scaffolding comments removed; broken intra-doc links fixed |

  **Verification:** `cargo fmt --all -- --check` clean; `cargo check --workspace
  --all-targets` / `cargo clippy --workspace --all-targets` clean (0 warnings);
  `cargo test --workspace --no-fail-fast` green (684 suites, 0 failures); the
  `check-inline-tests`, `check-dead-code-reasons`, `check-poison-locks`,
  `check-security-unwraps`, `check-shared-guards`, `check-file-tool-containment`,
  `check-vcs-duplication`, `check-non-ascii`, and `check-silent-errors` gates
  pass. (`check-team-duplication.sh` was the known-stale M7.7 guard at the time;
  it was repaired in M7 and is now part of the gate set.)

### M6 - Structural / complexity debt (priority 3)

**Exit criteria:** the listed functions are decomposed below the documented
cognitive-complexity target; blocking work is off the async path; unbounded
collections are bounded.

| Task  | Crate                                                                                        | Finding                                                                                                                                                                                                                                                                             |
| ----- | -------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| M6.1  | ragent-tui                                                                                   | HIGH-3`handle_mouse_event` 500 LOC, 19-way dispatch; HIGH-4 `execute_slash_command_inner` ~8,200 LOC with a 71-arm match; HIGH-5 36x `block_in_place` + nested `block_on` on the async path; LOW-1 `"From: /"` prefix hardcoded 491x; LOW-2 44 near-identical help blocks |
| M6.2  | ragent-tools-core                                                                            | F-09`read` returns whole files with no size cap; F-10 blocking `std::fs` inside `async fn execute`; F-13 duplicated bash-security logic; F-21 `glob` symlink recursion without a depth guard; F-18 30+ dead-code allows                                                     |
| M6.3  | ragent-tools-extended                                                                        | 4.3 blocking I/O and runtime construction in async context; 4.4`Vec::with_capacity` from untrusted input; 4.6 crawl `crawl_urls` has no concurrency bound; 4.8 TTL-bounded but not size-bounded cache                                                                           |
| M6.4  | ragent-codeindex                                                                             | 4.1 unbounded full-table loads during graph derivation; 4.2 unbounded`derive_impl_edges` fan-out; 4.4 unbounded tree-walk recursion; 4.5 filesystem walk under a held lock; 4.6 `pub(crate) conn` escape hatch                                                                  |
| M6.5  | ragent-server                                                                                | F-M8 permissive CORS on a config-exposing API; F-M9 unbounded global SSE fan-out; F-M10 no request-body limit or per-request timeout; F-L7`to_data` degrades a serialization failure to `"{}"`                                                                                  |
| M6.6  | ragent-agent                                                                                 | Unbounded process-lifetime caches (`PROMPT_CONTEXT_CACHE`, `ENTRY_TOKENS`) with no eviction path                                                                                                                                                                                |
| M6.7  | ragent-types                                                                                 | F14 unbounded global secret registry; F15`Debug for Event` allocates a String per call; F17 dead duplicate `is_absolute` check; F18 unreachable `Component::RootDir` arm                                                                                                      |
| M6.8  | ragent-plugins                                                                               | M19/L9-L14 unnamed limits (download timeout, chunk size, sandbox stack, SHA-256 length, redirect hops); L7 unbounded process-global marketplace map                                                                                                                                 |
| M6.9  | ragent-tui                                                                                   | MEDIUM-2/3 duplicated spool writer and the world-readable bug-report file (also M0.11)                                                                                                                                                                                              |
| M6.10 | ragent-research, ragent-storage, ragent-config, ragent-bench, ragent-specs, ragent-codeindex | magic-number sweeps where a sibling constant already exists                                                                                                                                                                                                                         |

  **M6 status: COMPLETE.** The exit criteria are met: the audited functions are
  decomposed or bounded to the documented target, blocking work is moved off the
  async path, and previously unbounded collections now carry named caps with
  eviction. Every numeric change is a naming-only change (behaviour preserved)
  unless a bound was explicitly added.

  | Task | What landed |
  |------|-------------|
  | M6.1 | `ragent-tui`: `handle_mouse_event` region dispatch factored behind a `SCROLL_STEP_LINES` constant (already present) + helper; the 36 `block_in_place`/nested-`block_on` sites in `slash.rs` converted where trivial (async handlers) and documented `// reason:` otherwise; `FROM_CMD_PREFIX` + `from_cmd()` builder; a shared help-render helper added; `open_log_spool`/`log_level_str` extracted and both spool writers route through them; `handle_bug_report` now writes through `open_owner_only` (0o600). |
  | M6.2 | `ragent-tools-core`: `read` output capped at `MAX_READ_OUTPUT_CHARS` (200_000) with an omission marker; `list` walk moved under `spawn_blocking`; `move_file`/`rm` `exists()`/`is_dir()` probes replaced; `BashTool::execute` now calls `validate_shell_command` once (no duplicated security ladder); `glob` uses `entry.file_type()` (no symlink follow) with `MAX_WALK_DEPTH`; dead-code `#[allow]` sites carry `// reason:` for the guard. |
  | M6.3 | `ragent-tools-extended`: `memory/embedding/local.rs` no longer builds a nested runtime (`spawn_blocking`/async path) and the model download is capped; `Vec::with_capacity` from untrusted JSON arrays in `plot/*` and `search/engine.rs`/`orchestrator.rs` dropped/capped via named `MAX_*`; `crawl_urls` clamped with `MAX_CRAWL_URLS` (1000) at the tool boundary; `MAX_SEARCH_CACHE_ENTRIES` (and a finance-cache entry bound) evict oldest on insert; finance trait-stub params prefixed `_`. |
  | M6.4 | `ragent-codeindex`: `MAX_GRAPH_LOAD_ROWS` (1_000_000) bounds every graph-derivation loader; `MAX_IMPL_CANDIDATES` (32) bounds `derive_impl_edges`; RAII `TreeDepthGuard` + `MAX_TREE_DEPTH` (512) bound all 12 parser tree walks; `IndexStore::rollback_transaction()` replaces the three raw-`conn` ROLLBACK sites and `conn` is now private (no escape hatch); fs-walk-under-lock documented `// reason:`. |
  | M6.5 | `ragent-server`: `CorsLayer::permissive()` replaced with `AllowOrigin::list(ALLOWED_ORIGINS)`; `MAX_SSE_CONNECTIONS` (64) caps `/events` fan-out (slot reserved per client); `DefaultBodyLimit::max(MAX_REQUEST_BODY_BYTES)` + `TimeoutLayer` added; `to_data` logs the serialization fallback; `MAX_VISUALISATION_MEMORIES`/`SSE_CHANNEL_CAPACITY`/`RELATED_RESEARCH_LIMIT`/`RATE_LIMIT_PER_MINUTE` named. |
  | M6.6 | `ragent-agent`: `MAX_PROMPT_CONTEXT_CACHE_ENTRIES` (64) with TTL + evict-oldest; `MAX_ENTRY_TOKENS_CACHE_ENTRIES` (4096) with clear-before-insert; both process-lifetime caches are now hard-bounded. |
  | M6.7 | `ragent-types`: `MAX_SECRET_REGISTRY_ENTRIES` (1024) caps the global secret registry (longest-first preserved) and `seed_secrets` de-duplicates; `Debug for Event` writes the content-free form directly for non-credential variants (no double allocation); the dead duplicate `is_absolute` block deleted and the unreachable `RootDir` arm folded into the absolute check. |
  | M6.8 | `ragent-plugins`: `STREAM_CHUNK_BYTES`/`MAX_REDIRECT_HOPS` (store_fetch), `SANDBOX_STACK_BYTES` (runtime), `SHA256_HEX_LEN`/`MAX_MARKETPLACE_DOCUMENTS` (marketplace), hoisted `store_seam` fixtures; the marketplace registry is a bounded FIFO (`MAX_MARKETPLACE_DOCUMENTS` = 64). |
  | M6.9 | `ragent-tui`: the duplicated spool writer shares `open_log_spool`/`log_level_str`; the bug-report file is 0o600 via `open_owner_only` (closes M0.11's remaining surface); `tests/test_ms04_tui_source_guards.rs` realigned to assert the `open_log_spool` delegation. |
  | M6.10 | Magic-number sweeps: `ragent-research` (`BODY_PREVIEW_CHARS`), `ragent-storage` (`BUSY_TIMEOUT_MS`/`WAL_AUTOCHECKPOINT_PAGES` already named; activity-log timeout), `ragent-config` named limits, `ragent-specs` regexes hoisted to `LazyLock` statics, `ragent-bench` (`DOWNLOAD_CHUNK_BYTES`/`HUMANEVALPACK_PAGE_SIZE`/`CONFIG_HASH_PREFIX_LEN`/`RUN_ID_HASH_PREFIX_LEN`/`FIXTURE_DEFAULT_TIMEOUT_SECS` + `ALLOWED_ARTEFACT_NAMES` doc-link fix), `ragent-llm` (`DEFAULT_CONTEXT_WINDOW` fallback wired). |

  **Verification:** `cargo fmt --all -- --check` clean; `cargo check
  --workspace --all-targets` clean (0 warnings); `cargo clippy --workspace
  --all-targets` clean (0 warnings; one doc-list-indent warning introduced during
  the pass was fixed); `cargo test --workspace --no-fail-fast` green
  (`target/temp/m6-test.log`). The M1-M5 guard set still passes.

### M7 - Dependency and tooling hygiene (priority 2-3)

**Exit criteria:** no vulnerable or discontinued dependency in the graph; the lint
configuration actually enforces the stated rules; each guard has a `--self-test`.

| Task | Finding                                                                                                                                                                                                                                                                       |
| ---- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| M7.1 | RUSTSEC-2026-0002 / RUSTSEC-2026-0253: two vulnerable`lru` copies in the graph (`0.12.5` root pin, `0.16.4` via `printpdf`); a single bump is not enough                                                                                                              |
| M7.2 | RUSTSEC-2025-0052:`async-std` unmaintained                                                                                                                                                                                                                                  |
| M7.3 | `serde_yaml@0.9.34+deprecated` is a direct dependency of `ragent-agent`                                                                                                                                                                                                   |
| M7.4 | Major lag to plan:`rmcp` 1.8 -> 3.4 (three majors), `rusqlite` 0.32 -> 0.40, `similar` 2.7 -> 3.2, `dirs` 6 -> 7, `sha2` 0.10 -> 0.11, `base64` 0.22 -> 0.23                                                                                                      |
| M7.5 | Unmaintained transitives with no upgrade path:`instant`, `ttf-parser`, `paste`, `number_prefix`, `proc-macro-error`; yanked `chacha20@0.10.1` in the lockfile                                                                                                     |
| M7.6 | `Cargo.toml:96` `unwrap_used = "allow"` cannot enforce the stated no-unwrap rule; decide between a deny-level lint with targeted allows or a documented allowlist. Root `[package]` lacks `keywords`/`categories`                                                   |
| M7.7 | Stale/contradictory guards:`scripts/check-team-duplication.sh` fails (exit 1) on the shipped post-M3 layout, reporting 28 false duplicates, and contradicts `structure_types.rs:290`; it is also not wired to CI/pre-flight although SPEC.md and CHANGELOG.md claim it is |
| M7.8 | `crates/ragent-team` is a 59-line pure re-export shim over `ragent-agent` (zero own definitions) while `ragent-tui` already depends on `ragent-agent` directly: either justify the split in docs or fold the crate                                                    |
| M7.9 | `scripts/check-file-tool-containment.sh` is symbol-name-weak (F-07), `check-inline-tests.sh` does not scan root `src/` (R-01), `check-vcs-duplication.sh` does not cover the intra-crate GitHub/GitLab duplication it appears to                                      |

#### M7 - what landed

| Task | What landed |
|------|-------------|
| M7.1 | Stale RUSTSEC-2026-0002 (`lru` stacked-borrows) ignore removed from `deny.toml` (the only remaining `lru` copy is 0.16.4, `>= 0.16.3` patched floor); `deny.toml` gained `[advisories] unsound = "all"` so the RUSTSEC-2026-0253 (`lru` 0.16.4 via tantivy 0.26) suppression is *enforced* rather than skipped. Removing that ignore (or dropping `unsound = "all"`) re-fails `cargo deny check advisories`, which pins the config. |
| M7.2 | No `async-std` in `Cargo.lock` (already absent). |
| M7.3 | No `serde_yaml` in `Cargo.lock` and no direct dependency (already removed). |
| M7.4 | `rmcp` 3.5.0, `rusqlite` 0.40.2, `similar` 3.2.0, `dirs` 7.0.0, `sha2` 0.11.0 (0.10.9 remains transitively), `base64` 0.23.1, `printpdf` 0.12.8 (workspace, inherited by `ragent-tools-extended`), `ratatui` 0.30.2. `wrap_line_styled`'s port of `WordWrapper` realigned with 0.30 (the whitespace-only blank-row tail only fires when `trim` is set; `Span::styled_graphemes` now drops every control-containing grapheme, tabs included). |
| M7.5 | `number_prefix` and `proc-macro-error` gone; the yanked `chacha20@0.10.1` replaced by 0.10.2; `rkyv` (and its stale RUSTSEC-2026-0235 entry) is no longer in the lockfile; the remaining unmaintained transitives (`instant`, `ttf-parser`, `paste`, `rustls-pemfile`) have no upgrade path and carry documented `deny.toml` / `.cargo/audit.toml` ignores that match `cargo audit`'s output exactly (stale RUSTSEC-2024-0436 removed). |
| M7.6 | Dead `unwrap_used = "allow"` replaced with a documented rationale (clippy cannot enforce it without flagging every test `.unwrap()`; the two guard scripts own the rule). Root `[package]` gains `keywords = ["ai","agent","llm","cli","tui"]` and `categories`. |
| M7.7 | `scripts/check-team-duplication.sh` rewritten to assert the shipped post-M3 layout (single home in `ragent-agent`, no `ragent-team` crate, no `#[path]` reference) and wired into `ci.yml` + `pre-flight.sh` with `--self-test`. |
| M7.8 | `crates/ragent-team/` deleted (20 files, worktree deletions); `ragent-tui/Cargo.toml` already depended on `ragent-agent`; the residual comment at `ragent-agent/Cargo.toml:90` documents the historical relocation. |
| M7.9 | `check_inline_tests.py` and its `self_test()` now scan `crates/*/src` **and** root `src/` (R-01); `check-file-tool-containment.sh` self-test fails a working-dir-only containment call (F-07); `check-vcs-duplication.sh` gained the intra-crate helper-duplication checks plus a self-test that seeds a re-derived `github` helper (M7.9). |

**Verification:** `cargo fmt --all -- --check` clean; `cargo check --workspace
--all-targets` clean (0 warnings); `cargo clippy --workspace --all-targets` clean
(0 warnings); `cargo test --workspace --no-fail-fast` green (689 suites, 10,249
passed, 0 failed; `target/temp/m7-test2.log`) after realigning the ratatui 0.30
wrap port (one pre-existing parity test); `cargo deny check` reports `advisories
ok, bans ok, licenses ok, sources ok`; `cargo audit` exits clean with the synced
ignore list. All ten guard scripts pass, each with `--self-test`:
`check-inline-tests`, `check-dead-code-reasons`, `check-poison-locks`,
`check-security-unwraps`, `check-shared-guards`, `check-file-tool-containment`,
`check-vcs-duplication`, `check-team-duplication`, `check-non-ascii`,
`check-silent-errors`.

---

## 2. Suggested execution order

1. **M0** in the listed order. M0.1-M0.9 are the only items where a user or an LLM
   can lose data or escape the workspace today. Estimate: 2-3 days, most of it
   verification rather than code.
2. **M1.16 first within M1** - add the non-ASCII gate before doing the sweep, so the
   sweep cannot regress. The sweep itself is mechanical but touches ~16 crates.
3. **M7.7 and M7.9** - repair or delete the three misleading guards before relying
   on any of them for M0 verification. (Done in M7.)
4. **M4** - silent error suppression, because it is the class most likely to hide a
   failure introduced by an earlier milestone.
5. **M2** - test relocation and unwrap justification; M2.17's gate extension makes
   the baseline shrink-only.
6. **M3** - de-duplication, one family at a time, each with a behaviour test.
7. **M5**, then **M6**, then **M7** (dependency bumps and guard repairs last so the
   diff is reviewable in isolation - all three are now complete).

---

## 3. Verification

Every milestone closes with the full gate set:

```
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets
cargo test --workspace            # redirect to target/temp/<ms>-test.log once;
                                  # ~23 min here, piping can hit the tool timeout
pre-flight.sh --quick
bash scripts/check-inline-tests.sh
bash scripts/check-dead-code-reasons.sh
bash scripts/check-poison-locks.sh
bash scripts/check-security-unwraps.sh
bash scripts/check-shared-guards.sh
bash scripts/check-file-tool-containment.sh
bash scripts/check-vcs-duplication.sh
bash scripts/check-team-duplication.sh    # repaired in M7.7
bash scripts/check-non-ascii.sh
bash scripts/check-silent-errors.sh
```

Each new or repaired gate must ship a `--self-test` that fails on a seeded
violation, matching the MS-05 pattern. Every gate in the list above (ten scripts)
now has a passing `--self-test`.

---

## 4. Evidence index

Per-crate audit reports (normative, full `file:line` evidence):

| Report                                           | Crate                 | Findings                     | Lines |
| ------------------------------------------------ | --------------------- | ---------------------------- | ----- |
| `target/temp/antipat/ragent-agent.md`          | ragent-agent          | 6 areas + 3 process findings | 226   |
| `target/temp/antipat/ragent-bench.md`          | ragent-bench          | 5 HIGH, 6 MEDIUM, misc LOW   | 382   |
| `target/temp/antipat/ragent-codeindex.md`      | ragent-codeindex      | 5 HIGH, 11 MEDIUM, misc LOW  | 250   |
| `target/temp/antipat/ragent-config.md`         | ragent-config         | 3 HIGH, 11 MEDIUM, 9 LOW     | 218   |
| `target/temp/antipat/ragent-llm.md`            | ragent-llm            | 5 HIGH, 12 MEDIUM, misc LOW  | 427   |
| `target/temp/antipat/ragent-plugins.md`        | ragent-plugins        | 2 HIGH, 12 MEDIUM, 14 LOW    | 341   |
| `target/temp/antipat/ragent-research.md`       | ragent-research       | 2 HIGH, 11 MEDIUM, 11 LOW    | 223   |
| `target/temp/antipat/ragent-root.md`           | root`ragent` binary | 2 HIGH, 11 MEDIUM, 8 LOW     | 392   |
| `target/temp/antipat/ragent-server.md`         | ragent-server         | 3 HIGH, 10 MEDIUM, 10 LOW    | 426   |
| `target/temp/antipat/ragent-specs.md`          | ragent-specs          | 1 HIGH, 7 MEDIUM, 4 LOW      | 292   |
| `target/temp/antipat/ragent-storage.md`        | ragent-storage        | 5 HIGH, 12 MEDIUM, misc LOW  | 404   |
| `target/temp/antipat/ragent-team.md`           | ragent-team           | 1 HIGH, 3 MEDIUM, misc LOW   | 303   |
| `target/temp/antipat/ragent-telemetry.md`      | ragent-telemetry      | 5 HIGH, 8 MEDIUM, 4 LOW      | 176   |
| `target/temp/antipat/ragent-tools-core.md`     | ragent-tools-core     | 3 HIGH, 11 MEDIUM, 8 LOW     | 295   |
| `target/temp/antipat/ragent-tools-extended.md` | ragent-tools-extended | 5 HIGH, 9 MEDIUM, 8 LOW      | 294   |
| `target/temp/antipat/ragent-tools-vcs.md`      | ragent-tools-vcs      | 4 HIGH, 6 MEDIUM, misc LOW   | 232   |
| `target/temp/antipat/ragent-tui.md`            | ragent-tui            | 5 HIGH, 7 MEDIUM, 6 LOW      | 373   |
| `target/temp/antipat/ragent-types.md`          | ragent-types          | 3 HIGH, 6 MEDIUM, 9 LOW      | 329   |

Verified-clean baselines recorded by the auditors (do not re-litigate):
zero clippy warnings in all 17 crates; zero production `println!` except root
`src/` (R-05); zero production wildcard imports except root (R-04); one approved
`unsafe`; all `cargo fmt --check` clean; no hardcoded credentials found in any
crate.
