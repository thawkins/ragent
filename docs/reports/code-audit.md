# Code Audit Report

**Project**: Rust workspace (edition 2024) — 19 library crates under `crates/` plus the root `ragent` binary; `cargo fmt --check` / `cargo clippy` / `cargo nextest` gates; standards in `AGENTS.md`, `AGENTS-LANGUAGE.md`, `ANTIPAT.md`
**Scope**: Full project (excluding `target/`, `vendor/`, `.git/`, `node_modules/`)
**Findings**: 81 total (21 high, 44 medium, 16 low)

---

## High Priority

### Security
- `get_env` redacts only on a name-substring match (`KEY`/`SECRET`/`TOKEN`/`PASSWORD`/`PASS`/`CREDENTIAL`) or a credential *shape*; connection strings (`scheme://user:pass@host`) match neither, so a `DATABASE_URL`-style variable is returned verbatim to the model — `crates/ragent-tools-core/src/get_env.rs:22` (and `crates/ragent-types/src/sanitize.rs:88`)
- SSRF guard is disabled by an environment variable read in the production path, not a `#[cfg(test)]` block: setting `RAGENT_TOOLS_EXTENDED_TEST_NO_SSRF` lets the fetch/crawl tools reach loopback, private IP ranges and internal services — `crates/ragent-tools-extended/src/masterfetch/security.rs:197`
- Sudo-askpass answer (a plaintext password) is republished as `Event::QuestionAnswered` and serialised into the SSE payload with no redaction — `crates/ragent-server/src/sse.rs:84`
- Free-text question answer (used for sudo passwords) is rendered on screen unmasked, unlike the provider API-key field which was explicitly masked for SEC-ragent-tui-002 — `crates/ragent-tui/src/layout.rs:9048` (and `crates/ragent-tui/src/input.rs:565`)
- `ProviderSetupStep::EnterKey.key_field` holds the typed API key in an `InputField` that derives `Debug` (printing `text` verbatim), and the enclosing enum also derives `Debug` — any `{:?}` of app state, panic payload or debug log writes a live provider credential — `crates/ragent-tui/src/app/state.rs:486` (and `crates/ragent-tui/src/input_field.rs:14`)
- `AzureResourceEntry` derives `Debug` while carrying an inline `api_key: Option<String>`, and is held in the `Debug`-derived `ProviderSetupStep::SelectAzureResource` — `crates/ragent-llm/src/providers/azure_resource.rs:51` (and `crates/ragent-tui/src/app/state.rs:523`)

### Dependencies
- `quick-xml` 0.39.4 carries two CVSS-7.5 DoS advisories (RUSTSEC-2026-0194 quadratic duplicate-attribute check; RUSTSEC-2026-0195 unbounded namespace-declaration allocation); patched 0.42.0 is already in the tree via `calamine` — `quick-xml@0.39.4 (current -> 0.42.0)`

### Standards
- Six production `#[allow(unsafe_code)]` sites lie outside the single approved `kill_process_group` in `crates/ragent-tools-core/src/bash.rs`, contradicting the stated "single approved production site" rule — `crates/ragent-agent/src/mcp/mod.rs:1396` (also `:1417`, `:1424`), `crates/ragent-agent/src/team/manager.rs:441`, `src/stderr_spool.rs:50`, `src/crash_dump.rs:257`
- 260 functions exceed 100 lines (41 exceed 300), far beyond the documented size/complexity standard — `crates/ragent-tui/src/app/slash.rs:3534` (`execute_slash_command_inner`, 8159 lines), `crates/ragent-agent/src/session/processor.rs:1804` (2273), `crates/ragent-tui/src/app/event_handler.rs:166` (2065), `crates/ragent-tui/src/layout.rs:3058` (1581)
- Production `.expect()` on user-facing paths in the TUI clipboard helper and the markdown worker thread, both reachable from normal UI interaction and neither carrying a `no-panic-ok` marker — `crates/ragent-tui/src/clipboard.rs:171` (and `crates/ragent-tui/src/app/md_worker.rs:313`)

### Logging
- Hook `stderr` is logged unredacted at `warn!`/`error!` while sibling sites in the same file pass through `redact_secrets`, so arbitrary hook output can carry credentials into logs — `crates/ragent-agent/src/hooks/mod.rs:857` (also `:875`, `:1181`, `:1200`, `:1424`)
- Dynamic-context command `stderr` is logged verbatim at `warn!` with no `redact_secrets` pass, unlike every other command-output log site — `crates/ragent-agent/src/skill/context.rs:282`

### Duplication
- Near-identical Anthropic SSE stream parsers (78% line-for-line: `event:`/`data:` framing, `[DONE]`, malformed-frame warn, `content_block_*` dispatch); the OpenAI family already shares one parser in `providers/sse.rs` — `crates/ragent-llm/src/providers/anthropic.rs:447` (and `crates/ragent-llm/src/providers/azure_resource.rs:490`, `crates/ragent-llm/src/providers/bedrock.rs:620`, `:985`)
- Zebra-band/cursor row helpers copied verbatim across two TUI windows, with a third literal copy of the same colours in `layout.rs` — `crates/ragent-tui/src/layout_active_agents.rs:140` (`cursor_line`), `:170` (`BAND_BG`), `:177` (`CURSOR_BG`), `:186` (`band_line`) vs `crates/ragent-tui/src/layout_teams.rs:175`, `:160`, `:166`, `:202` (and `crates/ragent-tui/src/layout.rs:159`)
- Panel "list composition" navigators built identically four times (connector category, connector marketplace, plugin-store category, plugin-store marketplace) — only the browser type differs — `crates/ragent-tui/src/layout.rs:1002` (also `:1067`, `:2082`, `:2150`)
- Plugin and connector `/marketplace` command family is a copy-paste fork (parse, add, remove, report renderers; 0.72-0.79 similarity, only the `kind`/label token differs) — `crates/ragent-plugins/src/commands.rs:285` vs `crates/ragent-connectors/src/commands.rs:265` (also `:411`/`:430`, `:448`/`:468`, `:375`/`:396`, `:566`/`:349`)
- Path-containment helpers duplicated between research and server (83% identical, including the SEC-064 doc comment); `scripts/check-shared-guards.sh` covers `contained_join` but not this walk-the-existing-prefix guard — `crates/ragent-research/src/session.rs:3868` (`longest_existing_prefix_within`), `:3856` vs `crates/ragent-server/src/routes/research.rs:94`, `:79`

