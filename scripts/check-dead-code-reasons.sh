#!/usr/bin/env bash
# CI guard: every `#[allow(dead_code)]` attribute in `src/` must have an
# explanatory comment nearby. This prevents regression of RMPLAN.md Milestone 7
# by forcing authors to document why dead-code lints are suppressed.
#
# Usage: bash scripts/check-dead-code-reasons.sh
# Exit code: 0 = every allow(dead_code) is documented, 1 = undocumented
#            suppressions found.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

FAIL=0
OFFENDING=()

# Search src/ directories for #[allow(dead_code)] attributes.
# For each match, look at the two lines before and after for a comment
# (`//` or `///`). If no comment is present, the suppression is undocumented.
while IFS=: read -r file line _rest; do
    window=$(sed -n "$((line - 2)),$((line + 2))p" "$file")
    if ! grep -qE '^\s*(//|///)' <<<"$window"; then
        OFFENDING+=("$file:$line")
        FAIL=1
    fi
done < <(grep -R -n "#\[allow(dead_code)\]" crates/*/src --include='*.rs' 2>/dev/null || true)

if [ "$FAIL" -ne 0 ]; then
    echo "ERROR: Found ${#OFFENDING[@]} undocumented #[allow(dead_code)] attribute(s)."
    echo "Every dead-code suppression must have an explanatory comment"
    echo "(// reason: ... or doc comment) within two lines of the attribute."
    echo ""
    echo "Undocumented attributes:"
    for loc in "${OFFENDING[@]}"; do
        echo "  $loc"
    done
    exit 1
fi

echo "OK: Every #[allow(dead_code)] attribute in src/ has an explanatory comment."
exit 0
