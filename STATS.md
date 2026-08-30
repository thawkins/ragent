# Project Statistics

**Version:** 1.0.70

**Update prompt:** Update @STATS.md to show the composition of the project, show breakdown by crate


## Project-wide Metrics

| Metric | Value |
|---|---|
| Total Rust lines | 379,961 |
| Total Rust files | 935 |
| Tests defined | 7,374 |
| Test files | 404 |
| Test binaries | ~420 (399 integration test files + 18 lib/bin targets) |
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
| `ragent-tui` | 102 | 66,201 | 56 | Ratatui terminal interface |
| `ragent-agent` | 192 | 62,868 | 65 | Agent/runtime layer: sessions, orchestration, MCP, memory, tool registry |
| `ragent-tools-extended` | 148 | 58,046 | 57 | Extended document/web/memory/codeindex tools |
| `ragent-research` | 74 | 43,230 | 23 | Research system: web/local gathering, synthesis, RESEARCH.md output |
| `ragent-codeindex` | 65 | 21,858 | 36 | Codebase indexing: tree-sitter parsing, SQLite store, Tantivy FTS, file watcher, semantic graph |
| `ragent-llm` | 47 | 20,758 | 19 | Provider clients and model/provider registry |
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
| **Total** | **935** | **379,961** | **404** | |

---

## Crate Size Distribution

```
ragent-tui             ██████████████████████████████  66,201 lines  (17.4%)
ragent-agent           ████████████████████████████    62,868 lines  (16.5%)
ragent-tools-extended  ██████████████████████████      58,046 lines  (15.3%)
ragent-research        ███████████████████             43,230 lines  (11.4%)
ragent-codeindex       ██████████                      21,858 lines  ( 5.8%)
ragent-llm             ██████████                      20,758 lines  ( 5.5%)
ragent-specs           ████████                        17,428 lines  ( 4.6%)
ragent-tools-core      ███████                         16,275 lines  ( 4.3%)
ragent-storage         ██████                           14,237 lines  ( 3.7%)
ragent-tools-vcs       █████                           13,149 lines  ( 3.5%)
ragent-telemetry       █████                           10,217 lines  ( 2.7%)
ragent-config          ████                             8,753 lines  ( 2.3%)
ragent-types           ████                             8,248 lines  ( 2.2%)
ragent-bench           ████                             8,390 lines  ( 2.2%)
ragent-server          ██                               5,033 lines  ( 1.3%)
ragent-team            █                                2,762 lines  ( 0.7%)
ragent-prompt_opt      ▏                                  685 lines  ( 0.2%)
```

---

## Test Distribution

| Crate | Test Files | Approx. Tests |
|-------|-----------:|--------------:|
| `ragent-tools-extended` | 57 | ~1,745 |
| `ragent-tui` | 56 | ~944 |
| `ragent-agent` | 65 | ~801 |
| `ragent-research` | 23 | ~787 |
| `ragent-specs` | 13 | ~596 |
| `ragent-codeindex` | 36 | ~372 |
| `ragent-tools-vcs` | 13 | ~354 |
| `ragent-llm` | 19 | ~335 |
| `ragent-storage` | 30 | ~278 |
| `ragent-telemetry` | 16 | ~257 |
| `ragent-types` | 15 | ~246 |
| `ragent-tools-core` | 16 | ~235 |
| `ragent-config` | 22 | ~186 |
| `ragent-server` | 5 | ~87 |
| `ragent-team` | 14 | ~81 |
| `ragent-bench` | 2 | ~60 |
| `ragent-prompt_opt` | 2 | ~8 |
| **Total** | **404** | **~7,374** |

---

## Key Architecture Ratios

- Test-to-code ratio: ~1 test per 51 lines (7,374 tests / 379,961 lines)
- Largest crate: `ragent-tui` (66,201 lines, 17.4%)
- Smallest crate: `ragent-prompt_opt` (685 lines, 0.2%)
- Median crate size: 14,237 lines (ragent-storage)
- Crates over 10k lines: 11 of 17
- Crates under 5k lines: 2 of 17 (team, prompt_opt); server (5,033) sits just above
- Top test-to-code ratios (lines per test): ragent-specs ~29, ragent-tools-extended ~33, ragent-storage ~51, ragent-research ~55, ragent-agent ~79