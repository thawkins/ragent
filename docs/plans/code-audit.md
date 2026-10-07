# Remediation Plan: Code Audit

**Source**: `docs/reports/code-audit.md`
**Project**: Rust 2024 Cargo workspace (18 crates including `ragent-surface` + root `ragent` binary)
**Scope**: Full project
**Input findings**: 77 total (16 high, 36 medium, 25 low)
**Plan status**: complete - M1, M2, M3, M4, M5, M6, M7, M8, M9 complete

---

## How to use this plan

- Tasks are grouped into **milestones** ordered by the Suggested Fix Order in the audit
  report (security first, cosmetics last). Milestones are largely sequential; tasks
  *within* a milestone are independent unless `depends_on` is set.
- Each task carries an ID (`T-<milestone><nn>`), the finding(s) it closes, the exact
  files, an effort class (**Quick** <30 min, **Medium** 30 min-2 h, **Complex** >2 h),
  and explicit acceptance criteria.
- A task is complete only when its acceptance criteria pass **and** no CI guard script
  regresses (see global verification at the end).

Effort totals (from the audit): 21 quick wins, 36 medium-effort items, 5 complex items.

---

## Milestone M1 - Security hardening (highest blast radius, lowest risk)

**Goal**: Close the `.gitignore` and secret-handling gaps before anything else.
**Exit criteria**: no credential/cert/db-sidecar file can be accidentally committed; every
log and error path that can carry secrets is redacted.

**Status: COMPLETE.** T-101..T-112 implemented and verified (`cargo build --workspace`,
`cargo fmt --check`, targeted `cargo clippy` on changed crates, the six CI guard scripts,
and the new/extended tests all pass).

| ID | Finding(s) | File(s) | Effort | Fix | Acceptance |
|----|-----------|---------|--------|-----|------------|
| T-101 | `.gitignore` missing cert/credential files | `.gitignore:50` | Quick | Add `*.pem`, `*.p12`, `*.pfx`, `service-account.json`, `credentials.json` | `git check-ignore` matches a sample of each; no tracked files newly ignored |
| T-102 | `.gitignore` missing SQLite sidecars | `.gitignore:48` | Quick | Add `*.db-wal`, `*.db-shm` | `git check-ignore x.db-wal` succeeds |
| T-103 | Dead `!.env.example` rule, no template | `.gitignore:22` | Quick | Add committed `.env.example` listing every credential env var ragent reads (no real values) | `.env.example` exists and is tracked; `!.env.example` resolve is no longer dead |
| T-104 | Full request body in debug log | `crates/ragent-llm/src/providers/ollama_cloud.rs:564` | Quick | Route `body_preview` through `redact_secrets` (or drop the body from the log) | Grep confirms no raw `ChatRequest` body in debug output; test asserts redaction |
| T-105 | Raw SSE frame/line in warn log | `crates/ragent-llm/src/providers/ollama.rs:585`, `openai_responses.rs:485` | Quick | Truncate and redact `data`/`line` before logging | Warn sites pass a redacted/truncated value |
| T-106 | Tool-args JSON dump in error log | `crates/ragent-agent/src/tool/team_create.rs:302` | Quick | Log argument **keys** only, never values | Error log line contains no `args_debug` values |
| T-107 | Unredacted provider error bodies (11 providers) | `crates/ragent-llm/src/providers/copilot.rs:485`, `openai.rs:374`, `anthropic.rs:408`, `gemini.rs:576`, `huggingface.rs:561`, `ollama.rs:481`, `ollama_cloud.rs:599`, `azure_foundry.rs:148`, `azure_resource.rs:402`, `openrouter.rs:939`, `bedrock.rs:1210` | Medium | Route every error body through a shared `redact_secrets` before log/`bail!` | A single shared helper is used by all 11 sites; unit test feeds a key-shaped token and asserts it is masked |
| T-108 | Server `/config` hardcoded redaction allow-list | `crates/ragent-server/src/routes/mod.rs:314` | Medium | Redact by config-schema secret flag (or deny-list of secret field names) instead of a literal key list | Adding a new `*_api_key`/`*_token` config field is redacted without editing this file; test covers it |
| T-109 | `get_env` tool redaction is name-substring only | `crates/ragent-tools-core/src/get_env.rs:16`, `:84` | Medium | Deny-list by value shape and/or require explicit opt-in env allow-list | A secret placed in a non-`KEY/SECRET/TOKEN/PASSWORD`-named var is not returned verbatim; test covers |
| T-110 | `resolve_secret` returns unvalidated plain `String` | `crates/ragent-tools-extended/src/channels.rs:111`, `gmail.rs:288` | Medium | Validate shape/type of env-sourced credentials before use | Invalid-shape credential is rejected with a clear error; test covers |
| T-111 | Ad-hoc credential env reads scattered outside config layer | `crates/ragent-agent/src/one_shot.rs:31`, `session/processor.rs:4435`, `session/loop_steps.rs:1720`, `crates/ragent-llm/src/providers/bedrock_credentials.rs:153`, `crates/ragent-tools-vcs/src/gitlab/auth.rs:110`, `crates/ragent-tui/src/app/reverse.rs:173`, `crates/ragent-tui/src/app/models.rs:683` | Medium | Route credential env access through the validated `ragent-config` layer | No direct `std::env::var` of credential vars outside config/credential modules (a documented allow-list may remain for AWS SDK-style reads) |
| T-112 | Per-process credential key path | `crates/ragent-storage/src/storage.rs:162` | Low | Verify the `0600` file write precedes first decrypt use; if not, reorder | Code inspection + test confirms permissions set before first read |

