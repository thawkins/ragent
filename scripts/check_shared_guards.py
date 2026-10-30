#!/usr/bin/env python3
"""Shared-helper coverage guard (SECTASKS MS-05 T-067/T-069).

The point of MS-05 is that each guard has ONE implementation. This scanner
fails when a crate re-derives a guard the workspace already owns:

  * a local `fn reject_option_like`, `fn contained_join`,
    `fn validate_identifier`, `fn validate_relative_component`
    outside `crates/ragent-types/src/guard.rs`;
  * a second secret registry (`static SECRET_REGISTRY`) outside
    `crates/ragent-types/src/sanitize.rs`;
  * a re-derived truncation helper (`fn truncate_chars`, `fn truncate_bytes`)
    outside `crates/ragent-types/src/strutil.rs` - use the shared helpers, or
    add a genuinely different variant to `strutil` so the variants stay
    co-located;
  * a re-derived marketplace-ledger writer (`fn marketplaces_array_mut`,
    `fn read_or_empty_object`) outside
    `crates/ragent-surface/src/marketplace_ledger.rs` - the `/plugins` and
    `/connectors` ledgers share one implementation parameterised by the JSON
    root key.

Scope: ``crates/*/src/**/*.rs``. ``#[cfg(test)]`` module bodies are blanked out
and ``tests``/``benches`` directories are skipped, because a test-local helper is
not a production duplicate.

Usage:
    python3 scripts/check_shared_guards.py
    python3 scripts/check_shared_guards.py --self-test

Exit code: 0 = single implementation, 1 = a duplicate guard was found.
"""

from __future__ import annotations

import pathlib
import re
import subprocess
import sys

SCAN_ROOT = pathlib.Path("crates")

# The one file allowed to define each guard, plus the documented adapter sites.
GUARD_HOME = "crates/ragent-types/src/guard.rs"
SANITIZE_HOME = "crates/ragent-types/src/sanitize.rs"
STRUTIL_HOME = "crates/ragent-types/src/strutil.rs"
LEDGER_HOME = "crates/ragent-surface/src/marketplace_ledger.rs"

# Call-site preserving adapters. These keep a crate-local name for existing
# callers but delegate to the shared implementation in the same function body,
# so the rule itself still has one home.
ADAPTER_SITES: dict[str, str] = {
    "crates/ragent-bench/src/data.rs": "contained_join",
    "crates/ragent-tools-vcs/src/git/mod.rs": "reject_option_like",
    "crates/ragent-tui/src/app/helpers.rs": "truncate_chars",
}

# (regex, human description)
DUPLICATE_PATTERNS: tuple[tuple[re.Pattern[str], str], ...] = (
    (
        re.compile(r"\bfn\s+reject_option_like\s*\("),
        "reject_option_like (use ragent_types::guard::reject_option_like)",
    ),
    (
        re.compile(r"\bfn\s+contained_join\s*\("),
        "contained_join (use ragent_types::guard::contained_join)",
    ),
    (
        re.compile(r"\bfn\s+validate_identifier\s*\("),
        "validate_identifier (use ragent_types::guard::validate_identifier)",
    ),
    (
        re.compile(r"\bfn\s+validate_relative_component\s*\("),
        "validate_relative_component (use ragent_types::guard::validate_relative_component)",
    ),
    (
        re.compile(r"\bstatic\s+SECRET_REGISTRY\b"),
        "a second secret registry (use ragent_types::sanitize)",
    ),
    (
        re.compile(r"\bfn\s+truncate_chars\s*\("),
        "a local truncate_chars "
        "(use ragent_types::strutil::truncate_chars / truncate_chars_no_ellipsis)",
    ),
    (
        re.compile(r"\bfn\s+truncate_bytes\s*\("),
        "a local truncate_bytes "
        "(use ragent_types::strutil::truncate_bytes / truncate_bytes_no_ellipsis)",
    ),
    (
        re.compile(r"\bfn\s+marketplaces_array_mut\s*\("),
        "a second marketplace ledger writer "
        "(use ragent_surface::marketplace_ledger)",
    ),
    (
        re.compile(r"\bfn\s+read_or_empty_object\s*\("),
        "a second marketplace ledger reader "
        "(use ragent_surface::marketplace_ledger)",
    ),
)

