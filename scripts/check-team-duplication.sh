#!/usr/bin/env bash
# CI guard (ANTIPAT M7.7): assert the team subsystem has exactly one home.
#
# History: the pre-M7 guard asserted that the team runtime lived in
# `crates/ragent-team/src/team/` and was compiled into `ragent-agent` via
# `#[path]` includes. REMPLAN M3 moved the sources natively into
# `crates/ragent-agent`, and ANTIPAT M7.8 deleted the `ragent-team` shim
# entirely, so that guard reported 28 false duplicates on the shipped layout
# and contradicted `crates/ragent-types/tests/test_structure_types.rs`. It is
# rewritten here around the CURRENT invariant:
#
#   1. The team runtime lives in `crates/ragent-agent/src/team/` and nowhere
#      else; every module is a plain `pub mod <name>;` declaration.
#   2. The team coordination tools live in `crates/ragent-agent/src/tool/team_*.rs`
#      and are declared as `pub mod team_*;` in `crates/ragent-agent/src/tool/mod.rs`.
#   3. No `ragent-team` crate and no `#[path]` attribute referencing
#      `ragent-team` may be re-introduced under `crates/ragent-agent/src/`.
#
# Usage: bash scripts/check-team-duplication.sh [--self-test]
# Exit code: 0 = clean, 1 = a duplicate/canonicality violation was found.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

TEAM_DIR="crates/ragent-agent/src/team"
TOOL_DIR="crates/ragent-agent/src/tool"
TOOL_MOD="$TOOL_DIR/mod.rs"
AGENT_SRC="crates/ragent-agent/src"
SHIM_CRATE="crates/ragent-team"

# Required team-runtime modules; each must exist as a file and be declared.
RUNTIME_MODULES=(classify config mailbox manager store swarm task)

fail() {
    echo "ERROR: $*"
}

run_checks() {
    local exit_code=0

    # 1a. The retired shim crate must not come back.
    if [ -e "$SHIM_CRATE" ]; then
        fail "the retired ragent-team crate is present at $SHIM_CRATE."
        echo "       The team runtime has a single home in ragent-agent/src/team/"
        echo "       (ANTIPAT M7.8); do not re-introduce the shim crate."
        exit_code=1
    fi

    # 1b. No unexpected file may live in the team runtime directory: every
    #     non-`mod.rs` file must be one of the canonical runtime modules.
    if [ -d "$TEAM_DIR" ]; then
        for f in "$TEAM_DIR"/*.rs; do
            [ -e "$f" ] || continue
            base=$(basename "$f" .rs)
            [ "$base" = "mod" ] && continue
            known=0
            for name in "${RUNTIME_MODULES[@]}"; do
                [ "$base" = "$name" ] && known=1
            done
            if [ "$known" -eq 0 ]; then
                fail "unexpected file in the team runtime directory: $f"
                echo "       The canonical runtime modules are: ${RUNTIME_MODULES[*]}."
                exit_code=1
            fi
        done
    fi

    # 1c. Team runtime modules exist and are plain `pub mod` declarations.
    for name in "${RUNTIME_MODULES[@]}"; do
        file="$TEAM_DIR/$name.rs"
        if [ ! -f "$file" ]; then
            fail "missing team runtime module: $file"
            exit_code=1
            continue
        fi
        if ! grep -qE "^pub mod $name;" "$TEAM_DIR/mod.rs"; then
            fail "$TEAM_DIR/mod.rs does not declare \`pub mod $name;\`."
            exit_code=1
        fi
    done

    # 1c. No `#[path]` attribute may reference ragent-team anywhere in the
    #     agent sources (mirrors test_structure_types.rs:290).
    if grep -rn "#\[path" "$AGENT_SRC" --include='*.rs' 2>/dev/null | grep -q "ragent-team"; then
        fail "a #[path] attribute referencing ragent-team exists under $AGENT_SRC:"
        grep -rn "#\[path" "$AGENT_SRC" --include='*.rs' 2>/dev/null | grep "ragent-team" | sed 's/^/       /'
        exit_code=1
    fi

    # 2. Team tools exist and are declared in tool/mod.rs.
    for src in "$TOOL_DIR"/team_*.rs; do
        [ -e "$src" ] || continue
        name=$(basename "$src" .rs)
        if ! grep -qE "^pub mod $name;" "$TOOL_MOD"; then
            fail "$TOOL_MOD does not declare \`pub mod $name;\` for $src."
            exit_code=1
        fi
    done

    # 3. Every declared team tool module must have a backing file (dangling
    #    declaration detection).
    while IFS= read -r decl; do
        name="${decl#pub mod }"
        name="${name%;}"
        case "$name" in
            team_*) [ -f "$TOOL_DIR/$name.rs" ] || {
                fail "$TOOL_MOD declares \`pub mod $name;\` but $TOOL_DIR/$name.rs is missing."
                exit_code=1
            } ;;
        esac
    done < <(grep -oE "^pub mod team_[a-z_]+;" "$TOOL_MOD" || true)

    return "$exit_code"
}

if [ "${1:-}" = "--self-test" ]; then
    # Seed a duplicate team-runtime file in the agent tree; run_checks must fail.
    seed="$TEAM_DIR/__selftest_dupe.rs"
    printf '//! Self-test fixture for scripts/check-team-duplication.sh.\n' > "$seed"
    set +e
    run_checks >/dev/null 2>&1
    rc=$?
    set -e
    rm -f "$seed"
    if [ "$rc" -eq 0 ]; then
        echo "ERROR: self-test failed - the guard passed a seeded extra runtime file."
        exit 1
    fi

    # Second half: a re-introduced `ragent-team` shim crate must be caught.
    seed_crate="$SHIM_CRATE"
    mkdir -p "$seed_crate/src"
    printf '' > "$seed_crate/src/lib.rs"
    set +e
    run_checks >/dev/null 2>&1
    rc=$?
    set -e
    rm -rf "$seed_crate"
    if [ "$rc" -eq 0 ]; then
        echo "ERROR: self-test failed - a re-introduced ragent-team crate passed the guard."
        exit 1
    fi

    echo "OK: self-test - the guard fails both an extra runtime file and a re-introduced shim crate."
    exit 0
fi

if run_checks; then
    echo "OK: the team runtime and tools have a single home in ragent-agent (no ragent-team shim)."
    exit 0
fi
exit 1