---

## Milestone M2 - Logging hygiene and level correctness

**Goal**: Correct log levels and remove leftover debug artifacts.
**Exit criteria**: no refusal/bookkeeping event logged at the wrong level; no milestone tags
or stray TODOs in shipped messages.

**Status: COMPLETE.** T-201..T-207 implemented and verified (`cargo check` on
`ragent-agent`/`ragent-llm`/`ragent-tui`, `cargo fmt --check`, targeted `cargo clippy`,
the CI guard scripts, and the affected `test_m6_resilience` suite all pass).

| ID | Finding(s) | File(s) | Effort | Fix | Acceptance |
|----|-----------|---------|--------|-----|------------|
| T-201 | Tool denials/blocks logged at `info!` | `crates/ragent-agent/src/session/processor.rs:3232`, `:3243`, `:3194` | Quick | Raise to `warn!` | Event-level assertion in the processor test harness (or grep check) |
| T-202 | Hot-path `info!` per chat request | `crates/ragent-llm/src/providers/copilot.rs:445` | Quick | Lower to `debug!` (match sibling providers) | Grep confirms `debug!` |
| T-203 | `info!` for cache-write bookkeeping | `crates/ragent-llm/src/providers/openai_responses.rs:568` | Quick | Lower to `debug!` | Grep confirms `debug!` |
| T-204 | Leftover milestone/dev tags in log text | `crates/ragent-agent/src/task/mod.rs:1106` (+4), `team/manager.rs:668`, `:733` | Quick | Strip `M7-T3:`/`M6-T2:` prefixes | Grep for `M[0-9]-T[0-9]:` returns none in message strings |
| T-205 | Unresolved TODO in hot HTTP retry path | `crates/ragent-llm/src/providers/http_client.rs:418` | Quick | Resolve, or replace with a tracked-issue reference | No bare `TODO` in the retry path |
| T-206 | Placeholder TODO comments in `/goal` handler | `crates/ragent-tui/src/app/slash.rs:13005`, `:13013`, `:13016`, `:13027` | Quick | Resolve, file a tracked issue, or annotate with a reason | No un-annotated `TODO` remains |
| T-207 | `unreachable!()` on data-driven dispatch | `crates/ragent-tui/src/app/state.rs:3268` | Quick | Restructure match to cover all arms without panic | No `unreachable!()` on the path; clippy clean |

---

## Milestone M3 - Shared utilities and small deduplication

**Goal**: Extract the low-risk shared helpers first; this unblocks the larger provider and
plugins/connectors consolidation and reduces test-helper duplication.
**Exit criteria**: each helper exists once; all call sites import it; behaviour unchanged
(existing tests pass).

