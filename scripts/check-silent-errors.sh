#!/usr/bin/env bash
# CI guard (ANTIPAT M4): fail if a production source gains a *new* masked
# error-suppression site (`let _ =` drop, `.ok().flatten()`, `unwrap_or(0|false)`)
# without a same-line `// INTENTIONAL: <reason>` justification.
#
# See scripts/check_silent_errors.py for the scanner and the baseline format.
#
# Usage: bash scripts/check-silent-errors.sh [--list|--self-test|--update-baseline]
# Exit code: 0 = clean, 1 = a new masked site was found.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

python3 "$ROOT/scripts/check_silent_errors.py" "$@"