### Testing
- Five test files exist but are never compiled or run: each crate sets `autotests = false` with one `[[test]] name = "all"`, yet these files have no `#[path]` line in `all.rs` (~54 test functions silently unexecuted) — `crates/ragent-tui/tests/all.rs` (missing `test_mcp_bridged_surfaces.rs`, `test_mcp_remove.rs`, `test_modules_arms_deep.rs`; 225 includes vs 228 files), `crates/ragent-agent/tests/all.rs` (missing `test_mcp_npx_cache_repair.rs`, `test_tool_argument_decode_sites.rs`; 202 vs 204)
- 318 tests whose only assertions are `is_ok()` / `is_err()` / `is_some()` with no value inspection (114 are bare `assert!(....is_ok())`), so a return-value regression passes — `crates/ragent-agent/tests/test_structured_memory_store.rs:10` (also `crates/ragent-agent/tests/test_spec_tools.rs:122`, `crates/ragent-connectors/tests/test_connector_descriptor.rs:81`)
- Coverage gap: `webfetch` / `http_request` / `websearch` (699 lines, three registered tools) appear only in one SSRF allowlist test; no behaviour tests for request building, error paths or HTML-to-text — `crates/ragent-tools-extended/src/webfetch.rs:1` (and `crates/ragent-tools-extended/src/http_request.rs:1`)
- Coverage gap: `document_extract` (`detect_document_format`, `extract_file_as_markdown`, 100+ lines, used by `archdoc::local_source` and research `--from-file`) has zero references in any `tests/*.rs` — `crates/ragent-tools-extended/src/document_extract.rs:58`

## Medium Priority

### Security
- `RAGENT_TOKEN` (the HTTP server bearer token) is read at startup but never registered with the secret-redaction registry, so it is masked only if it happens to match the generic pattern — `src/main.rs:1221` (contrast the seeded list at `src/main.rs:660`)
- The startup secret-registry seed omits several live credentials (`GEMINI_API_KEY`/`GOOGLE_API_KEY`, `XAI_API_KEY`, `OPENROUTER_API_KEY`, `HF_TOKEN`, `AZURE_AI_FOUNDRY_API_KEY`, `AWS_*`), and a bare 32-hex Azure key matches no pattern — `src/main.rs:660` (and `crates/ragent-types/src/sanitize.rs:88`)
- Ad-hoc direct environment reads outside the validated config/credential layer: GitLab URL/username bypass `credential_env`; MCP connect timeout and perf state read the raw environment — `crates/ragent-tools-vcs/src/gitlab/auth.rs:149` (also `crates/ragent-agent/src/mcp/mod.rs:354`, `crates/ragent-agent/src/perf/mod.rs:68`)
- Automation dispatch reads an arbitrary env-var name supplied by config (`token_env`) with no allowlist and sends the value as a bearer token, so a mis-set `token_env` exfiltrates any process secret — `crates/ragent-agent/src/automation/dispatch.rs:60`
- Credential values in MCP `env` maps (and the plugin-to-MCP bridge) are injected into child processes but never registered as secrets, so only regex shapes protect them if a child echoes them — `crates/ragent-agent/src/mcp/mod.rs:704` (and `crates/ragent-plugins/src/bridge.rs:210`)
- `.gitignore` gaps for credential-bearing paths this codebase writes and reads: no entry for `azureresources.json` (documented config with inline `api_key`), `.mcp.json`/`mcp.json` (server `env`/`headers` secrets), root-level `ragent.json`, `*.crt`/`*.cer`/`*.der`, `node_modules/`, `*.log` — `.gitignore:1`
- `/auto/{id}` webhook ingress sits on the unauthenticated outer router with no HMAC or signature verification (rate limit only), so anyone reaching the port can enqueue agent runs with attacker-controlled context — `crates/ragent-server/src/routes/automation.rs:54` (and `crates/ragent-server/src/routes/mod.rs:225`)

### Standards
- Crate-root error strategy is inconsistent: `ragent-llm` exposes `anyhow::Result` across its public `ChatClient`/stream trait with no error enum, `ragent-server` has zero `thiserror` types yet a public `anyhow::Result` entry point, while `ragent-connectors` (14) and `ragent-plugins` (10) are `thiserror`-only — `crates/ragent-llm/src/llm.rs:42` (and `crates/ragent-server/src/routes/mod.rs:144`, `crates/ragent-connectors/src/error.rs:17`)
- Masked-error handling is widespread and unremarked: 284 `let _ =` drops and 55 `map_err(|_|` sites discard the original error, deviating from the `?`-propagation dominant pattern — `crates/ragent-agent/src/session/loop_steps.rs:834` (also `crates/ragent-config/src/bash_lists.rs:131`, `crates/ragent-agent/src/hooks/mod.rs:601`)
- 580 wildcard `_ =>` arms in production code, including matches on the crate's own closed enums where an exhaustive arm is required by the stated rule — `crates/ragent-agent/src/memory/knowledge_graph.rs:64` (also `:111`, `:413`, `crates/ragent-agent/src/session/permissions.rs:140`)
- 63 `#[allow(dead_code)]` suppressions accumulate across production sources rather than removing dead code; `scripts/check-dead-code-reasons.sh` only checks they are commented, not that they are justified — `crates/ragent-agent/src/agent/mod.rs:1`
- Silent-error baseline is stale and no longer tracks reality: 29 of 89 entries exceed the current count and 3 reference deleted files, so the shrink-only gate cannot detect regressions — `scripts/security-unwrap-baseline.txt` (e.g. `crates/ragent-codeindex/src/search.rs` baseline 9 versus actual 0; `crates/ragent-llm/src/providers/tool_cache.rs` baseline 5 versus actual 0; `crates/ragent-tools-extended/src/office_write.rs` missing)
- `.ok()` used to swallow `Result` on paths that then silently proceed (config reload, store save, JSON parse) without an `// INTENTIONAL:` marker — `crates/ragent-agent/src/tool/team_cleanup.rs:88` (also `crates/ragent-llm/src/providers/azure_resource.rs:203`, `:213`, `crates/ragent-plugins/src/marketplace.rs:201`)
- `ragent-tools-core` returns `anyhow::Result` from the security-critical path-containment guard, giving callers an opaque error type where the thiserror convention would give a matchable one — `crates/ragent-tools-core/src/lib.rs:206` (and `:361`)

