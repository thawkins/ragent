# Project Statistics

**Version:** 1.0.95

**Update prompt:** Update @STATS.md to show the composition of the project, show breakdown by crate


## Project-wide Metrics

| Metric | Value |
|---|---|
| Total Rust lines | 440692 (437865 in `crates/` + 2827 in root `src/`/`examples/`) |
| Total Rust files | 1065 (workspace crates) + 5 (root `src/`/`examples/`) |
| Tests defined | ~8168 (6598 external test-file tests + 1570 inline `#[cfg(test)]`) |
| Test files | 493 external + 131 inline-bearing |
| Test binaries | ~497 (492 integration test files + 16 lib/bin targets) |
| Benchmark files | 13 (+1 in `vendor/html2text`) |
| Tools registered | 168 |
| Supported languages (code index) | 15+ (Rust, Python, TypeScript/JavaScript, Go, C/C++, Java, OpenSCAD, Terraform, CMake, Gradle, Maven) |
| Workspace crates | 17 |
| Authors | 1 |
| Version | 1.0.95 |

---

## Breakdown by Crate

The project is organised as a Cargo workspace of 17 focused crates. The table below
shows the file count, line count, and test-file count for each crate (including
`src/`, `tests/`, `benches/`, and `examples/` directories where present).

| Crate | Rust files | Rust lines | Test files |
|---|---|---|---|
| `ragent-agent` | 216 | 76023 | 82 |
| `ragent-bench` | 24 | 8436 | 3 |
| `ragent-codeindex` | 68 | 22898 | 39 |
| `ragent-config` | 37 | 9769 | 24 |
| `ragent-llm` | 51 | 23284 | 22 |
| `ragent-prompt_opt` | 3 | 687 | 2 |
| `ragent-research` | 101 | 55373 | 39 |
| `ragent-server` | 11 | 5697 | 5 |
| `ragent-specs` | 26 | 17699 | 13 |
| `ragent-storage` | 35 | 14271 | 30 |
| `ragent-team` | 15 | 2770 | 14 |
| `ragent-telemetry` | 25 | 10265 | 16 |
| `ragent-tools-core` | 54 | 17105 | 18 |
| `ragent-tools-extended` | 178 | 66640 | 69 |
| `ragent-tools-vcs` | 47 | 13157 | 13 |
| `ragent-tui` | 140 | 85387 | 87 |
| `ragent-types` | 34 | 8404 | 16 |

---

## Crate Size Distribution

```
ragent-tui             ██████████████████████████████ 85,387 lines (19.5%)
ragent-agent           ███████████████████████████ 76,023 lines (17.4%)
ragent-tools-extended  ███████████████████████ 66,640 lines (15.2%)
ragent-research        ███████████████████ 55,373 lines (12.6%)
ragent-llm             ████████ 23,284 lines ( 5.3%)
ragent-codeindex       ████████ 22,898 lines ( 5.2%)
ragent-specs           ██████ 17,699 lines ( 4.0%)
ragent-tools-core      ██████ 17,105 lines ( 3.9%)
ragent-storage         █████ 14,271 lines ( 3.3%)
ragent-tools-vcs       █████ 13,157 lines ( 3.0%)
ragent-telemetry       ████ 10,265 lines ( 2.3%)
ragent-config          ███ 9,769 lines ( 2.2%)
ragent-bench           ███ 8,436 lines ( 1.9%)
ragent-types           ███ 8,404 lines ( 1.9%)
ragent-server          ██ 5,697 lines ( 1.3%)
ragent-team            █ 2,770 lines ( 0.6%)
ragent-prompt_opt       687 lines ( 0.2%)
```

---

## Test Distribution

| Crate | Test Files | Approx. Tests |
|-------|-----------:|--------------:|
| `ragent-tui` | 87 | ~1,144 |
| `ragent-agent` | 82 | ~736 |
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
| `ragent-prompt_opt` | 2 | ~8 |
| **Total (external)** | **492** | **~6,589** |

Inline `#[cfg(test)]` modules in library sources contribute a further
1,570 test attributes (largest contributors: `ragent-research` 668,
`ragent-agent` 187, `ragent-specs` 136, `ragent-tools-extended` 135),
bringing the estimated total to ~8,168.

---

## Key Architecture Ratios

- Test-to-code ratio: ~1 test per 54 lines (8,168 tests / 440,692 lines)
- Largest crate: `ragent-tui` (85,387 lines, 19.5%)
- Smallest crate: `ragent-prompt_opt` (687 lines, 0.2%)
- Median crate size: 14,271 lines (ragent-storage)
- Crates over 10k lines: 10 of 17
- Crates under 5k lines: 2 of 17 (team, prompt_opt); server (5,697) sits just above

---

_Generated 2026-09-11 (v1.0.95 working tree, /docupdate)._

