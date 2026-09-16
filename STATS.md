# Project Statistics

**Version:** 1.0.104

**Update prompt:** Update @STATS.md to show the composition of the project, show breakdown by crate


## Project-wide Metrics

| Metric | Value |
|---|---|
| Total Rust lines | 458598 (455570 in `crates/` + 3028 in root `src/`/`examples/`) |
| Total Rust files | 1123 (workspace crates) + 5 (root `src/`/`examples/`) |
| Tests defined | ~8601 (6912 external test-file tests + 1669 inline `#[cfg(test)]` + 20 root `tests/`/`src/`) |
| Test files | 539 external + 131 inline-bearing |
| Test binaries | ~572 (539 integration test files + 32 lib/bin targets + 1 root bin) |
| Benchmark files | 17 (+1 in `vendor/html2text`) |
| Tools registered | 168 |
| Supported languages (code index) | 15+ (Rust, Python, TypeScript/JavaScript, Go, C/C++, Java, OpenSCAD, Terraform, CMake, Gradle, Maven) |
| Workspace crates | 16 |
| Authors | 1 |
| Version | 1.0.105 |

---

## Breakdown by Crate

The project is organised as a Cargo workspace of 16 focused crates. The table below
shows the file count, line count, and test-file count for each crate (including
`src/`, `tests/`, `benches/`, and `examples/` directories where present).

| Crate | Rust files | Rust lines | Test files |
|---|---|---|---|
| `ragent-agent` | 225 | 79434 | 89 |
| `ragent-bench` | 24 | 8436 | 3 |
| `ragent-codeindex` | 69 | 23584 | 40 |
| `ragent-config` | 40 | 10409 | 26 |
| `ragent-llm` | 51 | 23454 | 22 |
| `ragent-research` | 113 | 59583 | 47 |
| `ragent-server` | 11 | 5908 | 5 |
| `ragent-specs` | 26 | 17836 | 13 |
| `ragent-storage` | 37 | 14746 | 32 |
| `ragent-team` | 15 | 2770 | 14 |
| `ragent-telemetry` | 25 | 10265 | 16 |
| `ragent-tools-core` | 56 | 17745 | 20 |
| `ragent-tools-extended` | 186 | 69129 | 76 |
| `ragent-tools-vcs` | 55 | 14573 | 19 |
| `ragent-tui` | 154 | 89059 | 99 |
| `ragent-types` | 36 | 8639 | 18 |

---

## Crate Size Distribution

```
ragent-tui             ██████████████████████████████ 89,059 lines (19.5%)
ragent-agent           ███████████████████████████ 79,434 lines (17.4%)
ragent-tools-extended  ███████████████████████ 69,129 lines (15.2%)
ragent-research        ████████████████████ 59,583 lines (13.1%)
ragent-codeindex       ████████ 23,584 lines ( 5.2%)
ragent-llm             ████████ 23,454 lines ( 5.1%)
ragent-specs           ██████ 17,836 lines ( 3.9%)
ragent-tools-core      ██████ 17,745 lines ( 3.9%)
ragent-storage         █████ 14,746 lines ( 3.2%)
ragent-tools-vcs       █████ 14,573 lines ( 3.2%)
ragent-config          ███ 10,409 lines ( 2.3%)
ragent-telemetry       ███ 10,265 lines ( 2.3%)
ragent-types           ███ 8,639 lines ( 1.9%)
ragent-bench           ███ 8,436 lines ( 1.9%)
ragent-server          ██ 5,908 lines ( 1.3%)
ragent-team            █ 2,770 lines ( 0.6%)
```

---

## Test Distribution

| Crate | Test Files | Approx. Tests |
|-------|-----------:|--------------:|
| `ragent-tools-extended` | 76 | ~1,783 |
| `ragent-tui` | 99 | ~1,189 |
| `ragent-agent` | 89 | ~777 |
| `ragent-specs` | 13 | ~459 |
| `ragent-codeindex` | 40 | ~395 |
| `ragent-research` | 47 | ~387 |
| `ragent-tools-vcs` | 19 | ~297 |
| `ragent-storage` | 32 | ~285 |
| `ragent-llm` | 22 | ~267 |
| `ragent-tools-core` | 20 | ~251 |
| `ragent-config` | 26 | ~220 |
| `ragent-types` | 18 | ~189 |
| `ragent-telemetry` | 16 | ~187 |
| `ragent-server` | 5 | ~96 |
| `ragent-team` | 14 | ~81 |
| `ragent-bench` | 3 | ~49 |
| **Total (external)** | **539** | **~6,912** |

Inline `#[cfg(test)]` modules in library sources contribute a further
1,669 test attributes (largest contributors: `ragent-research` 681,
`ragent-agent` 192, `ragent-tools-extended` 140, `ragent-tui` 139, `ragent-specs` 136),
bringing the estimated total to ~8,601.

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

- Test-to-code ratio: ~1 test per 53 lines (8,601 tests / 458,598 lines)
- Largest crate: `ragent-tui` (89,059 lines, 19.5%)
- Smallest crate: `ragent-team` (2,770 lines, 0.6%)
- Median crate size: 16,146 lines (between `ragent-tools-core` and `ragent-tools-vcs`)
- Crates over 10k lines: 12 of 16
- Crates under 5k lines: 1 of 16 (team)

---

_Generated 2026-09-16 (v1.0.105)._
