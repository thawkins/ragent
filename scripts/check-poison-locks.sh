#!/usr/bin/env bash
# CI guard (FUNC-080): fail if production code reintroduces a poisoning-panic
# lock access. A single panic while holding a `Mutex`/`RwLock` poisons the
# lock, and every later `.lock().unwrap()` / `.lock().expect("...poisoned")`
# then panics unrelated callers for the process lifetime.
#
# The safe pattern is:
#   .lock().unwrap_or_else(std::sync::PoisonError::into_inner)
#
# Scope: every `crates/*/src/**/*.rs` file, excluding `#[cfg(test)]` modules
# and test/bench directories (which may use `.unwrap()` freely).
#
# Usage: bash scripts/check-poison-locks.sh [--self-test]
# Exit code: 0 = clean, 1 = offending lock access found.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [ "${1:-}" = "--self-test" ]; then
    # Seed a violation in a temp file inside the scan root and confirm the
    # scanner flags it (proves the guard actually fails on a seeded breach).
    seed="crates/ragent-types/src/__poison_guard_selftest.rs"
    printf 'fn f() { let l = std::sync::Mutex::new(0); let _ = l.lock().unwrap(); }\n' > "$seed"
    set +e
    python3 "$ROOT/scripts/check_poison_locks.py" >/dev/null 2>&1
    rc=$?
    set -e
    rm -f "$seed"
    if [ "$rc" -eq 0 ]; then
        echo "ERROR: self-test failed — the scanner did not flag a seeded violation."
        exit 1
    fi
    echo "OK: self-test passed — the scanner flags a seeded poisoning-panic lock."
    exit 0
fi

exec python3 "$ROOT/scripts/check_poison_locks.py"
