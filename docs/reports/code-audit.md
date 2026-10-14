# Code Audit Report

**Project**: Rust 2024 edition Cargo workspace (`ragent`) - 18 library crates under `crates/` plus a root binary in `src/`; tokio async, tracing, thiserror/anyhow, SQLite storage, axum HTTP server, ratatui TUI
**Scope**: full project
**Findings**: 76 total (12 high, 33 medium, 31 low)

---

## High Priority

### Standards
- Two duplicate `RagentError` enums with identical variants in separate crates; the `ragent-agent` copy is never constructed or referenced (dead, drift-prone) - `crates/ragent-types/src/error.rs:10` and `crates/ragent-agent/src/error.rs:12`
- `.unwrap()`/`.expect()` on production (non-test) paths, violating the no-panic rule - `crates/ragent-agent/src/tool/team_spawn.rs:247`, `crates/ragent-plugins/src/store.rs:150`, `crates/ragent-plugins/src/marketplace_ops.rs:331`, `crates/ragent-plugins/src/marketplace_ops.rs:338`, `crates/ragent-codeindex/src/tree_cache.rs:24`, `crates/ragent-research/src/research_name.rs:200`, `crates/ragent-research/src/manager.rs:892`

### Duplication
- `https_only_redirects()` redirect policy duplicated verbatim across three crates (one hardcodes `5`) - `crates/ragent-connectors/src/fetch.rs:357`, `crates/ragent-plugins/src/store_fetch.rs:275`, `crates/ragent-surface/src/manifest.rs:215`
- Identical atomic `write_config` JSON-splice helper (create_dir_all + to_vec_pretty + `json.tmp.<pid>` + rename + cache invalidate) copy-pasted - `crates/ragent-connectors/src/marketplace_ops.rs:349`, `crates/ragent-plugins/src/marketplace_ops.rs:353`
- Near-identical marketplace source-resolution suite (`resolve_source`, `owner_repo_slug`, `name_from_url`, `local_marketplace_name`) between the two crates - `crates/ragent-connectors/src/marketplace_ops.rs:67` and `crates/ragent-plugins/src/marketplace_ops.rs:70`
- Identical endpoint `parse()` validation (trim, empty check, https-only, host check) with the same three error variants - `crates/ragent-connectors/src/store_index.rs:277`, `crates/ragent-plugins/src/store_index.rs:285`

### Logging
- Raw SSE/NDJSON stream frame logged verbatim with no redaction (`line=%data` = full streamed model output); the sibling provider redacts the same data - `crates/ragent-llm/src/providers/ollama_cloud.rs:654`

### Security
- `GET /config` response redactor masks only keys whose *name* looks secret; it never value-scans, so `mcp.<id>.headers.Authorization` / `env.<VAR>` values are returned in cleartext to any authenticated caller - `crates/ragent-server/src/routes/mod.rs:325` and `:357` (see `:287`); `crates/ragent-config/src/config.rs:1449`
- `McpServerConfig` and `ProviderConfig`/`ApiConfig` derive `Debug` while holding raw secret values (env/headers incl. `Authorization`, `PRIVATE-TOKEN`), so any `{:?}`/`tracing::debug!(?cfg)` leaks them - `crates/ragent-config/src/config.rs:1437`, `:1228`, `:1249`

### Testing
- `bang_command.rs` entirely untested: `combine_command_output`, `truncate_command_output` (30k-char FIRST/LAST split), `build_bang_command_prompt`, `bang_command_prompt_from_output` - `crates/ragent-agent/src/bang_command.rs:24`
- `send_one_shot` (one-shot LLM dispatch path) has zero test references - `crates/ragent-agent/src/one_shot.rs:76`
- Permission-engine helpers untested apart from two `_for_test` shims: `is_interactive_tool`, `extract_resource_from_input`, `prompt_for_permission`, `is_hardwired_auto_approved_tool` - `crates/ragent-agent/src/session/permissions.rs:216`

## Medium Priority

