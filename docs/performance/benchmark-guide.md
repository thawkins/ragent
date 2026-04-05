# Benchmark Guide

This guide describes how to run the `ragent-core` performance benchmarks locally and interpret the results.

---

## Prerequisites

- Rust toolchain (edition 2024 — see workspace `Cargo.toml`)
- `cargo` — no additional tools required for criterion benchmarks

For flame-graph profiling, also install:

```bash
cargo install flamegraph      # requires perf on Linux or DTrace on macOS
```

---

## Running All Benchmarks

```bash
cargo bench -p ragent-core
```

This compiles and runs all five benchmark suites:

| Benchmark file | What it measures |
|---|---|
| `bench_file_ops.rs` | `EditStaging::commit_all` at various concurrency levels |
| `bench_orchestrator.rs` | `Coordinator::new`, job submission overhead, metrics snapshot |
| `bench_snapshot.rs` | `take_snapshot`, `restore_snapshot`, `incremental_save` |
| `bench_team.rs` | Mailbox `push`, `drain_unread`, `read_all` |
| `bench_tools.rs` | `GlobTool::execute` (50 files), `ReadTool::execute` (LRU-cached) |

Criterion stores HTML reports under `target/criterion/`. Open any `index.html` to view interactive charts.

---

## Running a Single Benchmark Suite

```bash
cargo bench -p ragent-core --bench bench_snapshot
```

---

## Running a Single Function

```bash
cargo bench -p ragent-core --bench bench_snapshot -- incremental_save
```

---

## Saving a Baseline

Use `--save-baseline` to record a named baseline for later comparison:

```bash
cargo bench -p ragent-core -- --save-baseline before_change
```

Then after making changes:

```bash
cargo bench -p ragent-core -- --baseline before_change
```

Criterion will report percentage regressions/improvements against the baseline.

---

## Generating a Flame Graph

```bash
# Linux (requires perf with kernel symbols)
cargo flamegraph -p ragent-core --bench bench_tools -- --bench

# The output is saved to flamegraph.svg in the current directory
```

---

## Interpreting Results

Criterion reports three statistics per benchmark:

- **lower bound / estimate / upper bound** — 95 % confidence interval for mean iteration time
- **thrpt** — optional throughput (elements/second), shown when configured with `b.throughput()`
- **change** — percentage change vs. the saved baseline (green = improvement, red = regression)

### Success Criteria (from `docs/plan/update-core.md`)

| Metric | Target |
|---|---|
| Median job submission latency | ≥ 30 % reduction |
| CPU usage (5 agents, 20 tool calls) | ≥ 20 % reduction |

---

## Where Results Are Stored

Criterion writes machine-readable JSON estimates to:

```
target/criterion/<bench_name>/<function_name>/estimates.json
```

These can be consumed by CI scripts to enforce regression budgets.

---

## CI Integration

Benchmarks run on every push via `.github/workflows/ci_benchmarks.yml`. That workflow uses `cargo bench` in non-comparison mode (no baseline) and emits timing data as job summary output. Future work (§6.1) will add automatic regression detection by comparing against a stored baseline artifact.
