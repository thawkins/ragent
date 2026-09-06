# Project Statistics

**Version:** 1.0.80

**Update prompt:** Update @STATS.md to show the composition of the project, show breakdown by crate


## Project-wide Metrics

| Metric | Value |
|---|---|
| Total Rust lines | 408,783 (406,234 in `crates/` + 2,549 in root `src/`/`examples/`) |
| Total Rust files | 1,001 (workspace crates) + 24 (root `src/`/`examples/`) |
| Tests defined | ~7,265 (5,689 in `tests/` + 1,576 inline `#[cfg(test)]`) |
| Test files | 450 (incl. 16 `inline/` helper files) |
| Test binaries | ~466 (450 integration test files + 16 lib/bin targets) |
| Benchmark files | 13 (+1 in `vendor/html2text`) |
| Tools registered | 168 |
| Supported languages (code index) | 15+ (Rust, Python, TypeScript/JavaScript, Go, C/C++, Java, OpenSCAD, Terraform, CMake, Gradle, Maven) |
| Workspace crates | 17 |
| Authors | 1 |
| Version | 1.0.80 |

---

## Breakdown by Crate

The project is organised as a Cargo workspace of 17 focused crates. The table below
shows the file count, line count, and test-file count for each crate (including
`src/`, `tests/`, `benches/`, and `examples/` directories where present).

| Crate | Rust Files | Lines | Test Files | Description |
|-------|-----------:|------:|-----------:|-------------|
| `ragent-tui` | 124 | 75,715 | 76 | Ratatui terminal interface |
| `ragent-agent` | 198 | 64,791 | 68 | Agent/runtime layer: sessions, orchestration, MCP, memory, tool registry |
| `ragent-tools-extended` | 153 | 58,016 | 57 | Extended document/web/memory/codeindex/plot tools |
| `ragent-research` | 102 | 55,251 | 39 | Research system: web/local gathering, synthesis, RESEARCH.md output |
| `ragent-codeindex` | 68 | 22,817 | 39 | Codebase indexing: tree-sitter parsing, SQLite store, Tantivy FTS, file watcher, semantic graph |
| `ragent-llm` | 49 | 22,714 | 20 | Provider clients and model/provider registry |
| `ragent-specs` | 26 | 17,464 | 13 | Spec lifecycle management: discovery, validation, status transitions, review, archival |
| `ragent-tools-core` | 51 | 16,554 | 16 | Core shell/file/search tools |
| `ragent-storage` | 35 | 14,268 | 30 | SQLite-backed storage, snapshots, encrypted credentials |
| `ragent-tools-vcs` | 47 | 13,153 | 13 | GitHub and GitLab tool surface |
| `ragent-telemetry` | 25 | 10,217 | 16 | OpenTelemetry instrumentation and OTLP export |
| `ragent-config` | 36 | 9,415 | 23 | Configuration types, defaults, and parsing |
| `ragent-bench` | 24 | 8,433 | 3 | Benchmark runner shared between TUI and CLI |
| `ragent-types` | 34 | 8,347 | 16 | Shared IDs, events, messages, and sanitization primitives |
| `ragent-server` | 11 | 5,622 | 5 | Axum HTTP routes and SSE streaming |
| `ragent-team` | 15 | 2,770 | 14 | Team coordination runtime and team tools |
| `ragent-prompt_opt` | 3 | 687 | 2 | Prompt optimization templates and completer abstraction |
| **Total** | **1,001** | **406,234** | **450** | |

---

## Crate Size Distribution

```
ragent-tui             ██████████████████████████████  75,715 lines  (18.6%)
ragent-agent           ██████████████████████████      64,791 lines  (15.9%)
ragent-tools-extended  ███████████████████████         58,016 lines  (14.3%)
ragent-research        ██████████████████████          55,251 lines  (13.6%)
ragent-codeindex       █████████                        22,817 lines  ( 5.6%)
ragent-llm             █████████                        22,714 lines  ( 5.6%)
ragent-specs           ███████                          17,464 lines  ( 4.3%)
ragent-tools-core      ███████                          16,554 lines  ( 4.1%)
ragent-storage         ██████                           14,268 lines  ( 3.5%)
ragent-tools-vcs       █████                            13,153 lines  ( 3.2%)
ragent-telemetry       ████                             10,217 lines  ( 2.5%)
ragent-config          ████                              9,415 lines  ( 2.3%)
ragent-bench           ███                               8,433 lines  ( 2.1%)
ragent-types           ███                               8,347 lines  ( 2.1%)
ragent-server          ██                                5,622 lines  ( 1.4%)
ragent-team            █                                 2,770 lines  ( 0.7%)
ragent-prompt_opt      ▏                                   687 lines  ( 0.2%)
```

---

## Test Distribution

| Crate | Test Files | Approx. Tests |
|-------|-----------:|--------------:|
| `ragent-tools-extended` | 57 | ~1,552 |
| `ragent-tui` | 76 | ~1,001 |
| `ragent-agent` | 68 | ~495 |
| `ragent-codeindex` | 39 | ~387 |
| `ragent-specs` | 13 | ~344 |
| `ragent-research` | 39 | ~315 |
| `ragent-storage` | 30 | ~278 |
| `ragent-tools-vcs` | 13 | ~268 |
| `ragent-telemetry` | 16 | ~187 |
| `ragent-types` | 16 | ~183 |
| `ragent-config` | 23 | ~175 |
| `ragent-tools-core` | 16 | ~174 |
| `ragent-llm` | 20 | ~99 |
| `ragent-server` | 5 | ~92 |
| `ragent-team` | 14 | ~81 |
| `ragent-bench` | 3 | ~50 |
| `ragent-prompt_opt` | 2 | ~8 |
| **Total (external)** | **450** | **~5,689** |

Inline `#[cfg(test)]` modules in library sources contribute a further
1,576 test attributes (largest contributors: `ragent-research` 666,
`ragent-agent` 178, `ragent-llm` 98, `ragent-specs` 138), bringing the
estimated total to ~7,265.

---

## Key Architecture Ratios

- Test-to-code ratio: ~1 test per 56 lines (7,265 tests / 408,783 lines)
- Largest crate: `ragent-tui` (75,715 lines, 18.6%)
- Smallest crate: `ragent-prompt_opt` (687 lines, 0.2%)
- Median crate size: 13,153 lines (ragent-tools-vcs)
- Crates over 10k lines: 11 of 17
- Crates under 5k lines: 2 of 17 (team, prompt_opt); server (5,622) sits just above