# Project Statistics

**Version:** 1.0.113

**Update prompt:** Update @STATS.md to show the composition of the project, show breakdown by crate


## Project-wide Metrics

| Metric | Value |
|---|---|
| Total Rust lines | 486657 (482947 in `crates/` + 3710 in root `src/`/`examples/`) |
| Total Rust files | 1210 (workspace crates) + 6 (root `src/`/`examples/`) |
| Tests defined | ~9273 (7562 external test-file tests + 1677 inline `#[cfg(test)]` + 34 root `tests/`/`src/`) |
| Test files | 596 external + ~120 inline-bearing |
| Test binaries | ~630 (596 integration test files + 33 lib/bin targets + 1 root bin) |
| Benchmark files | 17 (+1 in `vendor/html2text`) |
| Tools registered | 168 |
| Supported languages (code index) | 15+ (Rust, Python, TypeScript/JavaScript, Go, C/C++, Java, OpenSCAD, Terraform, CMake, Gradle, Maven) |
| Workspace crates | 17 |
| Specs on disk | 51 directories in `specs/` |
| Documentation | 27 per-category tool how-tos in `docs/howtos/tools/` (+ generated PDFs), 20 category how-tos, 77 slash-command docs |
| Authors | 1 |
| Version | 1.0.113 |

---

## Breakdown by Crate

The project is organised as a Cargo workspace of 17 focused crates. The table below
shows the file count, line count, and test-file count for each crate (including
`src/`, `tests/`, `benches/`, and `examples/` directories where present).

| Crate | Rust files | Rust lines | Test files |
|---|---|---|---|
| `ragent-agent` | 226 | 79523 | 90 |
| `ragent-bench` | 24 | 8436 | 3 |
| `ragent-codeindex` | 69 | 23584 | 40 |
| `ragent-config` | 44 | 11003 | 28 |
| `ragent-llm` | 52 | 23550 | 23 |
| `ragent-plugins` | 36 | 11689 | 17 |
| `ragent-research` | 114 | 60103 | 48 |
| `ragent-server` | 11 | 5854 | 5 |
| `ragent-specs` | 30 | 19421 | 17 |
| `ragent-storage` | 37 | 14746 | 32 |
| `ragent-team` | 15 | 2770 | 14 |
| `ragent-telemetry` | 25 | 10265 | 16 |
| `ragent-tools-core` | 56 | 17760 | 20 |
| `ragent-tools-extended` | 203 | 74350 | 85 |
| `ragent-tools-vcs` | 56 | 14627 | 20 |
| `ragent-tui` | 176 | 96590 | 120 |
| `ragent-types` | 36 | 8676 | 18 |

---

## Crate Size Distribution

```
ragent-tui             ██████████████████████████████ 96,590 lines (19.8%)
ragent-agent           ██████████████████████████ 79,523 lines (16.3%)
ragent-tools-extended  ███████████████████████ 74,350 lines (15.3%)
ragent-research        ████████████████████ 60,103 lines (12.3%)
ragent-codeindex       ████████ 23,584 lines ( 4.8%)
ragent-llm             ████████ 23,550 lines ( 4.8%)
ragent-specs           ██████ 19,421 lines ( 4.0%)
ragent-tools-core      ██████ 17,760 lines ( 3.6%)
ragent-storage         █████ 14,746 lines ( 3.0%)
ragent-tools-vcs       █████ 14,627 lines ( 3.0%)
ragent-plugins         ████ 11,689 lines ( 2.4%)
ragent-config          ███ 11,003 lines ( 2.3%)
ragent-telemetry       ███ 10,265 lines ( 2.1%)
ragent-types           ███ 8,676 lines ( 1.8%)
ragent-bench           ███ 8,436 lines ( 1.7%)
ragent-server          ██ 5,854 lines ( 1.2%)
ragent-team            █ 2,770 lines ( 0.6%)
```

---

## Test Distribution

| Crate | Test Files | Approx. Tests |
|-------|-----------:|--------------:|
| `ragent-tools-extended` | 85 | ~1,892 |
| `ragent-tui` | 120 | ~1,406 |
| `ragent-agent` | 90 | ~779 |
| `ragent-specs` | 17 | ~514 |
| `ragent-research` | 48 | ~402 |
| `ragent-codeindex` | 40 | ~395 |
| `ragent-tools-vcs` | 20 | ~298 |
| `ragent-storage` | 32 | ~285 |
| `ragent-llm` | 23 | ~273 |
| `ragent-tools-core` | 20 | ~251 |
| `ragent-config` | 28 | ~238 |
| `ragent-plugins` | 17 | ~234 |
| `ragent-types` | 18 | ~191 |
| `ragent-telemetry` | 16 | ~187 |
| `ragent-server` | 5 | ~96 |
| `ragent-team` | 14 | ~81 |
| `ragent-bench` | 3 | ~50 |
| **Total (external)** | **596** | **~7,562** |

Inline `#[cfg(test)]` modules in library sources contribute a further
1,677 test attributes (largest contributors: `ragent-research`, `ragent-agent`,
`ragent-tools-extended`, `ragent-tui`, `ragent-specs`), bringing the estimated
total to ~9,273.

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

- Test-to-code ratio: ~1 test per 52 lines (9,273 tests / 486,657 lines)
- Largest crate: `ragent-tui` (96,590 lines, 19.8%)
- Smallest crate: `ragent-team` (2,770 lines, 0.6%)
- Median crate size: 14,746 lines (`ragent-storage`)
- Crates over 10k lines: 13 of 17
- Crates under 5k lines: 1 of 17 (team)

---

_Generated 2026-09-21 (v1.0.113: TUI message input queue, four-row ALT-Q queue-control menu with `Show` queue-entry panel, `/queue` slash command, and `input_queue_capacity` config; a `/simplify all` quality pass over the diff and the HEAD~3 plugins/research/config work with no behaviour change; and a full-workspace clippy hygiene pass that removes the non-existent `clippy::assert_is_empty` allow from 244 files and makes `cargo clippy --all-targets -D warnings` clean)._
