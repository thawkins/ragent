# Code Audit Report

**Project**: Rust 2024 Cargo workspace (17 crates + root `ragent` binary)
**Scope**: Full project (excludes `target/`, `vendor/`, `.git/`, `node_modules/`, `dist/`, `build/`, `log/`)
**Findings**: 77 total (16 high, 36 medium, 25 low)

---

## High Priority

### Standards
- Inconsistent acronym casing for the same product within one crate: `GitHubClient`/`GitLabClient` vs `Github*`/`Gitlab*` tool types — `crates/ragent-tools-vcs/src/github/client.rs:86`, `crates/ragent-tools-vcs/src/gitlab/client.rs:14` (and `github_issues.rs:232`, `gitlab_issues.rs:398`)

### Duplication
- Two near-identical full `snapshot` modules (~326 vs ~342 lines); the agent copy is unreferenced and stale (live code uses `ragent_storage::snapshot`) — `crates/ragent-agent/src/snapshot/mod.rs:1` (and `crates/ragent-storage/src/snapshot.rs:1`)
- `build_request_body` for OpenAI-compatible providers is the same ~130-line body copied across six files — `crates/ragent-llm/src/providers/openai.rs:204`, `openrouter.rs:458`, `generic_openai.rs`, `copilot.rs:303`, `ollama.rs:264`, `ollama_cloud.rs:341`
- OpenAI SSE stream parser duplicated — `crates/ragent-llm/src/providers/openai.rs:387` (and `crates/ragent-llm/src/providers/openrouter.rs:590`)
- Provider→env-var API key resolution table duplicated in four places with near-identical match arms — `crates/ragent-agent/src/session/processor.rs:4426`, `crates/ragent-agent/src/one_shot.rs:26`, `crates/ragent-agent/src/research_adapter.rs:214`, `crates/ragent-llm/src/providers/router_client.rs:551`/`:578`

### Security
- `.gitignore` does not ignore certificate/credential files: `*.pem`, `*.p12`, `*.pfx`, `service-account.json`, `credentials.json` — `.gitignore:50`
- `.gitignore` ignores `*.db` but not the SQLite sidecars `*.db-wal`/`*.db-shm`, which can hold plaintext credential-store and session-history pages — `.gitignore:48`

### Logging
- Debug log emits up to 800 bytes of the raw request body (full chat messages / user prompt content) unredacted — `crates/ragent-llm/src/providers/ollama_cloud.rs:564`

### Testing
- No test coverage anywhere for the Azure AI Foundry provider client (273 lines of request-building/auth/model-discovery) — `crates/ragent-llm/src/providers/azure_foundry.rs:1`
- Core agent-loop step logic untested (1739 lines) — `crates/ragent-agent/src/session/loop_steps.rs:132`
- Orchestrator `Coordinator` public API (job spawning, first-success policy, metrics) has zero tests — `crates/ragent-agent/src/orchestrator/coordinator.rs:211`
- `CalculatorTool` (417 lines, expression parser) has no test — `crates/ragent-tools-core/src/calculator.rs:1`
- Small registered tools with no coverage: `get_env.rs:1`, `agent_complete.rs:1`, `bash_reset.rs:1`, `xlsx.rs:1` — `crates/ragent-tools-core/src/`
- Duplicated schema test suite (byte-identical bodies compiled and run twice) — `crates/ragent-tools-core/tests/inline/schema_tests.rs:9` (and `crates/ragent-tools-core/tests/test_schema_validation.rs:1`)
- Tests that assert nothing (empty/placeholder bodies) — `crates/ragent-agent/tests/test_precompiled_regexes.rs:15`, `:40`
- Source-scraping fragility: reads `src/app/slash.rs` and asserts on exact match-arm/help-text substrings — `crates/ragent-tui/tests/test_codeindex_backward_compat.rs:40`
- Non-`#[ignore]`d live-network test calls the real Ollama Cloud API in CI — `crates/ragent-llm/tests/test_ollama_cloud_real.rs:10`

## Medium Priority

