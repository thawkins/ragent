# Project Statistics

**Version:** 1.0.27

**Update prompt:** Update @STATS.md to show the composition of the project, show breakdown by crate


## Project-wide Metrics

| Metric | Value |
|---|---|
| Total Rust lines | 293,942 |
| Total Rust files | 761 |
| Tests defined | 5,149 |
| Test files | 304 |
| Benchmark files | 10 |
| Tools registered | ~150 |
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
| `ragent-tui` | 92 | 57,097 | 51 | Ratatui terminal interface |
| `ragent-agent` | 174 | 53,078 | 57 | Agent/runtime layer: sessions, orchestration, MCP, memory, tool registry |
| `ragent-tools-extended` | 105 | 45,020 | 39 | Extended document/web/memory/codeindex tools |
| `ragent-research` | 52 | 28,748 | 16 | Research system: web/local gathering, synthesis, RESEARCH.md output |
| `ragent-llm` | 45 | 19,687 | 18 | Provider clients and model/provider registry |
| `ragent-codeindex` | 48 | 15,543 | 25 | Codebase indexing: tree-sitter parsing, SQLite store, Tantivy FTS, file watcher |
| `ragent-tools-core` | 48 | 13,290 | 14 | Core shell/file/search tools |
| `ragent-telemetry` | 25 | 10,217 | 16 | OpenTelemetry instrumentation and OTLP export |
| `ragent-bench` | 23 | 8,389 | 2 | Benchmark runner shared between TUI and CLI |
| `ragent-tools-vcs` | 35 | 7,627 | 2 | GitHub and GitLab tool surface |
| `ragent-specs` | 21 | 7,298 | 10 | Spec lifecycle management: discovery, validation, status transitions, review, archival |
| `ragent-config` | 25 | 6,693 | 16 | Configuration types, defaults, and parsing |
| `ragent-types` | 24 | 6,193 | 10 | Shared IDs, events, messages, and sanitization primitives |
| `ragent-storage` | 12 | 5,866 | 9 | SQLite-backed storage, snapshots, encrypted credentials |
| `ragent-server` | 9 | 4,183 | 3 | Axum HTTP routes and SSE streaming |
| `ragent-team` | 15 | 2,758 | 14 | Team coordination runtime and team tools |
| `ragent-prompt_opt` | 3 | 681 | 2 | Prompt optimization templates and completer abstraction |
| `ragent` (root) | 2 | 1,464 | 0 | Binary entry point and CLI wiring |
| **Total** | **761** | **293,942** | **304** | |

---

## Crate Size Distribution

```
ragent-tui             ██████████████████████████████  57,097 lines  (19.5%)
ragent-agent           ███████████████████████████    53,078 lines  (18.2%)
ragent-tools-extended  ███████████████████████       45,020 lines  (15.4%)
ragent-research        ███████████████               28,748 lines  ( 9.8%)
ragent-llm             ██████████                    19,687 lines  ( 6.7%)
ragent-codeindex       ████████                      15,543 lines  ( 5.3%)
ragent-tools-core      ██████                         13,290 lines  ( 4.5%)
ragent-telemetry       █████                          10,217 lines  ( 3.5%)
ragent-bench           ████                            8,389 lines  ( 2.9%)
ragent-tools-vcs       ████                            7,627 lines  ( 2.6%)
ragent-specs           ███                             7,298 lines  ( 2.5%)
ragent-config          ███                             6,693 lines  ( 2.3%)
ragent-types           ███                             6,193 lines  ( 2.1%)
ragent-storage         ███                             5,866 lines  ( 2.0%)
ragent-server          ██                              4,183 lines  ( 1.4%)
ragent-team            █                               2,758 lines  ( 0.9%)
ragent-prompt_opt      ▏                                 681 lines  ( 0.2%)
```

---

## Test Distribution

| Crate | Test Files | Approx. Tests |
|-------|-----------:|--------------:|
| `ragent-tools-extended` | 39 | ~1,338 |
| `ragent-tui` | 51 | ~806 |
| `ragent-agent` | 57 | ~590 |
| `ragent-research` | 16 | ~563 |
| `ragent-llm` | 18 | ~328 |
| `ragent-codeindex` | 25 | ~237 |
| `ragent-telemetry` | 16 | ~257 |
| `ragent-specs` | 10 | ~213 |
| `ragent-types` | 10 | ~199 |
| `ragent-tools-core` | 14 | ~177 |
| `ragent-config` | 16 | ~128 |
| `ragent-team` | 14 | ~81 |
| `ragent-server` | 3 | ~70 |
| `ragent-bench` | 2 | ~59 |
| `ragent-tools-vcs` | 2 | ~49 |
| `ragent-storage` | 9 | ~46 |
| `ragent-prompt_opt` | 2 | ~8 |
| **Total** | **304** | **~5,149** |

---

## Key Architecture Ratios

| Ratio | Value |
|---|---|
| Tests per source file | ~11.5 |
| Test files / source files | ~0.68 |
| Avg. lines per source file | ~658 |
| Crates with benchmarks | 3 (`ragent-tui`, `ragent-server`, `ragent-codeindex`) |