### Logging
- Hook `feedback` (derived from hook stdout, 8 KiB cap) is logged unredacted at `warn!` — `crates/ragent-agent/src/tool/team_create.rs:370`
- Logs the first 80 bytes of user-authored memory content at `info!` (user content in logs; byte-truncated, not redacted) — `crates/ragent-agent/src/memory/extract.rs:738`
- Logs a 60-byte snippet of stored user content at `debug!` ("Skipping duplicate candidate") — `crates/ragent-agent/src/memory/extract.rs:664`
- User-visible operation failures logged at `debug` only (resume failure, skill invocation failure, bang-command output failure), invisible at the default level — `crates/ragent-tui/src/app/session_ops.rs:3039` (also `crates/ragent-tui/src/app/slash.rs:11902`, `crates/ragent-tui/src/app/session_ops.rs:877`)
- Debug-instrumentation helper `debug_log_input_transition` (`#[cfg(debug_assertions)]`, 12 call sites) left in production code — `crates/ragent-tui/src/app/session_ops.rs:3211` (also `crates/ragent-tui/src/app/input_handler.rs:451`, `:465`, `:474`, `:491`, `:503`, `:966`, `:1186`, `:1194`, `:1220`, `:1271`)
- Background-command `stdout`/`stderr` reader errors logged at `debug!`; a dead background stream is a real failure — `crates/ragent-tools-core/src/bg.rs:338` (and `:366`)
- Prometheus scrape response `write_all`/`flush` failures logged at `debug!` only, so a broken scrape is silently invisible — `crates/ragent-telemetry/src/prometheus.rs:442` (and `:445`)
- Repeated tool-failure tracking event logged at `debug!` with no `session_id`/`tool` structured fields, only interpolated into the message — `crates/ragent-agent/src/memory/extract.rs:455`

### Duplication
- Provider HTTP error-handling block repeated across nine providers (`read_body_capped` → `redact_secrets` → `tracing::warn!` → `bail!`), only the provider label varying — `crates/ragent-llm/src/providers/anthropic.rs:368` (also `gemini.rs:514`, `ollama.rs:343`, `ollama_cloud.rs:453`, `openrouter.rs:511`, `copilot.rs:384`, `huggingface.rs:451`, `azure_resource.rs:398`, `bedrock.rs:1164`)
- `data: ` SSE frame loop duplicated in five non-OpenAI providers (`strip_prefix`, `[DONE]`, `serde_json::from_str`, redacted-warn-and-continue), although `sse.rs` already owns this — `crates/ragent-llm/src/providers/ollama.rs:449` (also `copilot.rs:472`, `huggingface.rs:568`, `ollama_cloud.rs:545`, `openai_responses.rs:461`)
- `team_*` tools repeat the same two guards 18 times (required-parameter extraction and `find_team_dir(...).ok_or_else(...)`) — `crates/ragent-agent/src/tool/team_message.rs:58` (also `team_broadcast.rs:62`, `team_idle.rs:52`, `team_task_list.rs:44`, `team_status.rs:52`, `team_cleanup.rs:61`)
- `codeindex_*` tools re-derive the same missing-parameter `ToolOutput` by hand, though `codeindex_not_available`/`busy_output` helpers live in `codeindex_utils.rs` — `crates/ragent-tools-extended/src/codeindex_path.rs:70` (also `:95`, `crates/ragent-tools-extended/src/codeindex_explain.rs:62`)
- Eight char-boundary truncation helpers re-implement (or diverge from, by ellipsis and byte-vs-char semantics) the shared `ragent_types::strutil::truncate_chars` — `crates/ragent-research/src/cli.rs:862` (also `ragent-surface/src/harness.rs:60`, `ragent-agent/src/compaction/serializer.rs:167`, `ragent-research/src/comparison.rs:199`, `ragent-research/src/session/topic.rs:437`, `ragent-agent/src/session/history.rs:253`, `ragent-plugins/src/control.rs:695`, `ragent-tui/src/layout_teams.rs:248` vs `crates/ragent-types/src/strutil.rs:64`)
- Retry/permanent-error classification forked between agent and bench, both spelling out the same `invalid_request_error`/`model_not_supported` markers and `(400..500) && != 408 && != 429` rule — `crates/ragent-agent/src/session/history.rs:675` (and `:654`) vs `crates/ragent-bench/src/model.rs:551` (also `:566`, `:531`)
- Five different "write file atomically" implementations, plus a sixth half-covered variant that re-does the lock/temp/rename dance — `crates/ragent-surface/src/config_splice.rs:36` (also `crates/ragent-llm/src/providers/wire/trace.rs:318`, `crates/ragent-research/src/io.rs:202`, `crates/ragent-research/src/source_vault.rs:626`, `crates/ragent-specs/src/io.rs:123`, `crates/ragent-tui/src/app/state.rs:54`)
- GitHub and GitLab issue/PR markdown renderers are near-identical (same header/`**State**`/`**Author**`/`**URL**`/Labels/Assignees/body block); `scripts/check-vcs-duplication.sh` guards only `make_client`/`detect*` — `crates/ragent-tools-vcs/src/github/github_issues.rs:180` vs `crates/ragent-tools-vcs/src/gitlab/gitlab_issues.rs:184` (also `github_prs.rs:180` vs `gitlab_mrs.rs:163`)
- Magic strings/values re-spelled instead of using the existing shared constant: `"application/json"` at 18 provider sites, `format!("Bearer {}", key)` at 3 sites while `ragent_types::strutil::bearer_auth_header` exists, and the `"Showing {}-{} of {}  |  "` legend literal four times — `crates/ragent-llm/src/providers/anthropic.rs:356` (also `gemini.rs:503`, `openai.rs:239`, `openrouter.rs:491`, `ollama.rs:313`, `crates/ragent-tools-extended/src/masterfetch/search/tavily.rs:192`, `perplexity.rs:207`, `langsearch.rs:185`, `crates/ragent-tui/src/layout.rs:3488`, `:3593`, `:3684`, `:4602`)
- 20 TUI test files hand-roll a 65-73 line `make_app()` that duplicates `support::make_app` verbatim, differing only by a trailing bool — `crates/ragent-tui/tests/test_scrolling.rs:24` (also `test_status_expiry.rs:21`, `test_memory_panel.rs:45`, `test_router_setup.rs:138` vs `crates/ragent-tui/tests/support/mod.rs:30`)
- Identical scroll-window render block repeated three times (`total`/`visible`/`max_scroll`/`scroll_from_top` → `slice_flat_wrapped_window` → `Paragraph` → optional `ScrollbarState`) — `crates/ragent-tui/src/layout.rs:6246` (also `:6399`, `:7698`)

