# Code Audit Report

**Project**: ragent — Rust workspace (edition 2024, Rust 1.85+), 17 crates + root binary
**Scope**: full project (crates/*/src, crates/*/tests, src/, tests/, Cargo.toml, deny.toml, .gitignore)
**Findings**: 56 total (9 high, 19 medium, 28 low)

---

## High Priority

### Standards & Consistency
- Multiple `unsafe` blocks beyond the single approved site (`kill_process_group` in `crates/ragent-tools-core/src/bash.rs:1197`) — `crates/ragent-agent/src/mcp/mod.rs:1399,1418,1425`, `crates/ragent-agent/src/team/manager.rs:448`
- Widespread use of `Result<T, String>` instead of `anyhow::Result<T>` or `thiserror` — ~118 instances across crates (ragent-tui: 41, ragent-agent: 16, ragent-tools-extended: 15, ragent-connectors: 14, ragent-plugins: 8, ragent-tools-vcs: 5, ragent-bench: 9, ragent-types: 4)
- Same-file mixing of `anyhow::Result` and `Result<T, String>` error types — `crates/ragent-agent/src/skill/context.rs:82` vs `crates/ragent-agent/src/skill/context.rs:183`; `crates/ragent-connectors/src/harness.rs:87,100,108` vs `crates/ragent-connectors/src/harness.rs:492`

### Duplication
- Atomic write (write-tmp-then-rename) pattern duplicated across 15+ files — `crates/ragent-surface/src/config_splice.rs:47`, `crates/ragent-agent/src/mcp/enable_state.rs:98`, `crates/ragent-agent/src/team/task.rs:348`, `crates/ragent-agent/src/team/store.rs:354`, `crates/ragent-agent/src/team/mailbox.rs:367`, `crates/ragent-agent/src/task/mod.rs:79`, `crates/ragent-agent/src/session/history.rs:501`, `crates/ragent-agent/src/file_ops/mod.rs:259`, `crates/ragent-agent/src/updater/mod.rs:152`, `crates/ragent-research/src/io.rs:199`, `crates/ragent-research/src/source_vault.rs:632`, `crates/ragent-specs/src/io.rs:64`, `crates/ragent-config/src/config.rs:2448,2536,2635`, `crates/ragent-tools-vcs/src/github/auth.rs:86`, `crates/ragent-tools-extended/src/memory/embedding/local.rs:231`
- Corrupt-ledger load + quarantine-rename pattern duplicated 3× — `crates/ragent-agent/src/mcp/enable_state.rs:53-69`, `crates/ragent-connectors/src/store.rs:128-157`, `crates/ragent-plugins/src/store.rs:121-139`
- `scan_dirs` near-identical between connectors and plugins stores — `crates/ragent-connectors/src/store.rs:233`, `crates/ragent-plugins/src/store.rs:253`
- `resolve_api_key` env→storage fallback duplicated 3× — `crates/ragent-agent/src/one_shot.rs:26`, `crates/ragent-agent/src/research_adapter.rs:214`, `crates/ragent-llm/src/providers/router_client.rs:548`
- html2text thread-isolation pattern duplicated 3× — `crates/ragent-agent/src/reference/resolve.rs:183-191`, `crates/ragent-tui/src/app/md_worker.rs:302`, `crates/ragent-tools-extended/src/masterfetch/extractor.rs:467-478` (extractor version is `pub(crate)`, forcing agent/TUI re-implementation)

---

## Medium Priority

