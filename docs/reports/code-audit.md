# Code Audit Report

**Project**: Rust workspace (edition 2024, floor 1.91) — `ragent`, an AI coding agent CLI/TUI/HTTP server: root binary in `src/`, 19 crates under `crates/*`, 3,203 `.rs` files of which 1,240 are integration tests consolidated into one `tests/all.rs` binary per crate and run with `cargo nextest`
**Scope**: Full project
**Findings**: 55 total (11 high, 29 medium, 15 low)

---

## High Priority

### Security
- Tracked archive `scripts/getresult.zip` unpacks to `getresult.sh`, whose line 6 carries a clear-text `Authorization: Bearer <84-char token>` against a live-looking Azure OpenAI endpoint; the zip is committed and present at HEAD — `scripts/getresult.zip` (payload `getresult.sh:6`)
- Tracked archive `assets/config/ragent.json.zip` unpacks to `ragent.json` carrying a `tvly-dev-…` Tavily API key in clear text; the extracted `assets/config/ragent.json` is not matched by the root-anchored `/ragent.json` ignore rule — `assets/config/ragent.json.zip`
- `--dry-run` config-parse failure echoes `Problematic line: {problematic_line}` with no `redact_secrets` pass, unlike the fixed sister sites, so `"tavily_api_key": "<live key>"` is printed verbatim — `crates/ragent-agent/src/dry_run.rs:369`
- `READ_ONLY_PERMISSION_CATEGORIES` omits seven categories that tools actually declare (`git:read`, `github:read`, `gitlab:read`, `cron:read`, `agent:read`, `team:read`, `model:read`; only the bare `model` is listed), so read-only tools classify as write-capable and force an unattended sub-agent run into the automatic YOLO write scope — `crates/ragent-agent/src/session/permissions.rs:174` (vs `crates/ragent-tools-vcs/src/git/git_status.rs:42`, `crates/ragent-agent/src/tool/model_info.rs:105`)

### Logging
- Background-task spawn logs the raw `command` string with no `redact_secrets`, while the sibling capture path in the tools crate redacts the same value, so `curl -H "Authorization: Bearer …"` reaches the log verbatim — `crates/ragent-agent/src/background/mod.rs:215` (vs `crates/ragent-tools-core/src/bg.rs:455`)

### Duplication
- Two TUI browse panels are near-duplicates: `render_plugin_store_panel` (365 lines) and `render_connector_catalogue_panel` (353 lines) differ mainly in field/type names (0.73 similarity), with six sibling pairs — navigator (0.85), marketplace navigator (0.92), title (0.86), body (0.67), footer (0.77), empty line (0.64) — repeating the same shape — `crates/ragent-tui/src/layout.rs:1540` (and `:2087`, `:2155`, `:2245`, `:2304`, `:2360`, `:2334`) vs `crates/ragent-tui/src/layout.rs:642` (and `:1007`, `:1072`, `:1151`, `:1201`, `:1245`, `:1228`)
- 29 byte-identical functions (237 lines) copied between the plugin-store and connector-catalogue browsers: `next`, `previous`, `set_query`, `push_char`, `backspace`, `move_up`/`move_down`, `scroll_results_by`, `category_*`, `marketplace_*`, `ensure_*_visible` — `crates/ragent-tui/src/app/state.rs:2001-2954` vs `crates/ragent-connectors/src/browse.rs:76-1071`
- The Anthropic SSE stream loop is copy-pasted into the Azure Anthropic provider — 230 of ~590 lines match exactly after whitespace normalisation (blocks of 39 and 40 consecutive identical lines), differing only in provider label strings — `crates/ragent-llm/src/providers/anthropic.rs:460-540` vs `crates/ragent-llm/src/providers/azure_resource.rs:528-602`; the same stall-message + `append_stream_chunk` + `sse_buffer_exceeded` + `"SSE buffer exceeded {} bytes"` preamble is repeated in 9 provider modules instead of routing through the existing `providers/sse.rs` (`bedrock.rs:951`, `copilot.rs:433`, `huggingface.rs:529`, `ollama.rs:392`, `ollama_cloud.rs:503`, `openai_responses.rs:424`, `gemini.rs:569`, `azure_resource.rs:486`, `anthropic.rs:418`)

### Testing
- `validate_id` — the vault path-traversal guard rejecting empty ids, `..`, separators and dot-prefixed names — has no test; the crate's only test file covers `models::note` — `crates/ragent-hyperresearch/src/core/vault.rs:104`
- `Untrusted::fence` / `sanitise_marker`, the prompt-injection fencing that must prevent fetched text escaping its marker block, has no test — `crates/ragent-hyperresearch/src/core/untrusted.rs:30`
- `prompt_for_permission`, the interactive permission-decision await, is referenced by no test file — no timeout, deny or abandon coverage — `crates/ragent-agent/src/session/permissions.rs:529`

---

## Medium Priority

