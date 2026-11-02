# Project Statistics

**Version:** 1.0.161

**Update prompt:** Update @STATS.md to show the composition of the project, show breakdown by crate

> Metrics below are measured against the current working tree at the
> `1.0.161` state **plus the uncommitted research pass**. That pass adds the
> `newsletter` output format (a reader-facing layout that drops the audit-trail
> sections), names the report title with the LLM from the generated executive
> summary (`LlmAnalysisEngine::summarize_title` normalised by
> `clean_generated_title`) instead of truncating that summary's first 80
> characters, and widens a concept's `**Definition**` from one paragraph to
> three (roughly 120-200 words). Two test modules land with it
> (`tests/test_newsletter_format.rs`, `tests/test_title_extraction.rs`) and the
> `research` slash-command page, `docs/howtos/research.md`, README, QUICKSTART
> and TUI-QUICKSTART now document the new format.
>
> The `1.0.161` baseline gives research output
> files a **dated name** (`RESEARCH-<YYYY-MM-DD>.md` / `CORPA-<YYYY-MM-DD>.md`,
> resolved through `ResearchIo::find_research_md` /
> `resolve_research_md` with the legacy undated `RESEARCH.md` still accepted),
> so a `/research update` re-run keeps the earlier report instead of
> overwriting it; it also fixes an automation in-flight-marker ordering bug
> (`execute_run` cleared `running` after `finalise()` had persisted the terminal
> outcome), adds the missing in-process half of two test-isolation exclusions
> (`test_tool_rename_guard`'s `guard_lock()`, `ragent-telemetry`'s
> `global_env_lock()`), adds `docs/howtos/skills.md`, and records the first
> build/test/coverage cost measurement in `BUILDSTATS.md`. That measurement is
> the source of every coverage figure below.
>
> The `1.0.159` baseline (OTLP log-export and endpoint-resolution
> pass on top of the session-message-retention and `/db` operator-surface work,
> which sits on `1.0.158`'s CI dead-code-lint fix, the `1.0.157` CI-fix and
> clippy-cleanup pass, and the `1.0.156` version bump that publishes the
> `docs/reports/code-audit.md` remediation pass). The uncommitted pass adds a
> `telemetry.otel.logs` config block, a new `ragent-telemetry/src/logs.rs`
> module with the composing `TracingLogsLayer`, `otlp_http_signal_endpoint`
> (the signal-path resolver both the metrics and logs exporters now share),
> `/telemetry logs` plus the `Logs level` row in `/telemetry setup`, the
> `Storage::prune_messages_older_than` / `count_messages_older_than` pair with
> the startup retention pass, `ragent db prune` and the TUI `/db` surface, and
> the `journal_size_limit` bound on `ragent.db-wal`. A follow-up noise-and-
> correctness pass sits on top of it: the code index counts its skip reasons
> instead of logging one `trace!` per path (roughly 6,000 events per reindex),
> Ollama Cloud's per-frame line drops to `DEBUG`, the `TuiTracingLayer` and
> mirrored-stderr drains stop re-emitting records the OTLP layer already
> exported, `--log-level` defaults to `info` so tool calls reach the export, MCP
> config validation runs in YOLO mode too, and `allowed-tools` resolution folds
> ASCII case with an `any` pseudo entry. The `1.0.159` pass adds the
> opt-in `storage.retention` config section, two storage
> methods, one `InputAction` pair, four `App` state fields, and 25 tests across
> three crates. The `1.0.158` pass changes nothing but item
> visibility: 57 items that were declared `pub` inside private modules are
> now `pub(crate)`, which is the exact reachability they had, so the
> `Dead-code lint` job's `-D unreachable_pub` passes again. The `1.0.157`
> pass is behaviour-neutral apart from one runtime fix: autopilot no longer keeps
> running after the primary agent calls `agent_complete`, because the
> `Event::MessageStart` arm clears the `last_task_completed_at` latch only
> while `autopilot_enabled` is true. It also de-flakes the
> `test_openai_surface` permission-prompt wait (a single 30-second deadline
> instead of a per-event 200 ms window) and clears every clippy
> `redundant_pub_crate` finding by narrowing the affected items to the
> visibility their module actually grants. That earlier pass collapses four
> duplicated-guard findings
> into shared implementations (one SSE accumulation helper instead of nine
> hand-written copies, one `ANTHROPIC_VERSION`, one root-key-parameterised
> marketplace ledger shared by `/plugins` and `/connectors`, one
> `DEFAULT_LISTEN_ADDR`), strengthens the two guards that police that
> duplication, puts `#![deny(unsafe_code)]` on all 19 crate and binary roots,
> removes the debug-only `/inputdiag` command, and fixes the
> `truncate_chars`/`truncate_bytes` visible-character budget. It sits on the
> `1.0.154` CI fixes, which ship the `agentssetup` / `teamssetup`
> state-machine tightening: the single member-status writer
> (`TeamManager::set_member_status`) and its on-disk `status-history.jsonl`,
> the shared `tool::param_schema` parameter validation enforced by every
> sub-agent tool, the `hook-failures.jsonl` ledger, `team_task_cancel`, the
> ownership rule on `team_task_complete`, the `agents` config section
> (`completed_history`, `max_launching`), and the shared sub-agent launch set in
> `ragent-tui/src/app/launch.rs`. It rebuilds on the v1.0.152 `/research --age`
> work (typed `AgeSpan`, the `DateTime` tool, the `Alt+O` autopilot toggle), the
> v1.0.150 `uniformtools` tool-call model, the v1.0.149 `agentui` keyboard-first
> rework, the v1.0.138 consolidated test layout (see the `[1.0.138]` CHANGELOG
> entry and `docs/reports/testconsolidation-review.md`) and the v1.0.137
> coverage campaign. Figures are a 2026-11-01 re-measurement of the working
> tree (`crates/` measured with `target/` pruned).