### Standards & Consistency
- 118 library source files contain inline `#[cfg(test)]` modules, violating the project rule "Do not add new inline `#[cfg(test)]` modules to library source files" — worst offenders: `crates/ragent-research/src/` (41 files), `crates/ragent-agent/src/` (29), `crates/ragent-llm/src/` (17), `crates/ragent-tui/src/` (10)
- Inconsistent test module naming: `mod tests;` (106 files) vs `mod tests_tests;` (8 files) — `crates/ragent-agent/src/reference/parse.rs:143`, `crates/ragent-agent/src/skill/context.rs:338`, `crates/ragent-agent/src/skill/args.rs:190`, `crates/ragent-agent/src/skill/mod.rs:581`, `crates/ragent-agent/src/skill/loader.rs:501`, `crates/ragent-llm/src/providers/xai.rs:288`, `crates/ragent-llm/src/providers/huggingface.rs:1048`, `crates/ragent-specs/src/validate.rs:1731`
- Silent error swallowing with `let _ =` in production code — 30+ instances: `crates/ragent-agent/src/file_ops/mod.rs:261`, `crates/ragent-agent/src/hooks/mod.rs:431,1137`, `crates/ragent-agent/src/task/mod.rs:87`, `crates/ragent-agent/src/team/mailbox.rs:329,372,496,538`, `crates/ragent-agent/src/team/store.rs:360`, `crates/ragent-agent/src/session/verification.rs:164,171,184,187,189,191,199,200`, `crates/ragent-agent/src/orchestrator/coordinator.rs:432,441,458,468,476,492`
- `.expect()` on potentially fallible production code paths — `crates/ragent-tools-extended/src/masterfetch/http.rs:147`, `crates/ragent-tools-extended/src/masterfetch/tools/fetch.rs:767,770,777`, `crates/ragent-tools-extended/src/masterfetch/search/consensus.rs:374`, `crates/ragent-tools-extended/src/task.rs:252,636,652`, `crates/ragent-telemetry/src/shutdown.rs:98`, `crates/ragent-research/src/analysis/parser.rs:187,213,542`
- `.unwrap()` in production code — `crates/ragent-tui/src/layout.rs:4222` (`cur_style.unwrap()`), `crates/ragent-research/src/session/topic.rs:426` (`Regex::new(...).unwrap()`)

### Duplication
- `"Authorization: Bearer {key}"` header pattern repeated 19× — `crates/ragent-llm/src/providers/openai.rs:233,338`, `crates/ragent-llm/src/providers/openrouter.rs:133,504`, `crates/ragent-llm/src/providers/huggingface.rs:495,817`, `crates/ragent-llm/src/providers/ollama_cloud.rs:87,535`, `crates/ragent-llm/src/providers/copilot.rs:349,1058,1072,1351`, `crates/ragent-tools-extended/src/masterfetch/search/langsearch.rs:175`, `crates/ragent-tools-extended/src/masterfetch/search/tavily.rs:182`, `crates/ragent-tools-extended/src/masterfetch/search/perplexity.rs:196`, `crates/ragent-tools-extended/src/project_scaffold/remote.rs:155`, `crates/ragent-tools-vcs/src/github/client.rs:161`
- HTTP scheme validation (`starts_with("http://") || starts_with("https://")`) repeated 14× — `crates/ragent-agent/src/mcp/http.rs:160`, `crates/ragent-agent/src/mcp/mod.rs:125`, `crates/ragent-agent/src/reference/parse.rs:114`, `crates/ragent-agent/src/research_adapter.rs:1205`, `crates/ragent-config/src/telemetry.rs:124`, `crates/ragent-connectors/src/manifest.rs:303-306`, `crates/ragent-plugins/src/add.rs:307-310`, `crates/ragent-surface/src/marketplace.rs:91`, `crates/ragent-telemetry/src/subsystem.rs:506,656-661`, `crates/ragent-tools-extended/src/webfetch.rs:107`, `crates/ragent-tools-vcs/src/vcs_provider.rs:125,174,230-232`, `crates/ragent-tui/src/app/helpers.rs:384`
- Secret-key JSON redaction key-detection logic duplicated — `crates/ragent-server/src/routes/mod.rs:325-354` vs `crates/ragent-tui/src/app/slash.rs:3929-3934` (different suffix sets)
- `mask_key` one-line wrapper duplicated 6× — `crates/ragent-llm/src/providers/openrouter.rs:41`, `crates/ragent-tools-extended/src/masterfetch/search/langsearch.rs:290`, `crates/ragent-tools-extended/src/masterfetch/search/serper.rs:323`, `crates/ragent-tools-extended/src/masterfetch/search/tavily.rs:301`, `crates/ragent-tools-extended/src/masterfetch/search/perplexity.rs:358`, `crates/ragent-tools-extended/src/masterfetch/search/exa.rs:152`
- `truncate_snippet` one-line wrapper duplicated 5× — `crates/ragent-tools-extended/src/masterfetch/search/tavily.rs:288`, `crates/ragent-tools-extended/src/masterfetch/search/perplexity.rs:345`, `crates/ragent-tools-extended/src/masterfetch/search/serper.rs:310`, `crates/ragent-tools-extended/src/masterfetch/search/exa.rs:440`, `crates/ragent-tools-extended/src/masterfetch/search/openalex.rs:682`
- Snippet char-limit constant `200` defined 3× with 3 different names (one with typo "SNIPPETTE") — `crates/ragent-tools-extended/src/masterfetch/search/engine.rs:850`, `crates/ragent-tools-extended/src/masterfetch/search/exa.rs:68`, `crates/ragent-tools-extended/src/masterfetch/search/openalex.rs:94`

