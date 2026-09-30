#!/usr/bin/env bash
# CI guard (SECTASKS MS-05 T-067/T-069): fail if a crate re-derives a guard the
# workspace already owns (one implementation per guard).
#
# See scripts/check_shared_guards.py for the scanner.
#
# Usage: bash scripts/check-shared-guards.sh [--self-test]
# Exit code: 0 = single implementation, 1 = a duplicate guard was found.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

python3 "$ROOT/scripts/check_shared_guards.py" "$@"