**Status: COMPLETE.** T-301..T-309 implemented and verified (`cargo check --workspace`,
`cargo fmt --check`, targeted `cargo clippy`, the affected crate test suites, and the CI guard
scripts all pass). Each extracted helper has a single definition plus a new integration test:
`ragent_types::strutil::format_size` (T-301), `ragent_tools_extended::masterfetch::urlnorm::
extract_domain` (T-302), the shared `ragent_types::strutil::truncate_chars` reused by
`loop_state`/`trigger` (T-303), `ragent_agent::tool::team_memory_common` (T-304),
`ragent_tools_core::jsonl_log` (T-305), `ragent_config::list_config` (T-306),
`ragent_llm::providers::ollama_shared` (T-307), `ragent_tools_vcs::vcs_error` (T-308), and
`ragent_agent::mcp::probe::connect_and_list` (T-309).

| ID | Finding(s) | File(s) | Effort | Fix | Acceptance |
|----|-----------|---------|--------|-----|------------|
| T-301 | Duplicated `format_size` | `crates/ragent-agent/src/reference/resolve.rs:388`, `crates/ragent-tools-core/src/list.rs:183` | Medium | Move to a shared `ragent-types` helper | One definition; both call sites import it; tests pass |
| T-302 | Duplicated `extract_domain` | `crates/ragent-tools-extended/src/masterfetch/crawl/orchestrator.rs:495`, `robots.rs:733` | Medium | Unify into one helper (settle on `Option` or `Result`) | One definition + unit test; both call sites updated |
| T-303 | Duplicated `truncate_chars` | `crates/ragent-agent/src/loop_state.rs:417`, `crates/ragent-types/src/trigger.rs:231` | Medium | Move to `ragent-types` | One definition + unit test |
| T-304 | Duplicated `path_tag` within one crate | `crates/ragent-agent/src/tool/team_memory_read.rs:161`, `team_memory_write.rs:192` | Medium | Extract to shared helper in the same crate | One definition |
| T-305 | Duplicated JSONL log helpers | `crates/ragent-tools-core/src/edit_log.rs:151`, `:177`, `cron_log.rs:89`, `:116` | Medium | Extract a shared JSONL module | One `pick_log_file`/`append_json_line` pair |
| T-306 | Duplicated `Scope`/`config_path`/lock accessors | `crates/ragent-config/src/bash_lists.rs:68`, `:80`, `:134`, `dir_lists.rs:248`, `:262`, `:344` | Medium | Extract shared list-config helper | One implementation; both list modules delegate |
| T-307 | Duplicated `estimate_context_window` | `crates/ragent-llm/src/providers/ollama.rs:163`, `ollama_cloud.rs:226` | Medium | Share one implementation | One definition + test |
| T-308 | Duplicated `handle_response` error classifier | `crates/ragent-tools-vcs/src/github/client.rs:219`, `gitlab/client.rs:146` | Medium | Extract shared VCS error classifier (429/403/401 shape) | One classifier + tests for each status |
| T-309 | Duplicated `McpProbe` impl | `src/connectors.rs:318`, `crates/ragent-tui/src/app/connector.rs:460` | Medium | Extract shared `probe_connect`/`probe_call` helper | One impl; CLI + TUI both use it |

---

## Milestone M4 - Provider consolidation

**Goal**: Collapse the OpenAI-compatible provider duplication and the env-key resolver.
**Exit criteria**: one request-body builder, one SSE parser, one env-key resolver; provider
tests cover each provider that delegates to the shared code.

**Status: COMPLETE.** T-401..T-403 implemented and verified (`cargo build -p ragent-llm`,
`cargo fmt --check`, `cargo clippy -p ragent-llm`, the `ragent-llm` test suite, and the CI guard
scripts all pass). The three shared modules are live and every call site delegates:
`ragent_llm::provider::openai_compat` (T-401) is used by `openai`, `openrouter` (via `OpenAiClient`
for `generic_openai` and `xai`), `copilot`, `ollama`, and `ollama_cloud`;
`ragent_llm::provider::sse` (T-402) is used by `openai`/`openrouter`;
`ragent_llm::provider::env_key` (T-403) is used by the session processor, one-shot dispatch,
research adapter, and router client. New request-shape coverage in
`tests/test_openai_compat_builder.rs` (7 tests) plus `tests/test_env_key.rs` (2 tests) pins the
shared OpenAI dialect and each provider divergence.

