# Project Statistics

**Version:** 1.0.58

**Update prompt:** Update @STATS.md to show the composition of the project, show breakdown by crate


## Project-wide Metrics

| Metric | Value |
|---|---|
| Total Rust lines | 356,307 |
| Total Rust files | 883 |
| Tests defined | 6,750 |
| Test files | 361 |
| Test binaries | 343 |
| Benchmark files | 11 |
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
| `ragent-tui` | 97 | 62,834 | 55 | Ratatui terminal interface |
| `ragent-tools-extended` | 148 | 57,932 | 57 | Extended document/web/memory/codeindex tools |
| `ragent-agent` | 188 | 59,964 | 62 | Agent/runtime layer: sessions, orchestration, MCP, memory, tool registry |
| `ragent-research` | 69 | 41,422 | 19 | Research system: web/local gathering, synthesis, RESEARCH.md output |
| `ragent-llm` | 46 | 20,434 | 18 | Provider clients and model/provider registry |
| `ragent-codeindex` | 65 | 21,641 | 36 | Codebase indexing: tree-sitter parsing, SQLite store, Tantivy FTS, file watcher, semantic graph |
| `ragent-tools-core` | 51 | 15,407 | 16 | Core shell/file/search tools |
| `ragent-telemetry` | 25 | 10,217 | 16 | OpenTelemetry instrumentation and OTLP export |
| `ragent-bench` | 23 | 8,389 | 2 | Benchmark runner shared between TUI and CLI |
| `ragent-specs` | 26 | 17,351 | 13 | Spec lifecycle management: discovery, validation, status transitions, review, archival |
| `ragent-tools-vcs` | 35 | 7,627 | 2 | GitHub and GitLab tool surface |
| `ragent-config` | 31 | 8,259 | 20 | Configuration types, defaults, and parsing |
| `ragent-types` | 29 | 6,878 | 12 | Shared IDs, events, messages, and sanitization primitives |
| `ragent-storage` | 16 | 8,424 | 13 | SQLite-backed storage, snapshots, encrypted credentials |
| `ragent-server` | 10 | 4,260 | 4 | Axum HTTP routes and SSE streaming |
| `ragent-team` | 15 | 2,758 | 14 | Team coordination runtime and team tools |
| `ragent-prompt_opt` | 3 | 681 | 2 | Prompt optimization templates and completer abstraction |
| `ragent` (root) | 3 | 1,719 | 0 | Binary entry point and CLI wiring |
| **Total** | **883** | **356,307** | **361** | |

---

## Crate Size Distribution

```
ragent-agent           ██████████████████████████████  59,964 lines  (16.8%)
ragent-tui             █████████████████████████████   62,834 lines  (17.6%)
ragent-tools-extended  ███████████████████████████     57,932 lines  (16.3%)
ragent-research        █████████████████████           41,422 lines  (11.6%)
ragent-llm             ██████████                      20,434 lines  ( 5.7%)
ragent-codeindex       ██████████                      21,641 lines  ( 6.1%)
ragent-tools-core      ████████                        15,407 lines  ( 4.3%)
ragent-telemetry       █████                           10,217 lines  ( 2.9%)
ragent-bench           ████                             8,389 lines  ( 2.4%)
ragent-specs           █████████                        17,351 lines  ( 4.9%)
ragent-tools-vcs       ████                             7,627 lines  ( 2.1%)
ragent-config          ████                             8,259 lines  ( 2.3%)
ragent-types           ███                              6,878 lines  ( 1.9%)
ragent-storage         ████                             8,424 lines  ( 2.4%)
ragent-server          ██                               4,260 lines  ( 1.2%)
ragent-team            █                                2,758 lines  ( 0.8%)
ragent-prompt_opt      ▏                                  681 lines  ( 0.2%)
```

---

## Test Distribution

| Crate | Test Files | Approx. Tests |
|-------|-----------:|--------------:|
| `ragent-tools-extended` | 57 | ~1,745 |
| `ragent-tui` | 55 | ~902 |
| `ragent-agent` | 62 | ~781 |
| `ragent-research` | 19 | ~740 |
| `ragent-llm` | 18 | ~333 |
| `ragent-codeindex` | 36 | ~372 |
| `ragent-telemetry` | 16 | ~257 |
| `ragent-specs` | 13 | ~597 |
| `ragent-types` | 12 | ~222 |
| `ragent-tools-core` | 16 | ~229 |
| `ragent-config` | 20 | ~175 |
| `ragent-team` | 14 | ~81 |
| `ragent-server` | 4 | ~74 |
| `ragent-bench` | 2 | ~60 |
| `ragent-tools-vcs` | 2 | ~49 |
| `ragent-storage` | 13 | ~123 |
| `ragent-prompt_opt` | 2 | ~8 |
| **Total** | **361** | **~6,750** |

---

## Key Architecture Ratios

| Ratio | Value |
|-------|-------|
| Test files / source files | 0.41 |
| Tests per source file | ~7.6 |
| Lines per crate (avg) | 19,795 |
| Lines per file (avg) | 403 |
| Tools per crate (avg) | ~9.9 |