### Standards
- Inconsistent error-type name suffixes within crates: `...Error` vs `...Failure` (`ScanFailure`, `RemoteFailure`, `GitFailure`, `FetchFailure`) - `crates/ragent-agent/src/backend/mod.rs:89`, `crates/ragent-connectors/src/store.rs:207`, `crates/ragent-research/src/web_gatherer.rs:549`
- `IoError` newtype struct deriving `thiserror::Error` in a crate whose error type is the `PluginError` enum; generic name clashes with `std::io::Error` - `crates/ragent-plugins/src/error.rs:111`
- Identical `ScanFailure` struct duplicated verbatim across two crates - `crates/ragent-connectors/src/store.rs:207`, `crates/ragent-plugins/src/store.rs:209`
- Results silently swallowed with `let _ =` on production storage/persistence writes - `crates/ragent-agent/src/session/processor.rs:4150`, `:4202`, `:4531`, `crates/ragent-agent/src/tool/team_task_complete.rs:203`, `crates/ragent-agent/src/team/manager.rs:674`
- Blocking `std::thread::sleep` inside async fns (should be `tokio::time::sleep`) - `crates/ragent-agent/src/session/verification.rs:217`, `crates/ragent-agent/src/tool/os_info.rs:1443`
- Sync `std::fs::` calls embedded in files that also define `async fn` - `crates/ragent-tui/src/app/blueprints.rs:74`, `crates/ragent-agent/src/session/history.rs:491`, `crates/ragent-agent/src/mcp/enable_state.rs:54`
- Inconsistent builder method naming: `with_*` (137) vs `set_*` (88) for the same construction role, sometimes in one file - `crates/ragent-llm/src/providers/router.rs:53`, `crates/ragent-llm/src/providers/openrouter.rs:64`, `crates/ragent-tui/src/input_field.rs:29`
- `LoopStage` enum / `classify_stage` fn referenced only from tests; production uses only `classify_message` - `crates/ragent-agent/src/error.rs:73`, `:127`

### Duplication
- Char-boundary truncation with `...` reimplemented five times with minor fast-path variations - `crates/ragent-types/src/strutil.rs:21`, `crates/ragent-tui/src/app/helpers.rs:315`, `crates/ragent-tui/src/utils.rs:202`, `crates/ragent-tui/src/app/prompt.rs:172`, `crates/ragent-agent/src/session/processor.rs:4719`
- `truncate_snippet`/`truncate_snippet_bytes` duplicate `truncate_chars`/`truncate_bytes` (same `max-3` head + ellipsis) - `crates/ragent-tools-extended/src/masterfetch/search/engine.rs:885` vs `crates/ragent-types/src/strutil.rs:21`
- Secret-masking helpers with identical semantics (keep first/last N, star-fill middle) implemented three times - `crates/ragent-tools-extended/src/masterfetch/search/engine.rs:920`, `crates/ragent-tools-extended/src/masterfetch/search/openalex.rs:695`, `crates/ragent-tui/src/layout.rs:4025`
- "short id" helper duplicated: `utils::short_id` plus four inline `chars().take(8)` copies - `crates/ragent-tui/src/utils.rs:29` vs `crates/ragent-agent/src/session/processor.rs:4057`, `crates/ragent-agent/src/tool/wait_agents.rs:321`
- Elapsed-duration formatting reimplemented three different ways - `crates/ragent-tui/src/utils.rs:14`, `crates/ragent-tui/src/layout.rs:5797`, `crates/ragent-tui/src/app/models.rs:1935`
- `is_ollama`/`ollama_cloud` provider predicate duplicated inline instead of using `utils::is_ollama_family` - `crates/ragent-tui/src/utils.rs:9` vs `crates/ragent-tui/src/app/session_ops.rs:4224`, `crates/ragent-agent/src/session/processor.rs:4638`
- Atomic file-write (temp + sync + rename) reimplemented with divergent temp-name schemes across five modules - `crates/ragent-specs/src/io.rs:48`, `crates/ragent-research/src/io.rs:192`, `crates/ragent-config/src/config.rs:2489`, `crates/ragent-bench/src/data.rs:565`
- `reqwest::Client::builder()` timeout/pool/keepalive scaffolding rebuilt per module instead of a shared cached client - `crates/ragent-tools-vcs/src/http_client.rs:31` vs `crates/ragent-agent/src/backend/remote.rs:404`, `crates/ragent-agent/src/updater/mod.rs:47`, `crates/ragent-research/src/open_access.rs:125`

