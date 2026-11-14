# Code Audit Report

**Project**: Rust 2024 Cargo workspace - `ragent` AI coding agent (root binary + 19 library crates under `crates/`), `tracing` logging, `cargo nextest` with one consolidated `tests/all.rs` binary per crate
**Scope**: full project (source, tests, manifests, config; `target/`, `vendor/`, `.git/` excluded)
**Findings**: 91 total (20 high, 40 medium, 31 low)

**Implementation status (this pass):** Medium-effort items 1-10, 16, 20, 21 and
22 are implemented and verified (see the *Medium Effort* table below).
`cargo fmt --check`, `cargo clippy --workspace --all-targets` (zero warnings) and
the twelve code `check-*` guard scripts are clean, and
`cargo nextest run --workspace` is 16146/16146 (17 skipped). Medium 11-15 and
17-19 and 23-28, and every Complex item, remain open: the Complex table below
carries no status column because none of it is started.

**Review follow-ups (this pass).** Four points raised on the PR are addressed in
the code rather than the report: `guard::write_atomic` and the new
`guard::write_atomic_stream` are thin wrappers over `guard::write_atomic_with`,
whose `&dyn std::io::Write` sink takes the header, the borrowed body and the
trailing newline one part at a time, so `persist_task_output` never builds a
copy of the report - the completion handler keeps a single shared `Arc<str>`
(the same allocation `TaskEntry::result` stores) and streams that. A test asserts
the sink form, and a second asserts the target is untouched until every part
lands;
`test_vault_migrations` drives the production ledger write through `apply_pending`
from a vault that has **no** ledger at all (`read_version` reports a missing
ledger as version 0), so no `std::fs::write` reproduces the seed either, and
asserts the returned version, the ledger the loop created - read back and
compared against `migrations::ledger_bytes` - and the settled re-run;
the `cli_out` macro probe names the registered module
`test_cli_out_macros::test_macro_output_probe` (verified: that name selects
exactly one test, while the bare name selects none and still exits 0), is
neither `#[ignore]`d nor env-gated so a *wrong* filter panics instead of passing
silently, and the parent asserts the probe started, that the filter selected
exactly one test, and that the body finished; and every Complex item stays
unmarked, because none of the files this pass changed implements one.

**Fixed here: sidebar heading drops the description.** A project's `Properties`
description was shown only when the heading had at least six columns left after
its name and trailing hints, so at the default 32-column sidebar a long name plus
a remote kind (`laughing_kirch [github] v`) dropped it entirely and
`test_project_menu_properties_saves_description_and_backend` failed. The heading
now ranks name, then description, then remote kind and collapsed count, with the
chevron always kept: `laughing_kirch v (Agent home)`. The description takes the
room the name and chevron leave, and the kind/count are added only while they
still fit beside it, so nothing is pushed off the row.

---

## High Priority

