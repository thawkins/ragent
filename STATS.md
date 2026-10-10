# Project Statistics

**Version:** 1.0.131

**Update prompt:** Update @STATS.md to show the composition of the project, show breakdown by crate

> Metrics below are measured against the v1.0.131 tree (OpenHands parity delta,
> `catnav` navigator, and `browser` tool removal on top of the release line).


## Project-wide Metrics

| Metric | Value |
|---|---|
| Total Rust lines | 560,727 (554,543 in `crates/` + 6,184 in root `src/`/`tests/`) |
| Total Rust files | 1,520 (1,503 workspace crates + 17 root `src/`/`tests/`) |
| Tests defined | ~10,507 (`#[test]` / `#[tokio::test]` attributes across `crates/`, `src/`, and root `tests/`) |
| Test files | 857 external (849 workspace crates + 8 root `tests/`) + ~198 `#[cfg(test)]`-bearing sources |
| Test binaries | ~876 (857 integration test files + 18 lib/bin targets + 1 root bin) |
| Benchmark files | 17 (+1 in `vendor/html2text`) |
| Tools registered | 151 (`browser` removed in v1.0.130; `mf_screenshot` engine-gated) |
| Supported languages (code index) | 15+ (Rust, Python, TypeScript/JavaScript, Go, C/C++, Java, OpenSCAD, Terraform, CMake, Gradle, Maven) |
| Workspace crates | 18 |
| Specs on disk | 57 directories in `specs/` |
| Documentation | 23 per-category tool how-tos in `docs/howtos/tools/` (+ generated PDFs), 23 top-level how-tos, 83 slash-command docs |
| Authors | 1 |
| Version | 1.0.130 |

---

## Breakdown by Crate

The project is organised as a Cargo workspace of 18 focused crates. The table below
shows the file count, line count, and test-file count for each crate (including
`src/`, `tests/`, `benches/`, and `examples/` directories where present).

| Crate | Rust files | Rust lines | Test files |
|---|---|---|---|
| `ragent-agent` | 312 | 105,589 | 156 |
| `ragent-bench` | 28 | 8,619 | 6 |
| `ragent-codeindex` | 73 | 24,140 | 43 |
| `ragent-config` | 59 | 14,830 | 36 |
| `ragent-connectors` | 38 | 16,660 | 17 |
| `ragent-llm` | 76 | 24,736 | 41 |
| `ragent-plugins` | 57 | 20,886 | 32 |
| `ragent-research` | 159 | 60,648 | 93 |
| `ragent-server` | 22 | 9,847 | 13 |
| `ragent-specs` | 37 | 20,362 | 24 |
| `ragent-storage` | 40 | 15,555 | 35 |
| `ragent-surface` | 6 | 560 | 2 |
| `ragent-telemetry` | 30 | 10,585 | 21 |
| `ragent-tools-core` | 66 | 19,164 | 29 |
| `ragent-tools-extended` | 159 | 60,168 | 82 |
| `ragent-tools-vcs` | 66 | 15,970 | 25 |
| `ragent-tui` | 224 | 115,666 | 166 |
| `ragent-types` | 51 | 10,558 | 28 |

---

## Crate Size Distribution

```
ragent-tui            ############# 115,666 lines (20.9%)
ragent-agent          ########## 105,589 lines (19.0%)
ragent-tools-extended ####### 60,168 lines (10.9%)
ragent-research       ####### 60,648 lines (10.9%)
ragent-llm            ### 24,736 lines (4.5%)
ragent-codeindex      ### 24,140 lines (4.4%)
ragent-plugins        ## 20,886 lines (3.8%)
ragent-specs          ## 20,362 lines (3.7%)
ragent-tools-core     ## 19,164 lines (3.5%)
ragent-connectors     ## 16,660 lines (3.0%)
ragent-tools-vcs      ## 15,970 lines (2.9%)
ragent-storage        ## 15,555 lines (2.8%)
ragent-config         # 14,830 lines (2.7%)
ragent-telemetry      # 10,585 lines (1.9%)
ragent-types          # 10,558 lines (1.9%)
ragent-server         # 9,847 lines (1.8%)
ragent-bench          # 8,619 lines (1.6%)
ragent-surface        # 560 lines (0.1%)
```

---

## Test Distribution