### Logging
- Full skill-invocation argument string logged at `info!` with no redaction - `crates/ragent-agent/src/skill/invoke.rs:134`
- `eprintln!` used after the tracing subscriber is initialised (rule requires post-init diagnostics use tracing) - `src/acp_server.rs:55`
- Raw hook stdout logged at `trace!` (unbounded, no cap/redaction unlike the stderr path) - `crates/ragent-agent/src/hooks/mod.rs:635`
- PostToolUse hook warning text logged at `info!` (can echo user/tool data) - `crates/ragent-agent/src/session/processor.rs:3527`
- Router classification summary logged at `info!` embeds first 80 chars of the user prompt - `crates/ragent-llm/src/providers/router_client.rs:305`
- `info!` emitted on the per-invocation bash hot path ("Safe bash command auto-approved") - `crates/ragent-tools-core/src/bash.rs:1057`

### Security
- TUI `/config show` prints merged config with regex-only redaction and a hardcoded `is_secret_key` allowlist; nested `mcp.*.env`/`headers` and `provider.*.api.headers` fall through - `crates/ragent-tui/src/app/slash.rs:3926`, `:3933`
- Ad-hoc `std::env::var` credential reads bypass the canonical `credential_env` helper - `crates/ragent-tools-extended/src/masterfetch/tools/search_tool.rs:179`, `crates/ragent-llm/src/providers/generic_openai.rs:20`, `crates/ragent-llm/src/providers/xai.rs:243`, `crates/ragent-llm/src/providers/ollama.rs:63`
- `.gitignore` omits database/credential formats (`*.sqlite`/`*.sqlite3`, `id_rsa`/`id_ed25519`, `*.netrc`/`.npmrc`, `*.jks`/`*.keystore`, `.envrc`) - `.gitignore:47`
- `.ragent/` is ignored except a few re-included subtrees; `.ragent/research_vault/**` holds scraped pages with embedded credential-shaped strings and per-vault `vault.db` files - `.gitignore:64`

### Testing
- `event_to_sse` (public SSE wrapper that builds `SseEvent`) never called in tests; only inner `event_to_parts` is - `crates/ragent-server/src/sse.rs:1230`
- MCP enable-ledger `load`/`save`/`save_to`/`clear`/`to_map` covered only incidentally; `set_enabled`+`save_to` round-trip not directly asserted - `crates/ragent-agent/src/mcp/enable_state.rs:79`
- `setup_git_repo` copy-pasted near-identically across three VCS test files - `crates/ragent-tools-vcs/tests/test_git_single_spawn.rs:118`, `crates/ragent-tools-vcs/tests/test_git_tools.rs:12`, `crates/ragent-tools-vcs/tests/test_ms0_git_arg_guards.rs:29`
- `descriptor(...)` `ConnectorDescriptor` builder re-implemented in 11 connector test files (crate already has `tests/support/mod.rs`) - `crates/ragent-connectors/tests/test_catalogue_enrich.rs:26`, `crates/ragent-connectors/tests/test_connector_bridge.rs:34`, `crates/ragent-connectors/tests/test_connector_browse.rs:29`
- `web_source(...)` source factory duplicated across 7 research test files - `crates/ragent-research/tests/test_contradiction.rs:12`, `crates/ragent-research/tests/test_reconcile.rs:12`, `crates/ragent-research/tests/test_corpus_critic.rs:14`
- `make_processor(...)` (~60-line `SessionProcessor` literal) duplicated across 19 agent test files - `crates/ragent-agent/tests/test_loop_checkpoints.rs:192`, `crates/ragent-agent/tests/test_loop_error_stop.rs:187`
- Per-language `parse_<lang>(src)` helpers duplicated across 14 codeindex parser test files - `crates/ragent-codeindex/tests/test_go_parser.rs:10`, `crates/ragent-codeindex/tests/test_python_parser.rs:9`

