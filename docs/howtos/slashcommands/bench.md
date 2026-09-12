# /bench

> Benchmark runner: /bench list|init <suite-or-all-or-full>|show|run <target>|status|open last|cancel

## Overview

The `/bench` command drives the ragent benchmark harness (`ragent-bench` crate). It
initialises benchmark suites, runs them against configured LLM models, and inspects
results. The TUI arm parses its arguments with `ragent_bench::parse_bench_command`
and dispatches the parsed command to the bench runtime; everything below the
`/bench` prefix is handled by the bench crate, not the TUI itself.

Suites measure tool-calling behaviour, code quality, and end-to-end agent runs.
Results are stored under the bench output directory so later `run` invocations can
resume or compare against previous releases.

## Syntax

```
/bench list
/bench init <suite-or-all-or-full> [--full] [--language LANG] [--force-download] [--verify-only]
/bench show <suite>
/bench run <suite-or-profile-or-all> [flags]
/bench status
/bench open last
/bench cancel
/bench help
```

## Options / Subcommands

| Form | Description |
|------|-------------|
| `list` | List the registered benchmark suites and profiles |
| `init <suite-or-all-or-full>` | Initialise fixtures for a suite, all suites, or the full corpus |
| `init ... --full` | Build the complete (large) fixture set instead of the default sample |
| `init ... --language LANG` | Restrict fixture generation to one language |
| `init ... --force-download` | Re-download fixture inputs even if already cached |
| `init ... --verify-only` | Verify existing fixtures without generating or downloading |
| `run <suite-or-profile-or-all>` | Execute a suite, a named profile, or every suite |
| `run ... --limit N` / `--cap N` | Cap the number of benchmark cases executed |
| `run ... --samples K` | Number of samples per case |
| `run ... --subset NAME` | Run only the named subset |
| `run ... --release VERSION` | Tag the run against a release version |
| `run ... --scenario NAME` | Run one scenario |
| `run ... --language LANG` | Restrict the run to one language |
| `run ... --temperature F` | Override sampling temperature |
| `run ... --top-p F` | Override top-p sampling |
| `run ... --max-tokens N` | Override the completion token budget |
| `run ... --deterministic` | Force deterministic sampling settings |
| `run ... --since YYYY-MM-DD` | Only include data on or after this date |
| `run ... --until YYYY-MM-DD` | Only include data on or before this date |
| `run ... --resume` | Resume a previously interrupted run |
| `run ... --no-exec` | Parse and plan the run without executing tool calls |
| `run ... --yes` | Auto-approve prompts during the run |
| `status` | Show whether a bench run is active and its last known state |
| `open last` | Open the most recent bench result report |
| `cancel` | Request cancellation of the active benchmark run |
| `help` | Print the bench usage help |

## Examples

```
From: /bench list
Lists every benchmark suite with a short description and case count.
```

```
From: /bench init all
Initialises fixtures for every suite. Progress lines report each suite as it
is generated; use --verify-only afterwards to re-check them.
```

```
From: /bench run codebench --language rust --limit 10
Runs ten rust cases from the codebench suite with the current provider and
model.
```

```
From: /bench run all --deterministic --max-tokens 2048 --release 1.0.95
Runs every suite with deterministic sampling, a 2048-token cap, and stores
the result under release 1.0.95 for later comparison.
```

```
From: /bench status
Reports the state of any in-flight run; prints the last run summary when idle.
```

```
From: /bench cancel
Cancellation requested for the active benchmark run.
```

When no run is active, `cancel` prints "No active benchmark run." instead.

## Output

`init` and `run` stream progress into the message window as the harness works:
suite names, case indices, pass/fail outcomes, and aggregate scores. `status`
prints the run state; `open last` shows the stored result report; `cancel`
prints a confirmation line and the status bar shows
`[wait] bench: cancellation requested` while the flag is set.

## Related

- `/models`  -  verify the provider and model a bench run will target
- `/telemetry`  -  runtime metric counters recorded during runs
- `cargo bench -p ragent-tui` and friends for criterion micro-benchmarks
- docs/performance/benchmark-guide.md