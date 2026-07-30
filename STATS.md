# Project Statistics

**Version:** 0.1.0-beta.20

**Update prompt:** Update @STATS.md to show the composition of the project, show breakdown by crate


## Project-wide Metrics

| Metric | Value |
|---|---|
| Total Rust lines | 273,222 |
| Total Rust files | 680 |
| Tests defined | 4,623 |
| Test files | 211 |
| Benchmark files | 9 |
| Tools registered | ~114 |
| Supported languages (code index) | 15+ (Rust, Python, TypeScript/JavaScript, Go, C/C++, Java, OpenSCAD, Terraform, CMake, Gradle, Maven) |
| Workspace crates | 15 |
| Authors | 1 |

---

## Breakdown by Crate

The project is organised as a Cargo workspace of 15 focused crates. The table below
shows the file count, line count, and test-file count for each crate (including
`src/`, `tests/`, `benches/`, and `examples/` directories where present).

| Crate | Rust Files | Lines | Test Files | Description |
|-------|-----------:|------:|-----------:|-------------|
| `ragent-tui` | 83 | 52,312 | 43 | Ratatui terminal interface |
| `ragent-agent` | 165 | 51,424 | 42 | Agent/runtime layer: sessions, orchestration, MCP, memory, tool registry |
| `ragent-tools-extended` | 106 | 45,152 | 33 | Extended document/web/memory/codeindex tools |
| `ragent-research` | 34 | 22,142 | 9 | Research system: web/local gathering, synthesis, RESEARCH.md output |
| `ragent-llm` | 41 | 19,228 | 15 | Provider clients and model/provider registry |
| `ragent-codeindex` | 29 | 15,521 | 6 | Codebase indexing: tree-sitter parsing, SQLite store, Tantivy FTS, file watcher |
| `ragent-tools-core` | 45 | 11,531 | 13 | Core shell/file/search tools |
| `ragent-telemetry` | 21 | 10,123 | 12 | OpenTelemetry instrumentation and OTLP export |
| `ragent-bench` | 23 | 8,389 | 2 | Benchmark runner shared between TUI and CLI |
| `ragent-tools-vcs` | 33 | 7,063 | 1 | GitHub and GitLab tool surface |
| `ragent-specs` | 16 | 6,730 | 5 | Spec lifecycle management: discovery, validation, status transitions, review, archival |
| `ragent-config` | 23 | 6,506 | 15 | Configuration types, defaults, and parsing |
| `ragent-server` | 9 | 4,421 | 3 | Axum HTTP routes and SSE streaming |
| `ragent-storage` | 9 | 4,439 | 6 | SQLite-backed storage, snapshots, encrypted credentials |
| `ragent-types` | 18 | 3,140 | 5 | Shared IDs, events, messages, and sanitization primitives |
| `ragent-team` | 15 | 2,758 | 14 | Team coordination runtime and team tools |
| `ragent-prompt_opt` | 3 | 681 | 2 | Prompt optimization templates and completer abstraction |
| `ragent` (root) | 4 | 1,618 | 0 | Binary entry point and CLI wiring |
| **Total** | **677** | **273,222** | **211** | |

---

## Crate Size Distribution

```
ragent-tui             ████████████████████████████  52,312 lines  (19.1 %)
ragent-agent           ██████████████████████████    51,424 lines  (18.8 %)
ragent-tools-extended  ██████████████████████        45,152 lines  (16.5 %)
ragent-research        ███████████                   22,142 lines  ( 8.1 %)
ragent-llm             █████████                     19,228 lines  ( 7.0 %)
ragent-codeindex       ████████                      15,521 lines  ( 5.7 %)
ragent-tools-core      ██████                        11,531 lines  ( 4.2 %)
ragent-telemetry       █████                         10,123 lines  ( 3.7 %)
ragent-bench           ████                           8,389 lines  ( 3.1 %)
ragent-tools-vcs       ████                           7,063 lines  ( 2.6 %)
ragent-specs           ███                            6,730 lines  ( 2.5 %)
ragent-config          ███                            6,506 lines  ( 2.4 %)
ragent-server          ██                             4,421 lines  ( 1.6 %)
ragent-storage         ██                             4,439 lines  ( 1.6 %)
ragent-types           ██                             3,140 lines  ( 1.1 %)
ragent-team            █                              2,758 lines  ( 1.0 %)
ragent-prompt_opt      ▏                                681 lines  ( 0.2 %)
```

---

## Test Distribution

| Crate | Test Files | Approx. Tests |
|-------|-----------:|--------------:|
| `ragent-tools-extended` | 33 | ~1,281 |
| `ragent-tui` | 43 | ~742 |
| `ragent-agent` | 42 | ~555 |
| `ragent-research` | 9 | ~457 |
| `ragent-llm` | 15 | ~318 |
| `ragent-telemetry` | 12 | ~257 |
| `ragent-codeindex` | 6 | ~238 |
| `ragent-specs` | 5 | ~184 |
| `ragent-tools-core` | 13 | ~164 |
| `ragent-config` | 15 | ~119 |
| `ragent-team` | 14 | ~81 |
| `ragent-server` | 3 | ~72 |
| `ragent-bench` | 2 | ~59 |
| `ragent-tools-vcs` | 1 | ~43 |
| `ragent-types` | 5 | ~25 |
| `ragent-storage` | 6 | ~20 |
| `ragent-prompt_opt` | 2 | ~8 |
| **Total** | **211** | **~4,623** |

---

## Key Architecture Ratios

| Ratio | Value |
|---|---|
| UI + Server / Total | **20.8 %** (`ragent-tui` + `ragent-server`) |
| Core + Tools / Total | **42.2 %** (`ragent-agent` + all tool crates) |
| Research / Total | **8.1 %** (`ragent-research`) |
| Data + Index / Total | **7.3 %** (`ragent-storage` + `ragent-codeindex`) |
| Support / Total | **7.5 %** (`ragent-config` + `ragent-types` + `ragent-prompt_opt` + `ragent-telemetry`) |
| Avg lines per file | **402** |

---

*Last updated: 2026-02-16*
*Generated from `find . -name '*.rs' -not -path './target/*'`*