### Testing
- 15 near-duplicate test blocks re-testing the same shared helper (`engine::truncate_query_to`) through five one-line wrappers with trivially different inputs — `crates/ragent-tools-extended/tests/test_mf_exa.rs:146` (also `test_mf_openalex.rs:21`, `test_mf_perplexity.rs:20`, `test_mf_tavily.rs:16`, `test_mf_wikipedia.rs:323`)
- 11 tautological tests asserting hard-coded permission-category constants, one per tool, differing only by struct name — `crates/ragent-tools-extended/tests/test_mf_permission_categories.rs:48` (also `:54`, `:60`, `:66`, `:74`, `:80`, `:93`, `:110`, `:129`, `:153`, `:193`)
- Fragile UI tests pin exact frame glyphs and layout geometry, so any border/style change breaks them without a behaviour change — `crates/ragent-tui/tests/test_connector_catalogue_panel.rs:233`
- Wall-clock dependent tests: fixed `thread::sleep(200ms)` before asserting no events then a 500 ms `recv_timeout`; 1.5 s real sleep inside a deadline assertion — `crates/ragent-codeindex/tests/test_watcher.rs:46` (also `:86`, `:108`, `crates/ragent-research/tests/test_web_time_deadline.rs:61`, `crates/ragent-agent/tests/test_profiler.rs:17`)
- `#[path = "support/mod.rs"] mod support;` repeated in 138 test files, and TUI carries two parallel shared-helper copies with overlapping `app_with_session`/`make_app` — `crates/ragent-tui/tests/support/mod.rs:30` (and `crates/ragent-tui/tests/slash_support/mod.rs:1`)
- 128 test modules are `#[cfg(test)]`-included into `src` via `#[path = "../../tests/inline/..."]` (research alone: 41 modules, 16,877 lines), so those tests run only in the library unit-test target and are invisible to per-file `cargo nextest -E 'test(test_x::)'` selection — `crates/ragent-research/src/session.rs:1` (also `crates/ragent-agent/src/mcp/http.rs:568`, `crates/ragent-llm/src/providers/anthropic.rs:1`)
- Genuine inline (non-`#[path]`) `#[cfg(test)] mod tests;` declaration left in library source, contradicting the "no new inline modules" rule — `crates/ragent-tui/src/app.rs:116`
- Missing edge cases for the file-tool surface: no tests for unicode/space-in-path, non-UTF-8 content, or empty-file reads across read/edit/rm/copy/move; coverage is one happy-path file plus a func061 edge file — `crates/ragent-tools-core/tests/test_file_ops_happy_paths.rs:61` (and `crates/ragent-tools-core/tests/test_file_ops_func061.rs:39`)
- Coverage gap: `polarity.rs` (67 lines) has no test at all (its only mention is a forbidden-pattern allowlist string) and `page_summarizer.rs` (72 lines) is reached only incidentally — `crates/ragent-research/src/polarity.rs:1` (and `crates/ragent-research/src/page_summarizer.rs:1`)

### Dependencies
- Four copies of `windows-sys` in one lockfile — `windows-sys@0.52.0, 0.59.0, 0.60.2, 0.61.2 (4 versions)`
- 56 crates are present at more than one version in one lockfile (`rand` 3, `getrandom` 3, `rand_core` 3, `base64` 3, `syn` 3, `hashbrown` 3, `phf` 3), bloating the dependency graph — `rand@0.8.8/0.9.5/0.10.3 (3 versions)`

## Low Priority

### Security
- A predictable machine-derived key (`blake3::derive_key("ragent credential encryption v2", "$USER:$HOME")`) is still live as the decryption key for legacy `v2:` credential rows, so an attacker holding the SQLite file plus user/home can decrypt them — `crates/ragent-storage/src/storage.rs:275` (and `:515`)
- Hardcoded OAuth application client ID committed in source (`Iv1.b507a08c87ecfe98`); not a secret per se, but a fixed credential-identifier used for every install's device-flow login that cannot be rotated per deployment — `crates/ragent-tools-vcs/src/github/client.rs:17` (and `:133`)

