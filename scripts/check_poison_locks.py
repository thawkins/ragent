#!/usr/bin/env python3
"""CI guard (FUNC-080): reject poisoning-panic lock access in production code.

A `.lock().unwrap()`, `.lock().expect("...poisoned")`, `.read().unwrap()`, or
`.write().unwrap()` on a `std::sync` lock panics every later caller once the
lock is poisoned. The required pattern is:

    .lock().unwrap_or_else(std::sync::PoisonError::into_inner)

This scanner walks every ``crates/*/src/**/*.rs`` file, blanks out
``#[cfg(test)]`` module bodies, and reports any remaining poisoning-panic lock
access. Test and bench directories are skipped entirely.

Usage: python3 scripts/check_poison_locks.py
Exit code: 0 = clean, 1 = offending lock access found.
"""

from __future__ import annotations

import pathlib
import re
import sys

# Panicking lock-access patterns matched as literal substrings.
LOCK_PATTERNS = (
    (".lock().unwrap()", "Mutex/RwLock .lock().unwrap()"),
    (".read().unwrap()", "RwLock .read().unwrap()"),
    (".write().unwrap()", "RwLock .write().unwrap()"),
)

# `.lock().expect("... poisoned")` / `.read().expect(..)` / `.write().expect(..)`.
POISON_EXPECT = re.compile(
    r"\.(?:lock|read|write)\(\)\s*\.expect\(\s*\"[^\"]*[Pp]oison"
)


def blank_cfg_test_blocks(src: str) -> str:
    """Replace every ``#[cfg(test)]`` module body with spaces, preserving
    byte offsets so reported line numbers stay accurate."""
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
    offenders: list[str] = []
    for path in pathlib.Path("crates").rglob("*.rs"):
        parts = path.parts
        if "tests" in parts or "benches" in parts:
            continue
        src = path.read_text(encoding="utf-8", errors="replace")
        clean = blank_cfg_test_blocks(src)
        for lineno, line in enumerate(clean.splitlines(), 1):
            stripped = line.strip()
            for needle, label in LOCK_PATTERNS:
                if needle in line:
                    offenders.append(f"{path}:{lineno}: {label}: {stripped[:100]}")
            if POISON_EXPECT.search(line):
                offenders.append(f"{path}:{lineno}: poison expect: {stripped[:100]}")
    return offenders


def main() -> int:
    offenders = scan()
    if offenders:
        print(
            f"ERROR: {len(offenders)} poisoning-panic lock access(es) in production "
            "code (FUNC-080).",
            file=sys.stderr,
        )
        print(
            "Use `.lock().unwrap_or_else(std::sync::PoisonError::into_inner)` "
            "instead of `.unwrap()`/`.expect()`.",
            file=sys.stderr,
        )
        print("", file=sys.stderr)
        for entry in offenders:
            print(f"  {entry}", file=sys.stderr)
        return 1
    print("OK: no poisoning-panic lock access in production code.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