### Security
- `GitLabIntegrationConfig.token` stored in plaintext config and struct derives `Debug` verbatim (unlike `Config`/`ProviderConfig`/`McpServerConfig` which use `redacted_debug_impl!`) — `crates/ragent-config/src/config.rs:3872-3880`
- `BackendConfig.api_key` (remote server key) is plaintext in config and its struct derives `Debug` verbatim — `crates/ragent-config/src/config.rs:1751-1775`
- `OpenAiConfig.token` (bearer token) is plaintext in config and derives `Debug` verbatim — `crates/ragent-config/src/openai.rs:43-59`

---

## Low Priority

### Standards & Consistency
- Blocking `std::fs` calls in async functions — `crates/ragent-tui/src/app/state.rs` (8 calls), `crates/ragent-agent/src/session/history.rs` (4), `crates/ragent-agent/src/session/processor.rs` (4), `crates/ragent-agent/src/session/loop_capture.rs` (3)
- Mixed `std::sync::Mutex`/`RwLock` and `tokio::sync::Mutex`/`RwLock` — 155 uses of `std::sync` vs 28 uses of `tokio::sync`; no clear project convention for async contexts

### Duplication
- Redaction placeholder string inconsistent across 4 crates — `"[REDACTED]"` (`crates/ragent-types/src/sanitize.rs:382,395`), `"<redacted>"` (`crates/ragent-server/src/routes/mod.rs:310`, `crates/ragent-tui/src/app/slash.rs:3936`), `"redacted"` (`crates/ragent-telemetry/src/sensitive.rs:45`), `"***REDACTED***"` (`crates/ragent-tools-core/src/get_env.rs:59`)
- html2text render width `120` magic number hardcoded in 3 places — `crates/ragent-agent/src/reference/resolve.rs:185`, `crates/ragent-tui/src/app/md_worker.rs:302`, `crates/ragent-tools-extended/src/masterfetch/extractor.rs:50`
- `file_name().and_then(|n| n.to_str()).unwrap_or("")` pattern repeated 4× — `crates/ragent-agent/src/agent/mod.rs:245,1814,1815`, `crates/ragent-tools-core/src/glob.rs:184`
- Corrupt-ledger quarantine comment + rename duplicated verbatim — `crates/ragent-connectors/src/store.rs:154`, `crates/ragent-plugins/src/store.rs:136`

### Logging Hygiene
- No raw `println!`/`eprintln!`/`dbg!` found in production library code outside the documented CLI presentation surface exceptions (`src/main.rs`, `src/cli.rs`, `src/cli_surface.rs`, `src/connectors.rs`, `src/plugins.rs`, `src/acp_server.rs`, `src/panic_hook.rs`, `src/stderr_spool.rs`, `src/crash_dump.rs`)
- No `dbg!()` macros found in production code

