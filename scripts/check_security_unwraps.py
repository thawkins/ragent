#!/usr/bin/env python3
"""CI guard (SECTASKS MS-05 T-070): reject new panicking `.unwrap()`/`.expect()`
in production code on a security-relevant path.

The workspace treats "no `.unwrap()`/`.expect()` on user-facing paths" as a
review-enforced rule (AGENTS.md Core Code Rules). That relies on a reviewer, so
a new `.unwrap()` in a parser, a path builder, or a network client can ship
unnoticed — exactly how SEC-ragent-types-003/005 and SEC-ragent-llm-006 arose.

This scanner is a *baseline* gate: it records the current count of panicking
unwrap/expect calls per file in ``scripts/security-unwrap-baseline.txt`` and
fails when any file exceeds its baseline, or when a file that previously had
none gains one. Existing sites are grandfathered with their counts, so the gate
is enforceable today and still blocks every new occurrence.

Scope: ``crates/*/src/**/*.rs``. ``#[cfg(test)]`` module bodies are blanked out
and ``tests``/``benches`` directories are skipped entirely, because test and
bench code may use ``.unwrap()`` freely.

Allowlisted calls (documented, intentional) can be exempted either by adding a
trailing ``// no-panic-ok: <reason>`` comment on the same line, or by placing
the call inside a block guarded by ``// no-panic-ok: <reason>`` on its own line.

Usage:
    python3 scripts/check_security_unwraps.py
    python3 scripts/check_security_unwraps.py --update-baseline
    python3 scripts/check_security_unwraps.py --self-test

Exit code: 0 = clean, 1 = a new panicking call was found.
"""

from __future__ import annotations

import pathlib
import re
import subprocess
import sys

BASELINE_PATH = pathlib.Path("scripts/security-unwrap-baseline.txt")
SCAN_ROOT = pathlib.Path("crates")

# `.unwrap()` / `.expect("...")` — any receiver. Matched textually, as the
# poisoning-lock guard does, so no type information is required.
PANIC_PATTERNS = (
    (".unwrap()", "unwrap"),
    (".expect(", "expect"),
)

# An inline escape hatch, e.g. `let x = re.compile(..).expect("valid"); // no-panic-ok: compile-time constant`
NO_PANIC_MARKER = "no-panic-ok"

# Doc comments carry illustrative examples (``/// let x = parse(s).unwrap();``)
# that are not production call sites. Counting them let a file with many
# doc-comment examples absorb a *real* new panic and stay under its grandfathered
# baseline - a netting-out bypass (code-audit Medium 4). They are skipped now.
DOC_COMMENT_PREFIXES = ("///", "//!")

# `.expect(` arguments may legitimately mention these words; they are not
# panicking sites in the sense this guard cares about.
IGNORED_SUBSTRINGS = (
    "unwrap_or_else",
    "unwrap_or_default",
    "unwrap_or(",
    "expect_err",  # test-only helper, kept for completeness
)


def blank_cfg_test_blocks(src: str) -> str:
    """Replace every ``#[cfg(test)]`` module body with spaces, preserving byte
    offsets so reported line numbers stay accurate."""
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


def count_sites(path: pathlib.Path, src: str) -> int:
    """Count panicking unwrap/expect call sites in one production source file.

    Doc-comment lines (``///`` and ``//!``) are skipped: their examples are
    illustrative, not call sites, and counting them let real panics hide under
    a grandfathered baseline.
    """
    clean = blank_cfg_test_blocks(src)
    total = 0
    for line in clean.splitlines():
        if line.lstrip().startswith(DOC_COMMENT_PREFIXES):
            continue
        if NO_PANIC_MARKER in line:
            continue
        if any(skip in line for skip in IGNORED_SUBSTRINGS):
            continue
        for needle, _label in PANIC_PATTERNS:
            total += line.count(needle)
    return total


def scan() -> dict[str, int]:
    """Return ``{relative path: panicking site count}`` for the whole workspace."""
    counts: dict[str, int] = {}
    for path in sorted(SCAN_ROOT.rglob("*.rs")):
        parts = path.parts
        if "tests" in parts or "benches" in parts:
            continue
        src = path.read_text(encoding="utf-8", errors="replace")
        n = count_sites(path, src)
        if n:
            counts[str(path)] = n
    return counts


