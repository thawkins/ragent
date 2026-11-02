# Build statistics

Measured on 2026-11-01 on
`TimWorkFed01.local` (Linux, local NVMe filesystem).

- `cargo 1.99.1 (5f94df478 2026-08-27)`
- `rustc 1.99.1 (58cf8f9c3 2026-10-13)`
- `cargo-llvm-cov` 0.6.x, `jq`, `python3`

Disk figures are free-space deltas on the filesystem holding the repository,
probed immediately before and immediately after each step. A negative delta is
reported as space *freed*.

## Phase timings

| Step | Command | Time | Disk consumed | Disk free before | Disk free after |
|------|---------|------|---------------|------------------|-----------------|
| clean | `cargo clean` | 8s | freed 148.71 GiB | 995.80 GiB | 1144.50 GiB |
| release | `cargo build --release --workspace` | 8m 1s | 1.78 GiB | 1144.51 GiB | 1142.73 GiB |
| debug | `cargo build --workspace` | 2m 22s | 2.37 GiB | 1142.73 GiB | 1140.36 GiB |
| tests-1 (failed) | `cargo test --workspace --all-targets` | 7m 9s | 14.95 GiB | 1140.36 GiB | 1125.41 GiB |
| coverage-1 (failed) | `cargo llvm-cov --workspace --all-targets` | 12m 19s | 15.81 GiB | 1125.41 GiB | 1109.60 GiB |
| tests | `cargo test --workspace --all-targets` | 8m 49s | 0.30 GiB | 1108.74 GiB | 1108.43 GiB |
| coverage | `cargo llvm-cov --workspace --all-targets` | 9m 0s | freed 0.03 GiB | 1108.43 GiB | 1108.46 GiB |

Two rows carry a `-1 (failed)` suffix: the first test and coverage runs hit three
real test-isolation defects (see [Defects fixed during measurement](#defects-fixed-during-measurement)),
so their numbers are the cost of a *failing* run, not of the measured phase. The
unsuffixed `tests` and `coverage` rows are the green re-runs that produced the
coverage figures below.

The second `tests`/`coverage` pair is much cheaper than the first because the
incremental build and the instrumented `target/llvm-cov-target` tree already
existed; the first pair includes the from-scratch debug compile of every test
binary.

## Line coverage

`overall: 146001/168392 lines = 86.7%`

Method: `cargo llvm-cov --workspace --all-targets --no-fail-fast --json`, with
`--ignore-filename-regex '(^|/)(tests|benches|examples)/|target/'`, so test,
bench and example sources are excluded from both numerator and denominator.
Per-crate figures are summed from `.data[0].files` by the `crates/<name>/` path
prefix; everything else is the root binary, labelled `ragent(bin)`.

| Crate | Lines covered | Lines total | Coverage |
|-------|---------------|-------------|----------|
| ragent(bin) | 840 | 2566 | 32.74% |
| ragent-agent | 30610 | 35290 | 86.74% |
| ragent-bench | 3683 | 4282 | 86.01% |
| ragent-codeindex | 7646 | 8560 | 89.32% |
| ragent-config | 2859 | 3137 | 91.14% |
| ragent-connectors | 3852 | 4455 | 86.46% |
| ragent-llm | 6605 | 7563 | 87.33% |
| ragent-plugins | 4752 | 5260 | 90.34% |
| ragent-research | 14973 | 17419 | 85.96% |
| ragent-server | 2679 | 3023 | 88.62% |
| ragent-specs | 3821 | 4237 | 90.18% |
| ragent-storage | 3822 | 4154 | 92.01% |
| ragent-surface | 516 | 571 | 90.37% |
| ragent-telemetry | 1757 | 1983 | 88.6% |
| ragent-tools-core | 4593 | 5274 | 87.09% |
| ragent-tools-extended | 9151 | 10603 | 86.31% |
| ragent-tools-vcs | 2991 | 3417 | 87.53% |
| ragent-tui | 38919 | 44590 | 87.28% |
| ragent-types | 1932 | 2008 | 96.22% |

The root `ragent(bin)` entry is low by construction: `src/main.rs` and its `cli*`
siblings are the presentation surface, exercised through subprocess CLI tests
that run the built binary rather than calling into the library, so llvm-cov sees
little of their code as covered.

## Test totals

`cargo test --workspace --all-targets` (the green run):

```
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 48 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 15.81s
test result: ok. 368 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.00s
test result: ok. 2196 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 137.58s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 47 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 98 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.38s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 451 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 2.37s
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 428 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 369 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
test result: ok. 331 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
test result: ok. 332 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 1.09s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 494 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.44s
test result: ok. 697 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.81s
test result: ok. 481 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.37s
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 198 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.57s
test result: ok. 250 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 432 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 371 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.81s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 70 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 62 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 270 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.90s
test result: ok. 67 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.00s
test result: ok. 402 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.01s
test result: ok. 88 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 30.12s
test result: ok. 1789 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 31.27s
test result: ok. 86 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 397 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.16s
test result: ok. 142 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.26s
test result: ok. 3797 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 74.64s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 427 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.57s
```

Summed over **all 40 test binaries**: **15,240 tests passed, 0 failed, 16 ignored**.

`--all-targets` excludes doctests. CI additionally runs `cargo test --workspace`,
which *includes* doctests, so the CI pass count is higher than this figure by the
workspace's doctest count.

## Defects fixed during measurement

The first full-suite run failed with three genuine test-isolation bugs. None was
a measurement artefact; all three reproduce under `cargo test` (one process per
crate binary, parallel threads) while passing under `cargo nextest` (one process
per test), which is why they had gone unnoticed.

1. **`test_tool_rename_guard` (2 cases).** Most cases now scan a scratch copy of
   the tree, but `--self-test` plants a fixture inside
   `crates/ragent-tools-core/src/`, and `test_the_guard_finishes_in_a_few_seconds`
   scanned the checkout while it was there. Nextest's `tool-rename-guard` group
   orders these across processes; it does nothing inside one process, and the
   file's comment explicitly declined `#[serial]`. Added a file-local
   `guard_lock()` taken by all 16 cases - the in-process half of the exclusion,
   alongside the existing cross-process group.
2. **`test_automation_runtime::successful_run_writes_output_file`.** The case
   polls `get_run` until the outcome is terminal and *then* asserts
   `!is_running`, but `AutomationService::execute_run` cleared the in-flight
   marker only *after* `finalise()` had written the terminal record, so the two
   observations disagreed for the whole of that write. Fixed the ordering in
   `execute_run` (and the session-create error path alongside it): the marker is
   removed before the terminal record is persisted, so `is_running` and the
   stored outcome can never contradict each other. The test's assertion is
   unchanged.
3. **`test_export_health::observed::test_observed_exporter_success_queues_no_notice`.**
   Three cases drive the process-global `export_health()`, clearing it and then
   reading its counters; `ragent-telemetry` declared no crate-wide lock, so a
   sibling's failure interleaved. Added `global_env_mutex()` /
   `global_env_lock()` to `crates/ragent-telemetry/tests/all.rs` and took it in
   all three cases.

`cargo fmt --all --check` passes on the result.