### Standards
- Four parallel copies of the `Tool` trait, `ToolContext` and `ToolRegistry` with drifting field sets and doc comments, no shared definition — `crates/ragent-tools-core/src/lib.rs:173` and `crates/ragent-tools-extended/src/lib.rs:336` and `crates/ragent-tools-vcs/src/lib.rs:131` and `crates/ragent-agent/src/tool/mod.rs:583`
- Poison-recovery idiom inconsistent for the same lock class: `unwrap_or_else(std::sync::PoisonError::into_inner)` at ~206 sites vs `if let Ok(..) = x.lock()` at ~76 sites (a silent no-op the project's own FUNC-008 calls a bug) — `crates/ragent-agent/src/session/cache.rs:419` and `crates/ragent-agent/src/session/cache.rs:220` and `crates/ragent-plugins/src/host_api.rs:151`
- Two poisoning-recovery spellings inside one crate and even one file — `crates/ragent-agent/src/agent/custom.rs:92` and `crates/ragent-agent/src/agent/mod.rs:124`
- Error types hand-written (`impl Display` + `impl std::error::Error`) while 34 sibling error types in the same crates use `#[derive(thiserror::Error)]` — `crates/ragent-agent/src/backend/mod.rs:129` and `crates/ragent-research/src/manager.rs:46`
- `let _ = ctx.event_bus.publish(...)` where `publish` returns `()` (the binding is meaningless) while 232 other publish sites call it bare — `crates/ragent-agent/src/tool/structured_memory.rs:122` (also `:249`, `:366`, `:438`) and `crates/ragent-llm/src/providers/router_client.rs:333`

### Duplication
- Whole archive-download/extract pipeline duplicated across two crates with only the error type differing (`ArchiveKind`+`from_path`, `extract_archive_into`, `extract_archive`, `extract_zip`, `extract_tar_gz`, `safe_relative_path`; `MAX_EXTRACTED_BYTES`/`DOWNLOAD_TIMEOUT` declared in both) — `crates/ragent-plugins/src/add.rs:664` and `crates/ragent-connectors/src/manifest.rs:556`
- `fetch_bytes` copy-pasted between crates (blocking client + `https_only_redirects()` + timeout error mapping + redirect/status/size-cap ladder) — `crates/ragent-plugins/src/store_fetch.rs:195` and `crates/ragent-connectors/src/fetch.rs:275`
- `read_capped` (chunked read with running byte cap) implemented three times, two byte-identical; the third hardcodes an 8 KiB chunk instead of the shared 65 536 constant — `crates/ragent-connectors/src/fetch.rs:329` and `crates/ragent-plugins/src/store_fetch.rs:244` and `crates/ragent-surface/src/manifest.rs:137`
- `remove_trailing_commas` and its preceding markdown-fence-stripping block are verbatim copies — `crates/ragent-agent/src/team/swarm.rs:164` and `crates/ragent-research/src/web_gatherer/decomposer.rs:480`
- Process-wide shared `reqwest::Client` builder duplicated with the same four tuning constants (`MAX_IDLE_PER_HOST=8`, connect 30 s, request 120 s, 90 s pool idle, 1 min keepalive) — `crates/ragent-llm/src/providers/http_client.rs:18` and `crates/ragent-tools-vcs/src/http_client.rs:14`

### Logging
- `tracing::info!` dumps the raw Ollama Cloud done frame (`done_frame = %done_preview`, up to 500 bytes of streamed model output and tool-call arguments) with no `redact_secrets` pass, while the neighbouring parse-failure path in the same file does redact — `crates/ragent-llm/src/providers/ollama_cloud.rs:709`

### Security
- Project-local `.ragent/ragent.json` can set `automation.dispatch[].token_env` + `url`, and `merge_project` does **not** strip `automation` (unlike mcp/acp/openai/vcs/telemetry/hooks), so a hostile repo makes `resolve_token` read any process secret and POST it as a Bearer token to an attacker host — `crates/ragent-config/src/config.rs:3434`
- `resolve_token` reads an arbitrary environment-variable name taken straight from config with no namespace/allowlist guard (the connectors `env` shape has exactly that guard via `env_var_in_namespace`) — `crates/ragent-agent/src/automation/dispatch.rs:65` and `crates/ragent-connectors/src/auth.rs:543`
- Copilot token-exchange failure embeds the raw provider response body in the error string with no `redact_secrets` (every sibling provider redacts first) — `crates/ragent-llm/src/providers/copilot.rs:919`
- OpenAI Responses model-discovery failure embeds the unredacted error body into the error, which can surface as `StreamEvent::Error` — `crates/ragent-llm/src/providers/openai_responses.rs:175`

### Testing
- `ragent-hyperresearch` is almost entirely untested: 43 src files, only 3 test files, and most modules are 4-line placeholders - `core/config.rs:89` (`resolve`, 196 lines) and `core/migrations.rs:54` (`apply_pending`) have no test at all — `crates/ragent-hyperresearch/src/core/config.rs:89`
- bash security validation has no test for `validate_bash_syntax` or `strip_unquoted_backslash_escapes` (the SEC-029 escape-evasion defence); the inline suite covers only the other 5 layers — `crates/ragent-tools-core/src/bash.rs:926` and `crates/ragent-tools-core/src/bash.rs:782`
- `ragent-tools-vcs` gitlab pipeline tools (8 tools, 775 lines) have no behavioural test, only a registry-name assertion — `crates/ragent-tools-vcs/src/gitlab/gitlab_pipelines.rs:93` and `crates/ragent-tools-vcs/tests/test_vcs_registry_coverage.rs:112`
- root `src/` (5591 lines) has no test for `handle_new_command`/`handle_spec_status_command`/`handle_automation_command`/`parse_age_span`; only clap argument parsing is covered — `src/cli.rs:1553` and `src/cli.rs:1837` and `src/cli.rs:2112` and `src/cli.rs:387`
- `crates/ragent-storage/src/project_store.rs` (675 lines, ~20 mutators incl. `set_project_base_model`, `archive_project`, `create_session_in_project`) has no test referencing `project_store` by name — `crates/ragent-storage/src/project_store.rs:1`

## Medium Priority

### Standards
- `.ok();` discards a poison result with no `// INTENTIONAL:` / `// no-error-ok:` marker and no recovery, unlike sibling files in the same crate — `crates/ragent-plugins/src/lifecycle.rs:636`
- Async-trait attribute spelling mixed: bare `#[async_trait]` with a `use` (50 sites) vs fully-qualified `#[async_trait::async_trait]` (218 sites) — `crates/ragent-agent/src/mcp/mod.rs:208` and `crates/ragent-agent/src/tool/mod.rs:770`
- Error-context idiom inconsistent: `.context()`/`.with_context()` (614 uses) vs `map_err(|e| anyhow::anyhow!(...))` (74 uses); `task.rs` uses only the latter while its own crate's tool files use `.context(` — `crates/ragent-tools-extended/src/task.rs:308`
- `anyhow!` message formatting mixed: positional `"…: {}", e` (9 sites) vs inline capture `"…: {e}"` — `crates/ragent-config/src/config.rs:2022` and `crates/ragent-tools-extended/src/task.rs:308`
- `Tool::name()` impls return `&'static str` (169) while the trait and 17 impls use `&str`; both spellings appear in one file — `crates/ragent-agent/src/tool/mod.rs:890` and `crates/ragent-agent/src/tool/mod.rs:772`
- Tool `parameters_schema` delegates to a bespoke private helper instead of the two idioms every other tool uses (inline `json!` or `ragent_types::tool_args::no_parameters_schema()`) — `crates/ragent-agent/src/tool/model_info.rs:71` and `crates/ragent-tools-core/src/bash_reset.rs:31`

### Duplication
- Credential-keyed JSON redaction walk (`MAX_REDACT_DEPTH` + `redact_secret_values`/`redact_named_keys` + `redact_matched_values`) reimplemented instead of using the shared walker; both duplicate `ragent_types::sanitize::redact_json_value` and the depth limits disagree (8 vs 16) — `crates/ragent-server/src/routes/mod.rs:331` and `crates/ragent-llm/src/providers/wire/trace.rs:52` and `crates/ragent-types/src/sanitize.rs:384`
- `masked_url` one-line wrapper around `ragent_types::sanitize::mask_url_credentials` repeated in four modules — `crates/ragent-tools-extended/src/masterfetch/tools/crawl_tool.rs:431` and `crates/ragent-tools-extended/src/archdoc/url_source.rs:364` and `crates/ragent-research/src/document.rs:2378`
- User-agent literal hardcoded at ~19 call sites instead of one shared constant, and versions have already drifted (`"ragent/0.1"` x13, `"ragent/0.1.0"` x4, `"ragent/1.0 (…)"` x1, `"ragent/0.1 (…)"` x2) — `crates/ragent-tools-vcs/src/github/client.rs:166` and `crates/ragent-tools-vcs/src/gitlab/client.rs:85`
- Word-boundary truncation helper reimplemented three times with near-identical bodies, plus a byte-identical `truncate_str` pair — `crates/ragent-research/src/deck.rs:2372` and `crates/ragent-research/src/deck_enrich.rs:361` and `crates/ragent-research/src/deck_pptx.rs:593` and `crates/ragent-tui/src/widgets/message_widget.rs:108`
- ANSI/control-character scrubber reimplemented in the TUI instead of using the shared sanitizer, and is weaker (no bidi stripping; treats any ASCII letter as an ESC terminator) — `crates/ragent-tui/src/app/helpers.rs:348` and `crates/ragent-types/src/sanitize_terminal.rs:44`
- `escape_pipe` duplicated in the same crate with diverging behaviour (one escapes `|` only, the other also collapses `\r`/`\n`) — `crates/ragent-research/src/comparison.rs:311` and `crates/ragent-research/src/document.rs:2150`
- Terminal-status predicate for `MemberStatus` hand-written instead of calling the model's own method, and disagrees with it by including `ShuttingDown` — `crates/ragent-tui/src/layout_teams.rs:180` and `crates/ragent-tui/src/app/windows.rs:343` and `crates/ragent-agent/src/team/config.rs:239`
- `STREAM_CHUNK_BYTES: usize = 65_536` declared in four files of two crates — `crates/ragent-connectors/src/fetch.rs:45` and `crates/ragent-connectors/src/manifest.rs:55` and `crates/ragent-plugins/src/store_fetch.rs:34` and `crates/ragent-plugins/src/add.rs:72`
- Archive download byte-counter loop copy-pasted (Content-Length pre-check, chunk vec, running-total refusal with the same error shape) — `crates/ragent-plugins/src/add.rs:639` and `crates/ragent-connectors/src/manifest.rs:528`

### Logging
- Orphan 151-byte reproducer file at the repo root, referenced nowhere — `slice_panic.rs:1`
- 40 KB brace-nesting parser stress fixture tracked in git under `secverify/`, referenced by no build, test, or script — `secverify/rustproj/deep.rs:1`
- Runtime debug artifact committed: a crash-marker spool file (pid/exe/cwd/status) captured from a debugging run is tracked — `scripts/log/panics/last-crash.json:1`
- `tracing::warn!` fires from inside the per-frame render path and re-sets `tasks_cache_dirty = true`, so a persistently failing SQLite query logs one WARN per rendered frame — `crates/ragent-tui/src/layout.rs:7262`
- `tracing::info!` inside the per-tool-call loop of the tool phase: one INFO line per tool call per step for alias-spelling resolution, per-item chatter that belongs at `debug`/`trace` — `crates/ragent-agent/src/session/processor.rs:3471`
- `tracing::info!` for every excluded search hit in the relevance filter (same pattern at `:2469`, `:2497`, `:2559`, `:2861`), so a query with hundreds of hits emits hundreds of INFO records — `crates/ragent-research/src/web_gatherer.rs:2577`
- Debug-leftover comment left in production code ("// Log with actual deps for debugging") — `crates/ragent-tui/src/app/swarm.rs:511`
- Stream-loop debug scaffolding: a diagnostic comment appended to the closing brace of a `match` with the following statement re-indented to a non-standard column — `crates/ragent-llm/src/providers/ollama_cloud.rs:578`

### Security
- HuggingFace 401/403 branches `bail!` with the raw `body_text` before the `redact_secrets` call, so credentialed bodies escape unredacted — `crates/ragent-llm/src/providers/huggingface.rs:471`
- `.gitignore` omits `.git-credentials` (verified NOT ignored by the repo), `.cursor/`, `.aider*`; `.claude/settings.local.json` is covered only by a user-global ignore file, not the repo — `.gitignore:83`
- Container credentials are injected as argv `-e NAME=VALUE`, so the plaintext secret is visible in the host process table (`ps`) for the life of the `docker run` — `crates/ragent-agent/src/backend/container.rs:334`
- Hardcoded symmetric XOR key `ragent-obfuscation-key-v1` plus a legacy key derived from `USER`+`HOME`, still used to decrypt stored provider API keys — `crates/ragent-storage/src/storage.rs:129`
- AWS-shaped credential literals committed in the quickstart (an `AKIA…` access key id and a 40-char secret) that a reader can mistake for live values — `QUICKSTART.md:159`
- Ad-hoc `std::env::var` reads at the point of use that bypass the validated `ragent_config::credential_env` layer (`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `OLLAMA_HOST`) — `crates/ragent-tui/src/app/models.rs:834`

### Testing
- Tautological assertion that can never fail: `assert!(result.is_empty() || !result.is_empty())` — `crates/ragent-tui/tests/test_configured_provider_selection.rs:185`
- Duplicated tautology `assert!(builtin_allow.is_empty() || !builtin_allow.is_empty())`, and the same fact is already asserted properly elsewhere — `crates/ragent-tui/tests/test_dirs_show.rs:8` and `crates/ragent-config/tests/test_dir_lists_runtime.rs:69`
- `test_mask_key_short`/`test_mask_key_long` are byte-identical copies across 5 engine test files although all 5 production `mask_key` fns delegate to one shared `mask_api_key` — `crates/ragent-tools-extended/tests/test_mf_serper.rs:129` and `crates/ragent-tools-extended/tests/test_mf_langsearch.rs:175` and `crates/ragent-tools-extended/tests/test_mf_tavily.rs:131` and `crates/ragent-tools-extended/tests/test_mf_perplexity.rs:254` and `crates/ragent-tools-extended/tests/test_mf_exa.rs:483`
- `test_truncate_query_unicode_boundaries` duplicated with cosmetically different inputs across 5 engine files — `crates/ragent-tools-extended/tests/test_mf_openalex.rs:38` and `crates/ragent-tools-extended/tests/test_mf_tavily.rs:28` and `crates/ragent-tools-extended/tests/test_mf_perplexity.rs:32` and `crates/ragent-tools-extended/tests/test_mf_exa.rs:164` and `crates/ragent-tools-extended/tests/test_mf_wikipedia.rs:347`
- Leftover scratch probe registered in the TUI test binary: `probe_gaps` only `println!`s and asserts nothing ("delete before finishing") — `crates/ragent-tui/tests/test_probe_t022.rs:48` and `crates/ragent-tui/tests/all.rs:608`
- `run_target` is exercised only through `MockBenchModelRunner`, so the real generation/exec path is untested; `write_benchmark_workbook`'s resume-state and debug-sidecar paths are never read back — `crates/ragent-bench/src/runner.rs:218` and `crates/ragent-bench/src/workbook.rs:257`
- `crates/ragent-server/src/routes/research.rs` (1096 lines, 7 handlers) and the OpenAI streaming path are thin: `prepare_turn`/`run_completion` error and cancellation branches are not covered — `crates/ragent-server/src/routes/openai.rs:457` and `crates/ragent-server/src/routes/openai.rs:537`

### Dependencies
- 28 duplicate-version crates in the lockfile (`syn` 1/2/3, `getrandom` x3, `hashbrown` x3, `rand` x3, `rand_core` x3, `phf_shared`/`phf_generator` x3-4, `base64` x2, `crossterm` 0.28.1/0.29.0, `nom` 7/8, `strum` x2, `rustix` x2, `windows-sys` x3, `html5ever` 0.26.0/0.38.0, `markup5ever` x2, `tendril` x2, `siphasher` x2, `weezl` x2, `miniz_oxide` x2, `heck` x2, `linux-raw-sys` x2, `string_cache*` x2), driven by `printpdf`→`azul-layout`, `readability-rs` (pins `html5ever 0.26`), and a direct `crossterm 0.28` beside `ratatui`'s 0.29 — `Cargo.lock (cargo deny check bans: 28 warnings)`
- `lopdf` three breaking releases behind — `lopdf@0.44.0 (0.44.0 -> 0.47.0)`
- `pdf-extract` vendored fork two breaking releases behind upstream, shadowing the registry version so `outdated`/`audit` cannot see the delta — `pdf-extract@0.10.0 (0.10.0 -> 0.12.1)`
- `sysinfo` two breaking releases behind — `sysinfo@0.38.4 (0.38.4 -> 0.40.3)`

## Low Priority

### Standards
- `#[must_use]` on `pub fn new() -> Self` applied inconsistently (50 of 64), including adjacent constructors in the same file — `crates/ragent-tools-core/src/lib.rs:128` and `crates/ragent-tools-core/src/lib.rs:529`
- Chaining setter pattern applied to a single type: `set_*` returns `&mut Self` here while 23 other `set_*` methods in the workspace return `()` — `crates/ragent-research/src/item.rs:136` and `crates/ragent-agent/src/mcp/enable_state.rs:107`
- Tool name casing diverges from the snake_case convention used by every other registry key — `crates/ragent-tools-extended/src/task.rs:20` and `crates/ragent-agent/src/tool/plan.rs:1`
- `#[allow(dead_code)]` on a `pub` production function, contradicting the dead-code guard's intent — `crates/ragent-agent/src/session/prompt_builders.rs:175`

### Duplication
- `Retry-After` parsing + clamping duplicated with separately-defined limits — `crates/ragent-llm/src/providers/http_client.rs:496` and `crates/ragent-tools-vcs/src/gitlab/client.rs:757`
- `truncate_at_char_boundary` variants re-derived from `char_indices().nth()` rather than the shared helpers — `crates/ragent-agent/src/session/history.rs:253` and `crates/ragent-research/src/session/topic.rs:437`

### Logging
- Misleading `tracing::info!("Server auth token configured")` logged unconditionally at server start, whether or not any token is configured — `crates/ragent-server/src/routes/mod.rs:148`
- `tracing::error!` used for a non-error condition: the "acp-server feature not compiled in" help text is emitted at ERROR level — `src/acp_server.rs:62` (also `:88`)
- Entire function kept alive only by `#[allow(dead_code)]` (`finalize_assistant_message`, no caller) — `crates/ragent-agent/src/session/loop_steps.rs:1778`
- Dead-code suppressions retained purely for debug/profiling scaffolding (`llm_request_start`, `should_break`) — `crates/ragent-agent/src/session/loop_steps.rs:138`
- `base_url` logged unredacted (`base_url = %base_url`) instead of through `mask_url_credentials` — `crates/ragent-llm/src/providers/sse.rs:409` (same pattern at `crates/ragent-llm/src/providers/copilot.rs:1006`)
- `tracing::info!` emitted on every 30 s cron tick that finds due events, routine scheduler chatter that belongs at `debug` — `crates/ragent-tui/src/app/cron.rs:193`

### Security
- Secret-registry seed omits `HUGGINGFACE_API_KEY` / `HUGGING_FACE_HUB_TOKEN` (both accepted by `provider_credential_env` for huggingface), so an echoed value reaches the log/SSE unredacted — `src/main.rs:804`
- `GITLAB_URL` is read ad-hoc in three separate places, each re-implementing the same validation, rather than through one config accessor — `crates/ragent-tools-vcs/src/gitlab/auth.rs:149`
- `get_env` is registered under the `file:read` permission category, so an inspect-only sub-agent scope grants environment reads (mitigated only by name/value redaction) — `crates/ragent-tools-core/src/get_env.rs:121`
- Four near-identical env-read helpers coexist with differing blank/trim rules — `crates/ragent-config/src/credential_env.rs:19` and `crates/ragent-config/src/config.rs:1770` and `crates/ragent-hyperresearch/src/core/config.rs:149`

### Testing
- `crates/ragent-tui/src/project_properties.rs` (634 lines: `open`, `handle_key`, `handle_paste`, `handle_mouse`, `render`) is only reached indirectly; `handle_paste`/`handle_mouse` have no direct test — `crates/ragent-tui/src/project_properties.rs:413` and `crates/ragent-tui/src/project_properties.rs:442`
- Fragile exact-output assertions: 75 TUI tests assert on raw `screen_text`/painted buffers and 15 assert exact glyph strings, so any cosmetic layout change breaks them — `crates/ragent-tui/tests/test_output_view_live_steps.rs:242` and `crates/ragent-tui/tests/test_misc_modules_deep.rs:1249`
- Repeated ad-hoc setup that should live in `tests/support/`: 9 crates have no `support/` module (124 `Storage::open_in_memory` constructions, 7 inline `AgentInfo::new`, the same `#[path = "../src/session/prompt_builders.rs"]` reimported twice) — `crates/ragent-agent/tests/test_prompt_builders_more.rs:13` and `crates/ragent-agent/tests/test_session_text_helpers.rs:48`

### Dependencies
- `rusqlite` one breaking release behind — `rusqlite@0.41.1 (0.41.1 -> 0.42.0)`
- `tower-http` one breaking release behind — `tower-http@0.6.11 (0.6.11 -> 0.7.1)`
- `html2text` vendored fork one breaking release behind — `html2text@0.16.7 (0.16.7 -> 0.17.4)`
- `rand` one breaking release behind; `rand 0.10.3` is already in the tree transitively — `rand@0.9.5 (0.9.5 -> 0.10.3)`
- `rust_xlsxwriter` one breaking release behind — `rust_xlsxwriter@0.101.0 (0.101.0 -> 0.102.0)`
- `zip` five minor releases behind, carrying further extraction-path hardening — `zip@9.1.2 (9.1.2 -> 9.5.1)`
- `rmcp` minor-behind on the MCP client surface — `rmcp@3.9.0 (3.9.0 -> 3.10.0)`
- Two transitive crates a full major behind their parent's requirement — `pest_generator->syn@2.0.119 (2.0.119 -> 3.0.10)` and `getrandom->r-efi@5.3.0 (5.3.0 -> 6.0.0)`
- No git dependencies, but two path deps bypass registry checksum pinning and their upstream base commit is not recorded — `vendor/pdf-extract@0.10.0` and `vendor/html2text@0.16.7`
- Toolchain channel is floating `stable`, not a pinned version, while MSRV is 1.91 and the installed compiler is already 1.100.0 — `rust-toolchain.toml:2`
- 18 `warning[wildcard]` entries on workspace-internal path deps, intentional per `deny.toml` but unsilenceable if `wildcards` is raised to `"deny"` — `crates/ragent-agent/Cargo.toml:18` and `crates/ragent-agent/Cargo.toml:55`
- `cargo outdated --root-deps-only` sees only 1 of the 15 stale direct deps because it resolves against the root package rather than `[workspace.dependencies]` — `cargo outdated --root-deps-only (insufficient gate)`

---

# Implementation Plan

## Quick Wins (< 30 min each)

| # | Finding | File(s) | Fix |
|---|---------|---------|-----|
| 1 | Raw Ollama Cloud done frame logged unredacted | `crates/ragent-llm/src/providers/ollama_cloud.rs:709` | Wrap `done_preview` in `ragent_types::sanitize::redact_secrets` before the `tracing::info!`, matching the parse-failure branch three lines above |
| 2 | Copilot token-exchange error embeds raw body | `crates/ragent-llm/src/providers/copilot.rs:919` | Pipe `error_body` through `redact_secrets` in the `bail!`, as every sibling provider does |
| 3 | OpenAI Responses error body unredacted | `crates/ragent-llm/src/providers/openai_responses.rs:175` | Redact `body` before the `bail!`; apply the same to the stream-error path below it |
| 4 | HuggingFace 401/403 bail before redaction | `crates/ragent-llm/src/providers/huggingface.rs:471` | Move the `redact_secrets` call above the status branches, or redact `body_text` at both `bail!` sites |
| 5 | `resolve_token` accepts any env-var name from config | `crates/ragent-agent/src/automation/dispatch.rs:65` | Reuse `env_var_in_namespace` (or an equivalent allowlist) to reject names outside an agreed prefix |
| 6 | `automation` section not stripped by `merge_project` | `crates/ragent-config/src/config.rs:3434` | Add `automation` to the project-overlay deny list beside `mcp`/`acp`/`openai`/`vcs`/`telemetry`/`hooks`; add a regression test |
| 7 | `.gitignore` omits `.git-credentials`, `.cursor/`, `.aider*` | `.gitignore:83` | Add the three entries; add `.claude/settings.local.json` so the repo does not rely on a user-global ignore |
| 8 | Orphan reproducer file at repo root | `slice_panic.rs:1` | Delete the file |
| 9 | Git-tracked runtime crash spool | `scripts/log/panics/last-crash.json:1` | Delete and add `scripts/log/` to `.gitignore` |
| 10 | Unused parser stress fixture | `secverify/rustproj/deep.rs:1` | Delete the file and the `secverify/` tree if nothing else lives there |
| 11 | Tautological assertions | `crates/ragent-tui/tests/test_configured_provider_selection.rs:185`, `crates/ragent-tui/tests/test_dirs_show.rs:8` | Replace with a real assertion (assert the call does not panic and returns a `Vec`, or delete the line) |
| 12 | Scratch probe asserting nothing | `crates/ragent-tui/tests/test_probe_t022.rs:48` | Convert `probe_gaps` into a real assertion on the terminating-row exclusion, or delete the file and its `all.rs` line |
| 13 | `let _ = …publish(…)` where `publish` returns `()` | `crates/ragent-agent/src/tool/structured_memory.rs:122` (+ `:249`, `:366`, `:438`), `crates/ragent-llm/src/providers/router_client.rs:333` | Drop the meaningless `let _ =` binding (5 sites) |
| 14 | Debug-leftover comment | `crates/ragent-tui/src/app/swarm.rs:511` | Delete the comment |
| 15 | Re-indented stream-loop scaffolding | `crates/ragent-llm/src/providers/ollama_cloud.rs:578` | Move the comment to its own line above the statement and re-indent to rustfmt defaults |
| 16 | `tracing::warn!` inside per-frame render path | `crates/ragent-tui/src/layout.rs:7262` | Downgrade to `debug!`, or log once per failure streak rather than per frame |
| 17 | Per-tool-call INFO chatter | `crates/ragent-agent/src/session/processor.rs:3471` | Downgrade to `debug!` |
| 18 | Per-hit INFO chatter in search filter | `crates/ragent-research/src/web_gatherer.rs:2577` (+ `:2469`, `:2497`, `:2559`, `:2861`) | Downgrade to `debug!` |
| 19 | Cron tick INFO every 30 s | `crates/ragent-tui/src/app/cron.rs:193` | Downgrade to `debug!` |
| 20 | Misleading "Server auth token configured" | `crates/ragent-server/src/routes/mod.rs:148` | Log only when a token is actually set, and say which |
| 21 | ERROR level for non-error ACP help | `src/acp_server.rs:62` (also `:88`) | Downgrade to `warn!`/`error!` only on the genuine failure path |
| 22 | Unredacted `base_url` in warnings | `crates/ragent-llm/src/providers/sse.rs:409`, `crates/ragent-llm/src/providers/copilot.rs:1006` | Wrap in `ragent_types::sanitize::mask_url_credentials` |
| 23 | Secret-registry seed omits HuggingFace vars | `src/main.rs:804` | Add `HUGGINGFACE_API_KEY` and `HUGGING_FACE_HUB_TOKEN` to the seed array |
| 24 | `cargo update -p zip` / `-p rmcp` / `-p uuid` | `Cargo.lock` | Within-major lock bumps; no manifest edit needed |
| 25 | `sysinfo`, `rusqlite`, `tower-http`, `rust_xlsxwriter` bumps | `Cargo.toml:265`, `:224`, `:232`, `:270` | Bump the version strings; `cargo check` per affected crate |
| 26 | Toolchain channel floating | `rust-toolchain.toml:2` | Pin `channel = "1.91"` (or the current CI compiler) |
| 27 | AWS-shaped literals in quickstart | `QUICKSTART.md:159` | Replace with obviously-fake placeholders (`AKIAIOSFODNN7EXAMPLE` style is still recognisable; use `your-access-key-id`) |
| 28 | Vendored forks record no upstream base commit | `Cargo.toml:316`, `Cargo.toml:320` | Extend both comments with the upstream tag/commit the fork is based on |
| 29 | `#[allow(dead_code)]` on a `pub` function | `crates/ragent-agent/src/session/prompt_builders.rs:175` | Delete the function if unused, or document why it must stay |
| 30 | Dead profiling scaffolding | `crates/ragent-agent/src/session/loop_steps.rs:138`, `:1778` | Delete `llm_request_start`, `should_break`, `finalize_assistant_message` if no caller exists |

## Medium Effort (30 min - 2 hours each)

| # | Finding | File(s) | Fix |
|---|---------|---------|-----|
<<<<<<< Updated upstream
| 1 **Done** | `validate_id` untested | `crates/ragent-hyperresearch/src/core/vault.rs:104` | Add `tests/test_vault_id.rs` covering empty, `..`, `/`, `\`, leading `.`, and a valid id |
| 2 **Done** | `Untrusted::fence` untested | `crates/ragent-hyperresearch/src/core/untrusted.rs:30` | Add a test where the body contains the fence marker and assert it is sanitised |
| 3 **Done** | `prompt_for_permission` untested | `crates/ragent-agent/src/session/permissions.rs:529` | Add timeout/deny/allow tests over a scripted permission channel |
| 4 **Done** | `build_analyzer_request` untested | `crates/ragent-agent/src/security/mod.rs:104` | Assert the rendered prompt and JSON schema for a fixed tool input |
| 5 **Done** | Team tool handlers untested | `tool/team_task_list.rs:10`, `team_approve_plan.rs:14`, `team_memory_common.rs:1` | One test per handler, plus the reject-without-feedback error branch |
| 6 **Done** | GitLab pipeline `execute` paths untested | `crates/ragent-tools-vcs/src/gitlab/gitlab_pipelines.rs:93` | Mock the client and assert request shape + rendered output for each tool |
| 7 **Done** | Vault migrations untested | `crates/ragent-hyperresearch/src/core/migrations.rs:54` | Round-trip a v0 vault through `apply_pending` and assert the version bump |
| 8 **Done** | `cli_out` macros untested | `src/cli_out.rs:49` | Capture the writer and assert routing/format for each macro |
| 9 **Done** | `ragent-read_capped` ×3 | `store_fetch.rs:244`, `fetch.rs:329`, `manifest.rs:137` | Hoist one generic `read_capped` (parameterised over the error) into `ragent-types` |
| 10 **Partly done** | 51 `truncate*` helpers | `ragent_types::strutil` callers | Replace each body with a delegation to `truncate_chars`/`truncate_bytes`; fix the `...`/`…` disagreement |
| 11 | Plugin/connector store + marketplace + commands duplication | `store.rs`, `store_index.rs`, `marketplace_ops.rs`, `commands.rs` in both crates | Extract a generic store module (kind parameter over manifest name and error type) and have both crates instantiate it |
| 12 | `write_spec` duplicate | `ragent-tui/src/app/slash.rs:152`, `src/cli.rs:1917` | Move into `ragent-specs` and call from both |
| 13 | `McpProbe` duplicate | `ragent-tui/src/app/connector.rs:428`, `src/connectors.rs:317` | Move the probe into `ragent-connectors` and share |
| 14 | Retryable-error keyword lists duplicate | `ragent-bench/src/model.rs:531`, `ragent-agent/src/session/history.rs:716` | Extract `is_retryable_error(&str)` + status classifier into a shared crate |
| 15 | Three backoff schedules | `http_client.rs:350`, `team/manager.rs:115`, `bench/model.rs:504` | One `Backoff` helper with a configurable base/cap/jitter |
| 16 | TUI browse-panel renderers near-duplicate | `ragent-tui/src/layout.rs:642` vs `:1540` | Parameterise one renderer over a row-source trait; keep the panel-specific headings as data |
| 17 | 29 identical browser state-machine functions | `ragent-tui/src/app/state.rs:2001`, `ragent-connectors/src/browse.rs:76` | Extract a shared `BrowseState` (query, category, marketplace, scroll, cursor) generic over the entry type |
| 18 | SSE stream loop duplicated across providers | `providers/anthropic.rs:460`, `azure_resource.rs:528`, + 7 preamble sites | Route all providers through `providers/sse.rs`: one `consume_sse` taking a per-event closure |
| 19 | Tool-name casing / file-name mismatch | `tools-core` tool files | Pick PascalCase for the user-facing names (already 28) or snake_case, and rename files to match the registered name |
| 20 **Done** | Atomic write re-implemented 4× | `specs/io.rs:135`, `mcp/enable_state.rs:94`, `task/mod.rs:62`, `config.rs:2469` | One `atomic_write(path, bytes)` using the hardened `create_new` form |
| 21 **Done** | Blocking sleep in async path | `mcp/npx_cache.rs:179` | Replace with `tokio::time::sleep` and await it |
| 22 **Done** | Error handling `?` vs `let _ =` | `mcp/enable_state.rs:89`, `npx_cache.rs:114`, `task/mod.rs:87` | Propagate or log the discarded errors |
| 23 | Test fixture duplication | 23 TUI `make_app`, 22 `make_agent`, 3 server `AppState` | Delete locals, import from `tests/support`; extend `ragent-server/tests/support/mod.rs` |
| 24 | 52 test files write into `std::env::temp_dir()` | 52 test files | Point at `target/temp/<unique-name>` resolved from `env!("CARGO_MANIFEST_DIR")` |
| 25 | Vendored `pdf-extract` two breaking releases behind | `vendor/pdf-extract` | Bump to 0.12.1 and re-apply the local patch, or document the fork reason |
| 26 | Vendored `html2text` one breaking release behind | `vendor/html2text` | Bump to 0.17.4 and re-apply the local patch |
| 27 | `paste` unmaintained advisory | `Cargo.lock` | Clears with the orphan-entry prune (#21); otherwise swap to `pastey` |
| 28 | Duplicate crate versions in lockfile | `Cargo.lock` | `cargo update` the trailing copies (base64, crossterm, rand, phf, nix, bitflags) one at a time, checking `cargo tree -i` |
=======
| 1 | `read_capped` implemented three times | `crates/ragent-connectors/src/fetch.rs:329`, `crates/ragent-plugins/src/store_fetch.rs:244`, `crates/ragent-surface/src/manifest.rs:137` | Move one implementation into `ragent-types` (or `ragent-surface`) and have all three call it; delete the duplicated `STREAM_CHUNK_BYTES` constants with it |
| 2 | `fetch_bytes` copy-pasted across crates | `crates/ragent-plugins/src/store_fetch.rs:195`, `crates/ragent-connectors/src/fetch.rs:275` | Extract to a shared helper generic over the error constructor; keep the per-crate error types via a `map_err` at the call site |
| 3 | Archive download/extract pipeline duplicated | `crates/ragent-plugins/src/add.rs:664`, `crates/ragent-connectors/src/manifest.rs:556` | Extract `ArchiveKind`/`extract_*`/`safe_relative_path` into one module used by both crates; this also removes the duplicated `MAX_EXTRACTED_BYTES`/`DOWNLOAD_TIMEOUT`/byte-counter loop |
| 4 | `reqwest::Client` builder duplicated | `crates/ragent-llm/src/providers/http_client.rs:18`, `crates/ragent-tools-vcs/src/http_client.rs:14` | Hoist the four tuning constants and the builder into `ragent-types`; both crates construct from it |
| 5 | `remove_trailing_commas` + fence-stripping verbatim | `crates/ragent-agent/src/team/swarm.rs:164`, `crates/ragent-research/src/web_gatherer/decomposer.rs:480` | Move both to a shared JSON-recovery helper in `ragent-types` |
| 6 | Credential-keyed redaction walk reimplemented twice | `crates/ragent-server/src/routes/mod.rs:331`, `crates/ragent-llm/src/providers/wire/trace.rs:52` | Both should call `ragent_types::sanitize::redact_json_value`; reconcile the 8-vs-16 depth limits in one constant |
| 7 | `truncate_words`/`truncate_str` reimplemented four times | `crates/ragent-research/src/deck.rs:2372`, `deck_enrich.rs:361`, `deck_pptx.rs:593`, `crates/ragent-tui/src/widgets/message_widget.rs:108` | Consolidate on `ragent_types::strutil`; delete the local copies |
| 8 | Weaker ANSI scrubber in the TUI | `crates/ragent-tui/src/app/helpers.rs:348`, `crates/ragent-types/src/sanitize_terminal.rs:44` | Have `sanitize_for_display` delegate to `sanitize_terminal`; verify the TUI tests still pass |
| 9 | `escape_pipe` duplicated with diverging behaviour | `crates/ragent-research/src/comparison.rs:311`, `crates/ragent-research/src/document.rs:2150` | Keep the stricter version in one module and import it in both |
| 10 | User-agent literal at ~19 sites and drifting | `crates/ragent-tools-vcs/src/github/client.rs:166`, `crates/ragent-tools-vcs/src/gitlab/client.rs:85`, `crates/ragent-agent/src/reference/resolve.rs:31`, `crates/ragent-tools-extended/src/webfetch.rs:18` | One `USER_AGENT` constant using `concat!("ragent/", env!("CARGO_PKG_VERSION"))`, re-exported from `ragent-types` |
| 11 | `masked_url` wrapper repeated four times | `crates/ragent-tools-extended/src/masterfetch/tools/crawl_tool.rs:431`, `fetch.rs:253`, `archdoc/url_source.rs:364`, `crates/ragent-research/src/document.rs:2378` | Call `mask_url_credentials` directly at the use sites |
| 12 | Hand-written `BackendError` vs thiserror | `crates/ragent-agent/src/backend/mod.rs:129` | Convert to `#[derive(thiserror::Error)]` with `#[error("execution backend {backend} failed ({error_kind:?}): {message}")]` |
| 13 | Poison-recovery idioms inconsistent (~206 vs ~76 sites) | `crates/ragent-agent/src/session/cache.rs:419`, `crates/ragent-plugins/src/host_api.rs:151` | Pick `unwrap_or_else(PoisonError::into_inner)` as canonical, add one shared `lock_or_recover` helper, and convert the `if let Ok(..)` sites crate by crate (a scripted sweep reviewed per crate) |
| 14 | `.ok();` without an INTENTIONAL marker | `crates/ragent-plugins/src/lifecycle.rs:636` | Add the required `// INTENTIONAL: <reason>` comment or recover properly |
| 15 | `async_trait` spelling mixed (50 vs 218) | `crates/ragent-agent/src/mcp/mod.rs:208` | Standardise on the fully-qualified `#[async_trait::async_trait]`; drop the now-unused `use async_trait::async_trait` imports |
| 16 | Error-context idiom mixed (614 vs 74) | `crates/ragent-tools-extended/src/task.rs:308` | Convert the 74 `map_err(|e| anyhow!(...))` sites to `.context(..)`; also fixes the `{}` vs `{e}` formatting split |
| 17 | `Tool::name()` return type mixed | `crates/ragent-agent/src/tool/mod.rs:890` | Widen to `&'static str` in the trait, or leave `&str` and drop the extra `'static` — pick one and apply |
| 18 | `get_env` under `file:read` | `crates/ragent-tools-core/src/get_env.rs:121` | Move to its own read-only category (e.g. `system:read`) and update `READ_ONLY_PERMISSION_CATEGORIES` |
| 19 | Ad-hoc env reads bypassing `credential_env` | `crates/ragent-tui/src/app/models.rs:834`, `crates/ragent-tools-vcs/src/gitlab/auth.rs:149` | Route `AWS_ACCESS_KEY_ID`/`AWS_SECRET_ACCESS_KEY`/`OLLAMA_HOST`/`GITLAB_URL` through the config accessors; fold the four env-read helpers into one |
| 20 | Container secrets in argv | `crates/ragent-agent/src/backend/container.rs:334` | Pass credentials via `--env-file` (a 0600 temp file removed after spawn) instead of `-e NAME=VALUE` |
| 21 | Missing bash-layer tests | `crates/ragent-tools-core/src/bash.rs:926`, `:782` | Add `tests/test_bash_syntax_validation.rs` covering `validate_bash_syntax` and `strip_unquoted_backslash_escapes` incl. escape-evasion cases |
| 22 | GitLab pipeline tools untested | `crates/ragent-tools-vcs/src/gitlab/gitlab_pipelines.rs:93` | Add `tests/test_gitlab_pipelines_tools.rs` with a mock HTTP layer covering all 8 tools' success and error branches |
| 23 | `src/cli.rs` handlers untested | `src/cli.rs:1553`, `:1837`, `:2112`, `:387` | Add `tests/test_cli_handlers.rs` calling the four handlers directly (they are `pub`), including `parse_age_span` error cases |
| 24 | `project_store.rs` untested | `crates/ragent-storage/src/project_store.rs:1` | Add `crates/ragent-storage/tests/test_project_store.rs` over `open_in_memory`, one test per mutator |
| 25 | Duplicated engine test fixtures | `crates/ragent-tools-extended/tests/test_mf_serper.rs:129`, `test_mf_langsearch.rs:175`, `test_mf_tavily.rs:131`, `test_mf_perplexity.rs:254`, `test_mf_exa.rs:483` | Move `mask_key` and `truncate_query` assertions into one `tests/support/search_engine_contract.rs` helper and call it from each engine file |
| 26 | `lopdf` + `pdf-extract` fork upgrades | `Cargo.toml:277`, `Cargo.toml:276`, `vendor/pdf-extract/Cargo.toml:59` | Bump `lopdf` to `"0.47"` and rebase the vendored fork onto upstream 0.12.1; `cargo check -p ragent-tools-extended` and fix the `Document::load`/`Object` accessor changes |
| 27 | `crossterm` duplicate pair | `crates/ragent-tui/Cargo.toml` | Move the direct dep to `0.29` to match `ratatui-crossterm 0.1.2`; removes two of the 28 duplicate crates |
| 28 | `html2text` fork one major behind | `Cargo.toml:241`, `Cargo.toml:320`, `vendor/html2text/Cargo.toml:16` | Rebase onto 0.17.4 (the local change is one subtraction) or upstream the patch |
| 29 | `rand` duplicate major | `Cargo.toml` `[workspace.dependencies]` | Move the workspace `rand` dep to `"0.10"`; drops `rand 0.9.5` + `rand_core 0.9.5` |
| 30 | `cargo outdated --root-deps-only` blind to workspace deps | CI / `pre-flight.sh` | Add a freshness step that runs full `cargo outdated` and greps non-arrow rows, or parse `Cargo.lock` against crates.io |
>>>>>>> Stashed changes

## Complex (> 2 hours)

| # | Finding | File(s) | Fix |
|---|---------|---------|-----|
| 1 | Four parallel `Tool` trait / `ToolContext` / `ToolRegistry` copies | `crates/ragent-tools-core/src/lib.rs:173`, `crates/ragent-tools-extended/src/lib.rs:336`, `crates/ragent-tools-vcs/src/lib.rs:131`, `crates/ragent-agent/src/tool/mod.rs:583` | Define the trait, context and registry once in a leaf crate (`ragent-types` or a new `ragent-tools-api`) and re-export from each tools crate. Touches every tool impl across 4 crates - do it as a mechanical commit with `cargo check` per crate, then delete the three shadow copies |
| 2 | `MemberStatus` terminal predicate hand-written and divergent | `crates/ragent-tui/src/layout_teams.rs:180`, `crates/ragent-tui/src/app/windows.rs:343`, `crates/ragent-agent/src/team/config.rs:239` | Decide whether `ShuttingDown` is terminal for *rendering* (it currently is) but not for *waiting* (it currently is not), encode both as named methods on `MemberStatus`, and replace every hand-spelled `matches!` |
| 3 | `ragent-hyperresearch` coverage | `crates/ragent-hyperresearch/src/**` | Fill in the 4-line placeholder modules or mark them explicitly deferred, then add tests for `core::config::resolve` (196 lines), `core::migrations::apply_pending`, and `web::safe_http` (SSRF-critical). Scope decision needed first: placeholder modules with no implementation cannot be tested |
| 4 | `run_target` real execution path untested | `crates/ragent-bench/src/runner.rs:218`, `crates/ragent-bench/src/workbook.rs:257` | Introduce a seam for the generation/exec boundary so the real path can run against a local fixture, then assert the resume-state and debug-sidecar contents |
| 5 | Server route coverage | `crates/ragent-server/src/routes/research.rs`, `crates/ragent-server/src/routes/openai.rs:457`, `:537` | Build an `AppState` test harness (temp DB + `research_root` under `target/temp`) and cover each handler's error, cancellation and streaming-termination branches |
| 6 | 28 duplicate dependency versions | `Cargo.lock` | Land the two fork rebases (items 26/28) and the `crossterm`/`rand` bumps first - those account for most of the cluster - then re-run `cargo deny check bans` and set `multiple-versions = "deny"` with a shrinking `[[bans.skip]]` allow-list so the count cannot regress |
| 7 | Storage credential obfuscation | `crates/ragent-storage/src/storage.rs:129` | Retire the v1 XOR key and the `USER`+`HOME`-derived legacy key behind a one-shot migration to v3 AEAD; requires reading and rewriting every stored provider key, so it needs a migration test and a rollback path |
| 8 | Project-overlay trust boundary | `crates/ragent-config/src/config.rs:3434` | Beyond stripping `automation`, define and enforce *which* config sections a project-local file may set at all (an explicit allow-list rather than a deny-list), so the next added section does not reopen the hole |
| 9 | Fragile exact-output TUI assertions | `crates/ragent-tui/tests/**` | Introduce a semantic assertion helper (assert on the row containing a label, or on a structured panel state) and migrate the 75 `screen_text` tests incrementally. Large, low-risk-per-change, easy to do crate by crate |

## Suggested Fix Order

1. **Security leaks into logs and errors** (Quick Wins 1-5): redaction and env-var allowlisting first - these are the findings with a real attack path, and each is a few lines.
2. **Shared primitives** (Medium 1-11): extract `read_capped`, `fetch_bytes`, the archive pipeline, the client builder, the redaction walk, `truncate_words`, the user-agent constant and the sanitizer *before* touching anything downstream, so later refactors build on one copy instead of three.
3. **Dependency consolidation** (Medium 26-29, Complex 6): rebase the two vendored forks and land the `crossterm`/`rand` bumps; this collapses the duplicate cluster and unblocks `lopdf`/`html2text`, then the `bans` gate can be tightened.
4. **Pattern standardisation** (Medium 12-17, Complex 2): thiserror, poison recovery, `async_trait` spelling, error context, `Tool::name()` - mechanical sweeps best done after the shared primitives settle.
5. **Missing tests** (Medium 21-25, Complex 3-5): bash validation, GitLab pipelines, CLI handlers, `project_store`, then the larger harness work for bench and server routes. Tests over security-critical parsing come before the cosmetic ones.
6. **Debug artifacts and log hygiene** (Quick Wins 8-20, 29-30): delete the three stray files, fix the tautologies and the per-frame/per-item log levels - cheap, zero risk, and they stop polluting future log-based diagnosis.
7. **Cosmetic and structural leftovers** (Low items, Complex 1, 7-9): the `Tool` trait unification, storage migration, overlay allow-list and the fragile-assertion migration are the last tier because each is a multi-hour change with a wide blast radius.

## Verification

- [ ] `cargo fmt --check` passes (mandatory after every `.rs` edit per AGENTS.md)
- [ ] `cargo clippy --workspace --all-targets` reports 0 warnings
- [ ] `cargo check --workspace` reports 0 errors
- [ ] `cargo nextest run --workspace --no-fail-fast` passes (capture to a file; do not re-run a full suite for reinterpretation)
- [ ] `cargo test --doc` passes (nextest does not run doc tests)
- [ ] `cargo audit` and `cargo deny check advisories bans licenses sources` both exit 0
- [ ] `scripts/check-security-unwraps.sh` and `scripts/check-silent-errors.sh` baselines do not grow
- [ ] `scripts/check-test-env-isolation.sh` and `scripts/check-inline-tests.sh` pass after the test moves
- [ ] No new `println!`/`eprintln!` outside the sanctioned CLI-presentation surface
- [ ] New tests cover the previously-untested files: `bash.rs` syntax layers, `gitlab_pipelines.rs`, `src/cli.rs` handlers, `project_store.rs`
- [ ] A test run leaves the working tree byte-identical (`git status --porcelain` empty)
- [ ] `cargo deny check bans` duplicate count drops from 28 and the tightened `multiple-versions` gate holds
