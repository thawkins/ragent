# Benchmarking Guide

This guide explains how to use ragent's built-in benchmarking system from the TUI.

The benchmark runner lets you evaluate the **currently selected provider/model** against a set of
supported coding benchmarks, write normalized Excel workbooks, and compare results across suites
without changing output format between runs.

---

## 1. What the benchmark system does

The `/bench` command family provides a local benchmark workflow with:

- benchmark data initialization under `benches/data/<suite>/<language>/`
- background benchmark execution from the TUI
- one workbook per concrete benchmark suite
- a fixed workbook schema across all suites
- resume support for matching same-day reruns
- status, cancel, and "open last" support for background runs

Each benchmark run uses the **currently selected model** in the TUI. If you want to benchmark a
different provider or model, change the selected model first, then run `/bench`.

---

## 2. Core concepts

### 2.1 Suites

A **suite** is one benchmark, such as `humaneval` or `swebench-lite`.

### 2.2 Profiles

A **profile** is a named group of suites:

| Profile | Suites | Requires `--yes` |
| --- | --- | --- |
| `quick` | `humaneval`, `mbpp` | No |
| `standard` | `humaneval`, `mbpp`, `ds1000`, `repobench`, `crosscodeeval` | No |
| `agentic` | `swebench-lite`, `livecodebench` | Yes |

Profiles can be used with `/bench run`, but **not** with `/bench init`.

### 2.3 Virtual target: `all`

`all` is a virtual target that expands to **every registered benchmark suite**.

- `/bench init all` initializes data roots for every known suite
- `/bench run all --yes` runs every known suite

Because `all` includes expensive suites, it always requires `--yes` for execution.

### 2.4 Virtual target: `full`

`full` is a virtual init-only target for **complete upstream dataset ingestion across every
registered suite**.

```text
/bench init full
```

Today it is intentionally **gated** until every suite has a real full-data initializer. That keeps
the command honest: it should only succeed once the system can actually ingest complete upstream
benchmark data for the entire suite registry.

For now, the first implemented full-data suite initializers are:

- `humaneval`
- `mbpp`

Use suite-level full mode like this:

```text
/bench init humaneval --full
/bench init humaneval --full --language rust
/bench init mbpp --full
```

### 2.5 Data roots

Before you can run a suite, its local benchmark data must exist under:

```text
benches/data/<suite>/<language>/
```

Each suite has a default language. If you omit `--language`, both `/bench init` and `/bench run`
use that default automatically. Each initialized language partition includes a manifest and
normalized dataset fixtures used by `ragent-bench`.

### 2.6 Result workbooks

Each concrete suite writes one workbook to:

```text
benches/<suite>/<language>/<YYYY-MM-DD UTC>/<provider>/<model>.xlsx
```

Example:

```text
benches/humaneval/python/2026-05-01/anthropic/claude-sonnet-4-20250514.xlsx
```

If you run a profile or `all`, you get **multiple workbook files** - one per expanded suite.

---

## 3. Command reference

### 3.1 List supported targets

```text
/bench list
```

Shows:

- all registered suites
- the virtual `all` and `full` targets
- all named profiles

### 3.2 Show benchmark defaults

```text
/bench show
```

Shows:

- the currently selected model
- the built-in profiles
- the virtual `all` and `full` targets
- the last workbook path(s), if available

### 3.3 Initialize benchmark data

```text
/bench init <suite-or-all-or-full> [--full] [--language LANG] [--force-download] [--verify-only]
```

Examples:

```text
/bench init humaneval
/bench init humaneval --full
/bench init multipl-e --language rust
/bench init bigcodebench
/bench init all
/bench init full
/bench init humaneval --verify-only
/bench init swebench-lite --force-download
```

Behavior:

- creates or refreshes `benches/data/<suite>/<language>/`
- verifies the suite manifest and tracked files
- reports revision and case count
- supports `all` and `full`, but not profiles

Flags:

| Flag | Meaning |
| --- | --- |
| `--full` | Download and normalize the full upstream dataset for suites that support it |
| `--language <lang>` | Initialize a specific language partition; in full mode, omit it to pull every supported upstream language partition for that suite |
| `--force-download` | Rebuild the initialized data root even if a valid one already exists |
| `--verify-only` | Verify the initialized data root without mutating it |

Initialization modes:

- **Sample mode** is the default and writes local smoke-test fixtures.
- **Full mode** is enabled with `--full` for supported suites.
- **Virtual `full` target** means "run full mode for every suite" and stays gated until every suite
  supports full-data ingestion.

### 3.4 Run benchmarks

```text
/bench run <suite-or-profile-or-all> [flags]
```

Examples:

```text
/bench run humaneval
/bench run quick
/bench run standard --cap 10
/bench run livecodebench --release release_v6 --scenario codegeneration --yes
/bench run multipl-e --language python --samples 5 --yes
/bench run all --yes
```

Runs execute in the background. After starting a run, use `/bench status`, `/bench open last`,
and `/bench cancel` to monitor it.

### 3.5 Show run status

```text
/bench status
```