## Project-wide Metrics

| Metric | Value |
|---|---|
| Total Rust lines | 748,724 (741,416 in `crates/` + 7,308 in root `src/`/`tests/`) |
| Total Rust files | 1,930 (1,909 workspace crates + 21 root `src/`/`tests/`) |
| Tests defined | 15,268 (`#[test]` / `#[tokio::test]` attributes, including `flavor = ...` variants, across `crates/`, `src/`, and root `tests/`) |
| Test files | 1,209 under crate `tests/` trees (1,054 consolidated `test_*.rs` + 127 `tests/inline/*` modules, 18 `tests/all.rs` roots and 8 shared `support` helpers); the 18 `tests/all.rs` roots are the per-crate targets |
| Test binaries | 36 (18 consolidated `[[test]] name = "all"` integration targets - one per crate, 17 crate lib targets and the 1 root bin) |
| Tests executed | 15,240 passed / 0 failed / 16 ignored across **40 reporting targets** (`cargo test --workspace --all-targets`, see `BUILDSTATS.md`) |
| Line coverage | 86.7% (146,001 of 168,392 instrumented lines) |
| Benchmark files | 17 (+1 in `vendor/html2text`) |
| Tools registered | 150 in 21 families (`mf_screenshot` engine-gated above the shown count) |
| Supported languages (code index) | 15+ (Rust, Python, TypeScript/JavaScript, Go, C/C++, Java, OpenSCAD, Terraform, CMake, Gradle, Maven) |
| Workspace crates | 18 |
| Specs on disk | 61 directories in `specs/` |
| Research output formats | 6 (`report`, `executive-summary`, `comparison-table`, `source-bibliography`, `imrad`, `newsletter`) |
| Documentation | 24 top-level how-tos (including the new `skills.md`), 23 files in `docs/howtos/tools/` (22 per-category tool how-tos + `INDEX.md`, + generated PDFs), 83 files in `docs/howtos/slashcommands/` (82 command pages + `INDEX.md`, + generated PDFs) |
| Authors | 1 |
| Version | 1.0.161 |

The tool registry is unchanged this pass: research output naming is a file-layout
change inside `ragent-research`, and the automation fix is an ordering change in
`execute_run`, so nothing is added to or removed from the registry and the count
stays at 150 across 21 families. `ragent-research` is the crate that moves
materially (64,960 -> 65,569 lines, +609) for the dated-name primitives, the
resolution helpers, the LLM report-title derivation
(`LlmAnalysisEngine::summarize_title` + `clean_generated_title`), the
`newsletter` output format (`assemble_newsletter_body`, its synthesis prompt and
its brief) and the three-paragraph concept-definition prompt, plus the two new
test modules; `ragent-tui` (+153) and `ragent-telemetry` (+20) move only by
their test-side isolation changes, and `ragent-agent` (+38) by the `execute_run`
reordering and its test lock.