### Standards
- Bare `.unwrap()` on externally-derived HTML-parser input, no `// no-panic-ok`, unguarded against malformed markup — `crates/ragent-tools-extended/src/masterfetch/extractor.rs:905`
- `.unwrap()` on a user-facing path guarded only by `is_some()` — `crates/ragent-agent/src/tool/list_agents.rs:116`
- `.unwrap()` on `Vec::last()` of model-produced text in research synthesis — `crates/ragent-research/src/chapter.rs:123`
- Non-ASCII emoji in source comments violates the no-Unicode/no-emoji rule (~216 lines) — `crates/ragent-tui/src/widgets/message_widget.rs:278`
- Acronym capitalization inside a type name — `crates/ragent-bench/src/suites/multipl_e.rs:14`
- Missing `//!` module doc comment, inconsistent with the rest of the workspace — `crates/ragent-agent/src/orchestrator/registry.rs:1`, `orchestrator/router.rs:1`, `orchestrator/coordinator.rs:1`, `crates/ragent-agent/src/file_ops/api.rs:1`, `file_ops/wrapper.rs:1`, `crates/ragent-types/src/sanitize.rs:1`
- Inconsistent module-file layout: mostly `mod.rs` (45 sites) but `memory/embedding.rs`+`memory/embedding/` and `app.rs`+`app/` mix named-file style — `crates/ragent-agent/src/memory/embedding.rs:1`, `crates/ragent-tui/src/app.rs:1`

### Duplication
- `McpProbe` impl (`probe_connect` + `probe_call` bodies byte-identical) copied across CLI and TUI surfaces — `src/connectors.rs:318` (and `crates/ragent-tui/src/app/connector.rs:460`)
- `/plugins` and `/connectors` crates carry parallel copies of the same code with no shared crate — `crates/ragent-plugins/src/harness.rs:292` (and `crates/ragent-connectors/src/harness.rs:430`), plus `help.rs:59`/`:98`
- Store-path and ledger logic duplicated — `crates/ragent-plugins/src/store.rs:55` (and `crates/ragent-connectors/src/store.rs:77`)
- `estimate_context_window` identical (same heuristic and 7.0 fallback) — `crates/ragent-llm/src/providers/ollama.rs:163` (and `crates/ragent-llm/src/providers/ollama_cloud.rs:226`)
- `path_tag` identical within one crate — `crates/ragent-agent/src/tool/team_memory_read.rs:161` (and `team_memory_write.rs:192`)
- JSONL log helper pair `pick_log_file`/`append_json_line` duplicated verbatim — `crates/ragent-tools-core/src/edit_log.rs:151` (and `crates/ragent-tools-core/src/cron_log.rs:89`)
- `truncate_chars` duplicated — `crates/ragent-agent/src/loop_state.rs:417` (and `crates/ragent-types/src/trigger.rs:231`)
- `handle_response` error-classification block duplicated with the same 429/403/401 shape — `crates/ragent-tools-vcs/src/github/client.rs:219` (and `crates/ragent-tools-vcs/src/gitlab/client.rs:146`)
- `Scope` enum + `config_path` + lock-poisoning accessors duplicated — `crates/ragent-config/src/bash_lists.rs:68` (and `crates/ragent-config/src/dir_lists.rs:248`)

### Logging
- Error log includes the raw provider API error body without redaction; a provider echoing credentials would leak it — `crates/ragent-llm/src/providers/copilot.rs:485` (and unredacted `bail!` at `openai.rs:374`, `anthropic.rs:408`, `gemini.rs:576`, `huggingface.rs:561`, `ollama.rs:481`, `ollama_cloud.rs:599`, `azure_foundry.rs:148`, `azure_resource.rs:402`, `openrouter.rs:939`, `bedrock.rs:1210`)
- Warn logs the raw SSE frame/line content, which carries streamed model output — `crates/ragent-llm/src/providers/ollama.rs:585`, `crates/ragent-llm/src/providers/openai_responses.rs:485`
- Error log emits the full JSON tool-argument dump (`args_debug`); tool args are attacker-supplied and may carry secrets — `crates/ragent-agent/src/tool/team_create.rs:302`
- Per-request `tracing::info!` on the hot chat path should be `debug!` — `crates/ragent-llm/src/providers/copilot.rs:445`
- Tool denials/blocks logged at `info!`; refusal events belong at `warn!` — `crates/ragent-agent/src/session/processor.rs:3232`, `:3243`, `:3194`