| ID | Finding(s) | File(s) | Effort | Fix | Acceptance |
|----|-----------|---------|--------|-----|------------|
| T-401 | Duplicated OpenAI request-body builder (6 files) | `crates/ragent-llm/src/providers/openai.rs:204`, `openrouter.rs:458`, `generic_openai.rs` (via `OpenAiClient`), `copilot.rs:303`, `ollama.rs:264`, `ollama_cloud.rs:341` | Medium | Extract a shared `openai_compat` builder | One builder; 6 providers delegate; per-provider request-shape tests pass |
| T-402 | Duplicated OpenAI SSE stream parser | `crates/ragent-llm/src/providers/openai.rs:387`, `openrouter.rs:590` | Medium | Extract a shared SSE parser | One parser; both providers delegate; streaming tests pass |
| T-403 | Duplicated provider→env-key resolution (4 sites) | `crates/ragent-agent/src/session/processor.rs:4426`, `one_shot.rs:26`, `research_adapter.rs:214`, `crates/ragent-llm/src/providers/router_client.rs:551`, `:578` | Medium | Single resolver in `ragent-llm` | One resolver + unit test; all call sites delegate |

**Dependency**: T-403 overlaps T-111 (security); land T-403 first, then simplify T-111.

---

## Milestone M5 - Dead code and stale-duplicate removal

**Goal**: Delete unreferenced and duplicated code rather than refactor it.
**Exit criteria**: `cargo build`/`clippy` clean with no new dead-code allow reasons; removed
code has no live references.

**Status: COMPLETE.** T-501..T-505 implemented and verified (`cargo check --workspace --all-features`,
`cargo check --workspace --tests`, `cargo clippy --workspace -- -D warnings`, `cargo fmt --check`,
the `ragent-connectors`/`ragent-plugins`/`ragent-tools-core`/`ragent-agent`/`ragent-tui` test
suites, and the ten CI guard scripts all pass). The stale `ragent-agent::snapshot` module is
deleted (live code uses `ragent_storage::snapshot`); the duplicated inline schema suite is
removed (the external `test_schema_validation.rs` is the single source); the placeholder
`test_precompiled_regexes.rs` bodies now assert on concrete values; the dead `NeverProbe`
(CLI) and `NoProbe` (TUI) fallbacks and the unreachable `test` arm of
`run_connector_subcommand_env` are gone; and the new `ragent-surface` crate owns the one
implementation of `sample_for_schema`, `schema_type`, `truncate`, `step`, `attribution`,
`subcommand_of`, and `store_dirs*`/`store_entry_is_dir`, with `ragent-plugins` and
`ragent-connectors` delegating.

| ID | Finding(s) | File(s) | Effort | Fix | Acceptance |
|----|-----------|---------|--------|-----|------------|
| T-501 | Stale agent `snapshot` duplicate | `crates/ragent-agent/src/snapshot/mod.rs:1`, `crates/ragent-storage/src/snapshot.rs:1` | Medium | Delete the agent copy; keep `ragent-storage::snapshot` (fix the doc-link in `crates/ragent-telemetry/src/recorder.rs:1156`) | No references to `ragent_agent::snapshot`; build clean |
| T-502 | Duplicated schema test suite | `crates/ragent-tools-core/tests/inline/schema_tests.rs:9`, `tests/test_schema_validation.rs:1` | Quick | Delete one copy; keep a single source | The 7 tests run once; test count drops accordingly |
| T-503 | Empty/placeholder tests | `crates/ragent-agent/tests/test_precompiled_regexes.rs:15`, `:40` | Quick | Add real assertions or delete | No test body without an assertion |
| T-504 | Pluggable `McpProbe` dead fallback (`NeverProbe`) | `src/connectors.rs:371` | Low | Remove if unused after T-309 | No live reference |
| T-505 | `/plugins` and `/connectors` parallel harness/help/store code | `crates/ragent-plugins/src/harness.rs:292`, `help.rs:59`, `store.rs:55`; `crates/ragent-connectors/src/harness.rs:430`, `help.rs:98`, `store.rs:77` | Medium | Extract a shared harness/help/store module (or shared crate) | One implementation of `sample_for_schema`, `schema_type`, `truncate`, `step`, `attribution`, `subcommand_of`, `store_dirs*`; both crates delegate; tests pass |

---

## Milestone M6 - Test infrastructure and hygiene

**Goal**: Fix test-hygiene violations and remove fragile/weak test patterns.
**Exit criteria**: tests no longer use `/tmp`; no live-network test runs by default; shared
helpers replace copy-paste setup.

