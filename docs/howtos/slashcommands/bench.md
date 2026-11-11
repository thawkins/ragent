# /bench

> Benchmark runner: /bench list|init <suite-or-all-or-full>|show|run <target>|status|open last|cancel

## Overview

The `/bench` command drives the ragent benchmark harness (`ragent-bench` crate). The
TUI arm parses its arguments with `ragent_bench::parse_bench_command` and dispatches
the parsed command to the bench runtime; everything below the `/bench` prefix is
handled by the bench crate, not the TUI itself.

The full guide (concepts, every subcommand, run flags, suites, workbook layout,
workflows and troubleshooting) is [docs/userdocs/bench.md](../../userdocs/bench.md).

## Syntax

```
/bench list
/bench init <suite-or-all-or-full> [--full] [--language LANG] [--force-download] [--verify-only]
/bench show
/bench run <suite-or-profile-or-all> [flags]
/bench status
/bench open last
/bench cancel
/bench help
```

- Subcommands: [Command reference](../../userdocs/bench.md#3-command-reference).
- `run` flags (`--limit`/`--cap`, `--samples`, `--subset`, `--release`, `--scenario`,
  `--language`, `--temperature`, `--top-p`, `--max-tokens`, `--deterministic`,
  `--since`, `--until`, `--resume`, `--no-exec`, `--yes`):
  [Run flags](../../userdocs/bench.md#4-run-flags).
- `help` prints the bench usage help.

## Cancellation

`/bench cancel` prints a confirmation line and the status bar shows
`[wait] bench: cancellation requested` while the flag is set. When no run is active,
`cancel` prints "No active benchmark run." instead.

## Related

- `/model`  -  verify the provider and model a bench run will target
- `/telemetry`  -  runtime metric counters recorded during runs
- `cargo bench -p ragent-tui` and friends for criterion micro-benchmarks
