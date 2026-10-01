#!/usr/bin/env bash
# CI guard (spec `connectors` T-019; FR-038, NFR-001): the compiled default
# Claude connector-catalogue endpoint must be declared exactly once, as a single
# public constant in `ragent_connectors::store_index`, and no other production
# module may carry a second copy of that literal.
#
# This is the shell counterpart of the Rust regression test
# `the_default_endpoint_literal_appears_only_in_the_catalogue_registry`
# (crates/ragent-connectors/tests/test_connector_store_index.rs). It mirrors the
# plugin-store default-endpoint discipline (spec `pluginstores` NFR-001).
#
# Scope: production source only (`crates/*/src/**/*.rs` and `src/**/*.rs`). A
# copy of the literal in a test or a fixture is not a production module and is
# not a duplicate (the T-005 fetch test legitimately names the origin URL).
#
# Usage: bash scripts/check-connector-endpoint-literal.sh [--self-test]
# Exit code: 0 = single declaration, 1 = a duplicated literal was found.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# The one literal that must have a single production home.
LITERAL="https://api.anthropic.com/api/directory/servers"
# The single module allowed to declare it (FR-038, NFR-001).
CANONICAL="crates/ragent-connectors/src/store_index.rs"
# The single declaration form the canonical module must use (FR-034, NFR-001).
DECLARATION="pub const DEFAULT_CLAUDE_CATALOGUE_URL"

run_checks() {
    local exit_code=0

    # 1. Every production `.rs` file that carries the literal, other than the
    #    canonical module, is a duplicate declaration.
    local hits
    hits=$(grep -rlF --include='*.rs' "$LITERAL" crates/*/src src 2>/dev/null || true)

    local file
    for file in $hits; do
        if [ "$file" != "$CANONICAL" ]; then
            echo "ERROR: duplicated default connector-catalogue endpoint literal: $file"
            echo "       The default Claude connector-catalogue endpoint must be declared"
            echo "       exactly once, in $CANONICAL (FR-038, NFR-001)."
            echo "       Remove the copy and reference DEFAULT_CLAUDE_CATALOGUE_URL instead."
            exit_code=1
        fi
    done

    # 2. The canonical module must exist and declare the literal exactly once.
    if [ ! -f "$CANONICAL" ]; then
        echo "ERROR: canonical default-endpoint module is missing: $CANONICAL"
        exit_code=1
        return "$exit_code"
    fi

    local count
    count=$(grep -cF -- "$LITERAL" "$CANONICAL" || true)
    if [ "${count:-0}" -lt 1 ]; then
        echo "ERROR: $CANONICAL does not declare the default endpoint literal."
        echo "       Expected the single declaration of $LITERAL (FR-034, FR-038)."
        exit_code=1
    elif [ "$count" -ne 1 ]; then
        echo "ERROR: $CANONICAL declares the default endpoint literal $count times;"
        echo "       it must be declared exactly once (NFR-001)."
        exit_code=1
    fi

    # 3. The declaration must be a single public constant (NFR-001).
    local decls
    decls=$(grep -cF -- "$DECLARATION" "$CANONICAL" || true)
    if [ "${decls:-0}" -ne 1 ]; then
        echo "ERROR: $CANONICAL must declare the endpoint as a single public constant"
        echo "       (\`$DECLARATION\`); found ${decls:-0} declaration(s) (NFR-001)."
        exit_code=1
    fi

    return "$exit_code"
}

if [ "${1:-}" = "--self-test" ]; then
    # Seed a second copy of the literal in a real production module; the guard
    # must fail, and the clean tree must pass once the seed is removed.
    seed="crates/ragent-connectors/src/__selftest_dupe_endpoint.rs"
    printf '//! Self-test fixture for scripts/check-connector-endpoint-literal.sh.\npub const DUPE: &str = "%s";\n' "$LITERAL" > "$seed"
    set +e
    run_checks >/dev/null 2>&1
    rc=$?
    set -e
    rm -f "$seed"
    if [ "$rc" -eq 0 ]; then
        echo "ERROR: self-test failed - the guard passed a duplicated default-endpoint literal."
        exit 1
    fi

    if ! run_checks >/dev/null 2>&1; then
        echo "ERROR: self-test failed - the guard rejected the clean tree."
        exit 1
    fi

    echo "OK: self-test - the guard fails a duplicated literal and passes the clean tree."
    exit 0
fi

if run_checks; then
    echo "OK: the default connector-catalogue endpoint is declared once in $CANONICAL."
    exit 0
fi
exit 1
