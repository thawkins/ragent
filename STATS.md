# Project Statistics

**Version:** 1.0.124

**Update prompt:** Update @STATS.md to show the composition of the project, show breakdown by crate

> Metrics below are measured against the working tree on top of v1.0.124.


## Project-wide Metrics

| Metric | Value |
|---|---|
| Total Rust lines | 548,612 (542,810 in `crates/` + 5,802 in root `src/`/`tests/`/`examples/`) |
| Total Rust files | 1,509 (workspace crates) + 16 (root `src/`/`tests/`/`examples/`) |
| Tests defined | ~10,330 (`#[test]` / `#[tokio::test]` attributes across `crates/`, `src/`, and root `tests/`) |
| Test files | 763 external + ~200 inline-bearing |
| Test binaries | ~781 (763 integration test files + 17 lib/bin targets + 1 root bin) |
| Benchmark files | 17 (+1 in `vendor/html2text`) |
| Tools registered | 171 |
| Supported languages (code index) | 15+ (Rust, Python, TypeScript/JavaScript, Go, C/C++, Java, OpenSCAD, Terraform, CMake, Gradle, Maven) |
| Workspace crates | 17 |
| Specs on disk | 55 directories in `specs/` |
| Documentation | 27 per-category tool how-tos in `docs/howtos/tools/` (+ generated PDFs), 21 category how-tos, 79 slash-command docs |
| Authors | 1 |
| Version | 1.0.124 |

---

## Breakdown by Crate

The project is organised as a Cargo workspace of 17 focused crates. The table below
shows the file count, line count, and test-file count for each crate (including
`src/`, `tests/`, `benches/`, and `examples/` directories where present).

| Crate | Rust files | Rust lines | Test files |
|---|---|---|---|
| `ragent-agent` | 281 | 90,230 | 119 |
| `ragent-bench` | 28 | 8,619 | 5 |
| `ragent-codeindex` | 73 | 24,192 | 43 |
| `ragent-config` | 52 | 12,699 | 34 |
| `ragent-connectors` | 37 | 17,024 | 16 |
| `ragent-llm` | 66 | 24,295 | 24 |
| `ragent-plugins` | 56 | 21,152 | 31 |
| `ragent-research` | 158 | 60,863 | 92 |
| `ragent-server` | 15 | 6,467 | 8 |
| `ragent-specs` | 37 | 20,362 | 24 |
| `ragent-storage` | 39 | 15,246 | 34 |
| `ragent-telemetry` | 30 | 10,560 | 21 |
| `ragent-tools-core` | 95 | 18,628 | 25 |
| `ragent-tools-extended` | 218 | 75,610 | 88 |
| `ragent-tools-vcs` | 64 | 15,873 | 23 |
| `ragent-tui` | 211 | 110,703 | 150 |
| `ragent-types` | 49 | 10,287 | 27 |

---

## Crate Size Distribution

```
ragent-tui            ############ 110,703 lines (20.4%)
ragent-agent          ########## 90,230 lines (16.6%)
ragent-tools-extended ######## 75,610 lines (13.9%)
ragent-research       ####### 60,863 lines (11.2%)
ragent-llm            ### 24,295 lines (4.5%)
ragent-codeindex      ### 24,192 lines (4.5%)
ragent-plugins        ## 21,152 lines (3.9%)
ragent-specs          ## 20,362 lines (3.8%)
ragent-tools-core     ## 18,628 lines (3.4%)
ragent-connectors     ## 17,024 lines (3.1%)
ragent-tools-vcs      ## 15,873 lines (2.9%)
ragent-storage        ## 15,246 lines (2.8%)
ragent-config         # 12,699 lines (2.3%)
ragent-telemetry      # 10,560 lines (1.9%)
ragent-types          # 10,287 lines (1.9%)
ragent-bench          # 8,619 lines (1.6%)
ragent-server         # 6,467 lines (1.2%)
```

---

## Test Distribution

| Crate | Test Files | Approx. Tests |
|-------|-----------:|--------------:|
| `ragent-tools-extended` | 88 | ~2,037 |
| `ragent-tui` | 150 | ~1,755 |
| `ragent-agent` | 119 | ~1,147 |
| `ragent-research` | 92 | ~1,099 |
| `ragent-specs` | 24 | ~683 |
| `ragent-plugins` | 31 | ~435 |
| `ragent-codeindex` | 43 | ~411 |
| `ragent-tools-vcs` | 23 | ~403 |
| `ragent-llm` | 24 | ~376 |
| `ragent-connectors` | 16 | ~318 |
| `ragent-types` | 27 | ~300 |
| `ragent-config` | 34 | ~292 |
| `ragent-storage` | 34 | ~291 |
| `ragent-tools-core` | 25 | ~284 |
| `ragent-telemetry` | 21 | ~260 |
| `ragent-server` | 8 | ~114 |
| `ragent-bench` | 5 | ~63 |
| **Total (external)** | **763** | **~10,268** |

