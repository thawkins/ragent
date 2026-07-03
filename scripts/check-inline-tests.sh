#!/usr/bin/env bash
# CI guard: fail if any src/**/*.rs file adds a new #[cfg(test)] mod tests block.
# This prevents regression of the M8 test migration (REMPLAN.md Milestone 8).
#
# Usage: bash scripts/check-inline-tests.sh
# Exit code: 0 = pass (no new inline test blocks), 1 = fail (new inline test blocks found)

set -euo pipefail

# Find all #[cfg(test)] mod tests blocks in src/ directories.
# We allow existing ones that were too complex to migrate (private-item tests
# with heavy crate:: dependencies), but fail on NEW ones.
#
# The check: count files with 'mod tests' inside #[cfg(test)] in src/ dirs.
# If the count exceeds the baseline, fail.

BASELINE=109

COUNT=$(grep -rl "mod tests" crates/*/src --include='*.rs' 2>/dev/null | wc -l)

if [ "$COUNT" -gt "$BASELINE" ]; then
    echo "ERROR: Found $COUNT files with inline 'mod tests' in src/ (baseline: $BASELINE)"
    echo "New inline test blocks are not allowed in src/ files (AGENTS.md Test Organization)."
    echo "Move tests to the crate's tests/ directory instead."
    echo ""
    echo "Files with inline tests:"
    grep -rl "mod tests" crates/*/src --include='*.rs' 2>/dev/null | head -20
    exit 1
fi

echo "OK: $COUNT files with inline 'mod tests' in src/ (baseline: $BASELINE)"
exit 0