### Standards
- Version-suffix naming on a public function that has no non-`_v2` sibling, leaving the API name implying a deprecated predecessor that does not exist — `crates/ragent-tui/src/layout_statusbar.rs:191` (`render_status_bar_v2`)
- Box-drawing/Unicode glyphs retained in source string literals and comments where ASCII (`-`, `|`, `+`) is requested where practical — `crates/ragent-agent/src/agent/mod.rs:427` (also `crates/ragent-agent/src/reference/resolve.rs:363`, `crates/ragent-tools-core/src/list.rs:153`, `crates/ragent-tools-extended/src/codeindex_references.rs:92`)
- 48 `mod.rs` files remain in `crates/*/src` (28 in `ragent-agent` alone) although `mod.rs` is reserved for "legacy or deeply nested modules" — `crates/ragent-agent/src/agent/mod.rs:1` (also `event/`, `file_ops/`, `hooks/`, `mcp/`, `memory/`, `orchestrator/`, `session/`, `team/`, `tool/` `mod.rs`)
- Inconsistent `// no-panic-ok:` marking for the same infallible `String` write: some sites mark the `expect`, equivalent `let _ = write!` drops elsewhere carry no marker — `crates/ragent-tools-vcs/src/percent.rs:28` (marked) vs `crates/ragent-agent/src/agent/mod.rs:206`, `:213`, `:1600` (unmarked)

### Logging
- AEAD encryption failure logged with a bare message and no error/context fields, making the failure unattributable — `crates/ragent-storage/src/storage.rs:407`
- Tool-watchdog timeouts logged via `warn!("{}", msg)` — positionally formatted with no structured `tool`/`call_id`/`timeout_ms` fields — `crates/ragent-agent/src/session/processor.rs:3455` (and `:3534`)
- Inconsistent `%` vs `?` vs literal rendering for the same key across hook log sites (`exit_code = ?code` vs `exit_code = 1` vs `exit_code = ?out.status.code()`), so structured consumers see three types for one field — `crates/ragent-agent/src/hooks/mod.rs:874` (also `:1181`, `:1424`)
- Stale `TODO: Integrate with git module to get actual status` — the status bar currently hardcodes a "HEALTHY" git indicator, so the TODO masks a wrong signal rather than a missing nicety — `crates/ragent-tui/src/layout_statusbar.rs:819`

### Dependencies
- `tokenizers` pinned to 0.21 while the published line is 1.0 (pre-releases only, so no stable upgrade target), and it drags in `paste`, which is unmaintained (RUSTSEC-2024-0436), masked by an explicit ignore — `tokenizers@0.21.4 (current -> 1.0.0-rc.3, pre-release only)` / `paste@1.0.15 (unmaintained; pastey is the drop-in fork)`
- Advisory-ignore rationale is stale: `.cargo/audit.toml` and `deny.toml` justify the two quick-xml ignores with "blocked by ooxmlsdk compatibility", but `ooxmlsdk` is absent from `Cargo.lock` and patched 0.42.0 is already in the tree via `calamine` — `advisory-ignore-rationale (stale)`
- No `rust-version` MSRV declared in `[workspace.package]` or any crate manifest; only `edition = "2024"` implies Rust >= 1.85, and `rust-toolchain.toml` pins the floating `stable` channel — `rust-version-missing` / `rust-toolchain@stable`
- `libc` one patch release behind — `libc@0.2.190 (current -> 0.2.191)`
- `tokio` one minor release behind — `tokio@1.53.3 (current -> 1.54.1)`
- `uuid` one minor release behind — `uuid@1.28.0 (current -> 1.29.0)`

# Implementation Plan

## Quick Wins (< 30 min each)

| # | Finding | File(s) | Fix |
|---|---------|---------|-----|
| 1 | Five test files never compiled or run | `crates/ragent-tui/tests/all.rs`, `crates/ragent-agent/tests/all.rs` | Add the five missing `#[path = "test_x.rs"] mod test_x;` lines; run them and fix any that now fail |
| 2 | quick-xml DoS advisories | root `Cargo.toml:255` | Bump the direct `quick-xml` dependency to 0.42 and drop both `.cargo/audit.toml` / `deny.toml` ignore entries |
| 3 | Stale advisory-ignore rationale | `.cargo/audit.toml`, `deny.toml` | Remove the `ooxmlsdk` justification; re-record any ignore that is still required against a real in-tree consumer |
| 4 | SSRF test bypass reachable in production | `crates/ragent-tools-extended/src/masterfetch/security.rs:197` | Gate `ssrf_test_skip()` behind `#[cfg(test)]` (or a `#[cfg(feature = "test-support")]` feature) so release builds cannot disable the guard |
| 5 | Sudo-askpass password in SSE payload | `crates/ragent-server/src/sse.rs:84` | Redact the `response` field of `QuestionAnswered` in `to_data` via `redact_secrets`; add a test asserting the payload never contains the typed value |
| 6 | Question answer rendered unmasked | `crates/ragent-tui/src/layout.rs:9048`, `crates/ragent-tui/src/input.rs:565` | Render `pending_question_input` as `*`-masked exactly as the provider API-key field already is; only mask when the question is flagged `secret` |
| 7 | API key held in `Debug`-derived `InputField` | `crates/ragent-tui/src/app/state.rs:486`, `crates/ragent-tui/src/input_field.rs:14` | Hand-write `Debug for InputField` printing `text: "<redacted>"`, or store the key in a non-`Debug` wrapper newtype |
| 8 | `AzureResourceEntry` derives `Debug` with `api_key` | `crates/ragent-llm/src/providers/azure_resource.rs:51` | Hand-write `Debug` that redacts `api_key`; the enclosing `ProviderSetupStep` then inherits safety |
| 9 | Hook `stderr` logged unredacted | `crates/ragent-agent/src/hooks/mod.rs:857,875,1181,1200,1424` | Route all five through `redact_secrets(...)` as the sibling sites at `:829`/`:1162` already do |
| 10 | Dynamic-context `stderr` logged unredacted | `crates/ragent-agent/src/skill/context.rs:282` | Apply `redact_secrets` before the `warn!` |
| 11 | Hook `feedback` logged unredacted | `crates/ragent-agent/src/tool/team_create.rs:370` | Apply `redact_secrets` before the `warn!` |
| 12 | User memory content in `info!`/`debug!` | `crates/ragent-agent/src/memory/extract.rs:738,664` | Drop the content snippet; log length + a hash or id only |
| 13 | `.gitignore` credential-path gaps | `.gitignore` | Add `azureresources.json`, `.mcp.json`, `mcp.json`, `/ragent.json`, `*.crt`, `*.cer`, `*.der`, `node_modules/`, `*.log` |
| 14 | `RAGENT_TOKEN` not in the redaction registry | `src/main.rs:1221` | `register_secret` it next to the seeded list at `:660` |
| 15 | Registry seed omits live credentials | `src/main.rs:660`, `crates/ragent-types/src/sanitize.rs:88` | Add the five missing env vars to the seed; add a `scheme://user:pass@` pattern to `sanitize.rs` |
| 16 | `get_env` leaks connection strings | `crates/ragent-tools-core/src/get_env.rs:22` | Add `URL`, `DATABASE`, `DSN`, `CONN` to `SENSITIVE_PATTERNS` and add the connection-string shape to `sanitize.rs` |
| 17 | User-visible failures logged at `debug` | `crates/ragent-tui/src/app/session_ops.rs:3039,877`, `crates/ragent-tui/src/app/slash.rs:11902` | Promote to `warn!` |
| 18 | Reader/scrape failures logged at `debug` | `crates/ragent-tools-core/src/bg.rs:338,366`, `crates/ragent-telemetry/src/prometheus.rs:442,445` | Promote to `warn!` with structured fields |
| 19 | Unstructured watchdog + AEAD log lines | `crates/ragent-agent/src/session/processor.rs:3455,3534`, `crates/ragent-storage/src/storage.rs:407` | Move the message text into a field and add `tool`, `call_id`, `timeout_ms` / `error = ?e` |
| 20 | `exit_code` typed three different ways | `crates/ragent-agent/src/hooks/mod.rs:874,1181,1424` | Standardise on `exit_code = ?opt_u32` at all three sites |
| 21 | Stale TODO masking a wrong git indicator | `crates/ragent-tui/src/layout_statusbar.rs:819` | Delete the hardcoded "HEALTHY" default and either wire real status or render `-` |
| 22 | Version-suffix function name | `crates/ragent-tui/src/layout_statusbar.rs:191` | Rename `render_status_bar_v2` to `render_status_bar` (update call sites) |
| 23 | Missing MSRV declaration | root `Cargo.toml` `[workspace.package]` | Add `rust-version = "1.85"`; consider pinning `rust-toolchain.toml` to a concrete minor |
| 24 | Patch/minor dependency drift | `Cargo.toml` | `cargo update -p libc -p tokio -p uuid` (verify with `cargo nextest` after) |
| 25 | Stale silent-error baseline | `scripts/security-unwrap-baseline.txt` | Regenerate from the current tree so the shrink-only gate can detect regressions again |