Inline `#[cfg(test)]` modules in library sources contribute a further
~60 test attributes (root `src/`/`tests/`; the migration effort has moved the
bulk of these into each crate's `tests/` tree), bringing the estimated total to
~10,330.

---

## Test Coverage

Measured with `cargo llvm-cov --workspace --summary-only` (line coverage,
test/bench/example sources excluded). Reproduce with:

```bash
cargo llvm-cov --workspace --summary-only --ignore-filename-regex '(^|/)(tests|benches|examples)/|target/'
```

The most recent full workspace measurement (v1.0.95) reported:

**Workspace line coverage: 68.9%** (157,695 instrumented lines, 48,966 missed
 - i.e. 108,729 lines executed by the test suite). Function coverage is 67.8%
 (15,582 functions, 5,014 never called).

| Crate | Cover | Lines | Missed |
|---|---:|---:|---:|
| `ragent-specs` | 92.6% | 5,652 | 420 |
| `ragent-research` | 89.8% | 27,527 | 2,796 |
| `ragent-types` | 85.7% | 1,729 | 247 |
| `ragent-telemetry` | 85.2% | 2,870 | 426 |
| `ragent-codeindex` | 83.2% | 8,519 | 1,431 |
| `ragent-storage` | 77.5% | 3,723 | 838 |
| `ragent-tools-vcs` | 76.4% | 3,633 | 858 |
| `ragent-bench` | 76.2% | 4,410 | 1,051 |
| `ragent-tools-core` | 72.9% | 5,113 | 1,387 |
| `ragent-config` | 71.8% | 2,247 | 633 |
| `ragent-llm` | 70.1% | 8,399 | 2,510 |
| `ragent-tools-extended` | 65.6% | 16,087 | 5,535 |
| `ragent-agent` | 63.2% | 27,293 | 10,031 |
| `ragent-server` | 55.7% | 2,031 | 899 |
| `ragent-tui` | 49.3% | 36,884 | 18,718 |
| root bin (`src/`) | 24.9% | 1,575 | 1,183 |

Notes:

- The coverage figures above are the last full workspace measurement (v1.0.95);
  they are not re-measured on every documentation pass because
  `cargo llvm-cov` instruments and runs the entire workspace. The
  `ragent-plugins` crate (added in v1.0.112) and the `ragent-connectors` crate
  (added in the uncommitted connector work) are not part of that run.
- Line counts here are executable lines under LLVM profiling, which is smaller
  than the raw `wc -l` figures in the crate table above (declarations, blank
  and comment-only lines are not instrumented).
- The largest coverage gaps are the TUI event/input paths (`app/slash.rs`,
  `app/event_handler.rs`, `input.rs`) and the agent session processor - the
  parts that require a live LLM or terminal to exercise.
- Root binary sources (`src/cli.rs`, `src/main.rs`, `src/plugins.rs`,
  `src/connectors.rs`, `src/panic_hook.rs`) are only lightly covered (24.9%) because
  `cargo llvm-cov` does not drive the interactive TUI.

---

## Key Architecture Ratios

- Test-to-code ratio: ~1 test per 53 lines (10,330 tests / 548,612 lines)
- Largest crate: `ragent-tui` (110,703 lines, 20.4%)
- Smallest crate: `ragent-server` (6,467 lines, 1.2%)
- Median crate size: 18,628 lines (`ragent-tools-core`)
- Crates over 10k lines: 15 of 17
- Crates under 5k lines: 0 of 17

---

_Generated 2026-10-02 (v1.0.124 tree plus the uncommitted follow-up work -
`/memory clear` clears this project's structured memories behind a `Yes`/`No`
confirmation dialog (new `Storage::clear_memories_for_project`, `No` by default),
and the `command_catalog`/`SLASH_COMMANDS` `/memory` entries are realigned to the
subcommands the arm handles; the TUI now drives the session-start connector
lifecycle with live tool counts and a single interactive `claude` catalogue
surface carrying `--category` and a `c` cycle key; the TUI-019 render fixes put
the tool name immediately after the step counter and render `/mcp` as
one-server-per-row markdown/ASCII table; the `/tools` subcommand surface gained
`/tools list` and a help block; and the MCP orphan sweep reaps stdio servers
re-parented to the per-user service manager (`systemd --user`) as well as init,
matching per process - alongside the connector work itself (the
new `ragent-connectors` crate takes the workspace from 16 to 17 crates), the
ANTIPAT M1 ASCII sweep that replaces the TUI tool-category and status-bar emoji
with ASCII marker prefixes, `/yolo` persisting to the user-global config only, and
the spec plan parser `T-001..T-014` ranges - and the committed v1.0.122 security
and anti-pattern remediation sweep folding in the `ANTIPAT.md` M0 plus M2-M7 and
`SECTASKS.md` MS-05 work - `ragent-team` shim crate removed - and the
`ragent_types::guard` shared-guard module added. Earlier v1.0.121
work: rollback-capture remove-after-restore race fix (CI flake
`test_rollback_accept_restores_snapshot`), OpenSkills `.agents` discovery, Claude
marketplace `*-lsp` inline-manifest materialisation, conventional `skills/`
directory bridging, plus a `/simplify all` pass over the 50-file changed set. Tool
count is 171 registered (`tool_info` + `commands_info` added in v1.0.120).)_