### Standards
- Tool-name casing is inconsistent: 175 snake_case names alongside 28 PascalCase ones (`Bash`, `Edit`, `Write`, `Glob`, `Grep`, `DateTime`, `Skill`, `TaskCreate`, `EnterPlanMode`) — `crates/ragent-tools-core/src/write.rs`, `crates/ragent-tools-core/src/datetime.rs`, `crates/ragent-agent/src/tool/plan.rs:16`
- Four of twenty core tool files declare a different name from the one they register (`append_file.rs`→`append_to_file`, `mkdir.rs`→`make_directory`, `diff.rs`→`diff_files`, `multiedit.rs`→`multi_edit`) — `crates/ragent-tools-core/src/append_file.rs:21`, `mkdir.rs:18`, `diff.rs`, `multiedit.rs:138`
- Module layout is split across the workspace: `mod.rs` style in 8 crates (60 files) vs named-file style in 11 crates, contrary to the documented Rust-2018 layout — `crates/ragent-agent/src/agent/mod.rs` vs `crates/ragent-config/src` (no `mod.rs`)
- The atomic tmp-file + rename write is re-implemented four times, and only one of the four uses the hardened `create_new` form (SEC-059) — `crates/ragent-specs/src/io.rs:135` vs `crates/ragent-agent/src/mcp/enable_state.rs:94`, `crates/ragent-agent/src/task/mod.rs:62`, `crates/ragent-config/src/config.rs:2469`
- Blocking `std::thread::sleep` inside an async path, reached from `async fn connect_inner` — `crates/ragent-agent/src/mcp/npx_cache.rs:179` (from `crates/ragent-agent/src/mcp/mod.rs:675`, `:694`)
- Error handling is inconsistent inside one module family: `?` propagation in one sibling and silent discard in the next — `crates/ragent-agent/src/mcp/enable_state.rs:89` vs `crates/ragent-agent/src/mcp/npx_cache.rs:114` and `crates/ragent-agent/src/task/mod.rs:87`
- 122 library source files still carry inline `#[cfg(test)]` modules despite the "do not add new inline `#[cfg(test)]` modules" rule — `crates/ragent-tools-core/src/bash.rs:1832`, `crates/ragent-tools-core/src/askpass.rs:453`, `crates/ragent-tools-vcs/src/gitlab/client.rs:739`, `crates/ragent-tools-extended/src/masterfetch/search/mod.rs:547`
- `println!` in a library crate while the workspace provides `cliprintln!` for exactly this purpose — `crates/ragent-hyperresearch/src/cli/output.rs:23` (and `crates/ragent-hyperresearch/src/cli/mod.rs:21`) vs `src/cli_out.rs:30`
- Four parallel `Tool` / `ToolContext` / `ToolOutput` / `ToolRegistry` definitions with hand-written per-crate context marshalling instead of one shared trait — `crates/ragent-tools-core/src/lib.rs:103` (`:173`, `:522`), `crates/ragent-tools-extended/src/lib.rs:291` (`:363`, `:379`), `crates/ragent-tools-vcs/src/lib.rs:84` (`:106`, `:120`), `crates/ragent-agent/src/tool/mod.rs:129` (`:470`, `:564`; marshalling at `:785`, `:1327`, `:1436`)