## Medium Effort (30 min - 2 hours each)

| # | Finding | File(s) | Fix |
|---|---------|---------|-----|
| 1 | 318 vacuous `is_ok()`-only tests | `crates/*/tests/**` (114 bare sites) | Sweep crate-by-crate: replace `assert!(x.is_ok())` with `x.expect(...)` plus an assertion on the returned value; delete any test that has no observable behaviour to assert |
| 2 | Zero coverage for `webfetch`/`http_request`/`websearch` | `crates/ragent-tools-extended/src/webfetch.rs`, `http_request.rs` | Add `tests/test_web_tools_*.rs` covering request building, non-2xx handling, oversized-body capping and HTML-to-text against a local test server (`RAGENT_TOOLS_EXTENDED_TEST_NO_SSRF` scoped to the test process) |
| 3 | Zero coverage for `document_extract` | `crates/ragent-tools-extended/src/document_extract.rs:58` | Add format-detection + extraction tests over small fixture files in `target/temp/` |
| 4 | Zero coverage for `polarity` / `page_summarizer` | `crates/ragent-research/src/polarity.rs`, `page_summarizer.rs` | Add one `tests/test_research_polarity.rs` and one `test_research_page_summarizer.rs` in the crate's `all.rs` |
| 5 | File-tool edge cases untested | `crates/ragent-tools-core/tests/test_file_ops_*.rs` | Add unicode/space paths, non-UTF-8 bytes, empty-file read, and read-only-target-write cases |
| 6 | 20 hand-rolled `make_app()` copies in TUI tests | `crates/ragent-tui/tests/support/mod.rs:30` | Delete the local copies and `#[path = "support/mod.rs"] mod support;`; keep the trailing-bool difference as a `make_app_with(flag)` parameter |
| 7 | Two parallel TUI test helpers | `crates/ragent-tui/tests/support/mod.rs`, `slash_support/mod.rs` | Merge `app_with_session`/`make_app` into one module and re-point `slash_support` |
| 8 | `support/mod.rs` repeated in 138 files | `crates/*/tests/**` | Leave as-is per the `#[allow(clippy::duplicate_mod)]` convention; if touched, do it per crate with a filtered `cargo nextest` run |
| 9 | 128 `#[cfg(test)] #[path]` modules invisible to nextest filters | `crates/*/src/**` | Migrate crate-by-crate to `tests/test_*.rs` + `all.rs` includes; start with `ragent-research` (41 modules) as the largest win |
| 10 | Inline `mod tests;` in library source | `crates/ragent-tui/src/app.rs:116` | Move the module to `crates/ragent-tui/tests/test_app_inline.rs` and include it from `all.rs` |
| 11 | Near-duplicate `truncate_query_to` tests | `crates/ragent-tools-extended/tests/test_mf_{exa,openalex,perplexity,tavily,wikipedia}.rs` | Keep one parameterised test for the shared helper; delete the five wrappers' duplicated cases |
| 12 | Tautological permission-category tests | `crates/ragent-tools-extended/tests/test_mf_permission_categories.rs` | Replace the 11 per-struct assertions with a table-driven loop over `(tool, expected_category)` |
| 13 | Fragile glyph/geometry assertions | `crates/ragent-tui/tests/test_connector_catalogue_panel.rs:233` | Assert structural properties (row count, selected index, highlighted span presence) instead of exact frame characters |
| 14 | Wall-clock sleeps in tests | `crates/ragent-codeindex/tests/test_watcher.rs:46`, `crates/ragent-research/tests/test_web_time_deadline.rs:61`, `crates/ragent-agent/tests/test_profiler.rs:17` | Inject a clock/deadline or use `tokio::time::pause` with `advance` |
| 15 | Provider HTTP error block duplicated 9x | `crates/ragent-llm/src/providers/{anthropic,gemini,ollama,ollama_cloud,openrouter,copilot,huggingface,azure_resource,bedrock}.rs` | Extract one `fn api_error(provider, url, model, status, body) -> anyhow::Error` in `providers/mod.rs` |
| 16 | `data:` SSE loop duplicated 5x | `crates/ragent-llm/src/providers/{ollama,copilot,huggingface,ollama_cloud,openai_responses}.rs` | Reuse the existing frame iterator from `providers/sse.rs` |
| 17 | Anthropic SSE parsers duplicated (incl. Bedrock pair) | `anthropic.rs:447`, `azure_resource.rs:490`, `bedrock.rs:620,985` | Extract one `parse_anthropic_sse` into `providers/sse.rs`; parameterise by the event-name mapping |
| 18 | `team_*` guards repeated 18x | `crates/ragent-agent/src/tool/team_*.rs` | Add `required_str(&input, "team_name")?` and `resolve_team(&ctx, &team_name)?` helpers and use them everywhere |
| 19 | `codeindex_*` missing-parameter output re-derived | `crates/ragent-tools-extended/src/codeindex_{path,explain}.rs` | Add `missing_parameter(field) -> ToolOutput` to `codeindex_utils.rs` |
| 20 | 8 char-truncation helpers | `crates/ragent-{research,surface,agent,plugins,tui}/src/**` | Collapse onto `ragent_types::strutil::truncate_chars`; where ellipsis/byte semantics genuinely differ, add named variants there |
| 21 | Retry classification forked agent vs bench | `crates/ragent-agent/src/session/history.rs:675`, `crates/ragent-bench/src/model.rs:551` | Move the status-code extraction and permanent-error rule into one shared crate (`ragent-types` or a small `ragent-llm` helper) |
| 22 | Five atomic-write implementations | `ragent-surface/src/config_splice.rs:36`, `ragent-llm/.../wire/trace.rs:318`, `ragent-research/src/io.rs:202`, `ragent-research/src/source_vault.rs:626`, `ragent-specs/src/io.rs:123`, `ragent-tui/src/app/state.rs:54` | Promote one to `ragent-types::fsutil::write_atomic` (lock + temp + rename + fsync) and replace the rest |
| 23 | GitHub/GitLab renderers near-identical | `github_issues.rs:180` / `gitlab_issues.rs:184`, `github_prs.rs:180` / `gitlab_mrs.rs:163` | Extract a shared `render_issue_summary` / `render_pr_summary` taking the provider label |
| 24 | Magic strings for content-type, bearer header, legend | 18 provider sites; 3 search backends; `crates/ragent-tui/src/layout.rs:3488,3593,3684,4602` | Add `CONTENT_TYPE_JSON` to a provider const module, use `strutil::bearer_auth_header`, and add one `showing_range(a, b, n)` formatter |
| 25 | Zebra-band/cursor helpers copied 3x | `layout_active_agents.rs:140-186`, `layout_teams.rs:160-202`, `layout.rs:159` | Move `cursor_line`/`band_line`/`BAND_BG`/`CURSOR_BG` into a shared TUI style module |
| 26 | Panel list-composition built 4x | `crates/ragent-tui/src/layout.rs:1002,1067,2082,2150` | Extract one `render_list_navigator(kind, title, items, focus, area)` — this is the single composition the README claims exists |
| 27 | Plugin/connector marketplace command fork | `ragent-plugins/src/commands.rs:285-566`, `ragent-connectors/src/commands.rs:265-468` | Extract a shared marketplace subcommand parser + report renderers parameterised by catalogue kind |
| 28 | Path-containment guard duplicated | `ragent-research/src/session.rs:3856,3868`, `ragent-server/src/routes/research.rs:79,94` | Move `project_root_for`/`longest_existing_prefix_within` into `ragent-types` (or `ragent-storage`) and add it to `scripts/check-shared-guards.sh` |
| 29 | Scroll-window render block 3x | `crates/ragent-tui/src/layout.rs:6246,6399,7698` | Extract `render_scrolled_pane(view, block, area)` |
| 30 | `let _ =` / `map_err(|_|)` error discards | 284 + 55 sites (`session/loop_steps.rs:834`, `config/bash_lists.rs:131`, `hooks/mod.rs:601`, ...) | Add `// INTENTIONAL:` markers where the drop is deliberate, and propagate with `?` or log-with-context where it is not; sweep module-by-module |
| 31 | Wildcard `_ =>` on closed enums | 580 sites (`memory/knowledge_graph.rs:64,111,413`, `session/permissions.rs:140`, ...) | Make matches on this crate's own enums exhaustive; keep `_` only for external non-exhaustive types |
| 32 | `#[allow(dead_code)]` accumulation | 63 sites, e.g. `crates/ragent-agent/src/agent/mod.rs` | Extend `scripts/check-dead-code-reasons.sh` to require a reason in a fixed format, then re-audit and delete genuinely dead items |
| 33 | `.ok()` swallowing without a marker | `team_cleanup.rs:88`, `azure_resource.rs:203,213`, `marketplace.rs:201` | Replace with `if let Err(e) = ... { tracing::warn!(...); }` or add `// INTENTIONAL:` with the reason |
| 34 | `anyhow`/`thiserror` split by crate root | `ragent-llm/src/llm.rs:42`, `ragent-server/src/routes/mod.rs:144`, `ragent-connectors/src/error.rs:17` | Give `ragent-llm` a `thiserror`-based error enum (or a documented `# Errors` note) and add one to `ragent-server`; document the policy in `AGENTS-LANGUAGE.md` |
| 35 | `anyhow` on the security-critical containment guard | `crates/ragent-tools-core/src/lib.rs:206,361` | Introduce a `PathGuardError` thiserror enum and return it |
| 36 | Ad-hoc env reads outside the config layer | `gitlab/auth.rs:149`, `mcp/mod.rs:354`, `perf/mod.rs:68` | Route through `ragent_config::credential_env` / the existing config structs |
| 37 | `token_env` reads an arbitrary env var | `crates/ragent-agent/src/automation/dispatch.rs:60` | Reject names outside an allowlist prefix, or resolve the token from the credential store instead of the environment |
| 38 | MCP/plugin env secrets not registered | `crates/ragent-agent/src/mcp/mod.rs:704`, `crates/ragent-plugins/src/bridge.rs:210` | Call `register_secret` for every value injected into a child `env`/`headers` map |
| 39 | Webhook has no signature verification | `crates/ragent-server/src/routes/automation.rs:54`, `routes/mod.rs:225` | Add an HMAC-SHA256 `X-Ragent-Signature` check over the raw body with a per-automation secret; reject unsigned requests when a secret is configured |
| 40 | Debug-instrumentation helper left in | `crates/ragent-tui/src/app/session_ops.rs:3211` + 12 call sites in `input_handler.rs` | Delete `debug_log_input_transition` and its call sites once the regression it was added for is closed |
| 41 | Duplicate dependency versions | `Cargo.lock` | `cargo update` where semver-compatible, then add `[workspace.dependencies]` pins for `windows-sys`, `rand`, `getrandom`, `base64`, `hashbrown` until they converge |

