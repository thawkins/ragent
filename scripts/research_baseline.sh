#!/usr/bin/env bash
# Optional real-stack baseline runner for Milestone A-003.
#
# Runs three representative `/research create` topics through the actual
# `ragent` binary, captures wall-clock time, and records peak RSS when
# `/usr/bin/time -v` is available. The script is safe to run manually:
# it exits early if no provider key is configured.
#
# Usage:
#   export ANTHROPIC_API_KEY="..."
#   ./scripts/research_baseline.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
RAGENT_BIN="${PROJECT_ROOT}/target/release/ragent"
REPORT_DIR="${PROJECT_ROOT}/target/temp"
REPORT="${REPORT_DIR}/research_baseline_real_stack_report.md"

if [[ ! -f "$RAGENT_BIN" ]]; then
    echo "Building release binary..."
    cargo build --release -p ragent
fi

# Detect at least one known provider key.
if [[ -z "${ANTHROPIC_API_KEY:-}${OPENAI_API_KEY:-}${GENERIC_OPENAI_API_KEY:-}${AZURE_AI_FOUNDRY_API_KEY:-}${OLLAMA_HOST:-}" ]]; then
    echo "No provider key detected; skipping real-stack baseline."
    echo "Set ANTHROPIC_API_KEY, OPENAI_API_KEY, GENERIC_OPENAI_API_KEY, etc. to run."
    exit 0
fi

mkdir -p "$REPORT_DIR"

cat > "$REPORT" <<'MD'
# Research System Real-Stack Baseline Report (Milestone A-003)

| Topic | Wall-clock (s) | Peak RSS (kB) | Sources |
|-------|---------------:|--------------:|--------:|
MD

topics=(
    "Rust async runtimes"
    "Structured logging in Rust"
    "SQLite WAL mode"
)

for topic in "${topics[@]}"; do
    safe_name=$(echo "$topic" | tr '[:upper:] ' '[:lower:]-' | tr -cd 'a-z0-9-')
    # Delete any prior run so we measure a fresh create.
    "$RAGENT_BIN" research delete "$safe_name" --yes 2>/dev/null || true

    start=$(date +%s.%N)
    if /usr/bin/time -v "$RAGENT_BIN" research create --no-tui --topic "$topic" "$safe_name" 2>"${REPORT_DIR}/${safe_name}.time"; then
        elapsed=$(awk "BEGIN {print $(date +%s.%N) - $start}")
        peak_rss=$(grep 'Maximum resident set size' "${REPORT_DIR}/${safe_name}.time" | awk '{print $6}')
        sources=$(grep -c '^- ' "research/${safe_name}/sources/web-"*.md 2>/dev/null || echo 0)
        echo "| $topic | $elapsed | ${peak_rss:-n/a} | $sources |" >> "$REPORT"
    else
        echo "| $topic | failed | n/a | 0 |" >> "$REPORT"
    fi
done

echo "Real-stack baseline report written to $REPORT"