### Security
- Ad-hoc `std::env::var` reads of credential-bearing vars scattered outside the validated config layer — `crates/ragent-agent/src/one_shot.rs:31`, `crates/ragent-agent/src/session/processor.rs:4435`, `crates/ragent-agent/src/session/loop_steps.rs:1720`, `crates/ragent-llm/src/providers/bedrock_credentials.rs:153`, `crates/ragent-tools-vcs/src/gitlab/auth.rs:110`, `crates/ragent-tui/src/app/reverse.rs:173`, `crates/ragent-tui/src/app/models.rs:683`
- `get_env` tool reads arbitrary process environment directly; redaction is name-substring only, so a secret in a differently-named var is returned verbatim to the model — `crates/ragent-tools-core/src/get_env.rs:16`, `:84`
- `resolve_secret` returns env-sourced channel/Gmail credentials as plain `String`, passed straight into config with no shape/type validation — `crates/ragent-tools-extended/src/channels.rs:111`, `crates/ragent-tools-extended/src/gmail.rs:288`

### Testing
- Diagnostic/generator scripts living as tests with no assertions (runs by default and touches live search) — `crates/ragent-agent/tests/test_mf_orchestrator_diag.rs:1`, `crates/ragent-agent/tests/dump_registries.rs:37`
- Duplicated sandbox helper with no shared module: `struct TempTree` defined independently in 17 `crates/ragent-plugins/tests/*.rs` and 10 `crates/ragent-connectors/tests/*.rs` files — `crates/ragent-plugins/tests/test_add.rs:13`
- Near-identical config test triples replicate the same merge/overlay/serialize assertion bodies — `crates/ragent-config/tests/test_serper_api_key.rs:14`, `test_perplexity_api_key.rs:14`, `test_langsearch_api_key.rs:14`
- Weak assertions dominate: only `.is_some()`/`.is_none()` on outcomes rather than the returned value — `crates/ragent-agent/src/tests/inline/runtime_tests.rs:22`, `crates/ragent-agent/tests/test_trigger_runtime.rs`, `crates/ragent-tools-extended/tests/test_archdoc_extract.rs`
- AGENTS.md violation: tests use `/tmp` instead of `target/temp` (24 sites) — `crates/ragent-tools-core/tests/test_think.rs:21`, `crates/ragent-agent/tests/test_conversation_search.rs:14`

### Dependencies
- duplicate-major in lockfile: thiserror 1.x and 2.x co-resident while workspace pins "2" — `thiserror@1.0.69`, `thiserror@2.0.21`
- duplicate-major in lockfile: rand three major lines co-resident — `rand@0.8.8`, `rand@0.9.5`, `rand@0.10.3`
- duplicate-major in lockfile: reqwest two lines co-resident — `reqwest@0.12.28`, `reqwest@0.13.5`
- `rand@0.8.8` declared in `crates/ragent-storage` (2 majors behind) — `rand@0.8.8 -> 0.10.3`
- `opentelemetry@0.29.1` declared in `crates/ragent-telemetry` (4 breaking 0.x releases behind) — `opentelemetry@0.29.1 -> 0.33.0`
- yanked crate pinned in committed lockfile — `yoke-derive@0.8.3 (yanked, via icu -> reqwest 0.12)`
- duplicate-major: lopdf 0.39 direct + 0.44 transitive (+ vendored 0.38) — `lopdf@0.39.0`, `lopdf@0.44.0`

## Low Priority

### Standards
- Inline fully-qualified paths instead of `use` imports, deviating from the dominant style — `crates/ragent-agent/src/tool/list_agents.rs:100`, `:128`
- `Regex::new(...).unwrap()` instead of the project-wide `.expect("valid ... regex")` convention — `crates/ragent-tui/src/app/slash.rs:12763`
- Test file names deviate from the `test_<component>_<scenario>` convention — `crates/ragent-agent/tests/dump_registries.rs`, `session_processor.rs`, `crates/ragent-research/tests/source_vault.rs`, `crates/ragent-types/tests/structure_types.rs`
- `#[cfg(test)] impl` block living in a production source file — `crates/ragent-research/src/tier_router.rs:43`