## Complex (> 2 hours)

| # | Finding | File(s) | Fix |
|---|---------|---------|-----|
| 1 | Six `unsafe` sites outside the single approved one | `crates/ragent-agent/src/mcp/mod.rs:1396,1417,1424`, `crates/ragent-agent/src/team/manager.rs:441`, `src/stderr_spool.rs:50`, `src/crash_dump.rs:257` | Replace each with a safe equivalent: `std::process::Command::process_group`/`killpg` via the approved helper, `nix`-style wrappers or `libc` behind one audited module; if a site is genuinely unavoidable, extend AGENTS.md's allowlist with a `// SAFETY:` contract per site (stderr spool and crash-dump are plausible candidates — they need owner sign-off, not silent approval) |
| 2 | 260 functions over 100 lines (41 over 300) | `ragent-tui/src/app/slash.rs:3534` (8159 lines), `agent/src/session/processor.rs:1804` (2273), `ragent-tui/src/app/event_handler.rs:166` (2065), `ragent-tui/src/layout.rs:3058` (1581) | Split the four monoliths first: dispatch-table + per-command modules for `execute_slash_command_inner`, per-step modules for the session processor, per-event-kind handlers for the event loop, and one module per panel for `layout.rs`. Add a clippy `too_many_lines` threshold + ratchet file to stop regressions |
| 3 | Production `expect()` reachable from the UI | `crates/ragent-tui/src/clipboard.rs:171`, `ragent-tui/src/app/md_worker.rs:313` | Propagate or degrade gracefully (clipboard unavailable → status message; worker panics → restart the worker and report); both need a reproduction test before the fix |
| 4 | 4.0/4.1-style `quick-xml` migration | root `Cargo.toml`, `deny.toml`, `.cargo/audit.toml` | If the bump breaks a consumer of the 0.39 API, migrate that call site (in `ragent-tools-extended`) rather than re-ignoring the advisories; then delete the ignores permanently |
| 5 | Predictable credential-encryption key for `v2:` rows | `crates/ragent-storage/src/storage.rs:275,515` | Re-encrypt legacy rows under the OS keyring / user-passphrase-derived key, then delete the derived-key path; needs a migration + rollback plan |

