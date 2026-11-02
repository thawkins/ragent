---
name: buildstats
description: Measure wall-clock time and disk usage of a clean release build, a debug build, the full workspace test suite and a coverage pass, then write BUILDSTATS.md and clean up.
disable-model-invocation: true
argument-hint: ""
allowed-tools:
  - any
---

# Build statistics

Measure how long each build phase takes and how much disk it consumes, then
write a single `BUILDSTATS.md` report at the repository root.

Run the five steps **in order**. For every step measure the time taken and the
free disk space immediately before and immediately after that step's command.
At the end print one table with a row per step (time taken, disk consumed) and
write that same table to `BUILDSTATS.md`. Finally run `cargo clean` again so the
tree is left as it was found.

## Robustness rules

Each step may run as its own `bash` invocation, so do **not** rely on shell
variables or functions surviving between commands. Use the fixed run directory
`target/buildstats/` and a sourced helper file instead of environment state.

- Create `target/buildstats/` **after** step 1 (`cargo clean` deletes `target/`).
- Every step reads/writes literal paths under `target/buildstats/`, never a
  variable from a previous shell.
- `BUILDSTATS.md` lives at the repo root so it survives the final clean.
- Precondition check (before step 1): `command -v df awk jq cargo-llvm-cov`.
  If `cargo-llvm-cov` is missing, run `cargo install cargo-llvm-cov` first.
- Skill bodies substitute positional placeholders, so never write a dollar
  sign followed by a digit in a command line here: it is replaced with the
  (empty) invocation argument, silently corrupting the command. Use `$(NF-2)`
  for `df` columns, an array slice for `run_step`, and Python/`jq` instead of
  number-indexed `awk`.

## Helper file

After step 1, write `target/buildstats/lib.sh` once with:

```bash
RUN="target/buildstats"
STEPS="$RUN/steps.tsv"

# Free bytes on the filesystem holding the repo. Column 4 of `df -Pk`
# ("Available") is `$(NF-2)`; `tail -1` tolerates a wrapped device name.
avail_bytes() { df -Pk "$PWD" | tail -1 | awk '{print $(NF-2) * 1024}'; }
now_ns()      { date +%s%N; }

# run_step <id> <label> <command...>
# Appends: id, label, seconds, before_bytes, after_bytes, exit_code
run_step() {
  local args=("$@")
  local id="${args[0]}" label="${args[1]}"
  local before t0 t1 after rc
  before=$(avail_bytes); t0=$(now_ns)
  "${args[@]:2}" >"$RUN/$id.log" 2>&1; rc=$?
  t1=$(now_ns); after=$(avail_bytes)
  printf '%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$id" "$label" "$(( (t1 - t0) / 1000000000 ))" \
    "$before" "$after" "$rc" >>"$STEPS"
  return $rc
}
```

Each later step begins with `. target/buildstats/lib.sh` and then calls
`run_step`. Probe `avail_bytes`/`now_ns` immediately before and after the
command; run nothing else inside that window. Let a step finish (success or
failure) before the next; record the real exit code and keep going if one step
fails. Give generous timeouts: build 1000 s, test 1500 s, coverage 3000 s.

## Steps

### 1. `cargo clean`

Measure before the run directory exists: probe free space, clean, probe free
space again, compute the delta inline.

```bash
av() { df -Pk "$PWD" | tail -1 | awk '{print $(NF-2) * 1024}'; }
before=$(av); t0=$(date +%s%N)
cargo clean
after=$(av); t1=$(date +%s%N)
mkdir -p target/buildstats
printf '%s\t%s\t%s\t%s\t%s\t%s\n' \
  clean 'cargo clean' "$(( (t1 - t0) / 1000000000 ))" "$before" "$after" 0 \
  >target/buildstats/steps.tsv
```

`cargo clean` frees space, so its "consumed" value is negative: report it as
space freed.

### 2. Full release build

```bash
. target/buildstats/lib.sh
run_step release 'cargo build --release --workspace' \
  cargo build --release --workspace
```

### 3. Full debug build

```bash
. target/buildstats/lib.sh
run_step debug 'cargo build --workspace' cargo build --workspace
```

### 4. Full workspace test suite (all targets, debug binaries)

```bash
. target/buildstats/lib.sh
run_step tests 'cargo test --workspace --all-targets' \
  cargo test --workspace --all-targets
```

