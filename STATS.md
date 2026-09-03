# Project Statistics

**Version:** 1.0.77

**Update prompt:** Update @STATS.md to show the composition of the project, show breakdown by crate


## Project-wide Metrics

| Metric | Value |
|---|---|
| Total Rust lines | 401,269 |
| Total Rust files | 986 |
| Tests defined | ~7,690 |
| Test files | 423 |
| Test binaries | ~441 (423 integration test files + 18 lib/bin targets) |
| Benchmark files | 13 |
| Tools registered | ~169 |
| Supported languages (code index) | 15+ (Rust, Python, TypeScript/JavaScript, Go, C/C++, Java, OpenSCAD, Terraform, CMake, Gradle, Maven) |
| Workspace crates | 17 |
| Authors | 1 |
| Version | 1.0.77 |

---

## Breakdown by Crate

The project is organised as a Cargo workspace of 17 focused crates. The table below
shows the file count, line count, and test-file count for each crate (including
`src/`, `tests/`, `benches/`, and `examples/` directories where present).

| Crate | Rust Files | Lines | Test Files | Description |
|-------|-----------:|------:|-----------:|-------------|
| `ragent-tools-extended` | 148 | 58,123 | 56 | Extended document/web/memory/codeindex tools |
| `ragent-tui` | 124 | 74,669 | 76 | Ratatui terminal interface |
| `ragent-agent` | 198 | 64,559 | 62 | Agent/runtime layer: sessions, orchestration, MCP, memory, tool registry |
| `ragent-research` | 95 | 52,832 | 33 | Research system: web/local gathering, synthesis, RESEARCH.md output |
| `ragent-codeindex` | 65 | 21,957 | 36 | Codebase indexing: tree-sitter parsing, SQLite store, Tantivy FTS, file watcher, semantic graph |
| `ragent-llm` | 49 | 22,714 | 15 | Provider clients and model/provider registry |
| `ragent-specs` | 26 | 17,464 | 12 | Spec lifecycle management: discovery, validation, status transitions, review, archival |
| `ragent-tools-core` | 51 | 16,554 | 14 | Core shell/file/search tools |
| `ragent-storage` | 35 | 14,268 | 30 | SQLite-backed storage, snapshots, encrypted credentials |
| `ragent-tools-vcs` | 47 | 13,153 | 13 | GitHub and GitLab tool surface |
| `ragent-telemetry` | 25 | 10,217 | 16 | OpenTelemetry instrumentation and OTLP export |
| `ragent-config` | 36 | 9,353 | 23 | Configuration types, defaults, and parsing |
| `ragent-bench` | 24 | 8,433 | 3 | Benchmark runner shared between TUI and CLI |
| `ragent-types` | 34 | 8,347 | 15 | Shared IDs, events, messages, and sanitization primitives |
| `ragent-server` | 11 | 5,169 | 5 | Axum HTTP routes and SSE streaming |
| `ragent-team` | 15 | 2,770 | 13 | Team coordination runtime and team tools |
| `ragent-prompt_opt` | 3 | 687 | 1 | Prompt optimization templates and completer abstraction |
| `ragent` (root) | 3 | 1,823 | 2 | Binary entry point and CLI wiring |
| **Total** | **986** | **401,269** | **423** | |

---

## Crate Size Distribution

```
ragent-tools-extended  ██████████████████████████████  58,123 lines  (14.5%)
ragent-tui             █████████████████████████████   74,669 lines  (18.6%)
ragent-agent           ███████████████████████████   64,559 lines  (16.1%)
ragent-research        ████████████████████            52,832 lines  (13.2%)
ragent-codeindex       ██████████                      21,957 lines  ( 5.5%)
ragent-llm             ██████████                      22,714 lines  ( 5.7%)
ragent-specs           ████████                        17,464 lines  ( 4.4%)
ragent-tools-core      ███████                         16,554 lines  ( 4.1%)
ragent-storage         ██████                          14,268 lines  ( 3.6%)
ragent-tools-vcs       ██████                          13,153 lines  ( 3.3%)
ragent-telemetry       █████                           10,217 lines  ( 2.5%)
ragent-config          ████                             9,353 lines  ( 2.3%)
ragent-bench           ████                             8,433 lines  ( 2.1%)
ragent-types           ████                             8,347 lines  ( 2.1%)
ragent-server          ██                               5,169 lines  ( 1.3%)
ragent-team            █                                2,770 lines  ( 0.7%)
ragent-prompt_opt      ▏                                  687 lines  ( 0.2%)
```

---

## Test Distribution

| Crate | Test Files | Approx. Tests |
|-------|-----------:|--------------:|
| `ragent-tools-extended` | 56 | ~1,745 |
| `ragent-tui` | 76 | ~1,080 |
| `ragent-agent` | 62 | ~817 |
| `ragent-research` | 33 | ~914 |
| `ragent-specs` | 12 | ~596 |
| `ragent-codeindex` | 36 | ~372 |
| `ragent-tools-vcs` | 13 | ~354 |
| `ragent-llm` | 15 | ~360 |
| `ragent-storage` | 30 | ~278 |
| `ragent-telemetry` | 16 | ~257 |
| `ragent-types` | 15 | ~249 |
| `ragent-tools-core` | 14 | ~236 |
| `ragent-config` | 23 | ~191 |
| `ragent-server` | 5 | ~89 |
| `ragent-team` | 13 | ~81 |
| `ragent-bench` | 3 | ~63 |
| `ragent-prompt_opt` | 1 | ~8 |
| **Total** | **423** | **~7,690** |

---

## Key Architecture Ratios

- Test-to-code ratio: ~1 test per 51 lines (7,374 tests / 379,961 lines)
- Largest crate: `ragent-tui` (66,201 lines, 17.4%)
- Smallest crate: `ragent-prompt_opt` (685 lines, 0.2%)
- Median crate size: 14,237 lines (ragent-storage)
- Crates over 10k lines: 11 of 17
- Crates under 5k lines: 2 of 17 (team, prompt_opt); server (5,033) sits just above
- Top test-to-code ratios (lines per test): ragent-specs ~29, ragent-tools-extended ~33, ragent-storage ~51, ragent-research ~55, ragent-agent ~79