**Status: COMPLETE.** T-601..T-608 implemented and verified (`cargo check --tests` across the
affected crates, `cargo fmt --check`, targeted `cargo test` on the changed suites, and the eight
CI guard scripts all pass). Test scratch paths now use `target/temp` (31 sites across 24 files);
`ragent-plugins` and `ragent-connectors` each have one `tests/support` `TempTree` (replacing 18
and 10 copy-pasted definitions) matching the `ragent-llm`/`ragent-tui` pattern; the diagnostic
`test_mf_orchestrator_diag`/`test_fts_diag` tests and the live-network `test_ollama_cloud_real`
tests are `#[ignore]`-gated; the three per-key config test files collapsed to the table-driven
`test_api_key_config_fields.rs`; the three scoreboard tests share `tests/support/scoreboard_fixture.rs`;
and the weak `.is_some()`/presence-only assertions bind and compare concrete values.

| ID | Finding(s) | File(s) | Effort | Fix | Acceptance |
|----|-----------|---------|--------|-----|------------|
| T-601 | Tests use `/tmp` instead of `target/temp` (24 sites) | `crates/ragent-tools-core/tests/test_think.rs:21`, `crates/ragent-agent/tests/test_conversation_search.rs:14` (+22) | Quick | Switch to `target/temp` | Grep for `/tmp` in tests returns none (outside documented exceptions) |
| T-602 | Duplicated `TempTree` sandbox helper (27 files) | `crates/ragent-plugins/tests/test_add.rs:13` (+16 in plugins, +10 in connectors) | Medium | Shared `tests/support` module per crate (match `llm`/`tui` pattern) | One `TempTree` definition per crate; all test files import it |
| T-603 | Diagnostic/generator scripts as tests | `crates/ragent-agent/tests/test_mf_orchestrator_diag.rs:1`, `dump_registries.rs:37` | Medium | Convert to `#[ignore]` or move to scripts | Default `cargo test` does not run them; no live search in CI |
| T-604 | Live-network test not `#[ignore]`d | `crates/ragent-llm/tests/test_ollama_cloud_real.rs:10` | Quick | Add `#[ignore]` gate (or feature gate) | Default test run never hits the network |
| T-605 | Near-identical config test triples | `crates/ragent-config/tests/test_serper_api_key.rs:14`, `test_perplexity_api_key.rs:14`, `test_langsearch_api_key.rs:14` | Medium | Table-driven test over the three fields | One parameterised test replaces three |
| T-606 | Research scoreboard test duplication | `crates/ragent-research/tests/test_scoreboard_report.rs:108`, `test_scoreboard_imrad.rs:111`, `test_scoreboard_reductions.rs:144` | Low | Shared `ResearchDocument` fixture builder | One fixture helper used by all three |
| T-607 | Weak `.is_some()`-only assertions | `crates/ragent-agent/src/tests/inline/runtime_tests.rs:22`, `tests/test_trigger_runtime.rs`, `crates/ragent-tools-extended/tests/test_archdoc_extract.rs` | Medium | Assert on returned values, not just presence | Assertions compare concrete values |
| T-608 | Test file naming convention | `crates/ragent-agent/tests/dump_registries.rs`, `session_processor.rs`, `crates/ragent-research/tests/source_vault.rs`, `crates/ragent-types/tests/structure_types.rs` | Low | Rename to `test_<component>_<scenario>` | Names match the convention |

---

## Milestone M7 - New test coverage

**Goal**: Cover the highest-risk untested modules.
**Exit criteria**: each named module has at least one test exercising its core logic;
coverage additions do not introduce inline `#[cfg(test)]` modules (tests live in `tests/`).

**Status: COMPLETE.** T-701..T-705 implemented and verified (`cargo test` on the five
new suites, `cargo fmt --check`, targeted `cargo clippy` on the changed crates, and the
eight CI guard scripts all pass; no inline `#[cfg(test)]` module was added). New external
suites: `crates/ragent-tools-core/tests/test_calculator.rs` (24 tests),
`test_agent_complete_tool.rs`, `test_bash_reset_tool.rs`, `test_xlsx.rs`;
`crates/ragent-llm/tests/test_azure_foundry_provider.rs` (5 tests);
`crates/ragent-agent/tests/test_loop_steps_harness.rs` (3 tests) and
`test_orchestrator_coordinator.rs` (13 tests). Fixing T-705 also corrected a real
production bug: `Coordinator` incremented `active_jobs` *after* spawning a job (twice,
and never for the synchronous paths), so the counter underflowed to `u64::MAX`; the
`ActiveJobsGuard::enter` constructor now performs the increment exactly once, before the
guard is installed. `finalize_assistant_message` (marked `#[allow(dead_code)]`) is
exercised in effect through the public `process_message` loop, which is the only path an
external test crate can reach.

