# Project Statistics

**Version:** 1.0.118

**Update prompt:** Update @STATS.md to show the composition of the project, show breakdown by crate

> Metrics below are measured against the v1.0.118 tree.


## Project-wide Metrics

| Metric | Value |
|---|---|
| Total Rust lines | 508,364 (504,596 in `crates/` + 3,768 in root `src/`/`examples/`) |
| Total Rust files | 1,255 (workspace crates) + 6 (root `src/`/`examples/`) |
| Tests defined | ~9,689 (`#[test]` / `#[tokio::test]` attributes across `crates/`, `src/`, and root `tests/`) |
| Test files | 630 external + ~200 inline-bearing |
| Test binaries | ~664 (630 integration test files + 33 lib/bin targets + 1 root bin) |
| Benchmark files | 17 (+1 in `vendor/html2text`) |
| Tools registered | 169 |
| Supported languages (code index) | 15+ (Rust, Python, TypeScript/JavaScript, Go, C/C++, Java, OpenSCAD, Terraform, CMake, Gradle, Maven) |
| Workspace crates | 17 |
| Specs on disk | 54 directories in `specs/` |
| Documentation | 27 per-category tool how-tos in `docs/howtos/tools/` (+ generated PDFs), 20 category how-tos, 78 slash-command docs |
| Authors | 1 |
| Version | 1.0.118 |

---

## Breakdown by Crate

The project is organised as a Cargo workspace of 17 focused crates. The table below
shows the file count, line count, and test-file count for each crate (including
`src/`, `tests/`, `benches/`, and `examples/` directories where present).

| Crate | Rust files | Rust lines | Test files |
|---|---|---|---|
| `ragent-agent` | 234 | 82,238 | 94 |
| `ragent-bench` | 24 | 8,436 | 3 |
| `ragent-codeindex` | 69 | 23,548 | 40 |
| `ragent-config` | 47 | 11,639 | 30 |
| `ragent-llm` | 52 | 23,540 | 23 |
| `ragent-plugins` | 52 | 19,249 | 28 |
| `ragent-research` | 114 | 60,102 | 48 |
| `ragent-server` | 11 | 5,871 | 5 |
| `ragent-specs` | 31 | 20,000 | 18 |
| `ragent-storage` | 37 | 14,717 | 32 |
| `ragent-team` | 15 | 2,770 | 14 |
| `ragent-telemetry` | 25 | 10,265 | 16 |
| `ragent-tools-core` | 56 | 17,747 | 20 |
| `ragent-tools-extended` | 203 | 74,682 | 85 |
| `ragent-tools-vcs` | 56 | 14,819 | 20 |
| `ragent-tui` | 193 | 106,282 | 136 |
| `ragent-types` | 36 | 8,691 | 18 |

---

## Crate Size Distribution

```
ragent-tui             ############################## 106,282 lines (21.1%)
ragent-agent           ####################### 82,238 lines (16.3%)
ragent-tools-extended  ##################### 74,682 lines (14.8%)
ragent-research        ################# 60,102 lines (11.9%)
ragent-codeindex       ####### 23,548 lines ( 4.7%)
ragent-llm             ####### 23,540 lines ( 4.7%)
ragent-specs           ###### 20,000 lines ( 4.0%)
ragent-plugins         ##### 19,249 lines ( 3.8%)
ragent-tools-core      ##### 17,747 lines ( 3.5%)
ragent-tools-vcs       #### 14,819 lines ( 2.9%)
ragent-storage         #### 14,717 lines ( 2.9%)
ragent-config          ### 11,639 lines ( 2.3%)
ragent-telemetry       ### 10,265 lines ( 2.0%)
ragent-types           ## 8,691 lines ( 1.7%)
ragent-bench           ## 8,436 lines ( 1.7%)
ragent-server          ## 5,871 lines ( 1.2%)
ragent-team            # 2,770 lines ( 0.5%)
```

---

## Test Distribution

| Crate | Test Files | Approx. Tests |
|-------|-----------:|--------------:|
| `ragent-tools-extended` | 85 | ~1,894 |
| `ragent-tui` | 136 | ~1,557 |
| `ragent-agent` | 94 | ~814 |
| `ragent-specs` | 18 | ~538 |
| `ragent-research` | 48 | ~402 |
| `ragent-codeindex` | 40 | ~395 |
| `ragent-plugins` | 28 | ~397 |
| `ragent-tools-vcs` | 20 | ~304 |
| `ragent-storage` | 32 | ~285 |
| `ragent-llm` | 23 | ~273 |
| `ragent-config` | 30 | ~258 |
| `ragent-tools-core` | 20 | ~251 |
| `ragent-types` | 18 | ~191 |
| `ragent-telemetry` | 16 | ~187 |
| `ragent-server` | 5 | ~96 |
| `ragent-team` | 14 | ~81 |
| `ragent-bench` | 3 | ~50 |
| **Total (external)** | **630** | **~7,973** |

Inline `#[cfg(test)]` modules in library sources contribute a further
~1,600 test attributes (largest contributors: `ragent-research`, `ragent-agent`,
`ragent-tools-extended`, `ragent-tui`, `ragent-specs`), bringing the estimated
total to ~9,689.

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
| `ragent-team` | 0.0% | 3 | 3 |
| root bin (`src/`) | 24.9% | 1,575 | 1,183 |

Notes:

- The coverage figures above are the last full workspace measurement (v1.0.95);
  they are not re-measured on every documentation pass because
  `cargo llvm-cov` instruments and runs the entire workspace. The
  `ragent-plugins` crate (added in v1.0.112) is not part of that run.
- Line counts here are executable lines under LLVM profiling, which is smaller
  than the raw `wc -l` figures in the crate table above (declarations, blank
  and comment-only lines are not instrumented).
- The largest coverage gaps are the TUI event/input paths (`app/slash.rs`,
  `app/event_handler.rs`, `input.rs`) and the agent session processor - the
  parts that require a live LLM or terminal to exercise.
- `ragent-team`'s library surface is thin glue over `ragent-types`; its logic
  is tested through the team integration tests, but the 3 instrumented lines
  never execute.
- Root binary sources (`src/cli.rs`, `src/main.rs`, `src/plugins.rs`,
  `src/panic_hook.rs`) are only lightly covered (24.9%) because
  `cargo llvm-cov` does not drive the interactive TUI.

---

## Key Architecture Ratios

- Test-to-code ratio: ~1 test per 52 lines (9,689 tests / 508,364 lines)
- Largest crate: `ragent-tui` (106,282 lines, 21.1%)
- Smallest crate: `ragent-team` (2,770 lines, 0.5%)
- Median crate size: 17,747 lines (`ragent-tools-core`)
- Crates over 10k lines: 13 of 17
- Crates under 5k lines: 1 of 17 (team)

---

_Generated 2026-09-25 (v1.0.118 tree: the uncommitted MCP-enablement work folded
in at release — `/mcp` lists plugin-contributed servers and reports their live
status, `/mcp connect <id>` / `/mcp disconnect <id>` enable/disable a server live
with the choice persisted in a global `mcp_state.json` ledger, `/plugins list`
shows each plugin's MCP server and tool counts, `/tools` lists visibility-disabled
tools, and plugin `mcpServers` entries are bridged by default. Metrics
re-measured across the workspace and docs re-rendered in the same pass.)_