## Low Priority

### Standards
- Mixed module layout (`mod.rs` vs 2018-style `foo.rs` + `foo/`) in the same crate - `crates/ragent-research/src/session.rs`, `crates/ragent-tui/src/app.rs`, `crates/ragent-tools-extended/src/memory/embedding.rs`
- Wildcard imports in production code (rule forbids outside tests/re-exports) - `crates/ragent-codeindex/src/scanner.rs:10`, `crates/ragent-tools-core/src/glob.rs:10`, `src/main.rs:466`
- `.expect("...regex should compile")` in a non-const production validation module instead of `LazyLock`/`OnceLock` - `crates/ragent-specs/src/validate.rs:118`
- `.unwrap()`-based doctests on public APIs (inconsistent with "prefer `?`" guidance) - `crates/ragent-storage/src/storage.rs:686`, `crates/ragent-codeindex/src/lib.rs:19`
- Inconsistent `Result` idiom: hand-rolled `type Result<T>` vs bare `anyhow::Result<T>` across crates - `crates/ragent-research/src/io.rs`, `crates/ragent-tools-core/src/lib.rs`

### Duplication
- Redirect-hop magic number `5` duplicated as literal/constant in four places - `crates/ragent-connectors/src/fetch.rs:48`, `crates/ragent-plugins/src/store_fetch.rs:38`, `crates/ragent-tools-extended/src/masterfetch/http.rs:43`, `crates/ragent-tools-extended/src/webfetch.rs:18`
- Display-sanitisation (strip ANSI/control chars) implemented in three modules with overlapping intent - `crates/ragent-tui/src/app/helpers.rs:333`, `crates/ragent-types/src/sanitize_terminal.rs:42`, `crates/ragent-research/src/gather_log.rs:295`
- `format_size` (bytes -> KB/MB, binary) and `format_tokens` (-> k/M, decimal) near-identical unit-scaling helpers - `crates/ragent-types/src/strutil.rs:48`, `crates/ragent-tui/src/layout.rs:4574`

### Logging
- Hook exit-code-2 path logs hook stderr at `info!` (capped but unredacted subprocess output) - `crates/ragent-agent/src/hooks/mod.rs:827`
- Durable persist failure logged at `debug!` (should be `warn`) - `crates/ragent-agent/src/session/processor.rs:1952`
- Tool-call argument serialisation failure logged at `debug!` rather than `warn` - `crates/ragent-llm/src/providers/tool_cache.rs:363`
- Model discovery/parse failures logged at `debug!` in a provider (sibling providers use `warn!`) - `crates/ragent-llm/src/providers/ollama_cloud.rs:92`
- `println!` CLI parity surfaces outside the documented exemption list - `src/connectors.rs:60`, `src/plugins.rs:62`
- Discovered API base URL logged at `debug!` - `crates/ragent-llm/src/providers/copilot.rs:1272`
- Copilot auth-resolved `info!` sits adjacent to token handling; worth redacting/summarising - `crates/ragent-llm/src/providers/copilot.rs:975`

### Security
- Hardcoded GitHub OAuth App client ID literal compiled as fallback (public identifier, easy to mistake for a credential) - `crates/ragent-tools-vcs/src/github/client.rs:17`
- Committed research source doc containing a placeholder `-----BEGIN OPENSSH PRIVATE KEY-----` block that scanners will flag - `research/harness/sources/web-49.md:58`
- `.env.example` is a safe comment-only template (verified: no real `.env`, `*.pem`, `*.key`, `credentials.json` tracked) - `.env.example:1`
- `get_env` redaction is name-substring + value-shape only; an innocuous-named or URL-userinfo secret is returned verbatim - `crates/ragent-tools-core/src/get_env.rs:22`
- AWS credential-chain selectors read directly via `std::env` (documented allow-list but ad-hoc) - `crates/ragent-llm/src/providers/bedrock_credentials.rs:90`
- Bearer token falls back to a fresh random UUID when the OpenAI surface is disabled; `RAGENT_TOKEN` resolved via direct `std::env::var` not the config layer - `src/main.rs:1156`