### Duplication
- `format_size(bytes)` byte-count formatter identical — `crates/ragent-agent/src/reference/resolve.rs:388` (and `crates/ragent-tools-core/src/list.rs:183`)
- `extract_domain(url)` host-lowercasing duplicated (one `Option`, one `Result`) — `crates/ragent-tools-extended/src/masterfetch/crawl/orchestrator.rs:495` (and `crates/ragent-tools-extended/src/masterfetch/robots.rs:733`)

### Logging
- `unreachable!()` panic on a data-driven dispatch path — `crates/ragent-tui/src/app/state.rs:3268`
- Leftover milestone/dev tags baked into log message text ("M7-T3:", "M6-T2:") — `crates/ragent-agent/src/task/mod.rs:1106`, `crates/ragent-agent/src/team/manager.rs:668`
- Unresolved TODO left in a hot HTTP retry path — `crates/ragent-llm/src/providers/http_client.rs:418`
- Placeholder TODO comments for unimplemented goal persistence/loading — `crates/ragent-tui/src/app/slash.rs:13005`, `:13013`, `:13016`, `:13027`
- `info!` emitted per response when cache-write tokens are non-zero; routine bookkeeping belongs at `debug!` — `crates/ragent-llm/src/providers/openai_responses.rs:568`

### Security
- Server `/config` redaction relies on a hardcoded key allow-list; a newly added secret-bearing field leaks silently to authenticated callers — `crates/ragent-server/src/routes/mod.rs:314`
- `.gitignore` negates `!.env.example` but no `.env.example` exists in the tree — `.gitignore:22`
- Derived per-process credential key path (`USER`+`HOME`) is non-secret; verify the `0600` file write precedes first decrypt use — `crates/ragent-storage/src/storage.rs:162`
- AWS long-term example key literal `AKIAIOSFODNN7EXAMPLE` in docs/comments/fixtures (documented placeholder) — `crates/ragent-llm/src/providers/bedrock.rs:27`
- Placeholder token literals in docs/howtos and specs (clearly marked, benign) — `docs/howtos/reverse.md:457`

### Dependencies
- `notify@7.0.0` declared in `crates/ragent-codeindex` (1 major behind) — `notify@7.0.0 -> 8.2.0`
- `thiserror@1.0.69` declared in 5 crates vs workspace "2" — `thiserror@1.0.69 -> 2.0.21`
- `rquickjs@0.10.0` declared in `crates/ragent-plugins` (4 breaking 0.x behind) — `rquickjs@0.10.0 -> 0.14.0`
- `which@7.0.3` declared in `crates/ragent-tools-core` — `which@7.0.3 -> 8.0.6`
- `chacha20poly1305@0.10.1` declared in `crates/ragent-storage` — `chacha20poly1305@0.10.1 -> 0.11.0`
- `tree-sitter@0.26.13` declared in `crates/ragent-codeindex` — `tree-sitter@0.26.13 -> 0.27.0`
- `reqwest@0.12.28` declared directly in `crates/ragent-llm` (bypasses workspace pin) — `reqwest@0.12.28 -> 0.13.5`
- `tokio@1.53.1` patch behind — `tokio@1.53.1 -> 1.53.2`
- `criterion@0.5.1` (dev, capped LOW) — `criterion@0.5.1 -> 0.8.2`

### Testing
- Research scoreboard tests duplicate `ResearchDocument` fixture setup/assertions across three files with no shared helper — `crates/ragent-research/tests/test_scoreboard_report.rs:108`, `test_scoreboard_imrad.rs:111`, `test_scoreboard_reductions.rs:144`

---

## Tooling Verification (Agent 6, real output)