## Suggested Fix Order

1. **Secrets and log redaction (Security + Logging quick wins, #1-#12 above)** — smallest diffs, highest blast-radius reduction; a leaked key in a log or SSE stream cannot be un-leaked later.
2. **Test-gate integrity (#1, #25, and the orphaned `all.rs` includes)** — until the five unrun files are compiled and the baseline regenerated, every later fix is verified by a gate that may be blind.
3. **Shared utilities and constants (Duplication #15-#29)** — extracting the shared helpers before touching behaviour avoids fixing the same bug in nine providers or four TUI panels.
4. **Security policy work (#36-#39 webhook, env reads, registered secrets)** — needs design decisions, so schedule after the mechanical passes.
5. **Structural refactors (Complex #1-#2)** — largest risk and largest review cost; do them one crate per PR with `cargo nextest -p <crate>` as the gate.
6. **Test-quality sweep (#6-#14)** — cheap once the helpers are shared, so it lands naturally after step 3.
7. **Cosmetic (naming, glyphs, `mod.rs` layout, MSRV)** — last, since they touch many files for zero behavioural gain.

## Verification

- [ ] `cargo fmt --check` passes with zero diffs
- [ ] `cargo clippy --workspace --all-targets` reports zero warnings (and the new `too_many_lines` ratchet is not exceeded)
- [ ] `cargo nextest run --workspace --no-fail-fast` passes, and the five previously-orphaned test files now appear in `cargo nextest list`
- [ ] `cargo test --workspace --doc` passes
- [ ] `cargo audit` reports zero vulnerabilities other than documented, rationalised, in-tree ignores
- [ ] `cargo deny check advisories bans sources` passes
- [ ] `scripts/check-security-unwraps.sh`, `check-silent-errors.sh`, `check-test-env-isolation.sh`, `check-shared-guards.sh`, `check-dead-code-reasons.sh`, `check-team-duplication.sh`, `check-vcs-duplication.sh` all pass against the regenerated baselines
- [ ] A grep for `RAGENT_TOOLS_EXTENDED_TEST_NO_SSRF` finds it only inside `#[cfg(test)]`-gated code
- [ ] A test asserts the `QuestionAnswered` SSE payload never contains a typed secret, and one asserts the masked question field renders `*` only
- [ ] A test asserts `Debug` output of `InputField`, `ProviderSetupStep` and `AzureResourceEntry` contains no credential
- [ ] `git status --porcelain` is empty after a full test run (no test wrote into the repository)