### Security
- Ad-hoc `std::env::var` reads bypass `credential_env::read_credential_env` for non-key config values — `GITLAB_URL` in `crates/ragent-tools-vcs/src/vcs_provider.rs:198`, `crates/ragent-tools-vcs/src/gitlab/auth.rs:149`, `crates/ragent-tui/src/app/reverse.rs:184`, `crates/ragent-tools-extended/src/project_scaffold/remote.rs:543`
- Ad-hoc `std::env::var` base-URL reads not routed through canonical env helper — `GENERIC_OPENAI_API_BASE`/`AZURE_AI_FOUNDRY_BASE` in `crates/ragent-llm/src/providers/router_client.rs:584-585`, `crates/ragent-agent/src/research_adapter.rs:186,200`, `crates/ragent-llm/src/providers/azure_foundry.rs:61,80`, `crates/ragent-bench/src/model.rs:440,457`
- `AWS_PROFILE`/`AWS_SHARED_CREDENTIALS_FILE`/`AWS_REGION`/`AWS_BEDROCK_REGION` read with raw `std::env::var` (no blank-trim parity) — `crates/ragent-llm/src/providers/bedrock_credentials.rs:90,112,135,142,234`
- Bedrock presence check reads `AWS_ACCESS_KEY_ID`/`AWS_SECRET_ACCESS_KEY` ad-hoc — `crates/ragent-tui/src/app/models.rs:834-841`
- TUI `/config show` redaction allowlist is top-level-key-only; nested secrets under differently-named keys survive — `crates/ragent-tui/src/app/slash.rs:3929-3934`
- `ProviderConfig.options` raw `serde_json::Value` map can carry credential-shaped values under innocuous keys — `crates/ragent-config/src/config.rs:1246-1264`
- MCP server `env`/`headers` maps hold tokens; `GET /config` and `/config show` rely on value-scan layer with depth cap (8) — `crates/ragent-config/src/config.rs:1465-1492`
- TUI tracing layer forwards every log record into the on-screen log panel with no redaction — `crates/ragent-tui/src/tracing_layer.rs:60-109`
- TUI drains raw mirrored stderr lines directly into the log panel unredacted — `crates/ragent-tui/src/lib.rs:848-859`
- `get_env` tool redacts by name-shape + regex, but a credential in a custom-named variable whose value matches no known pattern is returned verbatim — `crates/ragent-tools-core/src/get_env.rs:57-67`
- Hardcoded OAuth client ID `Iv1.b507a08c87ecfe98` compiled in — `crates/ragent-llm/src/providers/copilot.rs:61`, `crates/ragent-tools-vcs/src/github/client.rs:17`
- `crates/ragent-tui/security_findings.md` is a tracked security-review doc that could go stale — `crates/ragent-tui/security_findings.md`
- No hardcoded secrets found (no real API keys/tokens/passwords in source). AWS example keys in doc comments/tests are documentation placeholders. `.env` is gitignored; only `.env.example` (all commented) is tracked.

### Testing
- 118 inline `#[cfg(test)]` modules in library source files (also flagged under Standards — cross-category)
- Inconsistent test module naming `mod tests_tests;` (also flagged under Standards)
- All crates have `tests/` directories; no crate is entirely untested
- No superfluous or fragile tests identified in the sampled test files

### Dependencies
- Duplicate crate versions in Cargo.lock — 50+ crates have 2–4 simultaneous versions (base64, syn, rand, getrandom, crossterm, hashbrown, windows-sys, phf, etc.)
- Wildcard (`*`) version specifiers across workspace crates — ~90 dependencies unpinned in `Cargo.toml` files (Cargo.lock mitigates)
- `cargo audit` ignore list suppresses 4 advisories — RUSTSEC-2026-0194, RUSTSEC-2026-0195, RUSTSEC-2024-0436, RUSTSEC-2026-0253 permanently ignored in `.cargo/config.toml`
- `license-not-encountered` warnings (×2) in `cargo deny` — 2 stale entries in `deny.toml` license allow-list
- gcf@3.0.1 (3.0.1 -> 3.1.0) — minor update available, direct dep of `ragent-agent`
- rmcp@3.5.0 (3.5.0 -> 3.5.1) — patch update available, direct dep of `ragent-agent`
- sha2@0.11.0 (0.11.0 -> 0.11.1) — patch update available, workspace dep
- opentelemetry@0.33.0 (0.33.0 -> 0.33.1) — minor update available, direct deps of `ragent-telemetry`
- proptest@1.11.0 (1.11.0 -> 1.12.0) — minor update available, dev-dep of `ragent-research`
- r-efi@5.3.0 (5.3.0 -> 6.0.0) — 1 major behind, UEFI target only (no impact on current platforms)
- syn@2.0.119 (2.0.119 -> 3.0.9) — 1 major behind, transitive via `pest_generator` → `pest_derive` → `termwiz` (requires parent crate update)
- ~196 transitive dependencies have minor/patch updates available — `cargo update` for safe refresh