- `cargo audit` — 0 vulnerabilities; 1 allowed warning (yanked `yoke-derive`).
- `cargo deny check advisories` — `advisories ok`; no unsuppressed advisories.
- `cargo deny check bans licenses sources` — `bans ok, licenses ok, sources ok` (61 duplicate-version warnings).
- `Cargo.lock` committed and consistent with manifests (`cargo metadata --locked` passes).
- CI guard scripts pass: `check-inline-tests`, `check-poison-locks`, `check-security-unwraps`, `check-silent-errors`, `check-dead-code-reasons`.

---

# Implementation Plan

## Quick Wins (< 30 min each)

| # | Finding | File(s) | Fix |
|---|---------|---------|-----|
| 1 | `.gitignore` missing cert/credential files | `.gitignore:50` | Add `*.pem`, `*.p12`, `*.pfx`, `service-account.json`, `credentials.json` |
| 2 | `.gitignore` missing SQLite sidecars | `.gitignore:48` | Add `*.db-wal`, `*.db-shm` |
| 3 | Dead `!.env.example` rule, no template | `.gitignore:22` | Add a committed `.env.example` template (or remove the negate rule) |
| 4 | `unreachable!()` panic on dispatch path | `crates/ragent-tui/src/app/state.rs:3268` | Restructure match to cover all arms without panic |
| 5 | `Regex::new(...).unwrap()` vs `.expect(...)` | `crates/ragent-tui/src/app/slash.rs:12763` | Convert to `.expect("valid ... regex")` |
| 6 | Inline fully-qualified paths | `crates/ragent-agent/src/tool/list_agents.rs:100` | Hoist `crate::task::TaskStatus` into a `use` |
| 7 | Leftover milestone tags in log text | `crates/ragent-agent/src/task/mod.rs:1106`, `crates/ragent-agent/src/team/manager.rs:668` | Strip "M7-T3:"/"M6-T2:" prefixes |
| 8 | Unresolved TODO in HTTP retry path | `crates/ragent-llm/src/providers/http_client.rs:418` | Resolve or file a tracked issue |
| 9 | Placeholder TODO comments in `/goal` | `crates/ragent-tui/src/app/slash.rs:13005` | Resolve, file, or annotate with a reason |
| 10 | `info!` for tool denials | `crates/ragent-agent/src/session/processor.rs:3232` | Raise to `warn!` |
| 11 | Hot-path `info!` per chat request | `crates/ragent-llm/src/providers/copilot.rs:445` | Lower to `debug!` |
| 12 | `info!` for cache-write bookkeeping | `crates/ragent-llm/src/providers/openai_responses.rs:568` | Lower to `debug!` |
| 13 | Tests use `/tmp` | `crates/ragent-tools-core/tests/test_think.rs:21` (+23 sites) | Switch to `target/temp` |
| 14 | Empty/placeholder tests | `crates/ragent-agent/tests/test_precompiled_regexes.rs:15` | Add real assertions or delete |
| 15 | Duplicated schema test suite | `crates/ragent-tools-core/tests/test_schema_validation.rs:1` | Delete one copy, keep shared module |
| 16 | Test file naming | `crates/ragent-agent/tests/dump_registries.rs` (+3) | Rename to `test_<component>_<scenario>` |
| 17 | Raw SSE frame in warn log | `crates/ragent-llm/src/providers/ollama.rs:585` | Truncate/redact before logging |
| 18 | Full request body in debug log | `crates/ragent-llm/src/providers/ollama_cloud.rs:564` | Redact via `redact_secrets` |
| 19 | Tool-args JSON dump in error log | `crates/ragent-agent/src/tool/team_create.rs:302` | Log arg keys only, not values |
| 20 | Patch-level dep bumps | `crates/ragent-llm/Cargo.toml` | `tokio 1.53.1 -> 1.53.2` |
| 21 | Duplicate direct `reqwest` bypassing workspace pin | `crates/ragent-llm/Cargo.toml` | Use `workspace = true` |

## Medium Effort (30 min - 2 hours each)