Then capture the totals for the report:

```bash
grep -E "test result:|^running [0-9]+ tests" target/buildstats/tests.log \
  | tail -40 >target/buildstats/tests-summary.txt
cat target/buildstats/tests-summary.txt
```

`--all-targets` excludes doctests. CI also runs `cargo test --workspace`
(which includes doctests); note that difference in the report if the doctest
count matters.

### 5. Coverage (overall and per crate)

Instrumented coverage runs the suite again, so clear stale profdata first, then
export JSON and aggregate it.

```bash
. target/buildstats/lib.sh
coverage_impl() {
  cargo llvm-cov clean --workspace
  cargo llvm-cov --workspace --all-targets --no-fail-fast \
    --ignore-filename-regex '(^|/)(tests|benches|examples)/|target/' \
    --json --output-path "$RUN/coverage.json"
}
run_step coverage 'cargo llvm-cov --workspace --all-targets' coverage_impl
```

Confirm the schema (`jq '.data[0].totals' target/buildstats/coverage.json`)
before aggregating. Lines are grouped by crate directory under `crates/`;
anything else (the root `src/` binary) is labelled `ragent(bin)`.

```bash
jq -r '
  def crate_of: (capture("crates/(?<c>[^/]+)/")?.c) // "ragent(bin)";
  .data[0].files
  | map({c: (.filename | crate_of),
         lc: .summary.lines.count, lv: .summary.lines.covered})
  | group_by(.c)
  | map({crate: .[0].c,
         count: (map(.lc) | add),
         covered: (map(.lv) | add)})
  | map(. + {percent: (if .count > 0 then (.covered * 10000 / .count | round) / 100 else 0 end)})
  | sort_by(.crate)
' target/buildstats/coverage.json >target/buildstats/coverage_by_crate.json

jq -r '.data[0].totals.lines
  | "overall: \(.covered)/\(.count) lines = \((.covered * 10000 / .count | round) / 100)%"' \
  target/buildstats/coverage.json | tee target/buildstats/coverage-overall.txt
```

If the JSON shape differs, inspect `head -c 400 target/buildstats/coverage.json`
and adapt the `jq` paths before failing the step. This filter is stricter than
the workspace figure when a crate is measured alone; state the method used.

## 6. Report

Build the markdown table from the step log with Python (no number-indexed
`awk`, and no placeholders to substitute):

```bash
python3 - >target/buildstats/table.md <<'PY'
import csv

GIB = 1024 ** 3
steps = list(csv.reader(open("target/buildstats/steps.tsv"), delimiter="\t"))
print("| Step | Command | Time | Disk consumed | Disk free before | Disk free after |")
print("|------|---------|------|---------------|------------------|-----------------|")
for sid, label, secs, before, after, _rc in steps:
    secs = int(secs)
    delta = (int(before) - int(after)) / GIB
    note = f"freed {abs(delta):.2f} GiB" if delta < 0 else f"{delta:.2f} GiB"
    t = f"{secs // 60}m {secs % 60}s" if secs >= 60 else f"{secs}s"
    print(f"| {sid} | `{label}` | {t} | {note} | "
          f"{int(before) / GIB:.2f} GiB | {int(after) / GIB:.2f} GiB |")
PY
cat target/buildstats/table.md
```

Then write `BUILDSTATS.md` at the repo root containing: a short header (date,
host, `cargo -V`/`rustc -V`), that table, the overall line coverage from
`coverage-overall.txt`, the per-crate coverage table rendered from
`coverage_by_crate.json`, and the test totals from `tests-summary.txt`. Print
the table to stdout as well. The file must exist before the final clean.

## 7. Final `cargo clean`

Run `cargo clean` again and finish. Do not delete `BUILDSTATS.md`.

## Gotchas

- `cargo clean` removes `target/` entirely, including the run directory. Write
  `BUILDSTATS.md` at the repo root first, then clean.
- The root `.gitignore` ignores `/target/`, so no measurement artifacts are
  tracked.
- Running the tests dirties generated files under
  `crates/ragent-server/research/` (`INDEX.md`, `.index.json`). If they show as
  modified afterwards, restore only those generated files with
  `git restore -- crates/ragent-server/research/`; never touch other work.
- `--all-targets` builds benches but does not run them.