---

# Implementation Plan

## Quick Wins (< 30 min each)

| # | Finding | File(s) | Fix |
|---|---------|---------|-----|
| 1 | `mod tests_tests;` inconsistent naming (8 files) | `crates/ragent-agent/src/reference/parse.rs:143`, `crates/ragent-agent/src/skill/context.rs:338`, `crates/ragent-agent/src/skill/args.rs:190`, `crates/ragent-agent/src/skill/mod.rs:581`, `crates/ragent-agent/src/skill/loader.rs:501`, `crates/ragent-llm/src/providers/xai.rs:288`, `crates/ragent-llm/src/providers/huggingface.rs:1048`, `crates/ragent-specs/src/validate.rs:1731` | Rename `mod tests_tests;` to `mod tests;` (or rename the included file) in all 8 files |
| 2 | Snippet char-limit constant `200` defined 3× with different names (incl. typo) | `crates/ragent-tools-extended/src/masterfetch/search/engine.rs:850`, `crates/ragent-tools-extended/src/masterfetch/search/exa.rs:68`, `crates/ragent-tools-extended/src/masterfetch/search/openalex.rs:94` | Use a single `pub const SNIPPET_MAX_CHARS: usize = 200;` from `engine.rs` in all three files; fix "SNIPPETTE" typo |
| 3 | `mask_key` one-line wrapper duplicated 6× | `crates/ragent-llm/src/providers/openrouter.rs:41`, `crates/ragent-tools-extended/src/masterfetch/search/{langsearch,serper,tavily,perplexity,exa}.rs` | Delete the wrappers; call the shared helper directly at each call site |
| 4 | `truncate_snippet` one-line wrapper duplicated 5× | `crates/ragent-tools-extended/src/masterfetch/search/{tavily,perplexity,serper,exa,openalex}.rs` | Delete the wrappers; call `super::engine::truncate_snippet` directly |
| 5 | Redaction placeholder string inconsistent across 4 crates | `crates/ragent-types/src/sanitize.rs:382`, `crates/ragent-server/src/routes/mod.rs:310`, `crates/ragent-tui/src/app/slash.rs:3936`, `crates/ragent-telemetry/src/sensitive.rs:45`, `crates/ragent-tools-core/src/get_env.rs:59` | Standardise on `"[REDACTED]"` from `ragent-types::sanitize`; update all call sites |
| 6 | `.unwrap()` in production code (2 clear instances) | `crates/ragent-tui/src/layout.rs:4222`, `crates/ragent-research/src/session/topic.rs:426` | Replace with proper error handling (`?` or `if let Some` / `map_err`) |
| 7 | `.expect()` on obviously-infallible constants (safe to leave, but can be cleaned) | `crates/ragent-tools-extended/src/masterfetch/metadata.rs:40-92` (LazyLock regex expects) | These are safe (regex literals are valid); optionally add `#[allow(clippy::expect_used)]` at module level |
| 8 | Remove 2 stale `license-not-encountered` entries from `deny.toml` | `deny.toml` | Run `cargo deny check licenses 2>&1 | grep "not encountered"`, remove the 2 stale entries |
| 9 | `cargo update` for transitive minor/patch deps | `Cargo.lock` | Run `cargo update` then `cargo test --workspace` to verify |
| 10 | Targeted dep bumps: gcf, rmcp, sha2, opentelemetry, proptest | `Cargo.lock` | Run `cargo update -p gcf -p rmcp -p sha2 -p opentelemetry -p opentelemetry-otlp -p opentelemetry_sdk -p proptest` |
| 11 | `GitLabIntegrationConfig.token` derives verbatim `Debug` | `crates/ragent-config/src/config.rs:3872-3880` | Replace `#[derive(Debug)]` with `redacted_debug_impl! GitLabIntegrationConfig` (add `token` to redacted fields) |
| 12 | `BackendConfig.api_key` derives verbatim `Debug` | `crates/ragent-config/src/config.rs:1751-1775` | Replace `#[derive(Debug)]` with `redacted_debug_impl! BackendConfig` (add `api_key` to redacted fields) |
| 13 | `OpenAiConfig.token` derives verbatim `Debug` | `crates/ragent-config/src/openai.rs:43-59` | Replace `#[derive(Debug)]` with `redacted_debug_impl! OpenAiConfig` (add `token` to redacted fields) |
| 14 | `crates/ragent-tui/security_findings.md` stale audit artifact | `crates/ragent-tui/security_findings.md` | Move to `docs/reports/` or delete if superseded |

