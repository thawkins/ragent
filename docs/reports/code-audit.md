# Code Audit Report

**Project**: Rust 2024 Cargo workspace — `ragent`, an AI coding agent CLI/TUI (18 crates + root binary; tokio, ratatui, axum, SQLite, tracing, clippy pedantic/nursery)
**Scope**: Full project source (`crates/*/src`, `src/`, `Cargo.toml`, `deny.toml`, test suites) — `deep` was not a path, so no sub-directory scoping applied
**Findings**: 73 total (8 high, 19 medium, 46 low)

---

## High Priority

### Duplication
- Six per-engine `truncate_query`/`truncate_snippet` wrappers are pure one-line delegations to `engine::{truncate_query_to, truncate_snippet, truncate_snippet_bytes}` — collapse the indirection — `crates/ragent-tools-extended/src/masterfetch/search/exa.rs:281` (and `tavily.rs:245`, `perplexity.rs:264`, `openalex.rs:452`, `wikipedia.rs:575`, `serper.rs:310`) all forwarding to `engine.rs:874`
- UTF-8/char-boundary truncation reimplemented ~15x with identical `char_indices`/`chars().take()` logic — extract one shared `truncate_to_char_boundary` into `ragent-types`/`ragent-surface` — `crates/ragent-agent/src/session/history.rs:250` (and `crates/ragent-agent/src/task/mod.rs:1256`, `crates/ragent-research/src/cluster.rs:450`, `crates/ragent-research/src/session/topic.rs:432`, `crates/ragent-tui/src/app/helpers.rs:308`, `crates/ragent-tui/src/widgets/message_widget.rs:105`)
- `truncate_preview(s, max)` duplicated verbatim as a private helper in two crates — hoist to `ragent-surface::harness` — `crates/ragent-agent/src/session/processor.rs:4515` (and `crates/ragent-tools-core/src/edit_log.rs:477`)
- `truncate_content` exists twice with near-identical signatures/bodies — `crates/ragent-agent/src/reference/resolve.rs:250` (and `crates/ragent-tools-core/src/truncate.rs:49`; tools-core already takes `impl AsRef<str>`, so agent should reuse it)
- Provider request-body construction is copy-pasted across essentially every LLM provider — `build_request_body` appears 12x and `default_models` 16x; no shared builder in `ragent-llm` — `crates/ragent-llm/src/providers/` (openai.rs, anthropic.rs, gemini.rs, ollama.rs, openrouter.rs, bedrock.rs, azure_foundry.rs, copilot.rs, huggingface.rs, generic_openai.rs, openai_responses.rs, ollama_cloud.rs)
- Redaction/sensitive-value masking logic recurs in ~38 source files (20+ providers plus config, storage, telemetry, server, tools, tui) — should be a single primitive — `crates/ragent-llm/src/providers/` (all), `crates/ragent-config/src/config.rs`, `crates/ragent-storage/src/storage.rs`, `crates/ragent-telemetry/src/sensitive.rs`, `crates/ragent-server/src/sse.rs`

### Testing
- 128 inline `#[cfg(test)]` modules remain in library `src/`, violating the AGENTS-RUST rule that all tests live in each crate's `tests/` — concentrated in `crates/ragent-research/src/` (~40), `crates/ragent-llm/src/providers/` (18), `crates/ragent-agent/src/` (26) — representative site `crates/ragent-agent/src/task/mod.rs:1297`
- Nested `#[cfg(test)]` attributes sit inside function bodies (not a trailing `mod tests`), an unusual inline-test shape the removal tooling may miss — `crates/ragent-agent/src/research_adapter.rs:325` (and `:533`, `:709`)

---

## Medium Priority

