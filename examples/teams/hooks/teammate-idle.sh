#!/usr/bin/env bash
set -euo pipefail

# Example TeammateIdle quality gate hook.
# Expected env (if provided by runner):
# - TEAM_NAME
# - TEAMMATE_NAME
# - SUMMARY_FILE (path to teammate summary text)

if [[ -n "${SUMMARY_FILE:-}" && -f "${SUMMARY_FILE}" ]]; then
  if ! rg -qi "test|verify|validated" "${SUMMARY_FILE}"; then
    echo "Idle gate failed: teammate summary did not mention verification steps."
    exit 2
  fi
fi

echo "Idle gate passed for ${TEAM_NAME:-unknown-team}/${TEAMMATE_NAME:-unknown-teammate}."
exit 0
