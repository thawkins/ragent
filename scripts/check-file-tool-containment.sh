#!/usr/bin/env bash
# CI guard (SECTASKS MS-05 T-070, ANTIPAT M0.4/F-07): fail if a file tool stops
# calling an `allowed_roots`-aware path-containment helper.
#
# MS-01 T-005 found `multi_edit` was the only file tool that never called the
# existing `check_path_within_*` helpers, so an absolute path or a `..` path
# could write anywhere the agent's uid can write. The fix is per-tool, so the
# regression risk is a NEW file tool that reads or writes a caller-supplied
# path without a containment check.
#
# ANTIPAT F-07: the first version of this guard accepted ANY of
# `check_path_within_root|check_path_within_any_root|check_path_within_allowed_roots`,
# which meant a tool that only checked the working directory passed while
# silently ignoring `ToolContext.allowed_roots` (FUNC-068). Five tools
# (`diff`, `file_info`, `glob`, `open`, `apply_patch`) were green under the old
# guard for exactly that reason. The guard now demands the allowed-roots-aware
# entry point, or an explicit exemption with a reason.
#
# Usage: bash scripts/check-file-tool-containment.sh [--self-test]
# Exit code: 0 = every file tool is contained, 1 = an uncontained file tool found.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

TOOLS_DIR="crates/ragent-tools-core/src"

# Tool `name()` values that read or write a caller-supplied filesystem path.
FILE_TOOLS=(
    read write create append_to_file edit multi_edit rm make_directory
    move_file copy_file file_info diff_files glob grep list patch apply_patch
)

# Source files exempt from the check, with the reason. Keep this list short and
# justified: an entry here is a deliberate decision that the tool cannot touch a
# path outside the workspace.
#
# `open` IS contained (open.rs calls check_path_within_allowed_roots_cached via
# `resolve_target_path`); it is not exempt. It is listed here only as an
# example of the required comment shape.
EXEMPT=()

# The ONLY call that satisfies the containment requirement. The weaker
# `check_path_within_root_cached` / `check_path_within_any_root_cached` helpers
# ignore `allowed_roots` and must not be treated as containment.
CONTAINMENT_CALL='check_path_within_allowed_roots_cached'

if [ "${1:-}" = "--self-test" ]; then
    seed="$TOOLS_DIR/__containment_guard_selftest.rs"
    cat > "$seed" <<'RUST'
//! Self-test fixture for scripts/check-file-tool-containment.sh.
use serde_json::Value;
pub struct SelfTestTool;
impl SelfTestTool {
    pub fn name(&self) -> &str {
        "read"
    }
    pub async fn execute(&self, _input: Value) -> anyhow::Result<()> {
        Ok(())
    }
}
RUST
    set +e
    bash "$0" >/dev/null 2>&1
    rc=$?
    set -e
    rm -f "$seed"
    if [ "$rc" -eq 0 ]; then
        echo "ERROR: self-test failed - the guard passed a seeded violation."
        exit 1
    fi

    # Second half of the self-test: the weak helper alone must NOT satisfy the
    # guard (the exact F-07 hole).
    weak="$TOOLS_DIR/__containment_guard_selftest_weak.rs"
    cat > "$weak" <<'RUST'
//! Self-test fixture: weak (working-dir-only) containment must not pass.
use serde_json::Value;
pub struct SelfTestWeakTool;
impl SelfTestWeakTool {
    pub fn name(&self) -> &str {
        "read"
    }
    pub async fn execute(&self, _input: Value) -> anyhow::Result<()> {
        let cache = unreachable!();
        super::check_path_within_root_cached(std::path::Path::new("."), std::path::Path::new("."), cache)?;
        Ok(())
    }
}
RUST
    set +e
    bash "$0" >/dev/null 2>&1
    rc=$?
    set -e
    rm -f "$weak"
    if [ "$rc" -eq 0 ]; then
        echo "ERROR: self-test failed - a working-dir-only containment call passed the guard (F-07 hole open)."
        exit 1
    fi

    echo "OK: self-test - the guard fails both an uncontained tool and a working-dir-only tool."
    exit 0
fi

FAIL=0
CHECKED=0

is_exempt() {
    local name="$1"
    for e in "${EXEMPT[@]:-}"; do
        [ -z "$e" ] && continue
        [ "$e" = "$name" ] && return 0
    done
    return 1
}

for tool in "${FILE_TOOLS[@]}"; do
    # Locate the source file whose `name()` returns this tool. The literal is
    # anchored to a whole line so an incidental mention of the word (in a
    # docstring or a shared helper) does not attribute the tool to the wrong
    # file.
    matches=$(grep -rlE "^[[:space:]]*\"$tool\"[[:space:]]*$" "$TOOLS_DIR" --include='*.rs' 2>/dev/null || true)
    if [ -z "$matches" ]; then
        echo "WARN: no source file found for file tool '$tool' (moved or renamed?)"
        continue
    fi

    for file in $matches; do
        base=$(basename "$file")
        if is_exempt "$base"; then
            echo "SKIP: $base (exempt: $tool)"
            continue
        fi
        CHECKED=$((CHECKED + 1))
        if ! grep -qE "$CONTAINMENT_CALL" "$file"; then
            echo "ERROR: file tool '$tool' in $file does not call $CONTAINMENT_CALL()."
            echo "       Every file tool must confine a caller-supplied path to the"
            echo "       allowed roots (SECTASKS MS-01 T-005 / MS-05 T-070 / ANTIPAT F-06)."
            echo "       check_path_within_root_cached() alone ignores allowed_roots and"
            echo "       is not sufficient."
            FAIL=1
        fi
    done
done

if [ "$FAIL" -ne 0 ]; then
    exit 1
fi

echo "OK: $CHECKED file-tool source file(s) call $CONTAINMENT_CALL()."
exit 0