### Duplication
- API-key masking exists as a one-off `mask_key` in openrouter only while other providers hand-roll redaction — no shared key-mask helper in `ragent-llm` — `crates/ragent-llm/src/providers/openrouter.rs:41`
- Output truncation `truncate_output(String)` duplicated — `crates/ragent-tools-core/src/bash.rs:1226` (and `crates/ragent-tools-extended/src/pdf_common.rs:32`)
- `truncate` generic helper duplicated across unrelated crates — `crates/ragent-agent/src/compaction/serializer.rs:167` (and `crates/ragent-surface/src/harness.rs:60`, `crates/ragent-research/src/cli.rs:862`, `crates/ragent-plugins/src/control.rs:591`)
- Retry/backoff loops appear across ~40 files with no shared backoff utility — extract into `ragent-types` or a small `ragent-llm::retry` — `crates/ragent-llm/src/providers/http_client.rs` (and `crates/ragent-agent/src/mcp/http.rs`, `crates/ragent-telemetry/src/recorder.rs`)
- `truncate_str(&str, max) -> String` duplicated with same name/behaviour in two crates — `crates/ragent-agent/src/task/mod.rs:1256` (and `crates/ragent-tui/src/widgets/message_widget.rs:105`)

### Logging
- TUI logs a message-processing failure at `debug!` while the identical failure is `error!` in the server — raise to `error!` — `crates/ragent-tui/src/app/session_ops.rs:874` (and `:916`; compare `crates/ragent-server/src/routes/mod.rs:562`)
- Full Ollama Cloud request body (chat prompt + tool defs, up to 800 bytes) written to the debug log — redacted for secrets but still logs user content — gate behind a stricter flag or log metadata only — `crates/ragent-llm/src/providers/ollama_cloud.rs:528`
- Provider error-response body logged verbatim (redacted for secrets only) — confirm `redact_secrets` covers all credential shapes, else log status only — `crates/ragent-llm/src/providers/copilot.rs:374`

### Security
- Legacy v1 credential obfuscation key is a hardcoded, publicly-known constant (`b"ragent-obfuscation-key-v1"`); any v1 blob is trivially reversible and the literal ships in every binary — `crates/ragent-storage/src/storage.rs:113`
- Legacy v2 credential key is derived by `blake3::derive_key` over non-secret material (`$USER:$HOME`) with a documented prior hardcoded-literal fallback; key material is guessable if those values are known — `crates/ragent-storage/src/storage.rs:259`
- `deny.toml` suppresses quick-xml DoS advisories RUSTSEC-2026-0194/0195 on a "trusted XML input" assumption — an unmitigated DoS if ragent ever parses user- or web-supplied XML/OOXML — `deny.toml:26`
- `deny.toml` suppresses the `unsound` use-after-free advisory RUSTSEC-2026-0253; the suppression depends entirely on "ragent never calls `LruCache::pop()`" and has no CI guard enforcing it — `deny.toml:35`

### Testing
- `ragent-surface` (priority focus area) has a single test file for 4 source modules; thin coverage of surface helpers/attribution/tokeniser/store-dir resolution — `crates/ragent-surface/tests/test_surface_helpers.rs:1`
- `ragent-bench` is thinly covered: 22 `src` files vs 5 test files, with three inline modules still in `src/` — `crates/ragent-bench/src/data.rs:853` (and `:1412`, `:1529`; `crates/ragent-bench/src/model.rs:585`)
- Inline test module embedded in a 7,778-line `layout.rs`, the largest inline block remaining; hard to maintain and should be relocated — `crates/ragent-tui/src/layout.rs:7778`
- Inline test module in a 3,186-line `web_gatherer.rs` (research crate, largest concentration of inline tests) — `crates/ragent-research/src/web_gatherer.rs:3186`
- Both `crates/ragent-telemetry/tests/inline/` and `crates/ragent-tools-vcs/tests/inline/` directories exist alongside relocated top-level test files; dual layout risks duplication and is inconsistent with other crates' flat `tests/` layout — `crates/ragent-telemetry/tests/inline/` (and `crates/ragent-tools-vcs/tests/inline/`)
- `ragent-llm` provider clients (priority focus) keep inline modules across 18 provider files (anthropic, openai, gemini, bedrock, copilot, ollama, huggingface, xai, thinking, router_*) rather than the crate's `tests/` dir — `crates/ragent-llm/src/providers/anthropic.rs:656`