## Medium Effort (30 min - 2 hours each)

| # | Finding | File(s) | Fix |
|---|---------|---------|-----|
| 1 | Atomic write pattern duplicated across 15+ files | `crates/ragent-surface/src/config_splice.rs:47` + 14 other files listed above | Create a shared `ragent-surface::atomic_write::write_json_atomic(path, &value)` helper; replace all 15+ inline implementations |
| 2 | Corrupt-ledger load + quarantine-rename duplicated 3× | `crates/ragent-agent/src/mcp/enable_state.rs:53-69`, `crates/ragent-connectors/src/store.rs:128-157`, `crates/ragent-plugins/src/store.rs:121-139` | Extract into `ragent-surface::store::load_ledger_quarantine_on_error(path)` shared helper |
| 3 | `scan_dirs` near-identical between connectors and plugins | `crates/ragent-connectors/src/store.rs:233`, `crates/ragent-plugins/src/store.rs:253` | Extract shared `ragent-surface::store::scan_store_dirs(dirs, parse_fn)` generic over the manifest type |
| 4 | `resolve_api_key` env→storage fallback duplicated 3× | `crates/ragent-agent/src/one_shot.rs:26`, `crates/ragent-agent/src/research_adapter.rs:214`, `crates/ragent-llm/src/providers/router_client.rs:548` | Extract into `ragent-config::credential_env::resolve_provider_api_key(provider_id, storage)` shared helper |
| 5 | html2text thread-isolation duplicated 3× | `crates/ragent-agent/src/reference/resolve.rs:183-191`, `crates/ragent-tui/src/app/md_worker.rs:302`, `crates/ragent-tools-extended/src/masterfetch/extractor.rs:467-478` | Make `run_html2text_isolated` `pub` in `ragent-tools-extended`; have agent and TUI crates call it; add panic guard to TUI copy |
| 6 | `"Authorization: Bearer {key}"` repeated 19× | 12 files across `ragent-llm`, `ragent-tools-extended`, `ragent-tools-vcs` | Add `ragent-surface::http::bearer_auth_header(key) -> HeaderValue` helper; replace all 19 call sites |
| 7 | HTTP scheme validation repeated 14× | 12 files across 9 crates | Add `ragent-surface::url::is_http_url(s) -> bool` helper; replace all 14 call sites |
| 8 | Secret-key JSON redaction logic duplicated | `crates/ragent-server/src/routes/mod.rs:325-354`, `crates/ragent-tui/src/app/slash.rs:3929-3934` | Move `looks_like_secret_key` into `ragent-types::sanitize` as `pub fn is_secret_json_key(key: &str) -> bool`; unify suffix sets |
| 9 | Silent error swallowing with `let _ =` (30+ instances) | `crates/ragent-agent/src/orchestrator/coordinator.rs`, `crates/ragent-agent/src/team/mailbox.rs`, `crates/ragent-agent/src/session/verification.rs`, etc. | Audit each site: replace with `tracing::warn!` where the error matters, or add `// INTENTIONAL:` comment where it truly doesn't |
| 10 | `.expect()` on potentially fallible production paths | `crates/ragent-tools-extended/src/masterfetch/http.rs:147`, `crates/ragent-tools-extended/src/masterfetch/tools/fetch.rs:767,770,777`, `crates/ragent-tools-extended/src/masterfetch/search/consensus.rs:374`, `crates/ragent-tools-extended/src/task.rs:252,636,652`, `crates/ragent-telemetry/src/shutdown.rs:98`, `crates/ragent-research/src/analysis/parser.rs:187,213,542` | Replace with `?` or proper error propagation |
| 11 | Ad-hoc `std::env::var` reads bypassing canonical helper | `crates/ragent-tools-vcs/src/vcs_provider.rs:198`, `crates/ragent-llm/src/providers/bedrock_credentials.rs:90,112,135,142,234`, `crates/ragent-llm/src/providers/router_client.rs:584-585`, etc. | Route through `ragent_config::credential_env::read_env` or add a `read_config_env` variant for non-secret values |
| 12 | Cargo audit ignore list suppresses 4 advisories | `.cargo/config.toml` | Review each RUSTSEC advisory; remove stale suppressions; add comments explaining remaining ones |
| 13 | Wildcard (`*`) version specifiers in ~90 dependencies | `crates/*/Cargo.toml`, root `Cargo.toml` `[workspace.dependencies]` | Replace `*` with minimum versions (e.g. `gcf = "3.0"`) |
| 14 | TUI tracing layer forwards unredacted log records to on-screen panel | `crates/ragent-tui/src/tracing_layer.rs:60-109` | Pipe log messages through `redact_secrets_cow` before display |
| 15 | TUI drains raw stderr lines unredacted into log panel | `crates/ragent-tui/src/lib.rs:848-859` | Pipe stderr lines through `redact_secrets_cow` before display |

