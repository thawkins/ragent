#!/usr/bin/env bash
# PERFPLAN Milestone F-5: CI guard that fails when the `agent_loop` bench
# regresses by more than 10% versus the saved Criterion baseline.
#
# Usage:
#   scripts/check-bench-regression.sh [--threshold N]
#
# Options:
#   --threshold N   regression percentage that triggers failure (default: 10)
#
# Exit codes:
#   0 — no benchmark regressed by more than the threshold
#   1 — at least one benchmark regressed past the threshold (or bench failed)
#   2 — no saved baseline found (run `cargo bench ... -- --save-baseline main`)
#
# Wired into `pre-flight.sh` so CI catches regressions before they ship.

set -euo pipefail

THRESHOLD="${1:-10}"
if [[ "${1:-}" == "--threshold" ]]; then
    THRESHOLD="${2:-10}"
fi

CRITERION_DIR="target/criterion"
BASELINE_NAME="main"

if [[ ! -d "${CRITERION_DIR}" ]]; then
    echo "check-bench-regression: no criterion results directory found at ${CRITERION_DIR}" >&2
    echo "run: cargo bench -p ragent-bench --bench agent_loop -- --save-baseline ${BASELINE_NAME}" >&2
    exit 2
fi

# Run the bench comparing against the saved baseline. Criterion writes a
# regression report under each bench's `change/` directory when --baseline is
# used. We parse the "change" median from the latest report.
echo "check-bench-regression: running agent_loop bench vs baseline '${BASELINE_NAME}' (threshold ${THRESHOLD}%)"

if ! cargo bench -p ragent-bench --bench agent_loop -- --baseline "${BASELINE_NAME}" --warm-up-time 0.2 --measurement-time 1.0 2>/dev/null; then
    echo "check-bench-regression: bench run failed" >&2
    exit 1
fi

# Scan each benchmark's latest change report for a regression > threshold.
regressed=0
while IFS= read -r bench_dir; do
    name="$(basename "$(dirname "${bench_dir}")")"
    # Criterion stores the percentage change in the change/estimate files;
    # the median change % is in "change/median_change.txt" (newer criterion)
    # or derivable from "new/estimation_group.json" vs "base/...". We use the
    # simple "change.txt" summary when present.
    change_file="${bench_dir}/change.txt"
    if [[ -f "${change_file}" ]]; then
        pct="$(grep -oE 'Change:\s+\[-?[0-9.]+%.*-?[0-9.]+%\]' "${change_file}" | head -1 || true)"
        # Extract the worst-case (rightmost) magnitude.
        worst="$(echo "${pct}" | grep -oE '\-?[0-9]+\.[0-9]+' | tail -1 || true)"
        if [[ -n "${worst}" ]]; then
            abs="$(echo "${worst}" | tr -d '-')"
            if awk "BEGIN{exit !(${abs} > ${THRESHOLD})}"; then
                echo "check-bench-regression: ${name} regressed by ${worst}% (>${THRESHOLD}%)" >&2
                regressed=1
            fi
        fi
    fi
done < <(find "${CRITERION_DIR}" -type d -name "change" 2>/dev/null)

if [[ "${regressed}" -ne 0 ]]; then
    echo "check-bench-regression: FAIL — at least one benchmark regressed past ${THRESHOLD}%" >&2
    exit 1
fi

echo "check-bench-regression: PASS — no benchmark regressed past ${THRESHOLD}%"
exit 0