### Dependencies
- Over-broad/stale advisory suppressions in `.cargo/audit.toml` — 8 entries but at least 5 no longer apply (RUSTSEC-2026-0187 lopdf now 0.44.0; RUSTSEC-2024-0384 `instant` absent; RUSTSEC-2025-0134 `rustls-pemfile` absent; RUSTSEC-2026-0192 `ttf-parser` absent); `deny.toml` already documents these as un-ignored, so the two files disagree — `.cargo/audit.toml:5`

---

## Low Priority

### Standards
- Inconsistent error output channel: session export/import failures write to stderr via `eprintln!`, while the top-level error path writes to stdout via `println!("Error: {e}")` — `src/main.rs:1182` (and `:1213` vs `:1560`)
- Inconsistent CLI diagnostic prefixes across command families (`ragent-research:`, `ragent new:`, `ragent spec govcreate: [err]`) for the same class of user error — `src/cli.rs:746` (and `:1142`, `:1580`)
- Mixed `println!`/`eprintln!` for user-facing status vs error within a single handler; research subcommands print success to stdout but emit failures both with and without the `ragent-research:` prefix — `src/cli.rs:548` (vs `:740`)
- `src/cli.rs` (~1600 lines) and `src/main.rs` (~1560 lines) are oversized modules combining CLI parsing/dispatch with research, spec, scaffold, and session import/export orchestration; consider splitting by command family — `src/cli.rs:1` (and `src/main.rs:1`)

### Duplication
- `truncate_cell` duplicated — `crates/ragent-research/src/comparison.rs:199` (and `crates/ragent-tools-extended/src/pdf_write.rs:595`)
- Title/snippet word-boundary truncation duplicated — `crates/ragent-research/src/web_gatherer/title.rs:114` (and `crates/ragent-research/src/item.rs:628`)
- `as_any_static` boilerplate repeated 6x across crates (trait-object downcast shim) — candidate for a macro in `ragent-types` — `crates/*/src` (6 sites)
- `truncate_summary`/`truncate_snippet`/`truncate_body` family re-declared per module despite shared engine helpers — `crates/ragent-tools-extended/src/masterfetch/crawl/classify.rs:477` (and `masterfetch/extractor.rs:1010`, `masterfetch/http.rs:187`, `research/analysis/parser.rs:602`, `research/document.rs:2326`)

### Logging
- HTTP retry paths log the raw `reqwest` error (`error = %e`), which can embed the response body and request URL — sanitize/truncate before logging — `crates/ragent-llm/src/providers/http_client.rs:377` (and `:332`, `:381`)
- Rustdoc examples demonstrate logging a partial token (`&token[..8]`), teaching a credential-leak pattern — change examples to log `"<redacted>"` — `crates/ragent-llm/src/providers/copilot.rs:579` (and `:775`, `:935`)
- Near-verbatim credential-source message ("using `gh auth token` as the GitHub credential") — reword to avoid implying a token value is present — `crates/ragent-config/src/github.rs:112`
- Plugin-command prompt processing failure logged at `debug!`, out of step with error-class failures logged at `error!`/`warn!` elsewhere — `crates/ragent-tui/src/app/slash.rs:11299`
- Stale HuggingFace model cleanup failure logged at `debug!` though it affects user-visible model-list state — consider `warn!` — `crates/ragent-tui/src/app/init.rs:86`
- Plan-agent failure logged at `debug!` only — `crates/ragent-tui/src/app/event_handler.rs:138`
- Demo fn mixes `tracing::info!` with `println!` in the same body (allowed CLI surface, but inconsistent) — use one channel — `src/cli.rs:51` (vs `:85`)
- `println!("{:#?}", *config)` dumps the fully resolved config struct to stdout — relies on the custom `Debug` redaction impl; verify no secret field is added without a matching redaction — `src/main.rs:1383`
- Commented-out debug `println!` left in tree — remove — `examples/parallel_edit.rs:36` (outside stated source scope; flagged only)
- Stale `FR-XXX` placeholder marker left in a comment — replace with the real FR id or delete — `crates/ragent-research/src/session.rs:2225`

