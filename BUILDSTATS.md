# Build Statistics

Measured 2026-10-18 on TimWorkFed01.local: step timings at 08:17 UTC, test totals
and coverage re-measured at 11:27 UTC against the final working tree.

- `cargo 1.99.1 (5f94df478 2026-08-27)`
- `rustc 1.99.1 (58cf8f9c3 2026-10-13)`
- Filesystem: `/dev/nvme0n1p3`, mount `/home`
- CPU: 8 logical cores

## Step timings and disk usage

| Step     | Command                                      | Time   | Disk consumed    | Disk free before | Disk free after |
| -------- | -------------------------------------------- | ------ | ---------------- | ---------------- | --------------- |
| clean    | `cargo clean`                              | 9s     | freed 326.09 GiB | 809.95 GiB       | 1136.05 GiB     |
| release  | `cargo build --release --workspace`        | 7m 37s | 1.80 GiB         | 1136.99 GiB      | 1135.20 GiB     |
| debug    | `cargo build --workspace`                  | 2m 41s | 2.40 GiB         | 1135.20 GiB      | 1132.80 GiB     |
| tests    | `cargo test --workspace --all-targets`     | 9m 12s | 14.92 GiB        | 1132.80 GiB      | 1117.88 GiB     |
| coverage | `cargo llvm-cov --workspace --all-targets` | 9m 58s | 15.82 GiB        | 1117.88 GiB      | 1102.06 GiB     |

Each step was measured with free-space probes immediately before and after its command.
`cargo clean` frees space, so its delta is reported as space freed (it also removed the
pre-existing `target/` tree from earlier work, hence the large figure).

The step timings are from the 08:17 UTC run. The **Test totals** and **Test coverage**
sections below were re-measured later on 2026-10-18 (11:27 UTC) against the tree that
includes the last 17 `ragent-tui` deep-coverage suites, so the `tests` and `coverage`
rows no longer describe that tree: the tree now carries 633 more passing tests
(13,862 vs 13,229) and the coverage run instruments an 18 GiB `target/cov-full` tree.

## Test totals

`cargo test --workspace --all-targets` (56 targets built, 40 of them reporting a
result line): **13862 passed, 0 failed, 16 ignored**.
Note: `--all-targets` excludes doctests; CI additionally runs `cargo test --workspace`,
which includes doctests, so CI's total is marginally higher.

The 56 built targets are 19 consolidated `tests/all.rs` integration targets,
18 crate `src/lib.rs` unit-test targets, the root `src/main.rs` target, 17
`benches/` targets and 1 `examples/` target. The 40 that report a
`test result:` line are 19 `all.rs` (11,706 tests), 18 `lib.rs` (2,133 tests),
`src/main.rs` (23 tests), one bench and one example (0 tests each); the other
16 bench targets are criterion harnesses that print `Testing ... Success`
instead of a result line.

```
     Running unittests src/main.rs
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/all.rs
test result: ok. 44 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 31.86s
     Running unittests src/lib.rs
test result: ok. 361 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.01s
     Running tests/all.rs
test result: ok. 1605 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 102.76s
     Running benches/hot_paths.rs
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs
test result: ok. 47 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/all.rs
test result: ok. 98 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.42s
     Running unittests src/lib.rs
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/all.rs
test result: ok. 447 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 2.36s
     Running unittests src/lib.rs
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/all.rs
test result: ok. 384 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
     Running unittests src/lib.rs
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/all.rs
test result: ok. 361 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
     Running unittests src/lib.rs
test result: ok. 323 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running tests/all.rs
test result: ok. 195 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.52s
     Running unittests src/lib.rs
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/all.rs
test result: ok. 492 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.39s
     Running unittests src/lib.rs
test result: ok. 681 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.80s
     Running tests/all.rs
test result: ok. 471 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.39s
     Running unittests src/lib.rs
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/all.rs
test result: ok. 192 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.63s
     Running unittests src/lib.rs
test result: ok. 250 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/all.rs
test result: ok. 427 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running unittests src/lib.rs
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/all.rs
test result: ok. 346 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.59s
     Running unittests src/lib.rs
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/all.rs
test result: ok. 70 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running unittests src/lib.rs
test result: ok. 62 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
     Running tests/all.rs
test result: ok. 231 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.28s
     Running unittests src/lib.rs
test result: ok. 67 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.01s
     Running tests/all.rs
test result: ok. 394 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.01s
     Running unittests src/lib.rs
test result: ok. 88 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 30.13s
     Running tests/all.rs
test result: ok. 1806 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 31.60s
     Running unittests src/lib.rs
test result: ok. 86 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/all.rs
test result: ok. 397 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.19s
     Running unittests src/lib.rs
test result: ok. 142 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.28s
     Running tests/all.rs
test result: ok. 3352 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 67.18s
     Running unittests examples/toolchain_tc_pass.rs
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/all.rs
test result: ok. 394 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.30s
```

## Overall line coverage

`cargo llvm-cov --workspace --all-targets --no-fail-fast --json --output-path target/temp/cov-full.json`

Re-measured 2026-10-18 11:27 UTC against the current working tree (the
test-consolidation rollout plus the `ragent-tui` deep-coverage suites):

```
lines          137471/158779 = 86.58%
functions      13196/15727   = 83.91%
regions        196322/228846 = 85.79%
```

Method: the JSON report aggregates every file in `.data[0].files` (test, bench and
example sources excluded), lines are grouped by `crates/<name>/` directory, and
everything else (the root `src/` binary) is labelled `ragent(bin)`. This is the
whole-workspace instrumented figure in one run, so per-crate numbers include
cross-crate inlining effects and can differ slightly from measuring a crate in
isolation. The instrumented line total (158,779) is unchanged from the previous
measurement, so the rise from 83.41% is entirely newly executed code.

## Per-crate line coverage

| Crate                 | Lines covered | Total lines | Coverage |
| --------------------- | ------------- | ----------- | -------- |
| ragent(bin)           | 762           | 2386        | 31.94%   |
| ragent-agent          | 27388         | 31578       | 86.73%   |
| ragent-bench          | 3683          | 4282        | 86.01%   |
| ragent-codeindex      | 7625          | 8538        | 89.31%   |
| ragent-config         | 2769          | 3003        | 92.21%   |
| ragent-connectors     | 3774          | 4351        | 86.74%   |
| ragent-llm            | 6042          | 7017        | 86.11%   |
| ragent-plugins        | 4815          | 5334        | 90.27%   |
| ragent-research       | 14602         | 16939       | 86.20%   |
| ragent-server         | 2606          | 2959        | 88.07%   |
| ragent-specs          | 3810          | 4228        | 90.11%   |
| ragent-storage        | 3681          | 4019        | 91.59%   |
| ragent-surface        | 409           | 450         | 90.89%   |
| ragent-telemetry      | 1342          | 1458        | 92.04%   |
| ragent-tools-core     | 4521          | 5197        | 86.99%   |
| ragent-tools-extended | 9149          | 10605       | 86.27%   |
| ragent-tools-vcs      | 3005          | 3431        | 87.58%   |
| ragent-tui            | 35723         | 41170       | 86.77%   |
| ragent-types          | 1765          | 1834        | 96.24%   |

All 18 crates are now above 85% line coverage. `ragent-tui` is the largest gain
(75.14% -> 86.77%, +4,788 lines) from the 17 deep-coverage suites, followed by
`ragent-specs` (88.2% -> 90.11%) and `ragent-config` (91.41% -> 92.21%). The root
`src/` binary remains at 31.94% because `cargo llvm-cov` cannot drive the
interactive TUI entry points.
