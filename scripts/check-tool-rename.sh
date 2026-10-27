#!/usr/bin/env bash
# CI guard (spec `uniformtools` NFR-008 and NFR-009, tasks T-034, T-047 and
# T-048; FR-027 for the literal predicate): fail when any of the seven NFR-008
# predicates holds - a retired `before` tool spelling in an alias table, a
# resolver match arm, a default configuration value, a registry key or a
# non-test source literal, a second registration of one name, or a tool whose
# `execute` forwards wholly to an earlier-registered tool.
#
# The rename retired thirteen spellings (FR-023) and adopted the Claude Code
# names. Any remaining literal that names a tool by the old spelling is a stale
# site the compiler cannot catch - it silently keys on a retired name. This
# guard is the build-time catch.
#
# There is ONE implementation: scripts/check_tool_rename.py. This wrapper exists
# so .github/workflows/ci.yml and pre-flight.sh can invoke the guard exactly like
# every other check-* script, and the crate test
# (crates/ragent-agent/tests/test_tool_rename_guard.rs) runs the same Python file
# with --json and asserts the same verdict, the same hits and the same ASCII
# wording. No predicate logic lives here, so the two forms cannot drift.
#
# What the one implementation does (one run, reporting every offending file and
# line for EVERY predicate, each labelled with its letter):
#   - reads the thirteen (before, after) pairs out of the FR-023 table
#     (crates/ragent-types/src/tool_rename.rs) rather than repeating them, so the
#     guard cannot disagree with the table it enforces;
#   - scans crates/*/src/**/*.rs and src/*.rs, skipping tests/ subtrees, for Rust
#     double-quoted and backticked literals whose decoded value is exactly one of
#     the thirteen retired spellings (predicate (e)). Single-quoted literals are
#     Rust chars, so they are not scanned; comment prose is not scanned either;
#   - reads an alias table row for every `Alias { .. }` block, every resolver file
#     and every line carrying an alias field, reporting a retired spelling as a
#     key or a target (predicate (a)) and a spelling mapped onto itself ((a2));
#   - reads every resolver match arm and every hand-written `Rename::after_of(..)`
#     lookup (predicate (b));
#   - reads ragent-config's `default = ".."` serde attributes and every literal in
#     a `fn default_*()` body (predicate (c));
#   - reads every `fn name(&self)` body, because a tool's registry key is what it
#     returns (predicate (d)), and counts them across the tree so two tool types
#     claiming one name are caught (predicate (f));
#   - reads each `fn execute` body and reports one that forwards to an
#     earlier-registered tool while deciding nothing (predicate (g));
#   - skips the two structural exemptions (the FR-023 table itself and the
#     CHANGELOG.md entry) and audits the per-line exemption table in both
#     directions, so a dead or widened entry fails too.
#
# Hermetic and cheap (NFR-009): reads only files inside the repository, makes no
# network request, never opens tools.xlsx, and finishes in about two seconds on
# the full tree - fast enough for cargo test and the CI guard set.
#
# Implementation: scripts/check_tool_rename.py.
#
# Usage: bash scripts/check-tool-rename.sh [--check|--json|--self-test|--exemptions] [--root DIR]
#   --check       (default) print the report and exit 0 or 1
#   --json        print the machine-readable report (the cargo test's input)
#   --self-test   seed the stale-literal fixture, assert it is reported, remove it
#   --exemptions  print the reviewed exemption list for a reviewer to read
#   --root DIR    scan DIR instead of this checkout
# Exit code: 0 = no NFR-008 predicate holds anywhere in the tree
#            1 = one or more predicate hits (or exemption problems) found
#            ASCII-only diagnostics (NFR-009), each naming the predicate letter
#            and the exact edit that clears it.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

exec python3 "$ROOT/scripts/check_tool_rename.py" "$@"
