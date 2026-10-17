# Project Statistics

**Version:** 1.0.137

**Update prompt:** Update @STATS.md to show the composition of the project, show breakdown by crate

> Metrics below are measured against the v1.0.137 tree: the test-coverage
> campaign (roughly 80 new `tests/` suites and a set of inline
> test files that rise 13 of the 18 crates to >=85% line coverage - see the
> `[1.0.137]` CHANGELOG entry), on top of the v1.0.136 interactive
> `/plugins list` panel and the manifest/runtime tool merge fix
> (`merge_contributed_tools` / `merge_contributed_commands`), on top of the
> v1.0.135 security-hardening pass that closed the
> `docs/reports/security-audit.md` critical/high fix set - `merge_project`
> dropping project `mcp`/`acp`/provider `api`, the symlink guards, the
> bounded-HTTP read caps, the verified updater, the `http_request` header
> allowlist - plus the `ragent --version` flag and the shared-helper
> de-duplication in `ragent-types`/`ragent-surface` - on top of the v1.0.134
> MCP-discovery ergonomics, the `ask_user` default/fallback option, and the
> `/simplify all` cleanup.


## Project-wide Metrics

| Metric | Value |
|---|---|
| Total Rust lines | 639,187 (632,708 in `crates/` + 6,479 in root `src/`/`tests/`) |
| Total Rust files | 1,722 (1,704 workspace crates + 18 root `src/`/`tests/`) |
| Tests defined | ~13,111 (`#[test]` / `#[tokio::test]` attributes across `crates/`, `src/`, and root `tests/`: 13,048 in `crates/` + 63 in root) |
| Test files | 1,050 external (1,041 in `crates/` `tests/` + 9 root `tests/`) + `#[cfg(test)]`-bearing sources |
| Test binaries | ~1,078 (1,041 integration test files + 36 lib/bin targets + 1 root bin) |
| Benchmark files | 17 (+1 in `vendor/html2text`) |
| Tools registered | 151 (`browser` removed in v1.0.130; `mf_screenshot` engine-gated) |
| Supported languages (code index) | 15+ (Rust, Python, TypeScript/JavaScript, Go, C/C++, Java, OpenSCAD, Terraform, CMake, Gradle, Maven) |
| Workspace crates | 18 |
| Specs on disk | 57 directories in `specs/` |
| Documentation | 23 per-category tool how-tos in `docs/howtos/tools/` (+ generated PDFs), 23 top-level how-tos, 83 slash-command docs |
| Authors | 1 |
| Version | 1.0.137 |

---

## Breakdown by Crate

The project is organised as a Cargo workspace of 18 focused crates. The table below
shows the file count, line count, and test-file count for each crate (including
`src/`, `tests/`, `benches/`, and `examples/` directories where present).

| Crate | Rust files | Rust lines | Test files |
|---|---|---|---|
| `ragent-agent` | 371 | 128,573 | 215 |
| `ragent-bench` | 33 | 10,811 | 11 |
| `ragent-codeindex` | 80 | 25,227 | 50 |
| `ragent-config` | 71 | 17,325 | 48 |
| `ragent-connectors` | 44 | 19,235 | 21 |
| `ragent-llm` | 82 | 28,568 | 47 |
| `ragent-plugins` | 64 | 23,398 | 37 |
| `ragent-research` | 165 | 62,940 | 99 |
| `ragent-server` | 24 | 11,031 | 15 |
| `ragent-specs` | 37 | 20,468 | 24 |
| `ragent-storage` | 50 | 17,533 | 45 |
| `ragent-surface` | 17 | 2,253 | 8 |
| `ragent-telemetry` | 36 | 11,775 | 27 |
| `ragent-tools-core` | 73 | 21,409 | 36 |
| `ragent-tools-extended` | 164 | 62,052 | 87 |
| `ragent-tools-vcs` | 74 | 17,829 | 33 |
| `ragent-tui` | 256 | 138,797 | 198 |
| `ragent-types` | 63 | 13,484 | 40 |

---

## Crate Size Distribution

```
ragent-tui            ############# 138,797 lines (21.9%)
ragent-agent          ############ 128,573 lines (20.3%)
ragent-research       ###### 62,940 lines (9.9%)
ragent-tools-extended ###### 62,052 lines (9.8%)
ragent-llm            ### 28,568 lines (4.5%)
ragent-codeindex      ## 25,227 lines (4.0%)
ragent-plugins        ## 23,398 lines (3.7%)
ragent-tools-core     ## 21,409 lines (3.4%)
ragent-specs          ## 20,468 lines (3.2%)
ragent-connectors     ## 19,235 lines (3.0%)
ragent-tools-vcs      ## 17,829 lines (2.8%)
ragent-storage        ## 17,533 lines (2.8%)
ragent-config         ## 17,325 lines (2.7%)
ragent-types          # 13,484 lines (2.1%)
ragent-telemetry      # 11,775 lines (1.9%)
ragent-server         # 11,031 lines (1.7%)
ragent-bench          # 10,811 lines (1.7%)
ragent-surface        # 2,253 lines (0.4%)
```