| ID | Finding(s) | File(s) | Effort | Fix | Acceptance |
|----|-----------|---------|--------|-----|------------|
| T-701 | `CalculatorTool` untested (417 lines, parser) | `crates/ragent-tools-core/src/calculator.rs:1` | Medium | Add `tests/` coverage: valid expressions, precedence, divide-by-zero, malformed input | Tests cover happy path + error paths |
| T-702 | Small tools untested | `crates/ragent-tools-core/src/get_env.rs:1`, `agent_complete.rs:1`, `bash_reset.rs:1`, `xlsx.rs:1` | Medium | Add per-tool tests in `crates/ragent-tools-core/tests/` | Each tool has >=1 test |
| T-703 | Azure AI Foundry provider untested | `crates/ragent-llm/src/providers/azure_foundry.rs:1` | Medium | Add tests: request build, auth, model discovery | Tests cover the 3 responsibilities |
| T-704 | Agent-loop step logic untested (1739 lines) | `crates/ragent-agent/src/session/loop_steps.rs:132` | Complex | Build a fake-provider harness; integration-test `prepare_client`, `call_llm_step`, `finalize_assistant_message` | Deterministic loop test through a fake provider |
| T-705 | Orchestrator `Coordinator` untested | `crates/ragent-agent/src/orchestrator/coordinator.rs:211` | Complex | Test harness for `start_job_sync`, `start_job_first_success`, `start_job_async`, metrics | Job spawn / first-success / metrics asserted |

---

## Milestone M8 - Standards and cosmetic cleanup

**Goal**: Bring code into line with AGENTS.md / AGENTS-RUST.md conventions.
**Exit criteria**: naming/doc/emoji rules satisfied; no `unwrap()` on user-facing paths.

**Status: COMPLETE.** T-801..T-811 implemented and verified (`cargo check --workspace --tests`,
`cargo fmt --check`, targeted `cargo clippy` on the changed crates, the `ragent-tui`
`test_codeindex_backward_compat` and `ragent-research` `tier_router` suites, and the eight CI
guard scripts all pass). GitHub/GitLab acronym casing is standardised on the canonical form across
`ragent-tools-vcs` (all 31 tool types plus the shared helpers); the three user-facing/external-input
unwraps are gone (`extractor.rs` quoting, `list_agents.rs` status filter, `chapter.rs` chapter title);
emoji and box-drawing glyphs are stripped from production-source comments workspace-wide; the six
named modules carry `//!` headers; the `MultiPlEAdapter` -> `MultipleAdapter` rename is applied; the
`list_agents` `TaskStatus` path is hoisted into a `use`; the four `Regex::new(...)` sites in
`slash.rs` use `.expect("valid ... regex")`; the `CollectingTierRouterObserver` test double is a
real `pub` type in `tier_router.rs`; the orphaned `ragent-agent/src/memory/embedding/local.rs`
wrapper is deleted (the module-file layout rule - named-file parents, `mod.rs` only for
legacy/deeply-nested - is confirmed already applied across the workspace); and the
`test_codeindex_backward_compat` suite asserts against the `SLASH_COMMANDS` data model instead of
scraping `src/app/slash.rs` / `src/app/state.rs`.