### Testing
- `test_allowed_roots.rs` write/append/mkdir cases assert only `out.is_ok()` without verifying content/path - `crates/ragent-tools-core/tests/test_allowed_roots.rs:65`
- `test_mf_orchestrator_diag.rs` contains zero `assert` statements (an `#[ignore]`d diagnostic) - `crates/ragent-tools-extended/tests/test_mf_orchestrator_diag.rs:1`
- `truncate_bytes` coverage split across two overlapping test files - `crates/ragent-types/tests/test_strutil_unittest.rs:27`
- `split_bash_command` tests miss edge cases (empty string, lone separator, unterminated `$(`, `;` in single quotes) - `crates/ragent-agent/tests/test_ms03_bash_split.rs:1`
- Bash-obfuscation layer asserted only via a comment reference; real unit tests live in a legacy inline file - `crates/ragent-agent/tests/test_hardening_invariants.rs:159`

### Dependencies
- `serde_json@1.0.151 -> 1.0.154` (patch)
- `tokio@1.53.2 -> 1.53.3` (patch)
- `uuid@1.27.0 -> 1.28.0` (minor)
- `pest_generator -> syn@2.0.119 -> 3.0.9` (transitive, one major behind, non-root)
- Informational: 25 resolver-level "Removed" edges under `getrandom` (`cfg-if`, `libc`, `r-efi`, `wasip2`); no action required

_Note (not a finding): `cargo audit --json` reported 0 vulnerabilities (859 deps; 4 reviewed advisories ignored per `deny.toml`); `cargo deny check advisories` returned `advisories ok`; `Cargo.lock` is committed._

---

# Implementation Plan

## Quick Wins (< 30 min each)
| # | Finding | File(s) | Fix |
|---|---------|---------|-----|
| 1 | Raw SSE/NDJSON frame logged verbatim | `crates/ragent-llm/src/providers/ollama_cloud.rs:654` | Route through `redact_secrets` like `ollama.rs:440` |
| 2 | Full skill-invocation args logged | `crates/ragent-agent/src/skill/invoke.rs:134` | Redact/summarise before `info!` |
| 3 | Post-init `eprintln!` in ACP server | `src/acp_server.rs:55` | Switch to `tracing` |
| 4 | Raw hook stdout logged | `crates/ragent-agent/src/hooks/mod.rs:635` | Cap + redact like the stderr path |
| 5 | PostToolUse hook warning at `info!` | `crates/ragent-agent/src/session/processor.rs:3527` | Redact hook-supplied text |
| 6 | Router summary embeds user prompt | `crates/ragent-llm/src/providers/router_client.rs:305` | Drop/redact the prompt excerpt |
| 7 | `info!` on bash hot path | `crates/ragent-tools-core/src/bash.rs:1057` | Downgrade to `debug!` |
| 8 | Persist failure at `debug!` | `crates/ragent-agent/src/session/processor.rs:1952` | Raise to `warn!` |
| 9 | Tool-cache arg failure at `debug!` | `crates/ragent-llm/src/providers/tool_cache.rs:363` | Raise to `warn!` |
| 10 | Model discovery failures at `debug!` | `crates/ragent-llm/src/providers/ollama_cloud.rs:92` | Align to `warn!` |
| 11 | Hook stderr at `info!` | `crates/ragent-agent/src/hooks/mod.rs:827` | Cap + redact |
| 12 | `.gitignore` missing db/credential formats | `.gitignore:47` | Add `*.sqlite`, `*.sqlite3`, `id_rsa`, `id_ed25519`, `*.netrc`, `.npmrc`, `*.jks`, `*.keystore`, `.envrc` |
| 13 | `.unwrap()`/`.expect()` on production paths | `team_spawn.rs:247`, `store.rs:150`, `marketplace_ops.rs:331`, `tree_cache.rs:24`, `research_name.rs:200`, `manager.rs:892` | Return `Result` / use `?` |
| 14 | Blocking `std::thread::sleep` in async | `session/verification.rs:217`, `tool/os_info.rs:1443` | Use `tokio::time::sleep` |
| 15 | `let _ =` swallowed writes | `session/processor.rs:4150`, `team_task_complete.rs:203`, `team/manager.rs:674` | Log on error (`tracing::warn!`) |
| 16 | Wildcard imports in production | `ragent-codeindex/src/scanner.rs:10`, `ragent-tools-core/src/glob.rs:10`, `src/main.rs:466` | Explicit imports |
| 17 | Regex `.expect()` in specs validate | `crates/ragent-specs/src/validate.rs:118` | Hoist to `LazyLock` |
| 18 | `println!` outside CLI exemption | `src/connectors.rs:60`, `src/plugins.rs:62` | Switch to `tracing` or extend documented exemption |
| 19 | Patch/minor dependency bumps | `Cargo.toml` / `Cargo.lock` | `serde_json`, `tokio`, `uuid` |
| 20 | `test_mf_orchestrator_diag.rs` zero asserts | `crates/ragent-tools-extended/tests/test_mf_orchestrator_diag.rs:1` | Add asserts or move to a diagnostics doc |
| 21 | Placeholder key in research doc | `research/harness/sources/web-49.md:58` | Replace with `REDACTED` |
| 22 | LoopStage/classify_stage only test-referenced | `crates/ragent-agent/src/error.rs:73` | Remove dead API or wire into production |