Shows either:

- the currently active benchmark task, including summary and task ID, or
- the most recent completed benchmark run summary

### 3.6 Show the latest workbook paths

```text
/bench open last
```

Prints the latest workbook path(s) from the most recent completed benchmark run.

### 3.7 Cancel the active run

```text
/bench cancel
```

Requests shutdown of the active background benchmark run.

---

## 4. Run flags

The benchmark runner accepts these options on `/bench run`:

| Flag | Meaning |
| --- | --- |
| `--limit <N>` | Run only the first `N` cases from the suite data |
| `--cap <N>` | Alias for `--limit <N>` |
| `--samples <K>` | Generate `K` samples per case when the suite supports pass@k-style metrics |
| `--subset <name>` | Benchmark-specific subset selector |
| `--release <version>` | Benchmark-specific release/version selector |
| `--scenario <name>` | Benchmark-specific scenario selector |
| `--language <lang>` | Select the language partition to run; omit it to use the suite default |
| `--temperature <F>` | Override model temperature |
| `--top-p <F>` | Override top-p |
| `--max-tokens <N>` | Override completion max tokens |
| `--deterministic` | Force deterministic request settings where supported |
| `--since <YYYY-MM-DD>` | Inclusive lower date bound for time-based suites |
| `--until <YYYY-MM-DD>` | Inclusive upper date bound for time-based suites |
| `--resume` | Reuse a matching same-day workbook if benchmark, model, and config hash match |
| `--no-exec` | Generate outputs only and skip execution/evaluation |
| `--yes` | Required for expensive suites, expensive profiles, and `all` |

### 4.1 When to use `--yes`

The following suites are marked expensive and require `--yes`:

- `apps`
- `multipl-e`
- `swebench-lite`
- `swebench-verified`
- `livecodebench`
- `bigcodebench`

The `agentic` profile and the virtual `all` target also require `--yes`.

### 4.2 What `--resume` actually does

`--resume` is conservative. It only reuses a workbook when the same-day benchmark output matches:

- suite
- selected model/provider
- benchmark date folder
- effective run configuration hash

If any of those differ, a fresh workbook is written instead.

### 4.3 What `--no-exec` does

`--no-exec` still performs prompt construction and generation, but it skips the execution-backed
evaluation step. In workbook output this usually appears as **skipped** metrics rather than normal
pass/fail scoring.

### 4.4 What `--full` does

`--full` applies to `/bench init`, not `/bench run`.

When supported by a suite, it downloads the upstream benchmark data and normalizes it into the
local benchmark layout under `benches/data/<suite>/<language>/`. The current first-wave full-data
suite implementations are `humaneval` and `mbpp`.

---

## 5. Supported suites

The current benchmark system supports these suites:

| Suite | Purpose | Default | Supported languages | Expensive | Typical metrics | Current caveats |
| --- | --- | --- | --- | --- | --- | --- |
| `humaneval` | Function-level generation with HumanEvalPack multilingual coverage | `python` | `cpp`, `go`, `java`, `javascript`, `python`, `rust` | No | `pass_at_1`, `pass_at_k` | Sample fixtures stay Python-only, but `--full` pulls HumanEvalPack from HF `bigcode/humanevalpack`; omitting `--language` initializes every supported language partition |
| `mbpp` | Basic problem solving with BC-MBPP multilingual coverage | `python` | `python`, `cpp`, `csharp`, `dart`, `go`, `haskell`, `java`, `javascript`, `julia`, `kotlin`, `lua`, `php`, `r`, `rust`, `scala`, `typescript` | No | `accuracy` | Sample fixtures stay Python-only, but `--full` pulls BC-MBPP from HF `gabeorlanski/bc-mbpp`; omitting `--language` initializes every supported language partition |
| `apps` | Competitive-programming style generation | `python` | `python` | Yes | `accuracy`, `codebleu` | Heavy target; requires `--yes` |
| `ds1000` | Data-science code generation | `python` | `python` | No | `accuracy` | Current adapter is a local MVP scorer |
| `multipl-e` | Multi-language generation | `python` | `python`, `rust` | Yes | `pass_at_1`, `pass_at_k` | Rust comes from MultiPL-E translations derived from HumanEval/MBPP; native multilingual full-data init now exists for `humaneval` via HumanEvalPack and `mbpp` via BC-MBPP |
| `repobench` | Repository-level completion | `python` | `python` | No | `exact_match`, `edit_similarity` | Native local metrics |
| `crosscodeeval` | Cross-file completion | `python` | `python` | No | `completion_accuracy`, `edit_similarity` | Upstream repo is multilingual; raw language data is distributed separately on request |
| `swebench-lite` | Patch generation for real bug-fix tasks | `diff` | `diff` | Yes | `resolution_rate`, `instances_resolved` | Only `repair` is supported; current resolution is a native proxy |
| `swebench-verified` | Verified SWE-bench subset | `diff` | `diff` | Yes | `resolution_rate`, `instances_resolved` | Only `repair` is supported; current resolution is a native proxy |
| `livecodebench` | Contamination-aware coding evaluation | `python` | `python` | Yes | `pass_at_1`, `scenario_score` | Phase 6 currently supports only `codegeneration` |
| `bigcodebench` | Practical challenging code generation | `python` | `python` | Yes | `pass_at_1`, `pass_at_k`, `codebleu` | Heavy target; requires `--yes` |