---

## Test Distribution

| Crate | Test Files | Approx. Tests |
|-------|-----------:|--------------:|
| `ragent-tui` | 198 | ~2,747 |
| `ragent-agent` | 215 | ~1,973 |
| `ragent-tools-extended` | 87 | ~1,910 |
| `ragent-research` | 99 | ~1,158 |
| `ragent-specs` | 24 | ~683 |
| `ragent-llm` | 47 | ~520 |
| `ragent-plugins` | 37 | ~492 |
| `ragent-tools-vcs` | 33 | ~483 |
| `ragent-codeindex` | 50 | ~455 |
| `ragent-tools-core` | 36 | ~414 |
| `ragent-config` | 48 | ~400 |
| `ragent-types` | 40 | ~396 |
| `ragent-connectors` | 21 | ~361 |
| `ragent-storage` | 45 | ~346 |
| `ragent-telemetry` | 27 | ~294 |
| `ragent-server` | 15 | ~200 |
| `ragent-bench` | 11 | ~146 |
| `ragent-surface` | 8 | ~70 |
| **Total** | **1,041** | **~13,111** |

The per-crate test attributes above include both the external `tests/*.rs`
integration suites and the inline `#[cfg(test)]` modules in library sources
(`crates/` totals 13,048; the root `src/`+`tests/` tree adds 63). The
v1.0.137 test-coverage campaign added 170 new external test files
(roughly 80 in `ragent-agent` plus suites across 12 further crates) and a set
of inline test files, moving 13 of the 18 crates past 85% line coverage.

---

## Test Coverage

Measured with `cargo llvm-cov --workspace --summary-only` (line coverage,
test/bench/example sources excluded). Reproduce with:

```bash
cargo llvm-cov --workspace --summary-only --ignore-filename-regex '(^|/)(tests|benches|examples)/|target/'
```

The workspace measurement below reports the v1.0.137 tree (2026-10-17) with the
test-coverage campaign. The campaign raises 13 of the
18 crates to >=85% line coverage; where a fresh per-crate figure was not
re-measured in isolation the crate is left unchanged.

**Workspace line coverage: 69.8% -> 73.9%** (functions 69.4% -> 73.8%). The
pre-campaign baseline was 158,446 instrumented lines, 110,596 executed, 47,850
missed; region coverage was 68.6%. The prior v1.0.95 measurement was 68.9% line /
67.8% function, but it predated the `ragent-plugins`, `ragent-connectors`, and
`ragent-surface` crates, so the totals are not directly comparable.

| Crate | Baseline | After campaign |
|---|---:|---:|
| `ragent-plugins` | 90.2% | 90.3% |
| `ragent-specs` | 88.1% | 88.1% |
| `ragent-connectors` | 85.9% | 85.9% |
| `ragent-surface` | 70.2% | 90.9% |
| `ragent-types` | 82.2% | 96.3% |
| `ragent-telemetry` | 74.9% | 92.0% |
| `ragent-storage` | 81.1% | 91.5% |
| `ragent-config` | 75.4% | 90.5% |
| `ragent-codeindex` | 83.0% | 88.8% |
| `ragent-server` | 67.4% | 88.2% |
| `ragent-tools-vcs` | 75.4% | 86.9% |
| `ragent-llm` | 70.4% | 86.1% |
| `ragent-bench` | 75.2% | 86.0% |
| `ragent-tools-extended` | 79.1% | 86.0% |
| `ragent-research` | 83.2% | 85.8% |
| `ragent-tools-core` | 73.2% | 85.7% |
| `ragent-agent` | 65.1% | 85.6% |
| `ragent-tui` | 54.1% | 75.0% |
| root bin (`src/`) | 31.3% | ~31% |

Notes:

- The campaign covers 15 of the 18 crates with new or expanded suites. Tests
  only: no production logic changed (three `pub(crate)` items in
  `ragent-tools-vcs` - `run_status`, `filter_log_text`, and the GitLab
  `make_client`/`detect_project`/`detect` helpers - were widened to `pub` so
  their helpers could be exercised directly).
- The workspace total folds in all 18 crates, including `ragent-plugins` (added
  v1.0.112), `ragent-connectors` (added v1.0.123), and `ragent-surface` (added
  v1.0.129) that the pre-v1.0.95 measurement predated.