### Duplication
- `read_capped` (read a stream in chunks, refuse beyond `max_bytes`) is implemented four times, only the error variant differing — `crates/ragent-plugins/src/store_fetch.rs:244`, `crates/ragent-connectors/src/fetch.rs:329`, `crates/ragent-surface/src/manifest.rs:137`
- 51 hand-rolled `truncate*` helpers reimplement the three canonical functions in `ragent_types::strutil`, and disagree on `...` vs `…` and on whether the suffix counts toward the limit — `crates/ragent-research/src/cli.rs:929`, `crates/ragent-tools-extended/src/archdoc/extract.rs:360`, `crates/ragent-research/src/comparison.rs:199`, `crates/ragent-tui/src/widgets/message_widget.rs:105`, `crates/ragent-agent/src/task/mod.rs:1625`, `crates/ragent-agent/src/compaction/serializer.rs:167`, `crates/ragent-plugins/src/control.rs:695`, `crates/ragent-surface/src/harness.rs:60`
- The plugin store and connector catalogue are whole-cloth copies: `StoreLedger::load`/`save`, `scan_dirs`, `EndpointSource`, `StoreEndpoint::parse`/`as_url`/`as_str`/`Display`/`TryFrom`, the whole `effective_endpoints_with_source` body and `marketplace_name_for` — `crates/ragent-plugins/src/store.rs:118` vs `crates/ragent-connectors/src/store.rs:127`, and `crates/ragent-plugins/src/store_index.rs:180` vs `crates/ragent-connectors/src/store_index.rs:164`
- `add_marketplace`, `remove_marketplace`, `matches_entry`, `global_config_path`, `read_or_empty_object` and `marketplaces_array_mut` are the same 31-line body in both crates, differing only in the JSON key — `crates/ragent-plugins/src/marketplace_ops.rs:59` vs `crates/ragent-connectors/src/marketplace_ops.rs:52`
- `MarketplaceArgError::report` + `parse_marketplace_command` + `run_marketplace_add`/`run_marketplace_remove` duplicated with only the command noun changed (0.88-0.90 similarity) — `crates/ragent-plugins/src/commands.rs:264-300`, `:411-460` vs `crates/ragent-connectors/src/commands.rs:244-285`, `:430-480`
- `write_spec` is byte-identical (18 lines) in the TUI slash surface and the CLI — same `SpecCommand::write_govcreate_spec` call and error mapping — `crates/ragent-tui/src/app/slash.rs:152-169` vs `src/cli.rs:1917-1934`
- `McpProbe` (`probe_connect`/`probe_call`/`probe_disconnect`) duplicated between the TUI and CLI connector harness clients — `crates/ragent-tui/src/app/connector.rs:428-453` vs `src/connectors.rs:317-342`
- Retryable-error message matching duplicated with near-identical keyword lists and the same 4xx-but-not-408/429 classification — `crates/ragent-bench/src/model.rs:531-576` vs `crates/ragent-agent/src/session/history.rs:716-756` (permanent-status twin at `:675-708`)
- Three separate exponential-backoff schedules re-implemented with no shared helper — `crates/ragent-llm/src/providers/http_client.rs:350-352`, `crates/ragent-agent/src/team/manager.rs:115-140`, `crates/ragent-bench/src/model.rs:504-523`
- Three near-identical model-picker row renderers build the same padded table cells inline; the identical `for (col, value)` span-building block appears twice in one file plus a third in the model-assign dialog — `crates/ragent-tui/src/layout.rs:3497-3527` vs `crates/ragent-tui/src/layout.rs:4618-4638`

### Logging
- `warn!(?event, …)` formats a whole `StreamEvent`, whose derived `Debug` includes `TextDelta{text}`, `ReasoningDelta{text}` and `ToolCallDelta{args_json}`, so model output and tool arguments can be dumped — `crates/ragent-agent/src/one_shot.rs:124`
- Crawl, fetch and govcreate warn paths log the raw target URL with no `redact_secrets`, though the same crate has `mask_url_credentials` — `crates/ragent-tools-extended/src/masterfetch/tools/crawl_tool.rs:308` (also `:344`), `crates/ragent-tools-extended/src/masterfetch/tools/fetch.rs:273`, `crates/ragent-tools-extended/src/archdoc/url_source.rs:353`
- The input-transition spool writes the raw user input buffer (`before={:?} after={:?}`) to `log/logwindow/input-transitions.log` with no redaction, so a pasted API key is persisted in cleartext — `crates/ragent-tui/src/app/session_ops.rs:3426`
- Routine success ("setting user-global config key") logged at `warn!`, used as an always-on attribution trail rather than a warning condition — `crates/ragent-config/src/config.rs:2624`

### Security
- `hyperresearch::Config` derives `Debug` while holding `pub provider_keys: Vec<(String, String)>`; any `{:?}` leaks every provider API key, with no redacting impl unlike `ragent-config`'s `redacted_debug_impl!` set — `crates/ragent-hyperresearch/src/core/config.rs:43`
- `DeviceFlowStart` derives `Debug` and holds `pub device_code: String`, the OAuth credential the workspace explicitly treats as secret elsewhere (T-066) — `crates/ragent-llm/src/providers/copilot.rs:726`
- `GITLAB_URL` is trusted as the outbound API base URL with no scheme/host validation in three production sites, so a hostile env value receives the `PRIVATE-TOKEN` header — `crates/ragent-tools-vcs/src/gitlab/auth.rs:149`, `crates/ragent-tools-extended/src/project_scaffold/remote.rs:546`, `crates/ragent-tui/src/app/reverse.rs:184`
- `.gitignore` has no `*.zip` / `*.tar.gz` rule, which is why a credential-bearing archive has stayed tracked; a newly generated archive would be committed silently — `.gitignore:1`