| Crate | Test Files | Approx. Tests |
|-------|-----------:|--------------:|
| `ragent-tui` | 166 | ~1,856 |
| `ragent-tools-extended` | 82 | ~1,831 |
| `ragent-agent` | 156 | ~1,296 |
| `ragent-research` | 93 | ~1,098 |
| `ragent-specs` | 24 | ~683 |
| `ragent-plugins` | 32 | ~446 |
| `ragent-codeindex` | 43 | ~412 |
| `ragent-tools-vcs` | 25 | ~408 |
| `ragent-llm` | 41 | ~399 |
| `ragent-connectors` | 17 | ~318 |
| `ragent-config` | 36 | ~317 |
| `ragent-tools-core` | 29 | ~316 |
| `ragent-types` | 28 | ~303 |
| `ragent-storage` | 35 | ~295 |
| `ragent-telemetry` | 21 | ~260 |
| `ragent-server` | 13 | ~138 |
| `ragent-bench` | 6 | ~63 |
| `ragent-surface` | 2 | ~16 |
| **Total (external)** | **849** | **~10,455** |

Inline `#[cfg(test)]` modules in library sources contribute the remaining
test attributes (a handful under root `src/`/`tests/`; the migration effort has
moved the bulk of these into each crate's `tests/` tree), bringing the estimated
total to ~10,507.

---

## Test Coverage

Measured with `cargo llvm-cov --workspace --summary-only` (line coverage,
test/bench/example sources excluded). Reproduce with:

```bash
cargo llvm-cov --workspace --summary-only --ignore-filename-regex '(^|/)(tests|benches|examples)/|target/'
```

The most recent full workspace measurement (v1.0.95) reported:

**Workspace line coverage: 68.9%** (157,695 instrumented lines, 48,966 missed
 - i.e. 108,729 lines executed by the test suite). Function coverage is 67.8%
 (15,582 functions, 5,014 never called).

| Crate | Cover | Lines | Missed |
|---|---:|---:|---:|
| `ragent-specs` | 92.6% | 5,652 | 420 |
| `ragent-research` | 89.8% | 27,527 | 2,796 |
| `ragent-types` | 85.7% | 1,729 | 247 |
| `ragent-telemetry` | 85.2% | 2,870 | 426 |
| `ragent-codeindex` | 83.2% | 8,519 | 1,431 |
| `ragent-storage` | 77.5% | 3,723 | 838 |
| `ragent-tools-vcs` | 76.4% | 3,633 | 858 |
| `ragent-bench` | 76.2% | 4,410 | 1,051 |
| `ragent-tools-core` | 72.9% | 5,113 | 1,387 |
| `ragent-config` | 71.8% | 2,247 | 633 |
| `ragent-llm` | 70.1% | 8,399 | 2,510 |
| `ragent-tools-extended` | 65.6% | 16,087 | 5,535 |
| `ragent-agent` | 63.2% | 27,293 | 10,031 |
| `ragent-server` | 55.7% | 2,031 | 899 |
| `ragent-tui` | 49.3% | 36,884 | 18,718 |
| root bin (`src/`) | 24.9% | 1,575 | 1,183 |

Notes:

- The coverage figures above are the last full workspace measurement (v1.0.95);
  they are not re-measured on every documentation pass because
  `cargo llvm-cov` instruments and runs the entire workspace. The
  `ragent-plugins` crate (added in v1.0.112) and the `ragent-connectors` crate
  (added in v1.0.123) are not part of that run.
- Line counts here are executable lines under LLVM profiling, which is smaller
  than the raw `wc -l` figures in the crate table above (declarations, blank
  and comment-only lines are not instrumented).
- The largest coverage gaps are the TUI event/input paths (`app/slash.rs`,
  `app/event_handler.rs`, `input.rs`) and the agent session processor - the
  parts that require a live LLM or terminal to exercise.
- Root binary sources (`src/cli.rs`, `src/main.rs`, `src/plugins.rs`,
  `src/connectors.rs`, `src/panic_hook.rs`) are only lightly covered (24.9%) because
  `cargo llvm-cov` does not drive the interactive TUI.

---

## Key Architecture Ratios

- Test-to-code ratio: ~1 test per 53 lines (10,507 tests / 560,727 lines)
- Largest crate: `ragent-tui` (115,666 lines, 20.9%)
- Smallest crate: `ragent-surface` (560 lines, 0.1%)
- Median crate size: 17,912 lines (`ragent-tools-core` / `ragent-connectors` midpoint)
- Crates over 10k lines: 15 of 18
- Crates under 5k lines: 1 of 18

---

_Generated 2026-10-10 (v1.0.130 tree - the v1.0.130 set adds the OpenHands parity
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