## Medium Effort (30 min - 2 hours each)
| # | Finding | File(s) | Fix |
|---|---------|---------|-----|
| 1 | `GET /config` redactor is name-only | `crates/ragent-server/src/routes/mod.rs:287` | Value-scan with `sanitize::redact_secrets` before returning |
| 2 | TUI `/config show` regex-only redaction | `crates/ragent-tui/src/app/slash.rs:3926` | Reuse the value-level redactor for nested env/headers |
| 3 | `McpServerConfig`/`ProviderConfig`/`ApiConfig` derive `Debug` with secrets | `crates/ragent-config/src/config.rs:1437` | Hand-written redacting `Debug` impls |
| 4 | `https_only_redirects()` duplicated 3x | `connectors/fetch.rs:357`, `plugins/store_fetch.rs:275`, `surface/manifest.rs:215` | Extract one shared helper |
| 5 | Atomic `write_config` duplicated | `connectors/marketplace_ops.rs:349`, `plugins/marketplace_ops.rs:353` | Shared helper in a common crate |
| 6 | Marketplace source resolution duplicated | `connectors/marketplace_ops.rs:67`, `plugins/marketplace_ops.rs:70` | Merge into one parameterised impl |
| 7 | Endpoint `parse()` validation duplicated | `connectors/store_index.rs:277`, `plugins/store_index.rs:285` | Shared validation fn |
| 8 | Char-boundary truncation reimplemented 5x | `ragent-types/src/strutil.rs:21` (+4 sites) | Consolidate on `strutil` |
| 9 | Secret-masking helpers 3x | `masterfetch/search/engine.rs:920`, `openalex.rs:695`, `tui/layout.rs:4025` | One shared masking helper |
| 10 | Atomic file-write reimplemented 5x | `specs/io.rs:48`, `research/io.rs:192`, `config/config.rs:2489`, `bench/data.rs:565` | One `atomic_write` util |
| 11 | `reqwest::Client` scaffolding rebuilt per module | `ragent-tools-vcs/src/http_client.rs:31` (+4) | Share/cache a client factory |
| 12 | ScanFailure struct duplicated | `connectors/store.rs:207`, `plugins/store.rs:209` | Move to a shared type |
| 13 | Ad-hoc `env::var` credential reads | `masterfetch/tools/search_tool.rs:179`, `llm/providers/*.rs` | Route through `credential_env` |
| 14 | Duplicate `RagentError` (dead) | `crates/ragent-agent/src/error.rs:12` | Delete dead enum |
| 15 | `bang_command.rs` untested | `crates/ragent-agent/tests/` | Add `test_bang_command*.rs` |
| 16 | `send_one_shot` untested | `crates/ragent-agent/tests/` | Add coverage |
| 17 | Permission-engine helpers untested | `crates/ragent-agent/tests/` | Add tests via public/test shims |
| 18 | `event_to_sse` wrapper untested | `crates/ragent-server/tests/test_event_to_sse.rs` | Assert on `SseEvent` mapping |
| 19 | MCP enable-ledger round-trip unasserted | `crates/ragent-agent/tests/` | Add direct round-trip assertions |
| 20 | Test helper duplication (git/descriptor/web_source/make_processor/parse_lang) | VCS, connectors, research, agent, codeindex test dirs | Move into per-crate `tests/support` |
| 21 | Error suffix inconsistency (`Error` vs `Failure`) | `ragent-agent`, `ragent-connectors`, `ragent-research` | Standardise on `...Error` |
| 22 | `IoError` struct vs enum convention | `crates/ragent-plugins/src/error.rs:111` | Fold into `PluginError` |
| 23 | Sync `std::fs::` in async files | `tui/app/blueprints.rs:74`, `session/history.rs:491`, `mcp/enable_state.rs:54` | Use `tokio::fs` |
| 24 | Builder `with_*` vs `set_*` inconsistency | `llm/providers/router.rs:53`, `openrouter.rs:64`, `tui/input_field.rs:29` | Pick one convention |