### Testing
- Test fixtures are copy-pasted instead of shared: 23 `ragent-tui` files re-declare a local `make_app()` duplicating `support::make_app()`, 22 `ragent-agent` files duplicate `make_agent()`, and three `ragent-server` files rebuild `AppState` inline — `crates/ragent-tui/tests/support/mod.rs:30` vs `crates/ragent-tui/tests/test_scrolling.rs:24`, `test_status_expiry.rs:21`, `test_memory_panel.rs:45`; `crates/ragent-agent/tests/test_loop_error_stop.rs:282`; `crates/ragent-server/tests/test_automation_surface.rs`, `test_openai_surface.rs`, `test_research_routes.rs`
- 52 test files build scratch directories under `std::env::temp_dir()` — outside the workspace, contrary to the `target/temp` rule — and none removes them — `crates/ragent-tools-core/tests/test_allowed_roots.rs:21`
- Three file-local mutexes guard the same process-global `EDIT_LOG_MODE` instead of `crate::global_env_lock()`, so these files do not exclude each other — `crates/ragent-tools-core/tests/test_apply_patch.rs:255`, `crates/ragent-tools-core/tests/test_edit_integration.rs:578`, `crates/ragent-tools-core/tests/inline/edit_log_tests.rs:13`
- Three registered Team tool handlers have zero test references, including the "feedback is required when rejecting" branch — `crates/ragent-agent/src/tool/team_task_list.rs:10`, `crates/ragent-agent/src/tool/team_approve_plan.rs:14` (branch at `:66`), `crates/ragent-agent/src/tool/team_memory_common.rs:1`
- Seven GitLab pipeline/job tool `execute` paths are untested; the tests only assert the registry name exists — `crates/ragent-tools-vcs/src/gitlab/gitlab_pipelines.rs:93`
- `build_analyzer_request`, the LLM security-analyzer prompt/schema builder, is referenced by zero tests; only `parse_verdict`/`analyze_tool_action` are covered — `crates/ragent-agent/src/security/mod.rs:104`
- The vault schema migrations (`apply_pending`/`read_version`) have no test; no migration test file exists — `crates/ragent-hyperresearch/src/core/migrations.rs:54`
- `cli_out`'s stdout/stderr routing macros (169 lines, 10 production call sites) are untested — `src/cli_out.rs:49`

### Dependencies
- Vendored `pdf-extract` fork is two breaking releases behind upstream — `pdf-extract@0.10.0` (0.10.0 -> 0.12.1, path dep `vendor/pdf-extract`)
- `Cargo.lock` carries ~45 orphaned entries no longer in the dependency graph (868 lock entries vs 823 metadata packages; `paste`, `tokenizers`, `ort`, `onig`, `native-tls`, `openssl` and `ureq` are absent from `cargo tree`) — `Cargo.lock` stale

---

## Low Priority

### Standards
- Same "list" operation named with different verb prefixes (`github_get_actions` lists workflow runs) against `github_list_issues` / `github_list_prs` / `gitlab_list_pipelines` — `crates/ragent-tools-vcs/src/github/github_actions.rs:206`
- Directory-name stutter in module paths against the `git/status.rs` shape used elsewhere — `crates/ragent-tools-vcs/src/git/git_status.rs`, `.../gitlab/gitlab_issues.rs`, `crates/ragent-agent/src/tool/tool_info.rs`
- Mixed `.map_or(false, …)` and `.is_some_and(…)` for the same predicate in one file — `crates/ragent-agent/src/session/processor.rs:1725`, `:1791` vs `:2273`
- Acronym casing split across type names in the same domain (`JsonLdFields`/`JsonRpcRequest` vs `IdField`, `IID`/`ID`) — `crates/ragent-tools-extended/src/masterfetch/metadata.rs:335`, `crates/ragent-llm/src/providers/wire/mod.rs:120`
- Empty-parameter tool JSON schema hand-written eight times across seven tool files — `crates/ragent-tools-core/src/bash_reset.rs:30-36`, `crates/ragent-agent/src/tool/tool_info.rs:148-154`, `:187-193`, `crates/ragent-tools-extended/src/codeindex_status.rs:86-92`, `codeindex_reindex.rs:25-31`, `codeindex_communities.rs:33-39`, `crates/ragent-tools-extended/src/masterfetch/tools/version.rs:42-48`

### Logging
- `debug!` fires on every buffer-changing keystroke and mouse event from 12 call sites; `#[allow(unused_variables)]` on a `debug_`-prefixed method signals leftover diagnostic scaffolding — `crates/ragent-tui/src/app/session_ops.rs:3346`
- Fallback/non-fatal condition ("Git Bash not found; falling back to PowerShell 7+") logged at `info!` where `warn!` belongs — `crates/ragent-tools-core/src/bash.rs:131`
- Sub-agent launch failure, a user-visible `[warn]`/`[err]` path, recorded only at `debug!`, so the tracing log loses the failure — `crates/ragent-tui/src/app/launch.rs:147`
- Full search query text logged at `debug!` on every `codeindex_search` call from a hot path — `crates/ragent-codeindex/src/lib.rs:293`
- `trace!(api_base = ?result, …)` logs the discovered API base with no redaction guard, relying only on the value being non-secret — `crates/ragent-llm/src/providers/copilot.rs:1291`

