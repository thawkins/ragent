#!/usr/bin/env bash
# CI guard: fail if an integration test mutates or depends on a process global
# (environment variable, current working directory) without joining the
# crate's isolation lock.
#
# Why this exists: each crate builds ONE test binary from its whole `tests/`
# tree (`autotests = false` + a single `[[test]] name = "all"`), so every
# `tests/test_*.rs` file now shares one process. A `std::env::set_var` /
# `remove_var` / `set_current_dir` in one child file is therefore observed by
# every concurrently running test in the other child files. Before that rule
# each file was its own binary, so a file-local mutex (or `serial_test`'s
# per-target lock) was enough - which is exactly how the isolation got lost and
# why `test_experimental_flags::test_experimental_round_trips_through_config_load`
# failed roughly once in 40 unsharded runs.
#
# A test is considered isolated for a global when it holds a guard from ONE
# family, and every test touching a process global in that crate must share at
# least one family with all the others:
#   * serial  - `#[serial]` / `#[serial_test::serial]`
#   * mutex   - `crate::global_env_lock()` (the crate-wide lock in `tests/all.rs`)
#               or any `MutexGuard` acquisition (`let _lock = ...`)
# Two tests in one crate that each take a DIFFERENT, private mutex (or one is
# `#[serial]` while the other takes a file-local mutex) do not exclude each
# other - that is the failure this guard rejects.
#
# Scope: `crates/*/tests/**/*.rs`, excluding the consolidated `all.rs` root.
#
# Usage: bash scripts/check-test-env-isolation.sh [--self-test]
# Exit code: 0 = clean, 1 = an unisolated process-global access was found.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

seed="crates/ragent-types/tests/test___env_isolation_selftest.rs"

# Runs the scanner with the seeded fixture and reports whether it failed.
scan_with_seed() {
    set +e
    python3 "$ROOT/scripts/check_test_env_isolation.py" >/dev/null 2>&1
    rc=$?
    set -e
    rm -f "$seed"
    return "$rc"
}

if [ "${1:-}" = "--self-test" ]; then
    # 1. An unguarded `set_var` must be flagged.
    cat > "$seed" <<'RUST'
//! Scratch fixture for scripts/check-test-env-isolation.sh --self-test.

#[test]
fn selftest_unguarded_env_mutation() {
    unsafe { std::env::set_var("RAGENT_SELFTEST_UNGUARDED", "1") };
}
RUST
    if scan_with_seed; then
        echo "ERROR: self-test failed - the scanner did not flag a seeded unguarded env mutation."
        exit 1
    fi

    # 2. A non-reentrant re-acquisition (lock held, helper locks too) must be
    #    flagged: this is the shape that hangs the whole test binary.
    cat > "$seed" <<'RUST'
//! Scratch fixture for scripts/check-test-env-isolation.sh --self-test.

/// Returns the crate lock to its caller (a harness).
fn selftest_harness() -> std::sync::MutexGuard<'static, ()> {
    crate::global_env_lock()
}

#[test]
fn selftest_reacquires_the_env_lock() {
    let _held = crate::global_env_lock();
    let _also_held = selftest_harness();
    unsafe { std::env::set_var("RAGENT_SELFTEST_RELOCK", "1") };
}
RUST
    if scan_with_seed; then
        echo "ERROR: self-test failed - the scanner did not flag a non-reentrant env-lock re-acquisition."
        exit 1
    fi

    echo "OK: self-test passed - the scanner flags unguarded mutations and lock re-acquisition."
    exit 0
fi

exec python3 "$ROOT/scripts/check_test_env_isolation.py"