## Complex (> 2 hours)

| # | Finding | File(s) | Fix |
|---|---------|---------|-----|
| 1 | 118 inline `#[cfg(test)]` modules in library source files | `crates/ragent-research/src/` (41), `crates/ragent-agent/src/` (29), `crates/ragent-llm/src/` (17), `crates/ragent-tui/src/` (10), others | Migrate each inline test module to `tests/` directory per the project's documented migration strategies (see `docs/reports/testconsolidate-completion.md`). This is a large mechanical effort — best done crate-by-crate with `cargo test` verification after each. |
| 2 | ~118 uses of `Result<T, String>` instead of `anyhow::Result<T>` or `thiserror` | `crates/ragent-tui/src/` (41), `crates/ragent-agent/src/` (16), `crates/ragent-tools-extended/src/` (15), `crates/ragent-connectors/src/` (14), `crates/ragent-plugins/src/` (8), others | Introduce `thiserror` error types per module (or use `anyhow::Result` for internal functions), then migrate call sites. This is a cross-cutting refactor requiring careful API boundary analysis. |
| 3 | Multiple `unsafe` blocks beyond the approved `kill_process_group` site | `crates/ragent-agent/src/mcp/mod.rs:1399,1418,1425`, `crates/ragent-agent/src/team/manager.rs:448` | Audit each `unsafe` block: document safety invariants with `// SAFETY:` comments, add `#[allow(unsafe_code)]` scoping, or replace with safe alternatives if possible. |
| 4 | Duplicate crate versions in Cargo.lock (50+ crates) | `Cargo.lock` | Run `cargo tree -d` to identify which crates pull in duplicates; update parent crates or accept duplication where semver-incompatible. Some duplicates (syn 2.x/3.x) require upstream fixes. |
| 5 | Blocking `std::fs` calls in async functions | `crates/ragent-tui/src/app/state.rs`, `crates/ragent-agent/src/session/history.rs`, `crates/ragent-agent/src/session/processor.rs`, `crates/ragent-agent/src/session/loop_capture.rs` | Migrate to `tokio::fs::` or wrap in `tokio::task::spawn_blocking`. Requires async signature changes at call sites. |
| 6 | Mixed `std::sync`/`tokio::sync` mutex usage (155 vs 28) | Across all crates | Establish a project convention (e.g., `tokio::sync` in async contexts, `std::sync` for short-lived locks) and migrate incrementally. |
| 7 | `file_name().and_then(|n| n.to_str()).unwrap_or("")` pattern repeated 4× | `crates/ragent-agent/src/agent/mod.rs:245,1814,1815`, `crates/ragent-tools-core/src/glob.rs:184` | Extract into a shared `path_file_name(path: &Path) -> &str` helper in `ragent-types`. |