### Security
- 13 files matched by `.gitignore` are nonetheless tracked, proving ignore/track drift: `crates/ragent-llm/default.profraw`, `crates/ragent-tui/default.profraw`, `crates/ragent-tui/target/temp/humaneval-*.py`, `scripts/log/logwindow/stderr-20261014-021922.log`, `vendor/*/Cargo.toml.orig` — `scripts/log/logwindow/stderr-20261014-021922.log`
- `HYPERRESEARCH_VAULT` is turned straight into a `PathBuf` with no validation or containment check, bypassing the validated config layer — `crates/ragent-hyperresearch/src/core/config.rs:149`
- `env_locale` reads `LC_ALL`/`LC_MESSAGES`/`LANG` with raw `std::env::var`, bypassing the crate's own `read_env` helper and silently dropping non-UTF-8 values — `crates/ragent-config/src/i18n.rs:285` (same shape in `crates/ragent-tui/src/app/toolchain.rs:414` for `PATHEXT`)
- `credential_env::read_credential_env` uses `std::env::var(...).ok()?`, so a non-UTF-8 credential is indistinguishable from "unset" and the caller proceeds without a key — `crates/ragent-config/src/credential_env.rs:19`
- Documented example bearer token `"token": "sk-ragent-local"` in the Quickstart config block is short enough to evade the `sk-` redaction pattern (needs 20+ chars) and is presented as the value `ragent serve` requires — `QUICKSTART.md:495`

### Testing
- Tautological assert in a test: `assert_eq!(LogLevel::Info, LogLevel::Info)` plus two `assert_ne!` on enum variants asserts the derive, not the code — `crates/ragent-tui/tests/test_models_state_misc.rs:972`
- Test body is only `let _ = detect_language("a");` — it asserts nothing and cannot fail — `crates/ragent-tools-extended/tests/test_masterfetch_language.rs:35`

### Dependencies
- Vendored `html2text` fork is one breaking release behind upstream — `html2text@0.16.7` (0.16.7 -> 0.17.4, path dep `vendor/html2text`)
- `cargo audit` informational: unmaintained crate RUSTSEC-2024-0436, present only as an orphan lock entry — `paste@1.0.15` (1.0.15 -> `pastey` fork)
- 20 duplicate crate versions in the lockfile (`multiple-versions = "warn"`, so warnings only) — `base64` 0.22.1/0.23.1, `crossterm` 0.28.1/0.29.0, `rand` 0.8.8/0.9.5/0.10.3, `phf` 0.10.1/0.11.3/0.13.1, `html5ever` 0.26.0/0.38.0, `euclid` 0.20.14/0.22.14, `nix` 0.29.0/0.31.3, `bitflags` 1.3.2/2.13.2
- 9 of 19 crates hardcode `edition = "2024"` instead of `edition.workspace = true` (the value matches, so no build divergence) — `ragent-config`, `ragent-llm`, `ragent-specs`, `ragent-storage`, `ragent-telemetry`, `ragent-tools-core`, `ragent-tools-extended`, `ragent-tools-vcs`, `ragent-types`
- `package-lock.json` is checked in with no dependencies to justify it (the manifest is `{}`) — `package-lock.json` orphan lockfile
- `rust-toolchain.toml` pins `channel = "stable"` while the manifest floor is `rust-version = "1.91"`, so the floor is never exercised locally — `rust-toolchain.toml` unpinned channel
- Vendored `pdf-extract` declares `edition = "2018"` and no `rust-version`, bypassing the workspace 2024 / 1.91 policy — `pdf-extract@0.10.0` (`vendor/pdf-extract/Cargo.toml`)
- Patch-level drift only on direct deps (no major or minor lag) — `uuid` 1.29.0 -> 1.29.1, `flate2` 1.1.10 -> 1.1.11, `globset` 0.4.20 -> 0.4.21, `notify` 8.2.0 -> 8.2.1, `tree-sitter-cmake` 0.7.5 -> 0.7.6, `unicode-width` 0.2.2 -> 0.2.3

---

# Implementation Plan