### 5.1 Important current limitations

Be aware of these current MVP constraints:

1. `livecodebench` currently supports only the `codegeneration` scenario.
2. `swebench-lite` and `swebench-verified` currently support only the `repair` scenario.
3. Each initialized dataset is partitioned under `benches/data/<suite>/<language>/`; rerun `/bench init` if the requested language partition is missing.
4. `--no-exec` produces skipped metrics for suites that depend on execution.
5. SWE-bench resolution currently uses native patch-shape and diff-similarity heuristics rather
   than full repository materialization plus isolated upstream-style test execution.
6. The virtual `full` target is still gated until every suite has a real full-data initializer.

---

## 6. Workbook layout

Every benchmark workbook uses the same sheet names:

| Sheet | Purpose |
| --- | --- |
| `run` | One row describing the overall benchmark run and model/config metadata |
| `metrics` | Summary metrics for the suite |
| `cases` | Per-case results, hashes, token counts, durations, and errors |
| `artifacts` | Paths and hashes for related benchmark artifacts |

This fixed layout makes results directly comparable across suites.

### 6.1 What you will find in the workbook

- provider/model identity
- thinking configuration
- original benchmark command
- dataset revision
- project root and data root
- per-case status and score
- token counts and durations when available
- suite-level summary metrics

---

## 7. Recommended workflows

### 7.1 Fast smoke test for a selected model

```text
/bench init humaneval
/bench init humaneval --full
/bench init humaneval --full --language rust
/bench init mbpp --full
/bench init mbpp --full --language rust
/bench init mbpp
/bench run quick
/bench status
/bench open last
```

### 7.2 Full local initialization pass

```text
/bench init all
```

Use this once when preparing a project for broader benchmark work.

### 7.3 Pull a full upstream dataset for one supported suite

```text
/bench init humaneval --full
/bench init humaneval --full --language rust
/bench init mbpp --full
/bench init mbpp --full --language rust
```

This stores normalized full benchmark data locally and tracks the raw upstream download in the
suite manifest. `humaneval --full` now initializes every HumanEvalPack language partition, and
`mbpp --full` does the same for BC-MBPP, unless you narrow either command with `--language`.

### 7.4 Run one expensive suite safely

```text
/bench init livecodebench
/bench run livecodebench --release release_v6 --scenario codegeneration --limit 10 --yes
/bench status
```

### 7.5 Resume a same-day interrupted run

```text
/bench run standard --resume
```

If the selected model and effective configuration still match, the runner reuses the existing
workbook instead of writing a new one.

### 7.6 Benchmark everything

```text
/bench init all
/bench run all --yes
/bench status
/bench open last
```

This writes one workbook per suite.

### 7.7 Try the future complete-ingestion target

```text
/bench init full
```

At the moment this returns a clear gating error until every suite supports full-data ingestion.

---

## 8. Troubleshooting

### "Benchmark data for '<suite>' is not initialized"

Initialize the suite first:

```text
/bench init <suite>
```

If you want everything initialized:

```text
/bench init all
```

### "`/bench init full` says it is not ready yet"

That is expected until every suite has a full-data initializer. Use suite-level full mode for the
currently supported suites:

```text
/bench init humaneval --full
/bench init humaneval --full --language rust
/bench init mbpp --full
/bench init mbpp --full --language rust
```

### "This benchmark target requires `--yes`"

Re-run the command with `--yes`. This is required for expensive suites, the `agentic` profile,
and the virtual `all` target.

### A run finished, but metrics are skipped

Common causes:

- you ran with `--no-exec`
- the selected scenario is not supported for that suite
- the selected language does not match available fixture coverage for `multipl-e`

### `/bench init --verify-only` failed

The existing data root is invalid or stale. Rebuild it:

```text
/bench init <suite> --force-download
```

### `/bench open last` shows nothing useful

`/bench open last` only reports completed benchmark workbooks from the current app state. Run a
benchmark first, then retry it.

---

## 9. Best practices

1. Use `/bench show` before large runs to confirm the selected model.
2. Use `quick` for smoke tests and `standard` for repeatable local comparison.
3. Use `--full` only when you need real upstream benchmark data for a suite that supports it.
4. Use `--limit` before attempting expensive suites on a new model.
5. Use `--resume` for interrupted same-day runs.
6. Use `/bench open last` after profile or `all` runs so you can inspect every workbook path.
7. Keep benchmark runs comparable by avoiding unnecessary flag changes between reruns.

---

## 10. Minimal cheat sheet

```text
/bench list
/bench show
/bench init humaneval
/bench init humaneval --full
/bench init all
/bench init full
/bench run quick
/bench run standard --limit 10
/bench run all --yes
/bench status
/bench open last
/bench cancel
```
