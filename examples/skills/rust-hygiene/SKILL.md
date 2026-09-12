---
name: rust-hygiene
description: Run all CI and Security Audit checks locally (cargo check, cargo test, dead-code lint, clippy, fmt, cargo audit, cargo deny) and fix any issues surfaced
argument-hint: "[check-name or 'all']"
allowed-tools:
  - bash
  - read
  - edit
  - multiedit
  - write
  - create
  - grep
  - glob
  - Codeindex_searc
---
# Hygiene: Local CI + Security Audit

Run the same checks performed by the GitHub Actions **CI** and **Security Audit**
workflows locally, then fix any issues that surface.

## Checks to run

Execute each check below in order using the `bash` tool. For each check, report
PASS or FAIL with a one-line summary. If a check FAILS, attempt to fix the issue
before moving to the next check. After fixing, re-run the failed check to
confirm it now passes.

If `$ARGUMENTS` is a specific check name (e.g. `clippy`, `fmt`, `audit`,
`deny`, `test`, `check`, `dead-code`), run only that check. If `$ARGUMENTS` is
empty or `all`, run every check.

CRITICAL NOTE: the instruction is that ALL warnings and errors must be resolved, even if they are not in code that has not been changed, and even if they are in tests.

CRITICAL NOTE: the instruction is that ALL tests must pass, even if they are testing code that has not been changed.

---

### 1. cargo check

```
cargo check --workspace
```

### 2. Cargo machete

```
cargo-machete --workspace
```

Use a timeout of 1500 seconds. Ensure that all errors and warnings are resolved. Run cargo machete to determine which crates dependancies are not required, check and remove any unnecessary dependencies. Double-check that the project builds successfully after removing dependencies.

### 3. cargo check --tests

```
cargo check --tests --workspace
```

Use a timeout of 1500 seconds. Ensure that All errors and warnings are resolved

### 4. cargo test

```
cargo test --workspace
```

Use a timeout of 1500 seconds. Ensure that All tests pass. If any test fails, fix the issue and re-run the tests until they all pass. If there are timing issues then run the effected tests sequentialy.

Fix all tests regardless of wether they are pre-existing or not, if you cant fix them stop and report the resaons why.

### 5. Dead-code lint

```
RUSTFLAGS='-D unreachable_pub -D dead_code -D unused_imports' cargo check --workspace --lib --all-features
```

### 6. Dead-code reason check

```
bash scripts/check-dead-code-reasons.sh
```

Every `#[allow(dead_code)]` attribute must have an explanatory comment within
two lines. If this fails, add a comment explaining why the dead code is
suppressed.

### 7. Clippy

```
cargo clippy --workspace -- -D warnings -A clippy::used_underscore_items -A clippy::redundant_pub_crate -A clippy::wildcard_imports
```

Fix all clippy warnings. Do NOT suppress them with `#[allow(...)]` unless the
warning is a known false positive — add a comment explaining why.

### 8. Rustfmt

```
cargo fmt --all -- --check
```

If this fails, run `cargo fmt --all` to fix formatting, then re-check.

### 9. cargo audit (Security Audit)

Only run if `cargo-audit` is installed. If not installed, skip with a note.

```
cargo audit \
  --ignore RUSTSEC-2024-0370 \
  --ignore RUSTSEC-2024-0384 \
  --ignore RUSTSEC-2024-0436 \
  --ignore RUSTSEC-2025-0052 \
  --ignore RUSTSEC-2025-0119 \
  --ignore RUSTSEC-2026-0002 \
  --ignore RUSTSEC-2026-0185 \
  --ignore RUSTSEC-2026-0187 \
  --ignore RUSTSEC-2026-0190 \
  --ignore RUSTSEC-2026-0192 \
  --ignore RUSTSEC-2026-0194 \
  --ignore RUSTSEC-2026-0195 \
  --ignore RUSTSEC-2026-0235 \
  --ignore RUSTSEC-2026-0253
```

### 10. cargo deny (Security Audit)

Only run if `cargo-deny` is installed. If not installed, skip with a note.

```
cargo deny check
```

---

## Summary

After all checks complete, produce a summary table:

```
| # | Check               | Status |
|---|--------------------|--------|
| 1 | cargo check         | PASS   |
| 2 | cargo machete       | PASS   |
| 3 | cargo check --tests | PASS   |
| 4 | cargo test          | PASS   |
| 5 | Dead-code lint      | PASS   |
| 6 | Dead-code reason    | PASS   |
| 7 | Clippy              | PASS   |
| 8 | Rustfmt             | PASS   |
| 9 | cargo audit         | PASS   |
| 10 | cargo deny         | PASS   |
| ...                         |
```

List any issues that were found and fixed, and any that remain unresolved.