| ID | Finding(s) | File(s) | Effort | Fix | Acceptance |
|----|-----------|---------|--------|-----|------------|
| T-801 | Acronym casing mismatch (`GitHub`/`GitLab` vs `Github`/`Gitlab`) | `crates/ragent-tools-vcs/src/github/client.rs:86`, `gitlab/client.rs:14`, `github_issues.rs:232`, `gitlab_issues.rs:398` | Medium | Standardise on one form across both crates | All GitHub/GitLab type names consistent |
| T-802 | `.unwrap()` on HTML parser input | `crates/ragent-tools-extended/src/masterfetch/extractor.rs:905` | Medium | Guard or use `?` (add `// no-panic-ok` only if proven safe) | No bare unwrap on external input |
| T-803 | `.unwrap()` on user-facing paths | `crates/ragent-agent/src/tool/list_agents.rs:116`, `crates/ragent-research/src/chapter.rs:123` | Medium | Replace with `if let`/`ok_or` propagation | No unwrap on these paths |
| T-804 | Emoji in source comments (~216 lines) | `crates/ragent-tui/src/widgets/message_widget.rs:278` (+215) | Medium | Strip to ASCII per AGENTS.md (keep plain ASCII glyphs where UI needs them) | No non-ASCII in comments/identifiers |
| T-805 | Missing module doc comments | `crates/ragent-agent/src/orchestrator/registry.rs:1`, `router.rs:1`, `coordinator.rs:1`, `file_ops/api.rs:1`, `file_ops/wrapper.rs:1`, `crates/ragent-types/src/sanitize.rs:1` | Medium | Add `//!` headers | Each module has a `//!` doc block |
| T-806 | `MultiPlEAdapter` naming | `crates/ragent-bench/src/suites/multipl_e.rs:14` | Quick | Rename to a conventional form (e.g. `MultipleAdapter`) | Name updated at all references |
| T-807 | Inline fully-qualified paths | `crates/ragent-agent/src/tool/list_agents.rs:100`, `:128` | Quick | Hoist `crate::task::TaskStatus` into a `use` | Imported, not inlined |
| T-808 | `Regex::new(...).unwrap()` vs `.expect(...)` convention | `crates/ragent-tui/src/app/slash.rs:12763`, `:12767`, `:12772`, `:12778` | Quick | Convert to `.expect("valid ... regex")` | Matches convention |
| T-809 | `#[cfg(test)] impl` in production source | `crates/ragent-research/src/tier_router.rs:43` | Low | Move the test impl to `tests/` | No `#[cfg(test)]` impl in `src/` |
| T-810 | Module-file layout inconsistency | `crates/ragent-agent/src/memory/embedding.rs`, `crates/ragent-tui/src/app.rs` | Complex | Decide `mod.rs` vs named-file policy and apply consistently (document the rule) | Layout policy documented and applied |
| T-811 | Source-scraping test fragility (74 test files) | `crates/ragent-tui/tests/test_codeindex_backward_compat.rs:40` (+ `test_slash_commands.rs`) | Complex | Replace substring scraping of `src/app/slash.rs` with behavioural assertions | Tests no longer read source files |

---

## Milestone M9 - Dependency upkeep

**Goal**: Remove duplicate major lines, clear the yanked crate, and plan the large migrations.
**Exit criteria**: `cargo deny check` duplicate warnings reduced; `cargo audit` reports no
yanked crates; large migrations tracked as separate follow-ups.

**Status: COMPLETE.** T-901..T-908 implemented and verified (`cargo check --workspace`,
`cargo audit` reports no yanked crate, and `cargo fmt --check` passes). The workspace converges
on `thiserror 2`, `reqwest 0.13` (the `ragent-llm` pin now uses `workspace = true` rather than
pinning `0.12` directly), `rand 0.10`, and `criterion 0.8`; `ragent-storage` moves off its
standalone `rand 0.8`; `lopdf` converges on `0.44` and the dedicated `vendor/lopdf` crate is
deleted (the `MAX_OBJECT_DEPTH` recursion guard is no longer required because `lopdf` 0.44 bounds
object depth natively); `opentelemetry`/`opentelemetry_sdk`/`opentelemetry-otlp` migrate 0.29 ->
0.33 (the Prometheus scrape endpoint enables the new `experimental_metrics_custom_reader` feature);
`notify` 7 -> 8.2, `rquickjs` 0.10 -> 0.14, `tree-sitter` 0.26 -> 0.27, `chacha20poly1305`
0.10 -> 0.11, and `which` 7 -> 8 are applied. The residual `thiserror 1.0.69` lock entry is a
stale transitive of a build-dependency edge not exercised on the default target and does not
appear in the `cargo tree` dependency graph.

