# Project Statistics

**Version:** 1.0.121

**Update prompt:** Update @STATS.md to show the composition of the project, show breakdown by crate

> Metrics below are measured against the v1.0.121 tree.


## Project-wide Metrics

| Metric | Value |
|---|---|
| Total Rust lines | 512,787 (509,114 in `crates/` + 4,367 in root `src/`/`tests/`/`examples/`) |
| Total Rust files | 1,261 (workspace crates) + 8 (root `src/`/`tests/`/`examples/`) |
| Tests defined | ~9,735 (`#[test]` / `#[tokio::test]` attributes across `crates/`, `src/`, and root `tests/`) |
| Test files | 637 external + ~200 inline-bearing |
| Test binaries | ~671 (637 integration test files + 33 lib/bin targets + 1 root bin) |
| Benchmark files | 17 (+1 in `vendor/html2text`) |
| Tools registered | 171 |
| Supported languages (code index) | 15+ (Rust, Python, TypeScript/JavaScript, Go, C/C++, Java, OpenSCAD, Terraform, CMake, Gradle, Maven) |
| Workspace crates | 17 |
| Specs on disk | 54 directories in `specs/` |
| Documentation | 27 per-category tool how-tos in `docs/howtos/tools/` (+ generated PDFs), 20 category how-tos, 78 slash-command docs |
| Authors | 1 |
| Version | 1.0.120 |

---

## Breakdown by Crate

The project is organised as a Cargo workspace of 17 focused crates. The table below
shows the file count, line count, and test-file count for each crate (including
`src/`, `tests/`, `benches/`, and `examples/` directories where present).

| Crate | Rust files | Rust lines | Test files |
|---|---|---|---|
| `ragent-agent` | 240 | 85,210 | 98 |
| `ragent-bench` | 24 | 8,436 | 3 |
| `ragent-codeindex` | 69 | 23,548 | 40 |
| `ragent-config` | 47 | 11,651 | 30 |
| `ragent-llm` | 52 | 23,563 | 23 |
| `ragent-plugins` | 54 | 20,029 | 29 |
| `ragent-research` | 114 | 60,102 | 48 |
| `ragent-server` | 11 | 5,871 | 5 |
| `ragent-specs` | 31 | 20,000 | 18 |
| `ragent-storage` | 37 | 14,717 | 32 |
| `ragent-team` | 15 | 2,774 | 14 |
| `ragent-telemetry` | 25 | 10,265 | 16 |
| `ragent-tools-core` | 56 | 17,747 | 20 |
| `ragent-tools-extended` | 203 | 74,682 | 85 |
| `ragent-tools-vcs` | 56 | 14,819 | 20 |
| `ragent-tui` | 195 | 107,009 | 138 |
| `ragent-types` | 36 | 8,691 | 18 |

---

## Crate Size Distribution

```
ragent-tui            ############################# 107,009 lines (20.9%)
ragent-agent          ####################### 85,210 lines (16.6%)
ragent-tools-extended #################### 74,682 lines (14.6%)
ragent-research       ################ 60,102 lines (11.7%)
ragent-codeindex      ###### 23,548 lines (4.6%)
ragent-llm            ###### 23,563 lines (4.6%)
ragent-specs          ##### 20,000 lines (3.9%)
ragent-plugins        ##### 20,029 lines (3.9%)
ragent-tools-core     ##### 17,747 lines (3.5%)
ragent-tools-vcs      #### 14,819 lines (2.9%)
ragent-storage        #### 14,717 lines (2.9%)
ragent-config         ### 11,651 lines (2.3%)
ragent-telemetry      ### 10,265 lines (2.0%)
ragent-types          ## 8,691 lines (1.7%)
ragent-bench          ## 8,436 lines (1.6%)
ragent-server         ## 5,871 lines (1.1%)
ragent-team           # 2,774 lines (0.5%)
```

---

## Test Distribution

| Crate | Test Files | Approx. Tests |
|-------|-----------:|--------------:|
| `ragent-tools-extended` | 85 | ~2,034 |
| `ragent-tui` | 138 | ~1,704 |
| `ragent-agent` | 98 | ~1,036 |
| `ragent-specs` | 18 | ~677 |
| `ragent-research` | 48 | ~1,085 |
| `ragent-codeindex` | 40 | ~396 |
| `ragent-plugins` | 29 | ~410 |
| `ragent-tools-vcs` | 20 | ~390 |
| `ragent-storage` | 32 | ~285 |
| `ragent-llm` | 23 | ~371 |
| `ragent-config` | 30 | ~274 |
| `ragent-tools-core` | 20 | ~274 |
| `ragent-types` | 18 | ~258 |
| `ragent-telemetry` | 16 | ~257 |
| `ragent-server` | 5 | ~104 |
| `ragent-team` | 14 | ~81 |
| `ragent-bench` | 3 | ~63 |
| **Total (external)** | **637** | **~9,699** |

Inline `#[cfg(test)]` modules in library sources contribute a further
~40 test attributes (largest contributors: `ragent-research`, `ragent-agent`,
`ragent-tools-extended`, `ragent-tui`, `ragent-specs`), bringing the estimated
total to ~9,735.

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

- Test-to-code ratio: ~1 test per 53 lines (9,735 tests / 512,787 lines)
- Largest crate: `ragent-tui` (107,009 lines, 20.9%)
- Smallest crate: `ragent-team` (2,774 lines, 0.5%)
- Median crate size: 17,747 lines (`ragent-tools-core`)
- Crates over 10k lines: 13 of 17
- Crates under 5k lines: 1 of 17 (team)

---

_Generated 2026-09-26 (v1.0.121 tree: rollback-capture remove-after-restore race fix
(CI flake `test_rollback_accept_restores_snapshot`), OpenSkills `.agents` discovery,
Claude marketplace `*-lsp` inline-manifest materialisation, conventional `skills/`
directory bridging, plus a `/simplify all` pass over the 50-file changed set. Tool
count is now 171 registered (`tool_info` + `commands_info` added in v1.0.120).)_