### Security
- Credential-store write path is documented as potentially landing on a 0644 world-readable file, letting another local user read encrypted credentials/ciphertext for offline attack — `crates/ragent-storage/src/storage.rs:277`
- Bedrock provider opens a credentials file at an arbitrary path taken verbatim from `AWS_SHARED_CREDENTIALS_FILE` env var, with no containment check — `crates/ragent-llm/src/providers/bedrock_credentials.rs:234`
- `get_env` tool returns arbitrary process environment variables to the model; safety rests solely on ad-hoc name-substring and value-shape redaction, not an allowlist — `crates/ragent-tools-core/src/get_env.rs:131`
- `ragent_info` reports process environment values by probing a fixed key list (may surface tokens/keys not caught by the redactor) — `crates/ragent-agent/src/tool/ragent_info.rs:311`
- `os_info` and telemetry read `HOSTNAME` and other env vars directly into reported/exported attributes without going through the redaction registry — `crates/ragent-agent/src/tool/os_info.rs:1932`
- masterfetch search reads API keys by iterating caller/config-supplied env-var names via `std::env::var`, widening the secret-exfiltration surface from model-driven config — `crates/ragent-tools-extended/src/masterfetch/tools/search_tool.rs:179`
- Provider base URLs and keys are read from ambient env across many providers (router_client/generic_openai/azure_foundry/ollama) with no scheme/host validation, enabling endpoint redirection via a poisoned environment — `crates/ragent-llm/src/providers/router_client.rs:589`
- Connector auth resolves and stores secrets from ambient environment variables by arbitrary name, with the trust boundary resting on the connector descriptor rather than an allowlist — `crates/ragent-connectors/src/auth.rs:365`
- Bedrock credential resolution reads `AWS_PROFILE`/`AWS_REGION`/`AWS_BEDROCK_REGION` from env and silently follows profiles/files they point at, expanding the credential-loading attack surface — `crates/ragent-llm/src/providers/bedrock_credentials.rs:90`
- `.gitignore` ignores `.env*` but re-includes `.env.example`; no committed example file was found in scope, so the allow-rule currently protects nothing and risks a future example file carrying placeholder-but-real keys — `.gitignore:22`
- Inline bedrock tests embed realistic AWS secret keys (`wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY`) as string literals in source; these are the public AWS doc sample (not live) but normalise real-key-shaped fixtures in-repo — `crates/ragent-llm/src/tests/inline/bedrock_credentials_tests.rs:12`

### Testing
- `#[cfg(test)]` appears as a literal inside a test-fixture code string rather than as an attribute; confuses inline-test scanners — `crates/ragent-bench/tests/test_bench_core.rs:516`
- Stray `#[cfg(test)]` markers inside already-external test files suggest leftover migration noise to clean up — `crates/ragent-tools-extended/tests/test_pdf_expert_cff.rs:30` (and `:73`; `crates/ragent-tui/tests/test_research_viewer.rs:96`; `crates/ragent-codeindex/tests/test_rust_parser.rs:366`)
- `crates/ragent-agent/src/tests/inline/*` holds five relocated module tests under `src/` rather than `tests/`, contradicting the "tests belong in `tests/`" convention — `crates/ragent-agent/src/tests/inline/template_mod_tests.rs:1`
- Duplicate `#[cfg(test)]` markers at adjacent lines in the same file (likely a doubled attribute) — `crates/ragent-llm/src/providers/router_classifier.rs:979` (and `:983`; `crates/ragent-specs/src/impl_runner.rs:854`, `:858`)