The test-binary count is the headline effect of the consolidation rollout: the
tree still holds 1,049 consolidated `test_*.rs` files plus 127 `tests/inline/*`
modules, but each crate compiles them into
ONE `tests/all.rs` target (`autotests = false` + `[[test]] name = "all"` +
`#[path = "test_x.rs"] mod test_x;`), so a full run links 18 integration
binaries instead of ~1,039. Tests run under `cargo nextest`
(`.config/nextest.toml`); run one file by filter:
`cargo nextest run -p <crate> -E 'test(test_foo::)'`.

The registered-tool count is a live registry fact, not a constant: `ragent run
--dry-run` prints the per-family breakdown, and 21 families currently sum to
150 (`file: 27`, `team: 20`, `gitlab: 19`, `git: 18`, `github: 11`,
`codeindex: 10`, `none: 8`, `web: 5`, `cron: 5`, `spec: 5`, `agent: 5`,
`task: 4`, `bash: 3`, `plan: 2`, `system: 2`, and one each for `model`,
`network`, `shell`, `skill`, `storage`, `ask_user`). `team_task_cancel` is
what moved `team` 19 -> 20; the `agent` family reads 5 because `team_wait` is
classified `agent:spawn` (it always was), and `none` reads 8 because
`list_agents` moved to `agent:read`. `mf_screenshot` joins the registry only
when a drivable headless browser resolves on `PATH`, which is why the advertised
total is 150 rather than 151.

---

## Breakdown by Crate

The project is organised as a Cargo workspace of 18 focused crates. The table below
shows the file count, line count, and test-file count for each crate (including
`src/`, `tests/`, `benches/`, and `examples/` directories where present, with
`target/` pruned).

| Crate | Rust files | Rust lines | Test files |
|---|---|---|---|
| `ragent-agent` | 449 | 170,174 | 280 |
| `ragent-bench` | 34 | 10,844 | 12 |
| `ragent-codeindex` | 82 | 25,573 | 52 |
| `ragent-config` | 78 | 19,661 | 53 |
| `ragent-connectors` | 45 | 19,729 | 22 |
| `ragent-llm` | 98 | 39,119 | 57 |
| `ragent-plugins` | 66 | 23,647 | 39 |
| `ragent-research` | 172 | 65,569 | 105 |
| `ragent-server` | 27 | 11,205 | 18 |
| `ragent-specs` | 38 | 20,640 | 25 |
| `ragent-storage` | 57 | 18,979 | 52 |
| `ragent-surface` | 19 | 2,475 | 9 |
| `ragent-telemetry` | 42 | 14,383 | 31 |
| `ragent-tools-core` | 76 | 22,024 | 38 |
| `ragent-tools-extended` | 164 | 62,300 | 87 |
| `ragent-tools-vcs` | 75 | 18,121 | 34 |
| `ragent-tui` | 316 | 181,376 | 252 |
| `ragent-types` | 71 | 15,597 | 45 |
| **Total** | **1,909** | **741,416** | **1,209** |

---

## Crate Size Distribution

```

ragent-tui            ############  181,376 lines (24.5%)
ragent-agent          ###########   170,174 lines (23.0%)
ragent-research       ####          65,569 lines (8.8%)
ragent-tools-extended ####          62,300 lines (8.4%)
ragent-llm            ##            39,119 lines (5.3%)
ragent-codeindex      #             25,573 lines (3.5%)
ragent-plugins        #             23,647 lines (3.2%)
ragent-tools-core     #             22,024 lines (3.0%)
ragent-specs          #             20,640 lines (2.8%)
ragent-connectors     #             19,729 lines (2.7%)
ragent-config         #             19,661 lines (2.7%)
ragent-storage        #             18,979 lines (2.6%)
ragent-tools-vcs      #             18,121 lines (2.4%)
ragent-types          #             15,597 lines (2.1%)
ragent-telemetry      #             14,383 lines (1.9%)
ragent-server         #             11,205 lines (1.5%)
ragent-bench          #             10,844 lines (1.5%)
ragent-surface        #             2,475 lines (0.3%)
```

