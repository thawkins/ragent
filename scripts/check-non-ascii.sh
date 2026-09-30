#!/usr/bin/env bash
# CI guard (ANTIPAT M1.16): reject non-ASCII in production Rust source beyond
# the documented terminal-glyph allowlist.
#
# See scripts/check_non_ascii.py for the scanner and the allowlist rationale.
#
# Usage: bash scripts/check-non-ascii.sh [--self-test]
# Exit code: 0 = clean, 1 = a disallowed non-ASCII codepoint was found.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

python3 "$ROOT/scripts/check_non_ascii.py" "$@"
