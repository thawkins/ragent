# Project Statistics

**Version:** 1.0.96

**Update prompt:** Update @STATS.md to show the composition of the project, show breakdown by crate


## Project-wide Metrics

| Metric | Value |
|---|---|
| Total Rust lines | 441726 (438896 in `crates/` + 2830 in root `src/`/`examples/`) |
| Total Rust files | 1070 (workspace crates) + 5 (root `src/`/`examples/`) |
| Tests defined | ~8236 (6592 external test-file tests + 1644 inline `#[cfg(test)]`) |
| Test files | 495 external + 120 inline-bearing |
| Test binaries | ~500 (482 integration test files + 16 lib/bin targets + 1 root bin) |
| Benchmark files | 13 (+1 in `vendor/html2text`) |
| Tools registered | 168 |
| Supported languages (code index) | 15+ (Rust, Python, TypeScript/JavaScript, Go, C/C++, Java, OpenSCAD, Terraform, CMake, Gradle, Maven) |
| Workspace crates | 16 |
| Authors | 1 |
| Version | 1.0.96 |

---

## Breakdown by Crate

The project is organised as a Cargo workspace of 16 focused crates. The table below
shows the file count, line count, and test-file count for each crate (including
`src/`, `tests/`, `benches/`, and `examples/` directories where present).

| Crate | Rust files | Rust lines | Test files |
|---|---|---|---|
| `ragent-agent` | 217 | 76962 | 83 |
| `ragent-bench` | 24 | 8436 | 3 |
| `ragent-codeindex` | 68 | 22898 | 39 |
| `ragent-config` | 37 | 9769 | 24 |
| `ragent-llm` | 51 | 23283 | 22 |
| `ragent-research` | 102 | 55397 | 39 |
| `ragent-server` | 11 | 5557 | 5 |
| `ragent-specs` | 26 | 17699 | 13 |
| `ragent-storage` | 35 | 14271 | 30 |
| `ragent-team` | 15 | 2770 | 14 |
| `ragent-telemetry` | 25 | 10265 | 16 |
| `ragent-tools-core` | 54 | 17105 | 18 |
| `ragent-tools-extended` | 178 | 66640 | 69 |
| `ragent-tools-vcs` | 47 | 13157 | 13 |
| `ragent-tui` | 146 | 86283 | 91 |
| `ragent-types` | 34 | 8404 | 16 |

---

## Crate Size Distribution

```
ragent-tui             ██████████████████████████████ 86,283 lines (19.7%)
ragent-agent           ███████████████████████████ 76,962 lines (17.6%)
ragent-tools-extended  ███████████████████████ 66,640 lines (15.2%)
ragent-research        ███████████████████ 55,397 lines (12.6%)
ragent-llm             ████████ 23,283 lines ( 5.3%)
ragent-codeindex       ████████ 22,898 lines ( 5.2%)
ragent-specs           ██████ 17,699 lines ( 4.0%)
ragent-tools-core      ██████ 17,105 lines ( 3.9%)
ragent-storage         █████ 14,271 lines ( 3.2%)
ragent-tools-vcs       █████ 13,157 lines ( 3.0%)
ragent-telemetry       ████ 10,265 lines ( 2.3%)
ragent-config          ███ 9,769 lines ( 2.2%)
ragent-bench           ███ 8,436 lines ( 1.9%)
ragent-types           ███ 8,404 lines ( 1.9%)
ragent-server          ██ 5,557 lines ( 1.3%)
ragent-team            █ 2,770 lines ( 0.6%)
```

---

## Test Distribution

| Crate | Test Files | Approx. Tests |
|-------|-----------:|--------------:|
| `ragent-tui` | 91 | ~1,148 |
| `ragent-agent` | 83 | ~743 |
| `ragent-tools-extended` | 69 | ~1,716 |
| `ragent-codeindex` | 39 | ~387 |
| `ragent-specs` | 13 | ~458 |
| `ragent-research` | 39 | ~315 |
| `ragent-storage` | 30 | ~278 |
| `ragent-tools-vcs` | 13 | ~268 |
| `ragent-telemetry` | 16 | ~187 |
| `ragent-types` | 16 | ~183 |
| `ragent-config` | 24 | ~188 |
| `ragent-tools-core` | 18 | ~231 |
| `ragent-llm` | 22 | ~267 |
| `ragent-server` | 5 | ~92 |
| `ragent-team` | 14 | ~81 |
| `ragent-bench` | 3 | ~50 |
| **Total (external)** | **491** | **~6,592** |

Inline `#[cfg(test)]` modules in library sources contribute a further
1,644 test attributes (largest contributors: `ragent-research` 668,
`ragent-agent` 189, `ragent-tui` 139, `ragent-specs` 138),
bringing the estimated total to ~8,236.

---

## Test Coverage

Measured with `cargo llvm-cov --workspace --summary-only` (line coverage,
test/bench/example sources excluded). Reproduce with:

```bash
cargo llvm-cov --workspace --summary-only --ignore-filename-regex '(^|/)(tests|benches|examples)/|target/'
```

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

- Line counts here are executable lines under LLVM profiling, which is smaller
  than the raw `wc -l` figures in the crate table above (declarations, blank
  and comment-only lines are not instrumented).
- The largest coverage gaps are the TUI event/input paths (`app/slash.rs`,
  `app/event_handler.rs`, `input.rs`) and the agent session processor - the
  parts that require a live LLM or terminal to exercise.
- `ragent-team`'s library surface is thin glue over `ragent-types`; its logic
  is tested through the team integration tests, but the 3 instrumented lines
  never execute.
- Root binary sources (`src/cli.rs`, `src/main.rs`, `src/panic_hook.rs`) are
  only lightly covered (24.9%) because `cargo llvm-cov` does not drive the
  interactive TUI.

---

## Key Architecture Ratios

- Test-to-code ratio: ~1 test per 54 lines (8,236 tests / 441,726 lines)
- Largest crate: `ragent-tui` (86,283 lines, 19.7%)
- Smallest crate: `ragent-team` (2,770 lines, 0.6%)
- Median crate size: 14,271 lines (ragent-storage)
- Crates over 10k lines: 10 of 16
- Crates under 5k lines: 1 of 16 (team); server (5,557) sits just above

---

_Generated 2026-09-11 (v1.0.95 working tree, /docupdate uncommitted pass)._
