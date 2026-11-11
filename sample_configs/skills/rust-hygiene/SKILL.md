---
name: rust-hygiene
description: Run all CI and Security Audit checks locally (cargo check, cargo nextest, dead-code lint, guard scripts, clippy, fmt, cargo audit, cargo deny) and fix any issues surfaced
argument-hint: "[check-name or 'all']"
allowed-tools:
  - any
---

# Hygiene: Local CI + Security Audit

Run the same checks performed by the GitHub Actions **CI** and **Security Audit**
workflows locally, then fix any issues that surface.

## Checks to run

Execute each check below in order using the `bash` tool. For each check, report
PASS or FAIL with a one-line summary. If a check FAILS, attempt to fix the issue
before moving to the next check. After fixing, re-run the failed check to
confirm it now passes.

If `$ARGUMENTS` is a specific check name (e.g. `check`, `test`, `dead-code`,
`guards`, `regression`, `clippy`, `fmt`, `audit`, `deny`, `machete`), run only
that check. If `$ARGUMENTS` is empty or `all`, run every CI check (1-10); run
the optional extra (11) only when asked for by name.

The commands below are copied from `.github/workflows/ci.yml` and
`.github/workflows/security-audit.yml`. If a workflow has changed since, the
workflow file wins: run what it runs.

CRITICAL NOTE: the instruction is that ALL warnings and errors must be resolved, even if they are not in code that has not been changed, and even if they are in tests.

CRITICAL NOTE: the instruction is that ALL tests must pass, even if they are testing code that has not been changed.

---

### 1. cargo check

```
cargo check --workspace
```

### 2. cargo check --tests

```
cargo check --tests --workspace
```

Not a separate CI step (CI compiles the tests in step 3); it surfaces test
compile errors and warnings before the long test run. Use a timeout of 1500
seconds. Ensure that All errors and warnings are resolved

### 3. cargo nextest run

```
cargo nextest run --workspace --features ragent-tools-extended/test-ssrf-bypass --profile ci --no-fail-fast
cargo test --workspace --doc
```

This is CI's `Test (nextest)` step: the `ci` profile from `.config/nextest.toml`
and the `test-ssrf-bypass` feature the fetch/crawl integration tests need.
Nextest does not run doc tests, so the doc pass stays on `cargo test`.

Use a timeout of 1500 seconds. Ensure that All tests pass. If any test fails, fix the issue and re-run the tests until they all pass. If there are timing issues then run the effected tests sequentialy.

CRITICAL NOTE: Fix all tests regardless of wether they are pre-existing or not, if you cant fix them stop and report the resaons why.

### 4. Dead-code lint

```
RUSTFLAGS='-D unreachable_pub -D dead_code -D unused_imports' cargo check --workspace --lib --all-features
```

### 5. Dead-code reason check

```
bash scripts/check-dead-code-reasons.sh
```

Every `#[allow(dead_code)]` attribute must have an explanatory comment within
two lines. If this fails, add a comment explaining why the dead code is
suppressed.

### 6. Guard scripts

CI's `poison-lock-guard` and `security-guards` jobs run each guard twice: first
with `--self-test` (it seeds a violation and exits 0 only if the guard catches
it), then against the tree. Run them one at a time: some self-tests seed their
fixture inside the checkout.

```
failed=0
for g in poison-locks file-tool-containment security-unwraps shared-guards \
         vcs-duplication connector-endpoint-literal team-duplication \
         inline-tests test-env-isolation silent-errors tool-rename; do
  if ! { bash "scripts/check-$g.sh" --self-test && bash "scripts/check-$g.sh"; }; then
    echo "FAIL: $g"
    failed=1
  fi
done
[ "$failed" -eq 0 ]
```

The block exits non-zero if any guard's self-test or tree scan failed.

Fix the code, not the guard: each script prints the file and line it rejects.

### 7. Regression suites

Run every `run:` line of the `ms0-regression` job in
`.github/workflows/ci.yml` (and the two `cargo nextest run -p ragent-types`
lines at the end of `security-guards`). Most overlap step 3, but the telemetry
suite needs its own feature flag and is only exercised here:

```
cargo nextest run -p ragent-telemetry --features telemetry -E "test(test_metric_toggles::)"
```

### 8. Clippy

```
cargo clippy --workspace -- -D warnings -A clippy::used_underscore_items -A clippy::redundant_pub_crate -A clippy::wildcard_imports
```

Fix all clippy warnings. Do NOT suppress them with `#[allow(...)]` unless the
warning is a known false positive — add a comment explaining why.

### 9. Rustfmt

```
cargo fmt --all -- --check
```

If this fails, run `cargo fmt --all` to fix formatting, then re-check.

### 10. cargo audit and cargo deny (Security Audit)

Only run each if it is installed. If not installed, skip with a note.

```
cargo audit
cargo deny check
```

Pass no `--ignore` flags. Advisory suppressions live in `.cargo/audit.toml`
(read automatically by `cargo audit`) and `deny.toml` `[advisories].ignore`,
kept identical; an advisory that fails here is a real finding.

### 11. Optional extra: cargo-machete (not run by CI)

```
cargo-machete --workspace
```

Only when asked for by name. Use a timeout of 1500 seconds. Check each
reported dependency before removing it (machete reports false positives for
crates used only through macros or features), then confirm the project still
builds.

---

## Summary

After all checks complete, produce a summary table:

```
| #  | Check                | Status |
|----|----------------------|--------|
| 1  | cargo check          | PASS   |
| 2  | cargo check --tests  | PASS   |
| 3  | cargo nextest run    | PASS   |
| 4  | Dead-code lint       | PASS   |
| 5  | Dead-code reason     | PASS   |
| 6  | Guard scripts        | PASS   |
| 7  | Regression suites    | PASS   |
| 8  | Clippy               | PASS   |
| 9  | Rustfmt              | PASS   |
| 10 | cargo audit / deny   | PASS   |
```

List any issues that were found and fixed, and any that remain unresolved.