## Quick Wins (< 30 min each)
| # | Finding | File(s) | Fix |
|---|---------|---------|-----|
| 1 **Done** | Live Tavily key in tracked archive | `assets/config/ragent.json.zip` | Delete the zip, rotate the key, add `assets/config/ragent.json*` to `.gitignore` (already partly present — verify the pattern actually matches the zip) |
| 2 **Done** | Live Azure bearer token in tracked archive | `scripts/getresult.zip` | Delete the zip, rotate the credential, purge it from history if the repo is shared |
| 3 **Done** | `.gitignore` has no archive rule | `.gitignore:1` | Add `*.zip`, `*.tar.gz`, `*.tgz` |
| 4 **Done** | Ignored-but-tracked files | 13 paths | `git rm --cached` each listed file; they are build/log/vendor scratch |
| 5 **Done** | `--dry-run` echoes raw credential line | `crates/ragent-agent/src/dry_run.rs:369` | Pass `problematic_line` through `redact_secrets` before formatting |
| 6 **Done** | Background command logged unredacted | `crates/ragent-agent/src/background/mod.rs:215` | Apply `crate::sanitize::redact_secrets(command)` as `bg.rs:455` does |
| 7 **Done** | Raw URLs logged on warn paths | `crawl_tool.rs:308`, `:344`, `fetch.rs:273`, `url_source.rs:353` | Route through `mask_url_credentials` / `redact_secrets` |
| 8 **Done** | Raw input persisted to transition spool | `crates/ragent-tui/src/app/session_ops.rs:3426` | Redact `before_input`/`after_input` before writing the line |
| 9 **Done** | Read-only category allowlist incomplete | `crates/ragent-agent/src/session/permissions.rs:174` | Add the seven declared `*:read` categories and a test asserting every registered tool's category is classified |
| 10 **Done** | `Config` Debug leaks provider keys | `crates/ragent-hyperresearch/src/core/config.rs:43` | Replace the derive with a redacting `Debug` impl (follow `redacted_debug_impl!`) |
| 11 **Done** | `DeviceFlowStart` Debug leaks `device_code` | `crates/ragent-llm/src/providers/copilot.rs:726` | Hand-write `Debug` redacting `device_code` |
| 12 **Done** | `GITLAB_URL` unvalidated (3 sites) | `gitlab/auth.rs:149`, `remote.rs:546`, `reverse.rs:184` | Validate scheme/host once in `load_config` and reuse |
| 13 **Done** | `println!` in a library crate | `crates/ragent-hyperresearch/src/cli/output.rs:23`, `cli/mod.rs:21` | Use `cliprintln!` from `src/cli_out.rs` |
| 14 **Done** | Routine success logged at `warn!` | `crates/ragent-config/src/config.rs:2624` | Demote to `debug!` (or `info!` if the trail must stay on) |
| 15 **Done** | `StreamEvent` Debug dumped | `crates/ragent-agent/src/one_shot.rs:124` | Log only the variant name, not `?event` |
| 16 **Done** | Tautological and empty tests | `test_models_state_misc.rs:972`, `test_masterfetch_language.rs:35` | Delete both, or assert real behaviour |
| 17 **Done** | TUI log-level nits | `bash.rs:131`, `launch.rs:147`, `codeindex/lib.rs:293` | `info!`→`warn!`, `debug!`→`warn!`, `debug!`→`trace!` |
| 18 **Done** | Patch-level dependency drift | `uuid`, `flate2`, `globset`, `notify`, `tree-sitter-cmake`, `unicode-width` | `cargo update -p <crate>` |
| 19 **Done** | Orphan lockfiles / manifests | `package-lock.json`, `package.json` | Delete both — the workspace has no Node dependency |
| 20 **Done** | `edition` hardcoded in 9 crates | 9 `crates/*/Cargo.toml` | Replace with `edition.workspace = true` |
| 21 **Not a defect** | Lockfile entries that look orphaned | `Cargo.lock` | No action. The 44 entries flagged are live edges of optional dependencies: `ragent-tools-extended` declares `ort`, `tokenizers` and `ndarray` as `optional = true` behind the `embeddings` feature, so `cargo metadata` omits them unless that feature is enabled. Verified: `cargo metadata --features embeddings` lists all three as live, and pruning the blocks makes cargo re-add them on the next `cargo check` (and would break `--features embeddings`). Lockfile left at 857 packages, stable across a full workspace check. |
| 22 **Done** | `EDIT_LOG_MODE` guarded by three different mutexes | `test_apply_patch.rs:255`, `test_edit_integration.rs:578`, `edit_log_tests.rs:13` | Switch all three to `crate::global_env_lock()` |
| 23 **Done** | `.map_or` vs `.is_some_and` | `crates/ragent-agent/src/session/processor.rs:1725`, `:1791` | Standardise on `is_some_and` |
| 24 **Done** | Empty-parameter schema hand-written 8× | 7 tool files | Add one `no_parameters_schema()` helper in the tool module |
| 25 **Done** | Quickstart example token below redaction threshold | `QUICKSTART.md:495` | Make it obviously fake (`sk-example-not-a-real-key-000000`) |

