#!/usr/bin/env bash
# CI guard: ensure the ragent-agent team subsystem remains a thin source-level
# re-export of the ragent-team implementation and does not re-introduce local
# copies of either the runtime modules or the coordination tools.
#
# Two duplication vectors are guarded:
#
# 1. Runtime modules — only `mod.rs` may live in `crates/ragent-agent/src/team/`,
#    and it must `#[path]`-include every `ragent-team/src/team/*.rs` file.
#
# 2. Coordination tools — no `team_*.rs` file may exist in
#    `crates/ragent-agent/src/tool/`. The tools live canonically in
#    `crates/ragent-team/src/tools/` and are compiled into `ragent-agent` via
#    `#[path]` includes in `crates/ragent-agent/src/tool/mod.rs`.

set -euo pipefail

RUNTIME_SRC_DIR="crates/ragent-team/src/team"
RUNTIME_DUP_DIR="crates/ragent-agent/src/team"
TOOLS_SRC_DIR="crates/ragent-team/src/tools"
TOOLS_DUP_DIR="crates/ragent-agent/src/tool"

exit_code=0

# ── 1. Runtime modules ────────────────────────────────────────────────────────
# Only mod.rs should exist in ragent-agent/src/team/.
for f in "$RUNTIME_DUP_DIR"/*.rs; do
    [ -e "$f" ] || continue
    name=$(basename "$f")
    if [ "$name" != "mod.rs" ]; then
        echo "ERROR: duplicate team runtime source file detected: $f"
        echo "       ragent-agent/src/team/ must contain only mod.rs."
        exit_code=1
    fi
done

# Verify mod.rs uses #[path] includes for the expected runtime source files.
for src in "$RUNTIME_SRC_DIR"/*.rs; do
    name=$(basename "$src")
    if [ "$name" = "mod.rs" ]; then
        continue
    fi
    expected_path="../../../ragent-team/src/team/$name"
    if ! grep -qF "#[path = \"$expected_path\"]" "$RUNTIME_DUP_DIR/mod.rs"; then
        echo "ERROR: ragent-agent/src/team/mod.rs does not source-include $name via #[path]."
        exit_code=1
    fi
done

# ── 2. Coordination tools ─────────────────────────────────────────────────────
# No team_*.rs file may exist physically under ragent-agent/src/tool/.
for f in "$TOOLS_DUP_DIR"/team_*.rs; do
    [ -e "$f" ] || continue
    name=$(basename "$f")
    echo "ERROR: duplicate team tool source file detected: $f"
    echo "       team tools live in crates/ragent-team/src/tools/ and are"
    echo "       compiled into ragent-agent via #[path] includes in"
    echo "       crates/ragent-agent/src/tool/mod.rs. Remove the local copy."
    exit_code=1
done

# Verify tool/mod.rs #[path]-includes every team_*.rs from ragent-team/src/tools.
for src in "$TOOLS_SRC_DIR"/team_*.rs; do
    [ -e "$src" ] || continue
    name=$(basename "$src")
    expected_path="../../../ragent-team/src/tools/$name"
    if ! grep -qF "#[path = \"$expected_path\"]" "$TOOLS_DUP_DIR/mod.rs"; then
        echo "ERROR: ragent-agent/src/tool/mod.rs does not source-include $name via #[path]."
        exit_code=1
    fi
done

if [ "$exit_code" -ne 0 ]; then
    exit "$exit_code"
fi

echo "OK: ragent-agent team runtime and tools are thin #[path] re-exports of ragent-team."