- Line counts here are executable lines under LLVM profiling, which is smaller
  than the raw `wc -l` figures in the crate table above (declarations, blank
  and comment-only lines are not instrumented).
- The largest remaining gaps are the TUI event/input paths
  (`app/slash.rs`, `app/event_handler.rs`, `input.rs`) and the agent session
  processor - the parts that require a live LLM or terminal to exercise.
- Root binary sources (`src/cli.rs`, `src/main.rs`, `src/plugins.rs`,
  `src/connectors.rs`, `src/panic_hook.rs`) remain lightly covered because
  `cargo llvm-cov` does not drive the interactive TUI.

---

## Key Architecture Ratios

- Test-to-code ratio: ~1 test per 49 lines (13,111 tests / 639,187 lines)
- Largest crate: `ragent-tui` (138,797 lines, 21.9%)
- Smallest crate: `ragent-surface` (2,253 lines, 0.4%)
- Median crate size: ~19,852 lines (`ragent-tools-core` / `ragent-connectors` midpoint)
- Crates over 10k lines: 17 of 18
- Crates under 5k lines: 1 of 18

---

_Generated 2026-10-17 (v1.0.137 tree - the test-coverage campaign -
roughly 80 new `tests/` suites raising 13 of 18 crates to >=85% line coverage,
on top of the interactive `/plugins list` panel and
the manifest/runtime tool merge fix, on top of the v1.0.135
`docs/reports/security-audit.md` critical/high fix set:
`Config::merge_project` now drops a project overlay's `mcp`, `acp`, and provider
`api` blocks (SEC-001/002/005), the new `ragent_types::guard`
`reject_symlink_components`/`reject_symlink_escape`/`open_owner_only` guards close
the symlink escapes across the specs/config/crash/askpass/TUI image paths
(SEC-006/018/027/028/059), the bounded-HTTP read caps and redirect limits land in
the MCP/masterfetch/GitHub/open-access clients (SEC-014/032/033/036/042/065/066),
the binary updater requires a verified SHA-256 digest (SEC-043), the
`http_request` header allow-list replaces the deny-list (SEC-035), the plugin
host-call sinks and the connector command/env boundaries are bounded
(SEC-009/031/071), and the shared `strutil`/`sanitize`/`store` helpers are routed
through every call site - plus the `ragent --version` flag and the code-audit
Quick Wins on top of the
v1.0.134 MCP-discovery ergonomics, `/simplify all` working
tree - the four new `ragent-surface` shared modules (`config_splice`, `marketplace`,
`net`, `scan`) and the delegation of the plugin/connector marketplace writers to
`write_atomic_json`, the collapse of the four redacting `Debug` impls into
`macro_rules! redacted_debug_impl!`, the `redact_secrets_cow` allocation fix, the
`check_path_within_allowed_roots_cached` empty-roots fast path, and the TUI
`now_unix_ms` / `resolved_default_index` / `resolve_default_index` / `format_mmss`
de-duplication (with the question-expiry `unreachable!()` removed) - on top of the
v1.0.133 `/plugins` browse-panel refinements (reversed-video navigator titles above
each `ALL` row, hit-test areas moved below the header, and per-row `Name (N)` counts), the
bottom-edge search box with a click-to-clear button, and the `git+<url>#<ref>` install
checkout fix in `ragent-plugins` (`plugins` FR-024), on top of the v1.0.132 merged
connector and plugin marketplace browsers (additional marketplaces, a per-row
marketplace column, and off-loop description/version enrichment), the connector and
plugin-store alphabetical id sorts (`connectors` FR-046, `pluginstores` FR-050), the
connector-catalogue pinned heading row (`connectors` FR-045), and the plugin-store
navigator tag-row withdrawal (`catnav` FR-021) - on top of the v1.0.130 release line. The v1.0.130 set adds the OpenHands parity
delta on top of the release line: pluggable execution backends with a
health-visible registry and a TUI `/backend` switcher, sandbox secret injection,
an ACP client and a feature-gated ACP server, an OpenAI-compatible inbound surface
plus a generated OpenAPI 3.1 contract and HTTP MCP transport, the automation
service with webhook ingress, scheduler, run history, and backend confinement,
AgentSkills (`.agents/skills/`) discovery, and an LLM security analyzer; the
plugin-store category navigator (`catnav`); and the `browser` CDP tool and its
`browser` visibility switch removed, taking the registered count 152 -> 151 across
22 categories. **Dependency and standards work** on this tree: the `ragent-agent`,
`ragent-server`, and `ragent-tools-extended` manifests gain the new modules'
dependencies and the workspace gains the opt-in `acp-server` / `headless-browser`
features. **OpenHands quality gates** (spec `openhands` T-019): the engine-gated
`mf_screenshot` capability is registered only when a drivable headless browser is
present, so the default build advertises 148 tools across 21 categories. **Prior
release line.** Builds on v1.0.129: the `docs/plans/code-audit.md`
remediation plan through M9 as uncommitted working-tree work. Dependency upkeep
(M9): the yanked `yoke-derive` is cleared; all crates converge on `thiserror 2`,
`reqwest 0.13`, `rand 0.10`, and `criterion 0.8`; `ragent-storage` moves from
`rand 0.8` to `0.10`; `lopdf` converges on `0.44` and the dedicated `vendor/lopdf`
crate is deleted; `opentelemetry` 0.29 -> 0.33 (`experimental_metrics_custom_reader`
for the Prometheus scrape endpoint); `notify` 8.2, `rquickjs` 0.14, `tree-sitter`
0.27, `chacha20poly1305` 0.11, `which` 8. Dead-code removal (M5): the stale
`ragent-agent::snapshot` module and the duplicated inline schema test suite are
deleted, placeholder tests gain real assertions, the dead connector `NeverProbe`/
`NoProbe` fallbacks go, and the new 18th `ragent-surface` crate owns the shared
`/plugins`/`/connectors` surface glue. Test hygiene (M6): scratch paths move to
`target/temp`, `TempTree` support modules replace 28 copy-pasted sandboxes,
diagnostic and live-network tests are `#[ignore]`-gated, the config-key test
triples collapse to one table-driven test, and four test files are renamed to the
`test_<component>_<scenario>` convention. New coverage (M7): the calculator, the
small tools (`agent_complete`, `bash_reset`, `xlsx`, `get_env`), the Azure AI
Foundry provider, the agent-loop step harness, and the orchestrator `Coordinator`
all gain `tests/` suites (fixing a real `active_jobs` underflow). Standards (M8):
GitHub/GitLab acronym casing, the three user-facing unwraps, emoji/box-drawing
stripping, `//!` module headers, the `MultipleAdapter` rename, and the
source-scraping test rewrite. Security (M1): `.gitignore` gains the SQLite
sidecar and certificate/credential patterns, and a tracked `.env.example`
template lands. Also fixes the config-write comparison folding a corrupt file to
`Null`. Builds on
v1.0.128: the `ragent.json` write path lands in a uniquely named temp file,
`fsync`-ed and renamed over the target, so an interrupted write can no longer
silently reset a persisted YOLO flag; runtime-flag persistence edits only the
single top-level key in the raw global file (`Config::set_global_bool_key`); every
MCP connect runs under a bounded per-attempt timeout with a single timeout retry;
the TUI startup no longer blocks on the MCP connect loop; MCP connect failures are
recorded as `McpStatus::Failed` and reported instead of being dropped; the
`ragent_info` tool reports runtime execution details (pid, uptime, memory, thread
count) alongside build metadata; runtime-flag writes and the load-time state are
logged with attribution; and `/spec reverse --folder` scaffolds before the token
gate. Builds on
v1.0.126: the Office / LibreOffice document
tools (`office_read/write/info`, `libre_read/write/info`) and their
`office_common` / `libreoffice_*` modules removed (only the PDF family
`pdf_read` / `pdf_write` remains, sharing a new `pdf_common` helper), the
`tool_visibility.office` switch and the OOXML/ODF `DocumentFormat` variants
dropped, and a `/simplify all` pass over the changed set (pdf_write image-path
containment, single-parse pdf_read, single-pass connector lookup). Builds on
v1.0.125: the
`os_info` host-introspection tool (GPU adapters, graphics-API versions, physical
hardware) and its `/osinfo show [--no-probe]` slash command, the research
`--papers` default flip (scholarly engines now excluded by default), and three
new tracked research items (`codemigrate`, `connectors`, `vendormarketplace`) -
alongside earlier `/memory clear`, the session-start connector lifecycle, the
MCP orphan sweep, and the committed v1.0.122 security and anti-pattern
remediation sweep folding in the `ANTIPAT.md` M0 plus M2-M7 and `SECTASKS.md`
MS-05 work - `ragent-team` shim crate removed - and the `ragent_types::guard`
shared-guard module added. Earlier v1.0.121 work: rollback-capture
remove-after-restore race fix (CI flake
`test_rollback_accept_restores_snapshot`), OpenSkills `.agents` discovery, Claude
marketplace `*-lsp` inline-manifest materialisation, conventional `skills/`
directory bridging, plus a `/simplify all` pass over the 50-file changed set. Tool
count is 151 registered (the `browser` CDP tool and its `browser` visibility switch
removed this cycle; 6 Office/LibreOffice document tools removed in v1.0.126; 8 finance tools and 6 plot tools removed post v1.0.125; `os_info` added in v1.0.125; `tool_info` +
`commands_info` added in v1.0.120).)_