| # | Finding | File(s) | Fix |
|---|---------|---------|-----|
| 1 | Acronym casing GitHub/GitLab vs Github/Gitlab | `crates/ragent-tools-vcs/src/github/client.rs:86` | Standardise on one form across both crates |
| 2 | `.unwrap()` on HTML parser input | `crates/ragent-tools-extended/src/masterfetch/extractor.rs:905` | Guard or use `?`; add `// no-panic-ok` only if proven safe |
| 3 | `.unwrap()` on user-facing paths | `crates/ragent-agent/src/tool/list_agents.rs:116`, `crates/ragent-research/src/chapter.rs:123` | Replace with `if let`/`ok_or` propagation |
| 4 | Emoji in source comments | `crates/ragent-tui/src/widgets/message_widget.rs:278` (+215) | Strip emoji to ASCII per AGENTS.md |
| 5 | Missing module doc comments | `crates/ragent-agent/src/orchestrator/registry.rs:1` (+5) | Add `//!` headers |
| 6 | `MultiPlEAdapter` naming | `crates/ragent-bench/src/suites/multipl_e.rs:14` | Rename to `MultiPleAdapter` |
| 7 | Duplicated `McpProbe` impl | `src/connectors.rs:318` | Extract shared probe helper |
| 8 | Duplicated `format_size` | `crates/ragent-agent/src/reference/resolve.rs:388` | Move to shared `ragent-types` helper |
| 9 | Duplicated `extract_domain` | `crates/ragent-tools-extended/src/masterfetch/crawl/orchestrator.rs:495` | Unify into one helper |
| 10 | Duplicated `estimate_context_window` | `crates/ragent-llm/src/providers/ollama.rs:163` | Share one implementation |
| 11 | Duplicated `path_tag` | `crates/ragent-agent/src/tool/team_memory_read.rs:161` | Extract to shared helper |
| 12 | Duplicated JSONL log helpers | `crates/ragent-tools-core/src/edit_log.rs:151` | Extract shared JSONL module |
| 13 | Duplicated `truncate_chars` | `crates/ragent-agent/src/loop_state.rs:417` | Move to `ragent-types` |
| 14 | Duplicated `handle_response` classification | `crates/ragent-tools-vcs/src/github/client.rs:219` | Extract shared VCS error classifier |
| 15 | Duplicated `Scope`/`config_path`/lock accessors | `crates/ragent-config/src/bash_lists.rs:68` | Extract shared list-config helper |
| 16 | Duplicated store-path logic (plugins/connectors) | `crates/ragent-plugins/src/store.rs:55` | Extract shared store helper crate or module |
| 17 | Parallel plugins/connectors harness+help code | `crates/ragent-plugins/src/harness.rs:292` | Extract shared harness/help module |
| 18 | Stale agent snapshot duplicate | `crates/ragent-agent/src/snapshot/mod.rs:1` | Delete; keep `ragent-storage::snapshot` |
| 19 | Duplicated provider env-key resolution (4 sites) | `crates/ragent-agent/src/session/processor.rs:4426` | Single resolver in `ragent-llm` |
| 20 | Duplicated OpenAI request-body builder (6 files) | `crates/ragent-llm/src/providers/openai.rs:204` | Extract shared `openai_compat` builder |
| 21 | Duplicated OpenAI SSE parser | `crates/ragent-llm/src/providers/openai.rs:387` | Extract shared SSE parser |
| 22 | Unredacted provider error bodies (11 providers) | `crates/ragent-llm/src/providers/copilot.rs:485` | Route error bodies through `redact_secrets` |
| 23 | Ad-hoc credential env reads | `crates/ragent-agent/src/one_shot.rs:31` (+7) | Route through validated config layer |
| 24 | `get_env` tool redaction by name-substring only | `crates/ragent-tools-core/src/get_env.rs:16` | Deny-list by value-shape / require opt-in |
| 25 | `resolve_secret` unvalidated plain `String` | `crates/ragent-tools-extended/src/channels.rs:111` | Add shape/type validation |
| 26 | Server `/config` hardcoded redaction allow-list | `crates/ragent-server/src/routes/mod.rs:314` | Redact by config-schema secret flag, not a literal list |
| 27 | Duplicated `TempTree` sandbox helper (27 files) | `crates/ragent-plugins/tests/test_add.rs:13` | Shared `tests/support` module per crate |
| 28 | Diagnostic scripts as tests | `crates/ragent-agent/tests/test_mf_orchestrator_diag.rs:1` | Convert to `#[ignore]` or a script |
| 29 | Live-network test not gated | `crates/ragent-llm/tests/test_ollama_cloud_real.rs:10` | Add `#[ignore]` gate |
| 30 | Missing tests: CalculatorTool, Azure Foundry, small tools | `crates/ragent-tools-core/src/calculator.rs:1` (+6) | Add `tests/` coverage |
| 31 | Duplicated config test triples | `crates/ragent-config/tests/test_serper_api_key.rs:14` | Table-driven test |
| 32 | Weak `.is_some()`-only assertions | `crates/ragent-agent/src/tests/inline/runtime_tests.rs:22` | Assert on returned values |
| 33 | Deduplicate major dependency lines | `Cargo.toml` / crate manifests | Align `thiserror`, `rand`, `reqwest` to one major line |
| 34 | `rand@0.8.8` in ragent-storage (2 majors behind) | `crates/ragent-storage/Cargo.toml` | Upgrade to `0.10` |
| 35 | `opentelemetry@0.29.1` (4 behind) | `crates/ragent-telemetry/Cargo.toml` | Plan 0.33 migration |
| 36 | Yanked `yoke-derive@0.8.3` | `Cargo.lock` | `cargo update -p yoke-derive` to a non-yanked release |

