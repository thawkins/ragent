#!/usr/bin/env bash
# CI guard (SECTASKS MS-05 T-070): fail if production code adds a panicking
# `.unwrap()`/`.expect()` beyond the recorded baseline.
#
# See scripts/check_security_unwraps.py for the scanner and the baseline format.
#
# Usage: bash scripts/check-security-unwraps.sh [--self-test|--update-baseline]
# Exit code: 0 = clean, 1 = a new panicking call was found.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

python3 "$ROOT/scripts/check_security_unwraps.py" "$@"