## Complex (> 2 hours)
| # | Finding | File(s) | Fix |
|---|---------|---------|-----|
| 1 | `RagentError` drift risk across two crates | `ragent-types/src/error.rs:10`, `ragent-agent/src/error.rs:12` | Single canonical error type + workspace-wide migration |
| 2 | `pest_generator -> syn` one major behind | `Cargo.lock` | Bump `pest_generator` once a `syn 3`-compatible release exists |
| 3 | `Result` idiom split (`type Result<T>` vs `anyhow`) | `ragent-research`, `ragent-tools-core`, `ragent-llm`, `ragent-config` | Decide one workspace error convention and migrate |
| 4 | Module layout split (`mod.rs` vs 2018-style) | `ragent-research`, `ragent-tui`, `ragent-tools-extended` | Normalise tree crate-by-crate |
| 5 | `.ragent/research_vault` credential-shaped content | `.gitignore:64`, `.ragent/research_vault/**` | Audit/purge vault content and tighten ignore rules |

## Suggested Fix Order
1. **Security redaction and logging leaks**: highest blast radius; fix value-level redaction (`/config`, TUI `/config`, config `Debug`) and the verbatim stream/hook/args logs first - they can expose credentials in a running session.
2. **Quick Win hygiene**: severity-level logging corrections, `.gitignore` gaps, production `unwrap`/blocking-sleep/`let _ =` fixes, wildcard imports - low-risk, high-noise-reduction.
3. **Duplicate shared helpers**: extract the redirect policy, atomic write, endpoint validation, truncation and masking helpers into shared crates, then delete the copies.
4. **Error-type and naming consistency**: consolidate `RagentError`, `ScanFailure`, error suffixes, builder convention.
5. **Test coverage and setup dedupe**: add tests for untested security-sensitive paths (`bang_command`, permissions, SSE wrapper, MCP ledger), then move duplicated test helpers into `tests/support`.
6. **Dependency and structural refactors**: patch/minor bumps now, the `syn`/`pest_generator` and module-layout work last.

## Verification
- [ ] Rust compiler reports 0 errors: `cargo check --workspace`
- [ ] No warnings: `cargo clippy --workspace -- -D warnings -A clippy::used_underscore_items -A clippy::redundant_pub_crate -A clippy::wildcard_imports`
- [ ] Formatting clean: `cargo fmt --check`
- [ ] All tests pass: `timeout 1500 cargo test --workspace`
- [ ] `cargo audit` / `cargo deny check advisories` still report no new advisories
- [ ] New tests cover previously-untested files (`bang_command`, `one_shot`, permissions helpers, `event_to_sse`, MCP enable ledger)
- [ ] No `tracing` call logs raw secrets/stream frames (re-grep `line=%data`, `args = %`, `stdout = %`)
