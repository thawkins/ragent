#!/usr/bin/env bash
# CI guard (ANTIPAT M2.17): fail if any production source declares a *genuine*
# inline #[cfg(test)] module (a `mod x { ... }` body) rather than a
# `#[cfg(test)] #[path = "../tests/inline/.."] mod x;` hook that names an
# external test file.
#
# This supersedes the old count-any-`mod tests` guard, which also counted the
# idiomatic #[path] hooks and never scanned the root src/ tree.
#
# See scripts/check_inline_tests.py for the scanner and the baseline format.
#
# Usage: bash scripts/check-inline-tests.sh [--list|--self-test]
# Exit code: 0 = clean, 1 = a genuine inline test block was found.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

python3 "$ROOT/scripts/check_inline_tests.py" "$@"