| ID | Finding(s) | File(s) | Effort | Fix | Acceptance |
|----|-----------|---------|--------|-----|------------|
| T-901 | Yanked `yoke-derive@0.8.3` in committed lockfile | `Cargo.lock` | Medium | `cargo update -p yoke-derive` to a non-yanked release | `cargo audit` reports no yanked crate |
| T-902 | Duplicate `thiserror` 1.x + 2.x | `thiserror@1.0.69` in 5 crates vs workspace `2` | Medium | Align all crates to `thiserror 2` via `workspace = true` | One `thiserror` major in the lockfile |
| T-903 | Duplicate `reqwest` 0.12 + 0.13; direct pin in `ragent-llm` bypasses workspace | `reqwest@0.12.28`, `reqwest@0.13.5`, `crates/ragent-llm/Cargo.toml` | Medium | Use `workspace = true`; converge on one major | One `reqwest` major in the lockfile |
| T-904 | Duplicate `rand` 0.8/0.9/0.10; `ragent-storage` on 0.8 (2 majors behind) | `rand@0.8.8`, `rand@0.9.5`, `rand@0.10.3`, `crates/ragent-storage/Cargo.toml` | Medium | Upgrade `ragent-storage` to `0.10`; converge majors | One `rand` major in the lockfile |
| T-905 | Duplicate `lopdf` 0.39 direct + 0.44 transitive (+ vendored 0.38) | `Cargo.toml`, `vendor/` | Complex | Coordinate `ooxmlsdk`/`quick-xml` upgrade; unblocks RUSTSEC suppressions | Single `lopdf` line; `deny.toml` suppressions removable |
| T-906 | `opentelemetry@0.29.1` (4 releases behind) | `crates/ragent-telemetry/Cargo.toml` | Medium | Plan and execute the 0.33 migration (OTEL API/field changes) | Telemetry builds + exports on the new line |
| T-907 | Individual outdated deps (one major / 0.x) | `notify@7.0.0`, `rquickjs@0.10.0`, `which@7.0.3`, `chacha20poly1305@0.10.1`, `tree-sitter@0.26.13` | Medium | Upgrade each (verify API deltas) | Each crate builds + tests pass |
| T-908 | Patch-level bumps | `tokio@1.53.1 -> 1.53.2`, `criterion@0.5.1 -> 0.8.2` (dev, low) | Quick | Bump patch/dev versions | `cargo update` clean; build passes |

---

## Cross-cutting dependencies

- T-403 (env-key resolver) **before** T-111 (env reads) - collapse the resolver first, then
  route remaining reads through config.
- T-309 (shared `McpProbe`) **before** T-504 (remove `NeverProbe`).
- T-501 (delete stale snapshot) is independent but should precede any snapshot test work.
- T-401/T-402 (provider consolidation) must land **together** with provider tests (T-703,
  and any existing provider tests) to avoid a window where request/stream shapes are untested.

---

## Suggested execution order

1. **M1 Security** - highest blast radius, lowest risk (gitignore + redaction).
2. **M2 Logging** - one-line level/tag fixes.
3. **M3 Shared utilities** - unblocks larger dedup; reduces test-helper copy-paste.
4. **M4 Provider consolidation** - large surface, land with provider tests.
5. **M5 Dead-code removal** - delete stale duplicates and duplicated test suite.
6. **M6 Test infrastructure** - `target/temp`, ignore-gates, shared helpers, strong asserts.
7. **M7 New coverage** - simple tools → Azure → complex loop/orchestrator harnesses.
8. **M8 Standards/cosmetic** - naming, docs, emoji, layout policy, source-scraping tests.
9. **M9 Dependencies** - dedup, yanked clear, then the `opentelemetry`/`lopdf` migrations.

---

## Global verification checklist

- [ ] `cargo build --workspace` reports 0 errors
- [ ] `cargo clippy --workspace` reports 0 warnings
- [ ] `cargo fmt --check` passes on every edited `.rs` file
- [ ] `cargo test --workspace` passes; new tests cover previously-untested files (T-701..T-705)
- [ ] `cargo deny check advisories bans licenses sources` remains `ok`
- [ ] `cargo audit` reports 0 vulnerabilities and no yanked crates
- [ ] CI guard scripts pass: `scripts/check-inline-tests.sh`, `check-poison-locks.sh`,
      `check-security-unwraps.sh`, `check-silent-errors.sh`, `check-dead-code-reasons.sh`,
      `check-team-duplication.sh`, `check-vcs-duplication.sh`, `check-file-tool-containment.sh`
- [ ] No new `println!`/`eprintln!`/`dbg!` outside the documented CLI/panic surface
- [ ] No new `#[cfg(test)]` module added to `src/`