def load_baseline() -> dict[str, int]:
    """Parse the checked-in baseline file into ``{path: count}``."""
    baseline: dict[str, int] = {}
    if not BASELINE_PATH.exists():
        return baseline
    for raw in BASELINE_PATH.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        path, _, count = line.rpartition(" ")
        if not path:
            continue
        try:
            baseline[path] = int(count)
        except ValueError:
            continue
    return baseline


def write_baseline(counts: dict[str, int]) -> None:
    """Rewrite the baseline file from the current scan."""
    lines = [
        "# Panicking unwrap/expect baseline (SECTASKS MS-05 T-070).",
        "# Format: <path> <count>. Regenerate with:",
        "#   python3 scripts/check_security_unwraps.py --update-baseline",
        "# The gate fails when a file exceeds its baseline or a file that had none",
        "# gains a panicking call. Mark an intentional call with a same-line",
        "# `// no-panic-ok: <reason>` comment to keep it out of the count.",
        "# Doc-comment examples (`///`, `//!`) are NOT counted: they are",
        "# illustrations, not call sites, and counting them would let a real new",
        "# panic hide under a grandfathered baseline (code-audit Medium 4).",
        "",
    ]
    for path, count in sorted(counts.items()):
        lines.append(f"{path} {count}")
    BASELINE_PATH.write_text("\n".join(lines) + "\n", encoding="utf-8")


def self_test() -> int:
    """Prove the guard fails on a seeded breach and counts only real call sites.

    Two properties are asserted, because the second was the actual bug
    (code-audit Medium 4):

    1. A new panicking call in a file with no baseline is flagged.
    2. Doc-comment examples are not counted, so a file cannot absorb a real new
       panic under a grandfathered baseline made up of ``///`` examples.
    """
    # Property 2 is checked directly against the counting function: the same
    # real call must count 1 whether or not doc examples surround it.
    doc_filled = (
        "//! Crate docs.\n"
        "/// Example.\n"
        "///\n"
        "/// ```\n"
        "/// let a: Option<u32> = Some(1);\n"
        "/// a.unwrap();\n"
        "/// a.expect(\"bad\");\n"
        "/// ```\n"
        "pub fn f() -> u32 {\n"
        "    let v: Option<u32> = Some(1);\n"
        "    v.unwrap()\n"
        "}\n"
    )
    direct = count_sites(pathlib.Path("selftest"), doc_filled)
    if direct != 1:
        print(
            "ERROR: self-test failed - doc-comment examples are counted as real "
            f"panicking calls (expected 1, got {direct}).",
            file=sys.stderr,
        )
        return 1

    seed = SCAN_ROOT / "ragent-types" / "src" / "__unwrap_guard_selftest.rs"
    seed.write_text(
        "pub fn f() -> u32 {\n"
        "    let v: Option<u32> = Some(1);\n"
        "    v.unwrap()\n"
        "}\n",
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
                "ERROR: self-test failed - the guard passed a seeded violation.",
                file=sys.stderr,
            )
            return 1
        print(
            "OK: self-test - the guard fails a seeded panicking unwrap and "
            "ignores doc-comment examples."
        )
        return 0
    finally:
        seed.unlink(missing_ok=True)


def main() -> int:
    if "--self-test" in sys.argv:
        return self_test()

    counts = scan()

    if "--update-baseline" in sys.argv:
        write_baseline(counts)
        print(f"OK: baseline updated with {len(counts)} file(s).")
        return 0

    baseline = load_baseline()
    offenders: list[str] = []
    for path, count in sorted(counts.items()):
        allowed = baseline.get(path, 0)
        if count > allowed:
            offenders.append(f"{path}: {count} panicking call(s), baseline {allowed}")

    if offenders:
        print(
            f"ERROR: {len(offenders)} file(s) exceed the panicking unwrap/expect "
            "baseline (SECTASKS MS-05 T-070).",
            file=sys.stderr,
        )
        print(
            "Propagate the error with `?`, or mark an intentional call with a "
            "same-line `// no-panic-ok: <reason>` comment.",
            file=sys.stderr,
        )
        print("", file=sys.stderr)
        for entry in offenders:
            print(f"  {entry}", file=sys.stderr)
        return 1

    total = sum(counts.values())
    print(
        f"OK: {total} panicking unwrap/expect call(s) across {len(counts)} file(s), "
        "within the recorded baseline."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