## Medium Effort (30 min - 2 hours each)
| # | Finding | File(s) | Fix |
|---|---------|---------|-----|
| 1 | `validate_id` untested | `crates/ragent-hyperresearch/src/core/vault.rs:104` | Add `tests/test_vault_id.rs` covering empty, `..`, `/`, `\`, leading `.`, and a valid id |
| 2 | `Untrusted::fence` untested | `crates/ragent-hyperresearch/src/core/untrusted.rs:30` | Add a test where the body contains the fence marker and assert it is sanitised |
| 3 | `prompt_for_permission` untested | `crates/ragent-agent/src/session/permissions.rs:529` | Add timeout/deny/allow tests over a scripted permission channel |
| 4 | `build_analyzer_request` untested | `crates/ragent-agent/src/security/mod.rs:104` | Assert the rendered prompt and JSON schema for a fixed tool input |
| 5 | Team tool handlers untested | `tool/team_task_list.rs:10`, `team_approve_plan.rs:14`, `team_memory_common.rs:1` | One test per handler, plus the reject-without-feedback error branch |
| 6 | GitLab pipeline `execute` paths untested | `crates/ragent-tools-vcs/src/gitlab/gitlab_pipelines.rs:93` | Mock the client and assert request shape + rendered output for each tool |
| 7 | Vault migrations untested | `crates/ragent-hyperresearch/src/core/migrations.rs:54` | Round-trip a v0 vault through `apply_pending` and assert the version bump |
| 8 | `cli_out` macros untested | `src/cli_out.rs:49` | Capture the writer and assert routing/format for each macro |
| 9 | `ragent-read_capped` ×3 | `store_fetch.rs:244`, `fetch.rs:329`, `manifest.rs:137` | Hoist one generic `read_capped` (parameterised over the error) into `ragent-types` |
| 10 | 51 `truncate*` helpers | `ragent_types::strutil` callers | Replace each body with a delegation to `truncate_chars`/`truncate_bytes`; fix the `...`/`…` disagreement |
| 11 | Plugin/connector store + marketplace + commands duplication | `store.rs`, `store_index.rs`, `marketplace_ops.rs`, `commands.rs` in both crates | Extract a generic store module (kind parameter over manifest name and error type) and have both crates instantiate it |
| 12 | `write_spec` duplicate | `ragent-tui/src/app/slash.rs:152`, `src/cli.rs:1917` | Move into `ragent-specs` and call from both |
| 13 | `McpProbe` duplicate | `ragent-tui/src/app/connector.rs:428`, `src/connectors.rs:317` | Move the probe into `ragent-connectors` and share |
| 14 | Retryable-error keyword lists duplicate | `ragent-bench/src/model.rs:531`, `ragent-agent/src/session/history.rs:716` | Extract `is_retryable_error(&str)` + status classifier into a shared crate |
| 15 | Three backoff schedules | `http_client.rs:350`, `team/manager.rs:115`, `bench/model.rs:504` | One `Backoff` helper with a configurable base/cap/jitter |
| 16 | TUI browse-panel renderers near-duplicate | `ragent-tui/src/layout.rs:642` vs `:1540` | Parameterise one renderer over a row-source trait; keep the panel-specific headings as data |
| 17 | 29 identical browser state-machine functions | `ragent-tui/src/app/state.rs:2001`, `ragent-connectors/src/browse.rs:76` | Extract a shared `BrowseState` (query, category, marketplace, scroll, cursor) generic over the entry type |
| 18 | SSE stream loop duplicated across providers | `providers/anthropic.rs:460`, `azure_resource.rs:528`, + 7 preamble sites | Route all providers through `providers/sse.rs`: one `consume_sse` taking a per-event closure |
| 19 | Tool-name casing / file-name mismatch | `tools-core` tool files | Pick PascalCase for the user-facing names (already 28) or snake_case, and rename files to match the registered name |
| 20 | Atomic write re-implemented 4× | `specs/io.rs:135`, `mcp/enable_state.rs:94`, `task/mod.rs:62`, `config.rs:2469` | One `atomic_write(path, bytes)` using the hardened `create_new` form |
| 21 | Blocking sleep in async path | `mcp/npx_cache.rs:179` | Replace with `tokio::time::sleep` and await it |
| 22 | Error handling `?` vs `let _ =` | `mcp/enable_state.rs:89`, `npx_cache.rs:114`, `task/mod.rs:87` | Propagate or log the discarded errors |
| 23 | Test fixture duplication | 23 TUI `make_app`, 22 `make_agent`, 3 server `AppState` | Delete locals, import from `tests/support`; extend `ragent-server/tests/support/mod.rs` |
| 24 | 52 test files write into `std::env::temp_dir()` | 52 test files | Point at `target/temp/<unique-name>` resolved from `env!("CARGO_MANIFEST_DIR")` |
| 25 | Vendored `pdf-extract` two breaking releases behind | `vendor/pdf-extract` | Bump to 0.12.1 and re-apply the local patch, or document the fork reason |
| 26 | Vendored `html2text` one breaking release behind | `vendor/html2text` | Bump to 0.17.4 and re-apply the local patch |
| 27 | `paste` unmaintained advisory | `Cargo.lock` | Clears with the orphan-entry prune (#21); otherwise swap to `pastey` |
| 28 | Duplicate crate versions in lockfile | `Cargo.lock` | `cargo update` the trailing copies (base64, crossterm, rand, phf, nix, bitflags) one at a time, checking `cargo tree -i` |

## Complex (> 2 hours)
| # | Finding | File(s) | Fix |
|---|---------|---------|-----|
| 1 | Four parallel `Tool`/`ToolContext`/`ToolOutput`/`ToolRegistry` definitions with hand-written marshalling | `tools-core/src/lib.rs:103`, `tools-extended/src/lib.rs:291`, `tools-vcs/src/lib.rs:84`, `agent/src/tool/mod.rs:129` | Move the trait + context into one crate that all tool crates depend on; delete the three copies and the marshalling shims at `agent/src/tool/mod.rs:785`, `:1327`, `:1436` |
| 2 | Three model-picker row renderers build the same padded cells | `ragent-tui/src/layout.rs:3497`, `:4618` | Extract a row-builder that takes the cell list, widths and alignment, then convert all three call sites |
| 3 | 122 files still carry inline `#[cfg(test)]` modules | `crates/*/src/**` | Continue the documented migration: move each module into `tests/test_<module>.rs` and register it in `tests/all.rs`; batch by crate |
| 4 | `mod.rs` vs named-file layout split across 8 crates | `crates/ragent-agent/src/**` and 7 others | Convert `foo/mod.rs` to `foo.rs` crate by crate, verifying `cargo check` after each |
| 5 | Directory-name stutter in module paths | `git/git_status.rs`, `gitlab/gitlab_issues.rs`, `tool/tool_info.rs` | Rename to `git/status.rs`, `gitlab/issues.rs`, `tool/info.rs` and fix the `mod` declarations |
| 6 | Tool "list" verb prefixes inconsistent | `github/github_actions.rs:206` and siblings | Rename to `github_list_actions` (a user-visible tool rename: needs the `check-tool-rename.sh` guard and a CHANGELOG entry) |
| 7 | `HYPERRESEARCH_VAULT` / `PATHEXT` / `LC_*` read ad-hoc | `core/config.rs:149`, `tui/app/toolchain.rs:414`, `config/i18n.rs:285` | Route each through the validated config layer, with a typed parse and an explicit non-UTF-8 policy |
| 8 | `read_credential_env` conflates non-UTF-8 with unset | `crates/ragent-config/src/credential_env.rs:19` | Return a three-state result (`Missing`/`Invalid`/`Present`) and surface `Invalid` at the call sites |

