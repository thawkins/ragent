# Project Statistics

**Version:** 1.0.91

**Update prompt:** Update @STATS.md to show the composition of the project, show breakdown by crate


## Project-wide Metrics

| Metric | Value |
|---|---|
| Total Rust lines | 436219 (433392 in `crates/` + 2827 in root `src/`/`examples/`) |
| Total Rust files | 1059 (workspace crates) + 5 (root `src/`/`examples/`) |
| Tests defined | ~8162 (6554 external test-file tests + 1608 inline `#[cfg(test)]`) |
| Test files | 490 external + 132 inline-bearing |
| Test binaries | ~495 (490 integration test files + 16 lib/bin targets) |
| Benchmark files | 13 (+1 in `vendor/html2text`) |
| Tools registered | 168 |
| Supported languages (code index) | 15+ (Rust, Python, TypeScript/JavaScript, Go, C/C++, Java, OpenSCAD, Terraform, CMake, Gradle, Maven) |
| Workspace crates | 17 |
| Authors | 1 |
| Version | 1.0.91 |

---

## Breakdown by Crate

The project is organised as a Cargo workspace of 17 focused crates. The table below
shows the file count, line count, and test-file count for each crate (including
`src/`, `tests/`, `benches/`, and `examples/` directories where present).

| Crate | Rust files | Rust lines | Test files |
|---|---|---|---|
| `ragent-agent` | 215 | 74905 | 81 |
| `ragent-bench` | 24 | 8436 | 3 |
| `ragent-codeindex` | 68 | 22898 | 39 |
| `ragent-config` | 37 | 9692 | 24 |
| `ragent-llm` | 51 | 23283 | 22 |
| `ragent-prompt_opt` | 3 | 687 | 2 |
| `ragent-research` | 101 | 55371 | 39 |
| `ragent-server` | 11 | 5697 | 5 |
| `ragent-specs` | 26 | 17694 | 13 |
| `ragent-storage` | 35 | 14271 | 30 |
| `ragent-team` | 15 | 2770 | 14 |
| `ragent-telemetry` | 25 | 10265 | 16 |
| `ragent-tools-core` | 54 | 17098 | 18 |
| `ragent-tools-extended` | 178 | 66640 | 69 |
| `ragent-tools-vcs` | 47 | 13157 | 13 |
| `ragent-tui` | 136 | 82080 | 86 |
| `ragent-types` | 34 | 8404 | 16 |

---

## Crate Size Distribution

```
ragent-tui             ██████████████████████████████ 82,080 lines (18.8%)
ragent-agent           ████████████████████████████ 74,905 lines (17.2%)
ragent-tools-extended  ████████████████████████ 66,640 lines (15.3%)
ragent-research        ████████████████████ 55,371 lines (12.7%)
ragent-llm             █████████ 23,283 lines ( 5.3%)
ragent-codeindex       █████████ 22,898 lines ( 5.2%)
ragent-specs           ███████ 17,694 lines ( 4.1%)
ragent-tools-core      ██████ 17,098 lines ( 3.9%)
ragent-storage         █████ 14,271 lines ( 3.3%)
ragent-tools-vcs       █████ 13,157 lines ( 3.0%)
ragent-telemetry       ████ 10,265 lines ( 2.4%)
ragent-config          ████ 9,692 lines ( 2.2%)
ragent-bench           ███ 8,436 lines ( 1.9%)
ragent-types           ███ 8,404 lines ( 1.9%)
ragent-server          ██ 5,697 lines ( 1.3%)
ragent-team            █ 2,770 lines ( 0.6%)
ragent-prompt_opt      █ 687 lines ( 0.2%)
```

---

## Test Distribution

| Crate | Test Files | Approx. Tests |
|-------|-----------:|--------------:|
| `ragent-tui` | 83 | ~1,093 |
| `ragent-agent` | 81 | ~724 |
| `ragent-tools-extended` | 57 | ~1,552 |
| `ragent-codeindex` | 39 | ~387 |
| `ragent-specs` | 13 | ~458 |
| `ragent-research` | 39 | ~315 |
| `ragent-storage` | 30 | ~278 |
| `ragent-tools-vcs` | 13 | ~268 |
| `ragent-telemetry` | 16 | ~187 |
| `ragent-types` | 16 | ~183 |
| `ragent-config` | 24 | ~185 |
| `ragent-tools-core` | 18 | ~231 |
| `ragent-llm` | 22 | ~267 |
| `ragent-server` | 5 | ~92 |
| `ragent-team` | 14 | ~81 |
| `ragent-bench` | 3 | ~50 |
| `ragent-prompt_opt` | 2 | ~8 |
| **Total (external)** | **475** | **~6,359** |

Inline `#[cfg(test)]` modules in library sources contribute a further
1,610 test attributes (largest contributors: `ragent-research` 668,
`ragent-agent` 190, `ragent-specs` 138, `ragent-tools-extended` 135),
bringing the estimated total to ~7,969.

---

## Key Architecture Ratios

- Test-to-code ratio: ~1 test per 53 lines (7,969 tests / 425,637 lines)
- Largest crate: `ragent-tui` (80,463 lines, 19.0%)
- Smallest crate: `ragent-prompt_opt` (687 lines, 0.2%)
- Median crate size: 14,271 lines (ragent-storage)
- Crates over 10k lines: 10 of 17
- Crates under 5k lines: 2 of 17 (team, prompt_opt); server (5,697) sits just above

---

_Generated 2026-09-08 (v1.0.88 working tree, /docupdate)._