### Dependencies
- `uuid` 1.26.1 -> 1.27.0 (minor) across all members — workspace pin in `Cargo.toml` — bump via `cargo update -p uuid`
- `pulldown-cmark` 0.12.2 -> 0.13.4 (1 major behind, ragent-tui) — `crates/ragent-tui/Cargo.toml` — review 0.13 breaking changes then bump
- `syn` 2.0.119 -> 3.0.6 (1 major behind) — `crates/ragent-config/Cargo.toml` (and `crates/ragent-types/Cargo.toml`) — bump when proc-macro deps permit
- `serial_test`/`serial_test_derive` 3.5.0 -> 4.0.1 (dev/build-only) — `crates/ragent-config/Cargo.toml` (and `crates/ragent-types/Cargo.toml`) — bump dev-dependency
- `tree-sitter` 0.27.0 -> 0.27.1 and `tree-sitter-language` 0.1.8 -> 0.1.9 (patch) — `crates/ragent-codeindex/Cargo.toml` — routine patch bump
- `unicode-width` 0.2.0 -> 0.2.2 (patch, ragent-tui) — `crates/ragent-tui/Cargo.toml` — routine patch bump
- Duplicate major versions in tree (multiple-versions = "warn") — `base64` 0.22.1/0.23.1, `html5ever` 0.26/0.38, `rand` 0.8/0.9/0.10, `getrandom` 0.2/0.3/0.4, `syn` 1/2/3, `hashbrown` 0.14/0.16/0.17 — transitive; consolidate via `[patch]`/`cargo update` only where a shared upstream version exists
- `libc` 0.2.189 -> 0.2.190 (patch) — root `Cargo.toml` — routine patch bump; the single approved `unsafe` site (`kill_process_group`) depends on it, re-verify after bump
- No CI freshness gate beyond `cargo deny`/`cargo audit` — advisory DB age and stale suppressions drift silently — add a scheduled `cargo audit`/`cargo outdated` job that fails on new advisories

---

## Notes / Positive Results
- No raw `println!`/`eprintln!`/`print!`/`eprint!`/`dbg!` calls exist in any library crate's `src/`; every `crates/*/src` hit is a doc-comment example, a generated-code string literal, or a comment. Real macro call sites live only in the allowed CLI presentation surface (`src/main.rs`, `src/cli.rs`, `src/connectors.rs`, `src/plugins.rs`) and the documented pre-tracing `src/panic_hook.rs` exception.
- No wildcard imports in production code (all `use ...::*` occurrences are `use super::*` inside test modules).
- `cargo audit`: 0 vulnerabilities; `Cargo.lock` present and tracked; `deny.toml` correctly sets `[advisories] unsound = "all"` and `[sources]` denies unknown registries/git.
- Caveat: the standards agent was interrupted before completing the `unwrap`/`expect`/`panic`/`unwrap_or_default`/`let _ =` sweep and the anyhow-vs-thiserror comparison; those categories are unverified rather than cleared.

---

# Implementation Plan

