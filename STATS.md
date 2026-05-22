# Project Statistics

**Version:** 0.1.0-alpha.89

**Update prompt:** Update @STATS.md to show the composition of the project, show breakdown by crate


## Project-wide Metrics

| Metric | Value |
|---|---|
| Total Rust lines | 175,840 |
| Total Rust files | 468 |
| Tests defined | 1,670 |
| Test files | 65 |
| Test lines | 21,286 |
| Benchmark files | 5 |
| Tools registered | ~111 |
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
| `ragent-agent` | 167 | 54,264 | 9 | Agent/runtime layer: sessions, orchestration, MCP, memory, tool registry |
| `ragent-tui` | 56 | 39,409 | 28 | Ratatui terminal interface |
| `ragent-codeindex` | 27 | 15,307 | 5 | Codebase indexing: tree-sitter parsing, SQLite store, Tantivy FTS, file watcher |
| `ragent-tools-extended` | 35 | 11,074 | 3 | Extended document/web/memory/codeindex tools |
| `ragent-llm` | 21 | 11,225 | 4 | Provider clients and model/provider registry |
| `ragent-tools-vcs` | 33 | 7,044 | 1 | GitHub and GitLab tool surface |
| `ragent-team` | 30 | 6,099 | 0 | Team coordination runtime and team tools |
| `ragent-tools-core` | 27 | 5,604 | 0 | Core shell/file/search tools |
| `ragent-bench` | 20 | 7,951 | 1 | Benchmark runner shared between TUI and CLI |
| `ragent-specs` | 11 | 3,979 | 3 | Spec lifecycle management: discovery, validation, status transitions, review, archival |
| `ragent-server` | 8 | 3,773 | 3 | Axum HTTP routes and SSE streaming |
| `ragent-config` | 12 | 3,737 | 6 | Configuration types, defaults, and parsing |
| `ragent-storage` | 3 | 2,498 | 0 | SQLite-backed storage, snapshots, encrypted credentials |
| `ragent-types` | 12 | 2,016 | 1 | Shared IDs, events, messages, and sanitization primitives |
| `ragent-prompt_opt` | 2 | 673 | 1 | Prompt optimization templates and completer abstraction |
| **Total** | **468** | **175,840** | **65** | |

---

## Crate Size Distribution

```
ragent-agent      ████████████████████████████████  54,264 lines  (30.9 %)
ragent-tui        █████████████��██████████          39,409 lines  (22.4 %)
ragent-codeindex  ██████████                        15,307 lines  ( 8.7 %)
ragent-llm        ████████                          11,225 lines  ( 6.4 %)
ragent-tools-extended  ████████                      11,074 lines  ( 6.3 %)
ragent-tools-vcs  █████                             7,044 lines  ( 4.0 %)
ragent-team       █████                             6,099 lines  ( 3.5 %)
ragent-tools-core ████                              5,604 lines  ( 3.2 %)
ragent-bench      █████                             7,951 lines  ( 4.5 %)
ragent-specs      ███                               3,979 lines  ( 2.3 %)
ragent-server     ███                               3,773 lines  ( 2.1 %)
ragent-config     ███                               3,737 lines  ( 2.1 %)
ragent-storage    ██                                2,498 lines  ( 1.4 %)
ragent-types      ██                                2,016 lines  ( 1.1 %)
ragent-prompt_opt █                                 673 lines    ( 0.4 %)
```

---

## Test Distribution

| Crate | Test Files | Approx. Tests |
|-------|-----------:|--------------:|
| `ragent-tui` | 28 | ~720 |
| `ragent-agent` | 9 | ~380 |
| `ragent-llm` | 4 | ~180 |
| `ragent-tools-extended` | 3 | ~110 |
| `ragent-config` | 6 | ~100 |
| `ragent-server` | 3 | ~60 |
| `ragent-specs` | 3 | ~55 |
| `ragent-codeindex` | 5 | ~50 |
| `ragent-bench` | 1 | ~5 |
| `ragent-types` | 1 | ~5 |
| `ragent-prompt_opt` | 1 | ~5 |
| `ragent-tools-vcs` | 1 | ~5 |
| `ragent-team` | 0 | 0 |
| `ragent-tools-core` | 0 | 0 |
| `ragent-storage` | 0 | 0 |
| **Total** | **65** | **~1,670** |

---

## Key Architecture Ratios

| Ratio | Value |
|---|---|
| UI + Server / Total | **24.6 %** (`ragent-tui` + `ragent-server`) |
| Core + Tools / Total | **58.5 %** (`ragent-agent` + all tool crates) |
| Data + Index / Total | **10.1 %** (`ragent-storage` + `ragent-codeindex`) |
| Support / Total | **6.8 %** (`ragent-config` + `ragent-types` + `ragent-prompt_opt`) |
| Test code / Total | **12.1 %** (21,286 test lines / 175,840 total lines) |
| Avg lines per file | **376** |

---

*Last updated: 2025-01-22*
*Generated from `find . -name '*.rs' -not -path './target/*'`*