# The shared implementation each adapter must reference, keyed by the guard.
DELEGATION_TARGETS: dict[str, str] = {
    "contained_join": "guard::contained_join",
    "reject_option_like": "guard::reject_option_like",
    "validate_identifier": "guard::validate_identifier",
    "validate_relative_component": "guard::validate_relative_component",
    "truncate_chars": "strutil::truncate_chars",
    "truncate_bytes": "strutil::truncate_bytes",
    "marketplaces_array_mut": "marketplace_ledger::",
    "read_or_empty_object": "marketplace_ledger::",
}


def blank_cfg_test_blocks(src: str) -> str:
    """Replace every ``#[cfg(test)]`` body with spaces, preserving offsets."""
    out = list(src)
    n = len(src)
    i = 0
    while True:
        m = src.find("#[cfg(test)]", i)
        if m == -1:
            break
        brace = src.find("{", m)
        if brace == -1:
            break
        depth = 0
        j = brace
        while j < n:
            ch = src[j]
            if ch == "{":
                depth += 1
            elif ch == "}":
                depth -= 1
                if depth == 0:
                    break
            j += 1
        for k in range(m, min(j + 1, n)):
            if out[k] != "\n":
                out[k] = " "
        i = j + 1
    return "".join(out)


def scan() -> list[str]:
    """Return a list of ``path:line: description`` duplicate-guard findings."""
    findings: list[str] = []
    for path in sorted(SCAN_ROOT.rglob("*.rs")):
        parts = path.parts
        if "tests" in parts or "benches" in parts:
            continue
        rel = str(path)
        src = path.read_text(encoding="utf-8", errors="replace")
        clean = blank_cfg_test_blocks(src)
        for lineno, line in enumerate(clean.splitlines(), 1):
            for pattern, description in DUPLICATE_PATTERNS:
                if not pattern.search(line):
                    continue
                if pattern.pattern.startswith(r"\bstatic"):
                    if rel == SANITIZE_HOME:
                        continue
                elif rel in (GUARD_HOME, STRUTIL_HOME, LEDGER_HOME):
                    continue
                else:
                    # An allowlisted adapter site is only acceptable when the
                    # function body actually delegates to the shared helper.
                    guard = ADAPTER_SITES.get(rel)
                    target = DELEGATION_TARGETS.get(guard or "")
                    if target and target in clean:
                        continue
                findings.append(f"{rel}:{lineno}: duplicate {description}")
    return findings


def self_test() -> int:
    """Seed a duplicate guard inside the scan root and confirm it is flagged."""
    seed = SCAN_ROOT / "ragent-types" / "src" / "__shared_guard_selftest.rs"
    seed.write_text(
        "pub fn validate_identifier(value: &str) -> bool { !value.is_empty() }\n",
        encoding="utf-8",
    )
    try:
        result = subprocess.run(
            [sys.executable, __file__],
            capture_output=True,
            text=True,
            check=False,
        )
        if result.returncode == 0:
            print(
                "ERROR: self-test failed - the guard passed a seeded duplicate.",
                file=sys.stderr,
            )
            return 1
        print("OK: self-test - the guard fails a seeded duplicate guard.")
        return 0
    finally:
        seed.unlink(missing_ok=True)


def main() -> int:
    if "--self-test" in sys.argv:
        return self_test()

    findings = scan()
    if findings:
        print(
            f"ERROR: {len(findings)} duplicate security guard(s) found "
            "(SECTASKS MS-05 T-067/T-069).",
            file=sys.stderr,
        )
        print(
            "Each guard has one implementation: ragent_types::guard for the "
            "path/operand/identifier/retry/cap helpers and ragent_types::sanitize "
            "for the secret registry.",
            file=sys.stderr,
        )
        print("", file=sys.stderr)
        for entry in findings:
            print(f"  {entry}", file=sys.stderr)
        return 1

    print("OK: every security guard has a single shared implementation.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