## Quick Wins (< 30 min each)
| # | Finding | File(s) | Fix |
|---|---------|---------|-----|
| 1 | Remove commented-out debug `println!` | `examples/parallel_edit.rs:36` | Delete the stale line |
| 2 | Stale `FR-XXX` placeholder | `crates/ragent-research/src/session.rs:2225` | Replace with real FR id or delete |
| 3 | Rustdoc examples logging partial token | `crates/ragent-llm/src/providers/copilot.rs:579,775,935` | Log `"<redacted>"` in examples |
| 4 | Reclassify `debug!` error-class logs | `crates/ragent-tui/src/app/session_ops.rs:874,916`; `slash.rs:11299`; `init.rs:86`; `event_handler.rs:138` | Raise to `warn!`/`error!` |
| 5 | Sanitize raw reqwest error in retry logs | `crates/ragent-llm/src/providers/http_client.rs:332,377,381` | Log status/cause only |
| 6 | Unify CLI error channel/prefixes | `src/main.rs:1182,1213,1560`; `src/cli.rs:548,740,746,1142,1580` | Route all errors to stderr with one prefix scheme |
| 7 | Prune stale audit suppressions | `.cargo/audit.toml:5` | Remove 5 obsolete advisory IDs to match `deny.toml` |
| 8 | Patch dependency bumps | `Cargo.toml` (libc, uuid); `crates/ragent-codeindex/Cargo.toml` (tree-sitter); `crates/ragent-tui/Cargo.toml` (unicode-width) | `cargo update -p <crate>` |
| 9 | Clean stray/duplicate `#[cfg(test)]` markers | `tests/test_pdf_expert_cff.rs:30,73`; `test_research_viewer.rs:96`; `test_rust_parser.rs:366`; `router_classifier.rs:979,983`; `impl_runner.rs:854,858`; `test_bench_core.rs:516` | Remove markers/literals |
| 10 | Reword credential-source messages | `crates/ragent-config/src/github.rs:112` | Remove token-implying phrasing |
| 11 | `mask_key` one-off vs hand-rolled redaction | `crates/ragent-llm/src/providers/openrouter.rs:41` | Promote to shared helper (feeds #13) |

## Medium Effort (30 min - 2 hours each)
| # | Finding | File(s) | Fix |
|---|---------|---------|-----|
| 1 | Shared truncation/char-boundary primitive | `ragent-types`/`ragent-surface`; ~15 call sites incl. `session/history.rs:250`, `task/mod.rs:1256`, `cluster.rs:450`, `helpers.rs:308`, `message_widget.rs:105` | Add `truncate_to_char_boundary`, replace all copies |
| 2 | Consolidate `truncate_preview`/`truncate_content`/`truncate_output`/`truncate`/`truncate_str`/`truncate_cell` | `ragent-surface::harness`; sites in agent/tools-core/tools-extended/research/plugins | Extract single helper set; delete duplicates |
| 3 | Shared provider request-body builder | `crates/ragent-llm/src/providers/*` (12 `build_request_body`, 16 `default_models`) | Add builder module; migrate all providers |
| 4 | Collapse per-engine truncate wrappers | `crates/ragent-tools-extended/src/masterfetch/search/*.rs` | Call `engine::*` directly |
| 5 | Shared redaction primitive | `ragent-llm`/`ragent-config`/`ragent-storage`/`ragent-telemetry` (~38 files) | Extract one masking primitive, route all sites |
| 6 | Shared retry/backoff utility | `ragent-llm::retry`/`ragent-types`; `http_client.rs`, `mcp/http.rs`, `recorder.rs` | Extract backoff helper |
| 7 | Gate user-content debug log | `crates/ragent-llm/src/providers/ollama_cloud.rs:528` | Log metadata only / stricter flag |
| 8 | Verify provider error-body redaction | `crates/ragent-llm/src/providers/copilot.rs:374` | Status-only logging or tighten redactor |
| 9 | Re-key legacy credential store (v1/v2) | `crates/ragent-storage/src/storage.rs:113,259,277` | Migrate off hardcoded/guessable keys; enforce 0600 |
| 10 | Guard/env-allowlist hardening | `get_env.rs:131`, `ragent_info.rs:311`, `os_info.rs:1932`, `search_tool.rs:179`, `auth.rs:365`, `router_client.rs:589`, `bedrock_credentials.rs:90,234` | Allowlist env names; validate base URL scheme/host; contain credential paths |
| 11 | Move inline tests out of `src/` (batch 1: providers + bench) | `crates/ragent-llm/src/providers/*`, `crates/ragent-bench/src/data.rs,model.rs` | Relocate to crate `tests/` per AGENTS-RUST |
| 12 | Normalise `tests/inline/` layout | `crates/ragent-telemetry/tests/inline/`, `crates/ragent-tools-vcs/tests/inline/`, `crates/ragent-agent/src/tests/inline/` | Flatten to `tests/` |
| 13 | Add coverage for thin crates | `crates/ragent-surface/tests/`, `crates/ragent-bench/tests/` | New tests for surface helpers + bench core |
| 14 | `.gitignore` `.env.example` re-include | `.gitignore:22` | Add tracked placeholder example file or drop the allow-rule |
| 15 | Major dep bumps | `crates/ragent-tui/Cargo.toml` (pulldown-cmark), `ragent-config`/`ragent-types` (syn, serial_test) | Review breaking changes, bump |

## Complex (> 2 hours)
| # | Finding | File(s) | Fix |
|---|---------|---------|-----|
| 1 | 128 inline `#[cfg(test)]` modules in `src/` | `crates/ragent-research/src/` (~40), `ragent-llm/src/providers/` (18), `ragent-agent/src/` (26) | Migrate all to `tests/`; add `check-inline-tests` guard to CI |
| 2 | Nested `#[cfg(test)]` inside fn bodies | `crates/ragent-agent/src/research_adapter.rs:325,533,709` | Restructure and relocate |
| 3 | Oversized inline blocks | `crates/ragent-tui/src/layout.rs:7778`, `crates/ragent-research/src/web_gatherer.rs:3186` | Split + relocate test modules |
| 4 | quick-xml RUSTSEC-2026-0194/0195 | `deny.toml:26`; `Cargo.lock` quick-xml 0.39.4 | Upgrade once `ooxmlsdk` compatible, or add XML input-size guards |
| 5 | lru RUSTSEC-2026-0253 unsound suppression | `deny.toml:35`; tantivy 0.26 path | Move tantivy off lru 0.16 or add CI guard asserting no `LruCache::pop()` |
| 6 | Oversized `cli.rs`/`main.rs` split | `src/cli.rs:1`, `src/main.rs:1` | Extract command families into modules |
| 7 | Duplicate major versions in tree | `base64`, `html5ever`, `rand`, `getrandom`, `syn`, `hashbrown` | Coordinated `[patch]`/upstream consolidation |
| 8 | Missing CI freshness gate | `.github/` workflows | Scheduled `cargo audit`+`cargo outdated` failing on new advisories |

## Suggested Fix Order
1. **Quick Wins #1–#11**: mechanical deletions, log-level fixes, doc wording, and patch bumps — low risk, immediate hygiene.
2. **Utilities/shared code first (#1–#6 Medium)**: shared truncation, redaction, retry, and provider-builder helpers — these are the prerequisites that make the duplication and later refactors land cleanly.
3. **Security hardening (#7–#10 Medium)**: credential re-keying, env allowlisting, URL validation, log gating — isolated, high-value, verify-only risk.
4. **Tests (#11–#14 Medium, then Complex #1–#3)**: relocate inline tests and add missing coverage after the source settles (moving tests after extraction avoids churn).
5. **Dependency work (#15 Medium, Complex #4–#7)**: major bumps and advisory resolution last, when the tree is stable.
6. **Cosmetic (#8 Complex, #6 Complex)**: module splits once logic has been consolidated.

## Verification
- [ ] `cargo build` and `cargo clippy --workspace --all-targets` report 0 errors/warnings
- [ ] `cargo fmt --check` passes on all edited `.rs` files
- [ ] `cargo test --workspace` passes; new tests cover previously-untested `ragent-surface` and `ragent-bench`
- [ ] `scripts/check-inline-tests.sh` reports 0 inline `#[cfg(test)]` modules under `crates/*/src`
- [ ] `cargo audit` and `cargo deny check` pass with `.cargo/audit.toml` and `deny.toml` suppressions reconciled
- [ ] Redaction test (`test_config_redaction.rs`) still passes after unifying the masking primitive
- [ ] Credential store can read legacy v1/v2 blobs and re-encrypt under the new key
