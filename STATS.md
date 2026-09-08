# Project Statistics

**Version:** 1.0.85

**Update prompt:** Update @STATS.md to show the composition of the project, show breakdown by crate


## Project-wide Metrics

| Metric | Value |
|---|---|
| Total Rust lines | 423176 (420626 in `crates/` + 2550 in root `src/`/`examples/`) |
| Total Rust files | 1025 (workspace crates) + 5 (root `src/`/`examples/`) |
| Tests defined | ~8385 (469 external test-file tests + 7916 inline `#[cfg(test)]`) |
| Test files | 469 external + 195 inline-bearing |
| Test binaries | ~484 (468 integration test files + 16 lib/bin targets) |
| Benchmark files | 13 (+1 in `vendor/html2text`) |
| Tools registered | 168 |
| Supported languages (code index) | 15+ (Rust, Python, TypeScript/JavaScript, Go, C/C++, Java, OpenSCAD, Terraform, CMake, Gradle, Maven) |
| Workspace crates | 17 |
| Authors | 1 |
| Version | 1.0.85 |

---

## Breakdown by Crate

The project is organised as a Cargo workspace of 17 focused crates. The table below
shows the file count, line count, and test-file count for each crate (including
`src/`, `tests/`, `benches/`, and `examples/` directories where present).

| Crate | Rust files | Rust lines | Test files |
|---|---|---|---|
| `ragent-agent` | 213 | 74037 | 80 |
| `ragent-bench` | 24 | 8436 | 3 |
| `ragent-codeindex` | 68 | 22898 | 39 |
| `ragent-config` | 37 | 9692 | 24 |
| `ragent-llm` | 49 | 22714 | 20 |
| `ragent-prompt_opt` | 3 | 687 | 2 |
| `ragent-research` | 102 | 55352 | 39 |
| `ragent-server` | 11 | 5697 | 5 |
| `ragent-specs` | 26 | 17464 | 13 |
| `ragent-storage` | 35 | 14271 | 30 |
| `ragent-team` | 15 | 2770 | 14 |
| `ragent-telemetry` | 25 | 10265 | 16 |
| `ragent-tools-core` | 51 | 16554 | 16 |
| `ragent-tools-extended` | 153 | 58037 | 57 |
| `ragent-tools-vcs` | 47 | 13153 | 13 |
| `ragent-tui` | 132 | 80195 | 82 |
| `ragent-types` | 34 | 8404 | 16 |

---

## Crate Size Distribution

```
ragent-tui             ██████████████████████████████  79,454 lines  (18.9%)
ragent-agent           ███████████████████████████     73,889 lines  (17.6%)
ragent-tools-extended  ██████████████████████          58,042 lines  (13.8%)
ragent-research        █████████████████████           55,251 lines  (13.2%)
ragent-codeindex       █████████                        22,817 lines  ( 5.4%)
ragent-llm             █████████                        22,714 lines  ( 5.4%)
ragent-specs           ███████                          17,464 lines  ( 4.2%)
ragent-tools-core      ███████                          16,554 lines  ( 3.9%)
ragent-storage         ██████                           14,268 lines  ( 3.4%)
ragent-tools-vcs       █████                            13,153 lines  ( 3.1%)
ragent-telemetry       ████                             10,247 lines  ( 2.4%)
ragent-config          ████                              9,714 lines  ( 2.3%)
ragent-bench           ███                               8,433 lines  ( 2.0%)
ragent-types           ███                               8,404 lines  ( 2.0%)
ragent-server          ██                                5,694 lines  ( 1.4%)
ragent-team            █                                 2,770 lines  ( 0.7%)
ragent-prompt_opt      ▏                                   687 lines  ( 0.2%)
```

---

## Test Distribution

| Crate | Test Files | Approx. Tests |
|-------|-----------:|--------------:|
| `ragent-tui` | 81 | ~1,880 |
| `ragent-agent` | 80 | ~1,100 |
| `ragent-tools-extended` | 57 | ~1,708 |
| `ragent-codeindex` | 39 | ~430 |
| `ragent-specs` | 13 | ~700 |
| `ragent-research` | 39 | ~382 |
| `ragent-storage` | 30 | ~309 |
| `ragent-tools-vcs` | 13 | ~299 |
| `ragent-telemetry` | 16 | ~208 |
| `ragent-types` | 16 | ~204 |
| `ragent-config` | 24 | ~194 |
| `ragent-tools-core` | 16 | ~191 |
| `ragent-llm` | 20 | ~109 |
| `ragent-server` | 5 | ~101 |
| `ragent-team` | 14 | ~89 |
| `ragent-bench` | 3 | ~55 |
| `ragent-prompt_opt` | 2 | ~9 |
| **Total (external)** | **468** | **~5,636** |

Inline `#[cfg(test)]` modules in library sources contribute a further
2,248 test attributes (largest contributors: `ragent-research` 666,
`ragent-specs` 295, `ragent-agent` 293, `ragent-tools-extended` 156),
bringing the estimated total to ~7,884.

---

## Key Architecture Ratios

- Test-to-code ratio: ~1 test per 54 lines (7,884 tests / 422,085 lines)
- Largest crate: `ragent-tui` (79,454 lines, 18.9%)
- Smallest crate: `ragent-prompt_opt` (687 lines, 0.2%)
- Median crate size: 14,268 lines (ragent-storage)
- Crates over 10k lines: 10 of 17
- Crates under 5k lines: 2 of 17 (team, prompt_opt); server (5,694) sits just above