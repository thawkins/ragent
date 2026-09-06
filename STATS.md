# Project Statistics

**Version:** 1.0.79

**Update prompt:** Update @STATS.md to show the composition of the project, show breakdown by crate


## Project-wide Metrics

| Metric | Value |
|---|---|
| Total Rust lines | 406,998 |
| Total Rust files | 1,001 |
| Tests defined | ~7,645 (6,135 in `tests/` + ~1,510 inline `#[cfg(test)]`) |
| Test files | 445 (incl. 16 `inline/` helper files) |
| Test binaries | ~461 (445 integration test files + 16 lib/bin targets) |
| Benchmark files | 13 |
| Tools registered | 168 |
| Supported languages (code index) | 15+ (Rust, Python, TypeScript/JavaScript, Go, C/C++, Java, OpenSCAD, Terraform, CMake, Gradle, Maven) |
| Workspace crates | 17 |
| Authors | 1 |
| Version | 1.0.79 |

---

## Breakdown by Crate

The project is organised as a Cargo workspace of 17 focused crates. The table below
shows the file count, line count, and test-file count for each crate (including
`src/`, `tests/`, `benches/`, and `examples/` directories where present).

| Crate | Rust Files | Lines | Test Files | Description |
|-------|-----------:|------:|-----------:|-------------|
| `ragent-tools-extended` | 152 | 57,773 | 56 | Extended document/web/memory/codeindex/plot tools |
| `ragent-tui` | 123 | 75,117 | 75 | Ratatui terminal interface |
| `ragent-agent` | 198 | 64,791 | 68 | Agent/runtime layer: sessions, orchestration, MCP, memory, tool registry |
| `ragent-research` | 102 | 55,251 | 39 | Research system: web/local gathering, synthesis, RESEARCH.md output |
| `ragent-codeindex` | 65 | 21,957 | 36 | Codebase indexing: tree-sitter parsing, SQLite store, Tantivy FTS, file watcher, semantic graph |
| `ragent-llm` | 49 | 22,714 | 20 | Provider clients and model/provider registry |
| `ragent-specs` | 26 | 17,464 | 13 | Spec lifecycle management: discovery, validation, status transitions, review, archival |
| `ragent-tools-core` | 51 | 16,554 | 16 | Core shell/file/search tools |
| `ragent-storage` | 35 | 14,268 | 30 | SQLite-backed storage, snapshots, encrypted credentials |
| `ragent-tools-vcs` | 47 | 13,153 | 13 | GitHub and GitLab tool surface |
| `ragent-telemetry` | 25 | 10,217 | 16 | OpenTelemetry instrumentation and OTLP export |
| `ragent-config` | 36 | 9,355 | 23 | Configuration types, defaults, and parsing |
| `ragent-bench` | 24 | 8,433 | 3 | Benchmark runner shared between TUI and CLI |
| `ragent-types` | 34 | 8,347 | 16 | Shared IDs, events, messages, and sanitization primitives |
| `ragent-server` | 11 | 5,622 | 5 | Axum HTTP routes and SSE streaming |
| `ragent-team` | 15 | 2,770 | 14 | Team coordination runtime and team tools |
| `ragent-prompt_opt` | 3 | 687 | 2 | Prompt optimization templates and completer abstraction |
| `ragent` (root) | 5 | 2,525 | 2 | Binary entry point and CLI wiring |
| **Total** | **1,001** | **406,998** | **445** | |

---

## Crate Size Distribution

```
ragent-tui             █████████████████████████████   75,117 lines  (18.5%)
ragent-agent           ███████████████████████████   64,791 lines  (15.9%)
ragent-tools-extended  ██████████████████████████████  57,773 lines  (14.2%)
ragent-research        ████████████████████            55,251 lines  (13.6%)
ragent-llm             ██████████                      22,714 lines  ( 5.6%)
ragent-codeindex       ██████████                      21,957 lines  ( 5.4%)
ragent-specs           ████████                        17,464 lines  ( 4.3%)
ragent-tools-core      ███████                         16,554 lines  ( 4.1%)
ragent-storage         ██████                          14,268 lines  ( 3.5%)
ragent-tools-vcs       ██████                          13,153 lines  ( 3.2%)
ragent-telemetry       █████                           10,217 lines  ( 2.5%)
ragent-config          ████                             9,355 lines  ( 2.3%)
ragent-bench           ████                             8,433 lines  ( 2.1%)
ragent-types           ████                             8,347 lines  ( 2.1%)
ragent-server          ██                               5,622 lines  ( 1.4%)
ragent-team            █                                2,770 lines  ( 0.7%)
ragent-prompt_opt      ▏                                  687 lines  ( 0.2%)
```

---

## Test Distribution

| Crate | Test Files | Approx. Tests |
|-------|-----------:|--------------:|
| `ragent-tools-extended` | 56 | ~1,548 |
| `ragent-tui` | 75 | ~990 |
| `ragent-agent` | 68 | ~644 |
| `ragent-research` | 39 | ~315 |
| `ragent-specs` | 13 | ~458 |
| `ragent-codeindex` | 36 | ~371 |
| `ragent-tools-vcs` | 13 | ~268 |
| `ragent-llm` | 20 | ~262 |
| `ragent-storage` | 30 | ~278 |
| `ragent-telemetry` | 16 | ~187 |
| `ragent-types` | 16 | ~183 |
| `ragent-tools-core` | 16 | ~220 |
| `ragent-config` | 23 | ~175 |
| `ragent-server` | 5 | ~92 |
| `ragent-team` | 14 | ~81 |
| `ragent-bench` | 3 | ~50 |
| `ragent-prompt_opt` | 2 | ~8 |
| **Total (external)** | **445** | **~6,135** |

Inline `#[cfg(test)]` modules in library sources contribute a further
~1,510 test attributes (largest contributors: `ragent-llm`, `ragent-tui`,
`ragent-agent`), bringing the estimated total to ~7,645.

---

## Key Architecture Ratios

- Test-to-code ratio: ~1 test per 53 lines (7,645 tests / 406,998 lines)
- Largest crate: `ragent-tui` (75,117 lines, 18.5%)
- Smallest crate: `ragent-prompt_opt` (687 lines, 0.2%)
- Median crate size: 14,268 lines (ragent-storage)
- Crates over 10k lines: 11 of 17
- Crates under 5k lines: 2 of 17 (team, prompt_opt); server (5,622) sits just above