## Suggested Fix Order

1. **Security Debug redaction (Quick Wins 11-13)**: Add `redacted_debug_impl!` to `GitLabIntegrationConfig`, `BackendConfig`, `OpenAiConfig` — prevents secret leaks via `{:?}` formatting. Highest ROI for security.
2. **Dependency hygiene (Quick Wins 8-10)**: `cargo update`, targeted dep bumps, clean `deny.toml` — reduces audit surface and picks up security patches.
3. **Dead wrapper removal (Quick Wins 3-4)**: Delete `mask_key`/`truncate_snippet` one-line wrappers — trivial cleanup, reduces confusion.
4. **Constant unification (Quick Win 2, 5)**: Fix snippet constant naming/typo, standardise redaction placeholder — trivial consistency wins.
5. **unwrap/expect cleanup (Quick Win 6, Medium 10)**: Fix the 2 `.unwrap()` and 9 `.expect()` production sites — reduces panic risk.
6. **Shared helpers (Medium 1-8)**: Extract atomic-write, corrupt-ledger, scan_dirs, resolve_api_key, html2text isolation, bearer header, HTTP scheme validation, secret-key detection into shared `ragent-surface`/`ragent-types` helpers. Start with atomic-write (15+ call sites, highest duplication).
7. **Error swallowing audit (Medium 9)**: Review 30+ `let _ =` sites; convert to `tracing::warn!` where appropriate.
8. **Env var routing (Medium 11)**: Route ad-hoc `std::env::var` reads through canonical helpers.
9. **TUI redaction (Medium 14-15)**: Add redaction to tracing layer and stderr drain in TUI.
10. **Wildcard dependency pinning (Medium 13)**: Replace `*` with minimum versions in `Cargo.toml` files.
11. **Test migration (Complex 1)**: Migrate 118 inline `#[cfg(test)]` modules to `tests/` directories, crate by crate (start with `ragent-research` — 41 files).
12. **Result<T, String> migration (Complex 2)**: Introduce `thiserror` types and migrate `Result<T, String>` usages.
13. **unsafe block audit (Complex 3)**: Document or replace the 4 unapproved `unsafe` blocks.
14. **Async/blocking fixes (Complex 5-6)**: Migrate blocking `std::fs` to `tokio::fs`, establish mutex convention.

## Verification

- [ ] `cargo fmt --check` passes
- [ ] `cargo clippy --workspace -- -D warnings -A clippy::used_underscore_items -A clippy::redundant_pub_crate -A clippy::wildcard_imports` passes
- [ ] `cargo check --workspace --all-targets` passes with 0 warnings
- [ ] `cargo test --workspace` passes (all tests green)
- [ ] `cargo audit` reports 0 vulnerabilities
- [ ] `cargo deny check` passes (advisories, bans, licenses, sources)
- [ ] No new inline `#[cfg(test)]` modules added (all new tests in `tests/`)
- [ ] No `unwrap()`/`expect()` on user-facing production paths
- [ ] All `unsafe` blocks have `// SAFETY:` comments and scoped `#[allow(unsafe_code)]`
- [ ] Scripts pass: `scripts/check-security-unwraps.sh`, `scripts/check-silent-errors.sh`, `scripts/check-inline-tests.sh`, `scripts/check-poison-locks.sh`