---

## Test Distribution

| Crate | Test Files | Approx. Tests |
|-------|-----------:|--------------:|
| `ragent-tui` | 252 | ~3,952 |
| `ragent-agent` | 280 | ~2,580 |
| `ragent-tools-extended` | 87 | ~1,893 |
| `ragent-research` | 103 | ~1,184 |
| `ragent-specs` | 25 | ~688 |
| `ragent-llm` | 57 | ~665 |
| `ragent-plugins` | 39 | ~494 |
| `ragent-tools-vcs` | 34 | ~483 |
| `ragent-codeindex` | 52 | ~458 |
| `ragent-config` | 53 | ~444 |
| `ragent-types` | 45 | ~429 |
| `ragent-tools-core` | 38 | ~422 |
| `ragent-storage` | 52 | ~371 |
| `ragent-connectors` | 22 | ~369 |
| `ragent-telemetry` | 31 | ~333 |
| `ragent-server` | 18 | ~209 |
| `ragent-bench` | 12 | ~146 |
| `ragent-surface` | 9 | ~70 |
| **Total** | **1,209** | **~15,190** |

The per-crate test attributes above include both the external `tests/` suites
(included as child modules of each crate's single `tests/all.rs` target) and the
inline `#[cfg(test)]` modules in library sources (`crates/` totals 15,190; the
root `src/`+`tests/` tree adds 67, for 15,257 workspace-wide). The dated
research-output pass adds the four new `io_tests` cases that pin the date
predicate, the picker and the creation-date canonical path. The
v1.0.137 test-coverage campaign added 170 new external test files (roughly 80 in
`ragent-agent` plus suites across 12 further crates) and a set of inline test
files, moving 13 of the 18 crates past 85% line coverage; the follow-up
`ragent-tui` deep-coverage work adds 17 more files (642 tests, ~16.1k lines) on
top of the phase-1 TUI suites, and the `agentui` line adds about 20
more TUI suites (roughly 900 tests) plus the suspension, write-target, and
recovery suites in `ragent-agent` and `ragent-config`. The keyboard-first rework
added the `test_agents_teams_cursor` (17 cases) and `test_output_view_footer`
suites and rewrote the window, hit-rect, keyboard-control, row-action, and wheel
suites around the block cursor; the setup-dialog chrome work adds
`test_setup_dialog_chrome` (14 cases) and `test_setup_selector_scroll` (7 cases),
and the connector-catalogue rebuild grows `test_connector_catalogue_*` to 54
cases across the panel, mouse and acceptance suites.

`BUILDSTATS.md` records that a full `cargo test --workspace --all-targets` run
executes **15,240 passed / 0 failed / 16 ignored** across 40 reporting test
targets (the 19 consolidated `tests/all.rs` integration targets, 18 crate
`src/lib.rs` targets, the 1 root `src/main.rs` target and the doc-test targets
`--all-targets` reports), measured on the 2026-11-02 1.0.161
tree against the v1.0.138 baseline of 13,862 passed. Note this is the
single-process `cargo test` shape that exposed the three isolation defects the
same report documents; the CI runner uses `cargo nextest`, which runs every test
in its own process.

---

## Test Coverage

Measured with `cargo llvm-cov --workspace --all-targets --no-fail-fast` (line
coverage, test/bench/example sources excluded). Reproduce with:

```bash
cargo llvm-cov --workspace --all-targets --no-fail-fast \
  --json --output-path target/temp/cov-full.json
```

The figures below were **re-measured 2026-11-01** against the current working
tree (`BUILDSTATS.md`), with `--ignore-filename-regex
'(^|/)(tests|benches|examples)/|target/'` so test, bench and example sources are
excluded from both numerator and denominator.

**Workspace line coverage: 69.8% -> 73.9% -> 86.58% -> 86.7%** across the
campaign, the deep-coverage follow-up and this re-measurement. The instrumented
line total is now 168,392 (up from 158,779 because the tree grew), of which
146,001 are executed. The pre-campaign baseline was 158,446 instrumented lines,
110,596 executed, 47,850 missed. The prior v1.0.95 measurement was 68.9% line /
67.8% function, but it predated the `ragent-plugins`, `ragent-connectors`, and
`ragent-surface` crates, so the totals are not directly comparable.

The `2026-11-01` column is the current figure; it moves by tenths of a point
rather than points because the intervening releases added production code
(the OTLP log pipeline, the `/db` retention surface, the automation service,
the dated research naming) alongside its tests.

| Crate | Baseline | After phase 1 | 2026-10-18 | 2026-11-01 |
|---|---:|---:|---:|---:|
| `ragent-plugins` | 90.2% | 90.3% | 90.27% | 90.34% |
| `ragent-types` | 82.2% | 96.3% | 96.24% | 96.22% |
| `ragent-storage` | 81.1% | 91.5% | 91.59% | 92.01% |
| `ragent-config` | 75.4% | 90.5% | 92.21% | 91.14% |
| `ragent-surface` | 70.2% | 90.9% | 90.89% | 90.37% |
| `ragent-specs` | 88.1% | 88.1% | 90.11% | 90.18% |
| `ragent-codeindex` | 83.0% | 88.8% | 89.31% | 89.32% |
| `ragent-server` | 67.4% | 88.2% | 88.07% | 88.62% |
| `ragent-telemetry` | 74.9% | 92.0% | 92.04% | 88.60% |
| `ragent-tools-vcs` | 75.4% | 86.9% | 87.58% | 87.53% |
| `ragent-llm` | 70.4% | 86.1% | 86.11% | 87.33% |
| `ragent-tui` | 54.1% | 75.0% | 86.77% | 87.28% |
| `ragent-agent` | 65.1% | 85.6% | 86.73% | 86.74% |
| `ragent-connectors` | 85.9% | 85.9% | 86.74% | 86.46% |
| `ragent-tools-core` | 73.2% | 85.7% | 86.99% | 87.09% |
| `ragent-tools-extended` | 79.1% | 86.0% | 86.27% | 86.31% |
| `ragent-bench` | 75.2% | 86.0% | 86.01% | 86.01% |
| `ragent-research` | 83.2% | 85.8% | 86.20% | 85.96% |
| root bin (`src/`) | 31.3% | ~31% | 31.94% | 32.74% |
| **Workspace** | **69.8%** | **73.9%** | **86.58%** | **86.70%** |

**All 18 crates are above 85% line coverage** except the root binary, which is
the CLI presentation surface and is exercised through subprocess tests the
profiler does not see. `ragent-telemetry` reads 88.60% (down from 92.04%)
because the OTLP log pipeline is a second exporter whose error paths need a
failing collector to reach. The largest single gain remains
`ragent-tui` (75.14% -> 87.28%, +4,788 covered lines in the first step) from the
17 deep-coverage suites. `ragent-llm` (+1.22 pts) and `ragent-surface` (+0.9 pts
in the first step) are the next largest movers.

Notes:

- The campaign covers 15 of the 18 crates with new or expanded suites. Tests
  only: no production logic changed (three `pub(crate)` items in
  `ragent-tools-vcs` - `run_status`, `filter_log_text`, and the GitLab
  `make_client`/`detect_project`/`detect` helpers - were widened to `pub` so
  their helpers could be exercised directly).
- The workspace total folds in all 18 crates, including `ragent-plugins` (added
  v1.0.112), `ragent-connectors` (added v1.0.123), and `ragent-surface` (added
  v1.0.129) that the pre-v1.0.95 measurement predated.
- Line counts here are executable lines under LLVM profiling, which is smaller
  than the raw `wc -l` figures in the crate table above (declarations, blank
  and comment-only lines are not instrumented).
- The deep-coverage follow-up closed most of the TUI gap: the 17 suites drive
  `app/slash.rs`, `app/event_handler.rs`, `layout.rs`,
  `widgets/message_widget.rs` and `input.rs` through public entry points,
  lifting `ragent-tui` from 75.14% to 87.28%.
- The largest remaining gap is the agent session processor's live-LLM paths -
  the parts that require a real provider stream to exercise.
- Root binary sources (`src/cli.rs`, `src/main.rs`, `src/plugins.rs`,
  `src/connectors.rs`, `src/panic_hook.rs`) remain lightly covered (32.74%)
  because `cargo llvm-cov` does not drive the interactive TUI.
- `ragent-research` slipped 0.24 pts to 85.96% in the 2026-11-01 re-measurement
  even though it gained coverage: the dated-naming resolution helpers add
  branches (a `try_exists` fast path, a directory scan, an error arm) that the
  new tests reach but cannot exercise exhaustively, so both numerator and
  denominator grew, the latter slightly faster.

---

## Key Architecture Ratios

- Test-to-code ratio: ~1 test per 49 lines (15,257 tests / 748,115 lines)
- Largest crate: `ragent-tui` (181,376 lines, 24.5%)
- Smallest crate: `ragent-surface` (2,475 lines, 0.3%)
- Median crate size: ~20,188 lines (between `ragent-config` and `ragent-connectors`)
- Crates over 10k lines: 17 of 18
- Crates under 5k lines: 1 of 18

Function definitions by crate (lexical lower bounds, 2026-10-21 tree):

```
ragent-tui             ############# 6,054 fns (22.8%)
ragent-agent           ########## 4,630 fns (17.4%)
ragent-tools-extended  ###### 2,956 fns (11.1%)
ragent-research        ##### 2,513 fns (9.4%)
ragent-codeindex       ## 1,130 fns (4.2%)
ragent-llm             ## 1,102 fns (4.1%)
ragent-tools-core      ## 1,001 fns (3.8%)
ragent-plugins         # 991 fns (3.7%)
ragent-specs           # 981 fns (3.7%)
ragent-tools-vcs       # 941 fns (3.5%)
ragent-connectors      # 907 fns (3.4%)
ragent-config          # 787 fns (3.0%)
ragent-types           # 590 fns (2.2%)
ragent-storage         # 585 fns (2.2%)
ragent-telemetry       # 550 fns (2.1%)
ragent-bench           # 412 fns (1.5%)
ragent-server          # 363 fns (1.4%)
ragent (root binary)   # 159 fns (0.6%)
ragent-surface         # 114 fns (0.4%)
```

Test share: 17,667 of 26,768 definitions (66%) live under `tests/`; the
most test-dominant crates by ratio are `ragent-tui` (82%), `ragent-tools-extended`
(73%) and `ragent-specs` (73%).

---

_Generated 2026-11-02 (v1.0.161 tree: dated research output
files (`RESEARCH-<YYYY-MM-DD>.md` / `CORPA-<YYYY-MM-DD>.md`, with the legacy
undated names still resolved), the `execute_run` in-flight-marker ordering fix,
the in-process halves of the two test-isolation exclusions,
`docs/howtos/skills.md`, and the first `BUILDSTATS.md` measurement - on top of
the v1.0.160 OTLP log export and session-message retention, the v1.0.153 team
state-machine tightening, the v1.0.152 `/research --age` work and the v1.0.149
`agentui` keyboard-first rework; see the CHANGELOG for the full line). The
previous generation note is retained below for the tree it measured: the
Agents/Teams block cursor replacing mouse row clicks (FR-033..FR-046,
FR-065, FR-069..FR-070; click-to-select retained), the `Esc`-only dismissal with
the FR-077 footer key legend replacing the Close bar button (FR-007, FR-018,
FR-032, FR-074/FR-075), the 70%-width Agents modal, the
`/team forcecleanup` shutdown race fix, the tool watchdog raised to 5000 s, the
`/provider`+`/model` setup-dialog catalogue chrome with cursor-windowed scrolling,
the `/connectors claude` rebuild on the shared plugin-store design language
(FR-047; `Installed` pseudo-category, marketplace navigator, three focusable
panes), the process-global test-isolation guard
(`scripts/check-test-env-isolation.sh` plus the per-crate `global_env_lock`), and
the two `rustls` advisory bumps clearing RUSTSEC-2026-0382/0383. On top
of the v1.0.142/1.0.143-1.0.146 Windows portability releases
(the 32 MiB main-thread stack, `same-file` containment, cfg-gated libc twins,
re-enabled Windows CI), the Agent and Team Member panel per spec `agentui`
(the Agents and Teams popup
windows, the teammate strip, row actions with confirmation modals, cooperative
sub-agent suspension, and the live event plumbing), the widened text-format
tool-call recovery (a fifth dialect family plus reasoning-channel scanning and
the transcript narration scrub), `/yolo on|off|status` with the `--config`
write-target fix, the mailbox sender-session fix (SEC-ragent-team-003), and the
Windows-safe `same-file` path-alias check. On top of the v1.0.138 test-consolidation rollout - one
`tests/all.rs` integration target per crate, 19 binaries instead of
~948, with every test-name set verified identical to the pre-change set; the
test-coverage campaign -
roughly 80 new `tests/` suites raising 13 of 18 crates to >=85% line coverage,
plus 17 `ragent-tui` deep-coverage suites (642 tests) -
on top of the interactive `/plugins list` panel and
the manifest/runtime tool merge fix, on top of the v1.0.135
`docs/reports/security-audit.md` critical/high fix set:
`Config::merge_project` now drops a project overlay's `mcp`, `acp`, and provider
`api` blocks (SEC-001/002/005), the new `ragent_types::guard`
`reject_symlink_components`/`reject_symlink_escape`/`open_owner_only` guards close
the symlink escapes across the specs/config/crash/askpass/TUI image paths
(SEC-006/018/027/028/059), the bounded-HTTP read caps and redirect limits land in
the MCP/masterfetch/GitHub/open-access clients (SEC-014/032/033/036/042/065/066),
the binary updater requires a verified SHA-256 digest (SEC-043), the
`http_request` header allow-list replaces the deny-list (SEC-035), the plugin
host-call sinks and the connector command/env boundaries are bounded
(SEC-009/031/071), and the shared `strutil`/`sanitize`/`store` helpers are routed
through every call site - plus the `ragent --version` flag and the code-audit
Quick Wins on top of the
v1.0.134 MCP-discovery ergonomics, `/simplify all` working
tree - the four new `ragent-surface` shared modules (`config_splice`, `marketplace`,
`net`, `scan`) and the delegation of the plugin/connector marketplace writers to
`write_atomic_json`, the collapse of the four redacting `Debug` impls into
`macro_rules! redacted_debug_impl!`, the `redact_secrets_cow` allocation fix, the
`check_path_within_allowed_roots_cached` empty-roots fast path, and the TUI
`now_unix_ms` / `resolved_default_index` / `resolve_default_index` / `format_mmss`
de-duplication (with the question-expiry `unreachable!()` removed) - on top of the
v1.0.133 `/plugins` browse-panel refinements (reversed-video navigator titles above
each `ALL` row, hit-test areas moved below the header, and per-row `Name (N)` counts), the
bottom-edge search box with a click-to-clear button, and the `git+<url>#<ref>` install
checkout fix in `ragent-plugins` (`plugins` FR-024), on top of the v1.0.132 merged
connector and plugin marketplace browsers (additional marketplaces, a per-row
marketplace column, and off-loop description/version enrichment), the connector and
plugin-store alphabetical id sorts (`connectors` FR-046, `pluginstores` FR-050), the
connector-catalogue pinned heading row (`connectors` FR-045), and the plugin-store
navigator tag-row withdrawal (`catnav` FR-021) - on top of the v1.0.130 release line. The v1.0.130 set adds the OpenHands parity
delta on top of the release line: pluggable execution backends with a
health-visible registry and a TUI `/backend` switcher, sandbox secret injection,
an ACP client and a feature-gated ACP server, an OpenAI-compatible inbound surface
plus a generated OpenAPI 3.1 contract and HTTP MCP transport, the automation
service with webhook ingress, scheduler, run history, and backend confinement,
AgentSkills (`.agents/skills/`) discovery, and an LLM security analyzer; the
plugin-store category navigator (`catnav`); and the `browser` CDP tool and its
`browser` visibility switch removed, taking the registered count 152 -> 151 across
22 categories. **Dependency and standards work** on this tree: the `ragent-agent`,
`ragent-server`, and `ragent-tools-extended` manifests gain the new modules'
dependencies and the workspace gains the opt-in `acp-server` / `headless-browser`
features. **OpenHands quality gates** (spec `openhands` T-019): the engine-gated
`mf_screenshot` capability is registered only when a drivable headless browser is
present, so the default build advertises 148 tools across 21 categories. **Prior
release line.** Builds on v1.0.129: the `docs/plans/code-audit.md`
remediation plan through M9. Dependency upkeep
(M9): the yanked `yoke-derive` is cleared; all crates converge on `thiserror 2`,
`reqwest 0.13`, `rand 0.10`, and `criterion 0.8`; `ragent-storage` moves from
`rand 0.8` to `0.10`; `lopdf` converges on `0.44` and the dedicated `vendor/lopdf`
crate is deleted; `opentelemetry` 0.29 -> 0.33 (`experimental_metrics_custom_reader`
for the Prometheus scrape endpoint); `notify` 8.2, `rquickjs` 0.14, `tree-sitter`
0.27, `chacha20poly1305` 0.11, `which` 8. Dead-code removal (M5): the stale
`ragent-agent::snapshot` module and the duplicated inline schema test suite are
deleted, placeholder tests gain real assertions, the dead connector `NeverProbe`/
`NoProbe` fallbacks go, and the new 18th `ragent-surface` crate owns the shared
`/plugins`/`/connectors` surface glue. Test hygiene (M6): scratch paths move to
`target/temp`, `TempTree` support modules replace 28 copy-pasted sandboxes,
diagnostic and live-network tests are `#[ignore]`-gated, the config-key test
triples collapse to one table-driven test, and four test files are renamed to the
`test_<component>_<scenario>` convention. New coverage (M7): the calculator, the
small tools (`agent_complete`, `bash_reset`, `xlsx`, `get_env`), the Azure AI
Foundry provider, the agent-loop step harness, and the orchestrator `Coordinator`
all gain `tests/` suites (fixing a real `active_jobs` underflow). Standards (M8):
GitHub/GitLab acronym casing, the three user-facing unwraps, emoji/box-drawing
stripping, `//!` module headers, the `MultipleAdapter` rename, and the
source-scraping test rewrite. Security (M1): `.gitignore` gains the SQLite
sidecar and certificate/credential patterns, and a tracked `.env.example`
template lands. Also fixes the config-write comparison folding a corrupt file to
`Null`. Builds on
v1.0.128: the `ragent.json` write path lands in a uniquely named temp file,
`fsync`-ed and renamed over the target, so an interrupted write can no longer
silently reset a persisted YOLO flag; runtime-flag persistence edits only the
single top-level key in the raw global file (`Config::set_global_bool_key`); every
MCP connect runs under a bounded per-attempt timeout with a single timeout retry;
the TUI startup no longer blocks on the MCP connect loop; MCP connect failures are
recorded as `McpStatus::Failed` and reported instead of being dropped; the
`ragent_info` tool reports runtime execution details (pid, uptime, memory, thread
count) alongside build metadata; runtime-flag writes and the load-time state are
logged with attribution; and `/spec reverse --folder` scaffolds before the token
gate. Builds on
v1.0.126: the Office / LibreOffice document
tools (`office_read/write/info`, `libre_read/write/info`) and their
`office_common` / `libreoffice_*` modules removed (only the PDF family
`pdf_read` / `pdf_write` remains, sharing a new `pdf_common` helper), the
`tool_visibility.office` switch and the OOXML/ODF `DocumentFormat` variants
dropped, and a `/simplify all` pass over the changed set (pdf_write image-path
containment, single-parse pdf_read, single-pass connector lookup). Builds on
v1.0.125: the
`os_info` host-introspection tool (GPU adapters, graphics-API versions, physical
hardware) and its `/osinfo show [--no-probe]` slash command, the research
`--papers` default flip (scholarly engines now excluded by default), and three
new tracked research items (`codemigrate`, `connectors`, `vendormarketplace`) -
alongside earlier `/memory clear`, the session-start connector lifecycle, the
MCP orphan sweep, and the committed v1.0.122 security and anti-pattern
remediation sweep folding in the `ANTIPAT.md` M0 plus M2-M7 and `SECTASKS.md`
MS-05 work - `ragent-team` shim crate removed - and the `ragent_types::guard`
shared-guard module added. Earlier v1.0.121 work: rollback-capture
remove-after-restore race fix (CI flake
`test_rollback_accept_restores_snapshot`), OpenSkills `.agents` discovery, Claude
marketplace `*-lsp` inline-manifest materialisation, conventional `skills/`
directory bridging, plus a `/simplify all` pass over the 50-file changed set. Tool
count is 151 registered (the `browser` CDP tool and its `browser` visibility switch
removed this cycle; 6 Office/LibreOffice document tools removed in v1.0.126; 8 finance tools and 6 plot tools removed post v1.0.125; `os_info` added in v1.0.125; `tool_info` +
`commands_info` added in v1.0.120).)_
