#!/usr/bin/env bash
# CI guard (spec `uniformtools` FR-027 / T-034 / NFR-008 predicate (e)):
# fail when a retired `before` tool spelling appears as a quoted or backticked
# literal in non-test Rust source outside the two structural exemptions.
#
# The rename retired thirteen spellings (FR-023) and adopted the Claude Code
# names. Any remaining literal that names a tool by the old spelling is a stale
# site the compiler cannot catch — it silently keys on a retired name. This
# guard is the build-time catch.
#
# What it does (one run, every predicate letter it failed):
#   - scans crates/*/src/**/*.rs and src/*.rs, skipping tests/ subtrees, for
#     Rust string literals whose decoded value equals one of the thirteen
#     retired `before` spellings (double-quoted, single-quoted, backticked);
#   - skips the two structural exemptions (the FR-023 table itself,
#     crates/ragent-types/src/tool_rename.rs, and the CHANGELOG.md entry —
#     the latter lives outside the scanned tree);
#   - exempts the per-line entries in EXEMPT_LINES (see the Python module for
#     the authoritative list), each of which carries the reason it is not a
#     tool name — FR-028 keeps the `plan_enter`/`plan_exit` permission-type
#     spellings unchanged, so the two files named in the task
#     (crates/ragent-config/src/permission.rs and crates/ragent-agent/src/agent/oasf.rs)
#     plus the same class in crates/ragent-agent/src/session/permissions.rs
#     and crates/ragent-agent/src/tool/aliases.rs legitimately carry them;
#   - knows the deliberately-not-migrated categories that T-033 audited: shell
#     program names, language ids, config section names, validation verb tables,
#     pluralize nouns, govcreate stage ids, slash-command triggers, safe-shell
#     allowlists, and doc-comment prose — a stolen literal outside those four
#     buckets is a defect (every exemption is reviewed individually — an entry
#     for a tool-name site is a defect).
#
# Implementation: the hot path lives in scripts/check_tool_rename.py (one
# implementation for both the script and the crate test, NFR-009). This shell
# wrapper exists so .github/workflows/ci.yml can invoke
# `bash scripts/check-tool-rename.sh` like every other guard, and to provide
# --self-test without duplicating the predicate logic.
#
# Usage: bash scripts/check-tool-rename.sh [--self-test] [--exemptions]
# Exit code: 0 = no retired spelling found where it would be a tool-name bug
#            1 = one or more stale literals found (or self-test misbehaved)
#            ASCII-only diagnostics (NFR-009), names the predicate letter.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [ "${1:-}" = "--exemptions" ]; then
    python3 scripts/check_tool_rename.py --exemptions
    exit 0
fi

if [ "${1:-}" = "--self-test" ]; then
    # Self-test: seed a fixture that violates predicate (e) and assert the
    # guard catches it. The seeded file is not checked in; it lives only for
    # this run and is removed even if the guard misbehaves.
    seed="crates/ragent-tools-core/src/__tool_rename_guard_selftest.rs"
    cat > "$seed" <<'RUST'
//! Self-test fixture for scripts/check-tool-rename.sh — not shipped.
pub fn fake_dispatch(tool: &str) {
    // This line pretends to be a stale tool-name site that kept the retired
    // spelling "bash" after the FR-023 rename. The guard should flag it.
    if tool == "bash" {
        let _ = tool;
    }
}
RUST
    set +e
    python3 scripts/check_tool_rename.py >/dev/null 2>&1
    rc=$?
    set -e
    rm -f "$seed"
    if [ "$rc" -eq 0 ]; then
        echo "ERROR: self-test failed — the guard passed a seeded stale literal (tool == \"bash\")."
        exit 1
    fi
    echo "OK: self-test passed — seeded violation detected."
    exit 0
fi

exec python3 scripts/check_tool_rename.py "$@"
