# Project Statistics

**Version:** 1.0.104

**Update prompt:** Update @STATS.md to show the composition of the project, show breakdown by crate


## Project-wide Metrics

| Metric | Value |
|---|---|
| Total Rust lines | 451153 (448324 in `crates/` + 2829 in root `src/`/`examples/`) |
| Total Rust files | 1106 (workspace crates) + 5 (root `src/`/`examples/`) |
| Tests defined | ~8421 (6771 external test-file tests + 1650 inline `#[cfg(test)]`) |
| Test files | 525 external + 131 inline-bearing |
| Test binaries | ~556 (524 integration test files + 32 lib/bin targets + 1 root bin) |
| Benchmark files | 17 (+1 in `vendor/html2text`) |
| Tools registered | 168 |
| Supported languages (code index) | 15+ (Rust, Python, TypeScript/JavaScript, Go, C/C++, Java, OpenSCAD, Terraform, CMake, Gradle, Maven) |
| Workspace crates | 16 |
| Authors | 1 |
| Version | 1.0.104 |

---

## Breakdown by Crate

The project is organised as a Cargo workspace of 16 focused crates. The table below
shows the file count, line count, and test-file count for each crate (including
`src/`, `tests/`, `benches/`, and `examples/` directories where present).

| Crate | Rust files | Rust lines | Test files |
|---|---|---|---|
| `ragent-agent` | 225 | 79023 | 89 |
| `ragent-bench` | 24 | 8436 | 3 |
| `ragent-codeindex` | 69 | 23358 | 40 |
| `ragent-config` | 40 | 10175 | 26 |
| `ragent-llm` | 51 | 23317 | 22 |
| `ragent-research` | 105 | 56474 | 40 |
| `ragent-server` | 11 | 5569 | 5 |
| `ragent-specs` | 26 | 17699 | 13 |
| `ragent-storage` | 37 | 14560 | 32 |
| `ragent-team` | 15 | 2770 | 14 |
| `ragent-telemetry` | 25 | 10265 | 16 |
| `ragent-tools-core` | 54 | 17244 | 18 |
| `ragent-tools-extended` | 184 | 68300 | 74 |
| `ragent-tools-vcs` | 51 | 13786 | 16 |
| `ragent-tui` | 153 | 88770 | 98 |
| `ragent-types` | 36 | 8578 | 18 |

---

## Crate Size Distribution

```
ragent-tui             ██████████████████████████████ 88,770 lines (19.8%)
ragent-agent           ███████████████████████████ 79,023 lines (17.6%)
ragent-tools-extended  ███████████████████████ 68,300 lines (15.2%)
ragent-research        ███████████████████ 56,474 lines (12.6%)
ragent-codeindex       ████████ 23,358 lines ( 5.2%)
ragent-llm             ████████ 23,317 lines ( 5.2%)
ragent-specs           ██████ 17,699 lines ( 3.9%)
ragent-tools-core      ██████ 17,244 lines ( 3.8%)
ragent-storage         █████ 14,560 lines ( 3.2%)
ragent-tools-vcs       █████ 13,786 lines ( 3.1%)
ragent-telemetry       ████ 10,265 lines ( 2.3%)
ragent-config          ███ 10,175 lines ( 2.3%)
ragent-types           ███ 8,578 lines ( 1.9%)
ragent-bench           ███ 8,436 lines ( 1.9%)
ragent-server          ██ 5,569 lines ( 1.2%)
ragent-team            █ 2,770 lines ( 0.6%)
```

---

## Test Distribution

| Crate | Test Files | Approx. Tests |
|-------|-----------:|--------------:|
| `ragent-tools-extended` | 74 | ~1,761 |
| `ragent-tui` | 98 | ~1,187 |
| `ragent-agent` | 89 | ~777 |
| `ragent-specs` | 13 | ~458 |
| `ragent-codeindex` | 40 | ~393 |
| `ragent-research` | 40 | ~326 |
| `ragent-storage` | 32 | ~285 |
| `ragent-tools-vcs` | 16 | ~274 |
| `ragent-llm` | 22 | ~267 |
| `ragent-tools-core` | 18 | ~235 |
| `ragent-config` | 26 | ~209 |
| `ragent-types` | 18 | ~189 |
| `ragent-telemetry` | 16 | ~187 |
| `ragent-server` | 5 | ~92 |
| `ragent-team` | 14 | ~81 |
| `ragent-bench` | 3 | ~50 |
| **Total (external)** | **525** | **~6,771** |

Inline `#[cfg(test)]` modules in library sources contribute a further
1,650 test attributes (largest contributors: `ragent-research` 674,
`ragent-agent` 189, `ragent-tui` 139, `ragent-specs` 138, `ragent-tools-extended` 135),
bringing the estimated total to ~8,421.

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
  `cargo llvm-cov` instruments and runs the entire workspace.
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

- Test-to-code ratio: ~1 test per 54 lines (8,421 tests / 451,153 lines)
- Largest crate: `ragent-tui` (88,770 lines, 19.8%)
- Smallest crate: `ragent-team` (2,770 lines, 0.6%)
- Median crate size: 15,902 lines (between `ragent-storage` and `ragent-tools-core`)
- Crates over 10k lines: 12 of 16
- Crates under 5k lines: 1 of 16 (team)

---

_Generated 2026-09-14 (v1.0.104)._