## Suggested Fix Order
1. **Security first (Quick Wins 1-12):** rotate and purge the two committed credentials, close the `.gitignore` gaps, and redact the four leaking log/error paths. These are live secrets and cost minutes each.
2. **Correctness guards (Medium 1-8, Quick Win 9):** the permission-category allowlist and the untested security primitives (`validate_id`, `Untrusted::fence`, `prompt_for_permission`) are behaviour that silently degrades if wrong.
3. **Shared helpers before refactors (Medium 9-15, Quick Wins 22-24):** `read_capped`, `truncate*`, `atomic_write`, backoff and retry classification are small, well-bounded extractions that shrink several later changes.
4. **Crate-level de-duplication (Medium 11-18):** the plugin/connector store pair and the provider SSE loop are the two largest duplications; each removes hundreds of lines.
5. **Test hygiene (Medium 23-24, Quick Win 22):** fixture consolidation and `target/temp` compliance stop the suite writing outside the workspace.
6. **Dependency and config tidying (Medium 25-28, Quick Wins 18-21):** lockfile prune, vendored fork bumps, edition inheritance.
7. **Architectural refactors last (Complex 1-8):** the shared `Tool` trait, `mod.rs` conversion and tool renames are large, cross-cutting and best done once the smaller work has stopped churning the same files.
8. **Cosmetic (Quick Wins 17, 23-25; Low nits):** log-level tuning, naming consistency and the `layout.rs` renderer extraction, once behaviour is settled.

## Verification
- [ ] `cargo fmt --check` reports 0 differences
- [ ] `cargo clippy --workspace --all-targets` reports 0 warnings
- [ ] `cargo check --workspace --all-targets` reports 0 errors
- [ ] `cargo nextest run --workspace --no-fail-fast` passes, with the new tests for `validate_id`, `Untrusted::fence`, `prompt_for_permission`, `build_analyzer_request`, the Team handlers and the GitLab pipeline tools included
- [ ] `scripts/check-test-env-isolation.sh` passes (catches the `EDIT_LOG_MODE` mutex split and any new env mutation)
- [ ] `scripts/check-silent-errors.sh`, `check-security-unwraps.sh`, `check-poison-locks.sh` and `check-dead-code-reasons.sh` pass
- [ ] `scripts/check-tool-rename.sh` passes after any tool rename, and the `/` menu, `tool_info` and the OpenAPI-adjacent docs agree with the registry
- [ ] `cargo audit` and `cargo deny check` report no vulnerabilities, no unmaintained crates outside the documented allowlist, and no new duplicate-version warnings
- [ ] `cargo tree -i paste` (and the other 45 orphans) reports "nothing depends on"; `Cargo.lock` metadata resolves cleanly
- [ ] A full test run leaves the working tree byte-identical (`git status` clean apart from intended edits) — no files written under `std::env::temp_dir()`, no scratch under tracked paths
- [ ] `git ls-files -i -c --exclude-standard` is empty except `.ragent/blueprints/**`, which carries its own `!` exception
