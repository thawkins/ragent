# Project Statistics

**Version:** 1.0.74

**Update prompt:** Update @STATS.md to show the composition of the project, show breakdown by crate


## Project-wide Metrics

| Metric | Value |
|---|---|
| Total Rust lines | 786,036 |
| Total Rust files | 975 |
| Tests defined | 7,800 |
| Test files | 435 |
| Test binaries | ~441 (420 integration test files + 21 lib/bin targets) |
| Benchmark files | 13 |
| Tools registered | ~169 |
| Supported languages (code index) | 15+ (Rust, Python, TypeScript/JavaScript, Go, C/C++, Java, OpenSCAD, Terraform, CMake, Gradle, Maven) |
| Workspace crates | 17 |
| Authors | 1 |

---

## Breakdown by Crate

The project is organised as a Cargo workspace of 17 focused crates. The table below
shows the file count, line count, and test-file count for each crate (including
`src/`, `tests/`, `benches/`, and `examples/` directories where present).

| Crate | Rust Files | Lines | Test Files | Description |
|-------|-----------:|------:|-----------:|-------------|
| `ragent-tui` | 108 | 68,201 | 75 | Ratatui terminal interface |
| `ragent-agent` | 199 | 64,868 | 67 | Agent/runtime layer: sessions, orchestration, MCP, memory, tool registry |
| `ragent-tools-extended` | 151 | 60,046 | 57 | Extended document/web/memory/codeindex tools |
| `ragent-research` | 78 | 45,230 | 29 | Research system: web/local gathering, synthesis, RESEARCH.md output |
| `ragent-codeindex` | 68 | 23,858 | 36 | Codebase indexing: tree-sitter parsing, SQLite store, Tantivy FTS, file watcher, semantic graph |
| `ragent-llm` | 49 | 22,758 | 20 | Provider clients and model/provider registry |
| `ragent-specs` | 26 | 17,428 | 13 | Spec lifecycle management: discovery, validation, status transitions, review, archival |
| `ragent-tools-core` | 51 | 16,275 | 16 | Core shell/file/search tools |
| `ragent-storage` | 35 | 14,237 | 30 | SQLite-backed storage, snapshots, encrypted credentials |
| `ragent-tools-vcs` | 47 | 13,149 | 13 | GitHub and GitLab tool surface |
| `ragent-telemetry` | 25 | 10,217 | 16 | OpenTelemetry instrumentation and OTLP export |
| `ragent-config` | 35 | 8,753 | 22 | Configuration types, defaults, and parsing |
| `ragent-bench` | 23 | 8,390 | 2 | Benchmark runner shared between TUI and CLI |
| `ragent-types` | 33 | 8,248 | 15 | Shared IDs, events, messages, and sanitization primitives |
| `ragent-server` | 11 | 5,033 | 5 | Axum HTTP routes and SSE streaming |
| `ragent-team` | 15 | 2,762 | 14 | Team coordination runtime and team tools |
| `ragent-prompt_opt` | 3 | 685 | 2 | Prompt optimization templates and completer abstraction |
| `ragent` (root) | 3 | 1,823 | 2 | Binary entry point and CLI wiring |
| **Total** | **975** | **786,036** | **435** | |

---

## Crate Size Distribution

```
ragent-tui             ██████████████████████████████  68,201 lines  ( 8.7%)
ragent-agent           ███████████████████████████   64,868 lines  ( 8.3%)
ragent-tools-extended  █████████████████████████     60,046 lines  ( 7.6%)
ragent-research        ███████████████████             45,230 lines  ( 5.8%)
ragent-codeindex       ███████████                     23,858 lines  ( 3.0%)
ragent-llm             ███████████                     22,758 lines  ( 2.9%)
ragent-specs           ████████                        17,428 lines  ( 2.2%)
ragent-tools-core      ███████                         16,275 lines  ( 2.1%)
ragent-storage         ██████                          14,237 lines  ( 1.8%)
ragent-tools-vcs       ██████                          13,149 lines  ( 1.7%)
ragent-telemetry       █████                           10,217 lines  ( 1.3%)
ragent-config          ████                             8,753 lines  ( 1.1%)
ragent-bench           ████                             8,390 lines  ( 1.1%)
ragent-types           ████                             8,248 lines  ( 1.0%)
ragent-server          ██                               5,033 lines  ( 0.6%)
ragent-team            █                                2,762 lines  ( 0.4%)
ragent-prompt_opt      ▏                                  685 lines  ( 0.1%)
```

---

## Test Distribution

| Crate | Test Files | Approx. Tests |
|-------|-----------:|--------------:|
| `ragent-tools-extended` | 57 | ~1,745 |
| `ragent-tui` | 75 | ~1,044 |
| `ragent-agent` | 67 | ~801 |
| `ragent-research` | 29 | ~787 |
| `ragent-specs` | 13 | ~596 |
| `ragent-codeindex` | 36 | ~372 |
| `ragent-tools-vcs` | 13 | ~354 |
| `ragent-llm` | 20 | ~335 |
| `ragent-storage` | 30 | ~278 |
| `ragent-telemetry` | 16 | ~257 |
| `ragent-types` | 15 | ~246 |
| `ragent-tools-core` | 16 | ~235 |
| `ragent-config` | 22 | ~186 |
| `ragent-server` | 5 | ~87 |
| `ragent-team` | 14 | ~81 |
| `ragent-bench` | 2 | ~60 |
| `ragent-prompt_opt` | 2 | ~8 |
| **Total** | **435** | **~7,800** |

---

## Key Architecture Ratios

- Test-to-code ratio: ~1 test per 51 lines (7,374 tests / 379,961 lines)
- Largest crate: `ragent-tui` (66,201 lines, 17.4%)
- Smallest crate: `ragent-prompt_opt` (685 lines, 0.2%)
- Median crate size: 14,237 lines (ragent-storage)
- Crates over 10k lines: 11 of 17
- Crates under 5k lines: 2 of 17 (team, prompt_opt); server (5,033) sits just above
- Top test-to-code ratios (lines per test): ragent-specs ~29, ragent-tools-extended ~33, ragent-storage ~51, ragent-research ~55, ragent-agent ~79