## Complex (> 2 hours)

| # | Finding | File(s) | Fix |
|---|---------|---------|-----|
| 1 | Agent-loop step logic untested (1739 lines) | `crates/ragent-agent/src/session/loop_steps.rs:132` | Build a fake-provider harness; integration-test the loop |
| 2 | Orchestrator `Coordinator` API untested | `crates/ragent-agent/src/orchestrator/coordinator.rs:211` | Test harness for spawn/first-success/metrics |
| 3 | Source-scraping fragility (74 test files) | `crates/ragent-tui/tests/test_codeindex_backward_compat.rs:40` | Replace substring scraping with behavioural assertions |
| 4 | `lopdf` 0.39 direct + 0.44 transitive (+vendored 0.38) | `Cargo.toml`, `vendor/` | Coordinate ooxmlsdk/quick-xml upgrade; unblocks RUSTSEC suppressions |
| 5 | Module-file layout inconsistency | `crates/ragent-agent/src/memory/embedding.rs` | Decide `mod.rs` vs named-file policy and apply consistently |

## Suggested Fix Order

1. **Security quick wins**: `.gitignore` cert/sidecar/env-template entries and secret redaction in logs — highest blast radius, lowest risk.
2. **Shared utilities & dedup of small helpers**: `format_size`, `extract_domain`, `truncate_chars`, `path_tag`, JSONL helpers, `Scope` config — unblocks the larger dedup work and reduces test-helper duplication.
3. **Provider consolidation**: shared OpenAI request-body builder, SSE parser, env-key resolver, and error-body redaction — large surface, must land together with provider tests.
4. **Dead-code & stale-duplicate removal**: agent `snapshot` module, duplicated schema test suite, empty tests.
5. **Test infrastructure**: shared `TempTree` support module, `/tmp` -> `target/temp`, weak-assertion tightening.
6. **New test coverage**: calculator, Azure Foundry, small tools, then the complex loop/orchestrator harnesses.
7. **Dependency upgrades**: dedup majors, `rand`, `yoke-derive`, then the `opentelemetry`/`lopdf` migrations.
8. **Cosmetic**: emoji removal, module doc comments, naming, TODO/tag cleanup.

## Verification
- [ ] `cargo build` and `cargo clippy --workspace` report 0 errors / 0 warnings
- [ ] `cargo fmt --check` passes on every edited `.rs` file
- [ ] `cargo test --workspace` passes; new tests cover previously-untested files
- [ ] `cargo deny check advisories bans licenses sources` remains `ok`
- [ ] `cargo audit` reports 0 vulnerabilities and no yanked crates
- [ ] CI guard scripts (`check-inline-tests`, `check-poison-locks`, `check-security-unwraps`, `check-silent-errors`, `check-dead-code-reasons`) still pass
- [ ] No new `println!`/`eprintln!`/`dbg!` outside the documented CLI/panic surface
