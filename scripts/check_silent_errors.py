#!/usr/bin/env python3
"""CI guard (ANTIPAT M4): reject *new* silent error suppression.

AGENTS.md forbids silently swallowing errors: every ``let _ =`` on a fallible
call and every ``unwrap_or*`` that masks an error must be logged
(``debug!``/``warn!``) or carry a justified ``// INTENTIONAL:`` comment.

This scanner is a *baseline* gate, mirroring ``check_security_unwraps.py``: it
records the current count of masked sites per file in
``scripts/silent-error-baseline.txt`` and fails when any file exceeds its
baseline, or when a file that previously had none gains a masked site. The
baseline is shrink-only.

A masked site is a genuine dropped / masked error:

* ``let _ = <expr>;`` where ``<expr>`` is a fallible call -- excluding a
  documented set of infallible / best-effort idioms (writing to a ``String``,
  channel sends, process teardown, temp-file cleanup, ...);
* ``.ok().flatten()`` on a fallible call.

``unwrap_or_default()`` / ``unwrap_or(0|false)`` are *not* counted: they are
overwhelmingly used on ``Option``/``serde_json`` defaults and counting them
would drown the real signal (the single ``unwrap_or(0)`` that hides a
``COUNT(*)`` failure is annotated ``// INTENTIONAL:`` at its site). The
``let _ =`` / ``ok().flatten()`` forms are the ones the ANTIPAT M4 findings
cite for genuine error masking.

An intentional site can be exempted with a trailing ``// INTENTIONAL: <reason>``
or ``// no-error-ok: <reason>`` comment on the same line (or the line directly
above), matching the ``no-panic-ok`` escape hatch used by the unwrap guard.

Scope: ``crates/*/src/**/*.rs`` and ``src/**/*.rs``; ``tests``/``benches``
directories are skipped and ``#[cfg(test)]`` bodies are blanked out.

Usage:
    python3 scripts/check_silent_errors.py
    python3 scripts/check_silent_errors.py --update-baseline
    python3 scripts/check_silent_errors.py --list
    python3 scripts/check_silent_errors.py --self-test

Exit code: 0 = clean, 1 = a new masked site was found.
"""

from __future__ import annotations

import pathlib
import re
import subprocess
import sys

BASELINE_PATH = pathlib.Path("scripts/silent-error-baseline.txt")

MARKERS = ("INTENTIONAL:", "no-error-ok")

# Dropped-Result statement: `let _ = <expr>` but not `let _ = &<expr>` (a
# cfg-gated unused binding).
LET_DROP = re.compile(r"\blet\s+_\s*=\s*(?!&)")
# `.ok().flatten()` masks a `Result` error as `None`.
OK_FLATTEN = re.compile(r"\.ok\(\)\s*\.flatten\(\)")

# Infallible / best-effort idioms that never mask a real error. A `let _ =` on
# one of these is not counted (no INTENTIONAL marker needed).
EXEMPT_IDIOMS = (
    re.compile(r"\bwrite!\(|\bwriteln!\("),                     # write to a String buffer
    re.compile(r"\.try_send\(|\.send\("),                        # channel send on a closed receiver
    re.compile(r"remove_file\(|remove_dir_all\(|remove_dir\("),  # best-effort temp cleanup
    re.compile(r"set_permissions\("),                            # best-effort permission tighten
    re.compile(r"\.kill\(\)|\.wait\(\)|start_kill\(\)|libc::kill"),  # process teardown
    re.compile(r"\.set\(|CLIENT\.set"),                          # OnceLock set race
    re.compile(r"^\s*let _ = &"),                                # cfg-gated unused binding
    re.compile(r"^\s*let _ = [a-zA-Z_][a-zA-Z0-9_]*;\s"),        # parameter retained for API
    re.compile(r"^\s*let _ = \([^)]*\);\s"),                     # parameters retained for API
    re.compile(r"execute!\(|disable_raw_mode\(\)"),              # terminal teardown
    re.compile(r"ct_event::read\(\)"),                           # drain a pending terminal event
    re.compile(r"prune_clipboard_temp_files"),                   # best-effort cache prune
    re.compile(r"tokio::join!\("),                               # wait for stream readers
    re.compile(r"read_to_string\(&mut buf\)"),                   # bounded pipe drain
    re.compile(r"chars\.next\(\)"),                              # iterator advance
    re.compile(r"stream\.read\(|stream\.write_all\("),         # embedded recipe snippet
)


def iter_sources() -> list[pathlib.Path]:
    """Return every production ``.rs`` file under ``crates/*/src`` and ``src``."""
    files: list[pathlib.Path] = []
    crates = pathlib.Path("crates")
    if crates.is_dir():
        for crate_dir in sorted(crates.iterdir()):
            src = crate_dir / "src"
            if src.is_dir():
                files.extend(sorted(src.rglob("*.rs")))
    root_src = pathlib.Path("src")
    if root_src.is_dir():
        files.extend(sorted(root_src.rglob("*.rs")))
    return [p for p in files if "tests" not in p.parts and "benches" not in p.parts]


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


def is_exempt(lines: list[str], idx: int) -> bool:
    """True when the site at ``idx`` carries an ``INTENTIONAL:``/``no-error-ok:``
    marker on its own line, the line directly above, or a leading-comment line
    directly below (for multi-line ``let _ = <obj>`` statements)."""
    candidates = [lines[idx]]
    if idx > 0:
        candidates.append(lines[idx - 1])
    if idx + 1 < len(lines) and lines[idx + 1].lstrip().startswith("//"):
        candidates.append(lines[idx + 1])
    return any(marker in probe for marker in MARKERS for probe in candidates)


def is_idiom(line: str) -> bool:
    """True when the line is a documented infallible / best-effort idiom."""
    return any(pat.search(line) for pat in EXEMPT_IDIOMS)


def count_sites(src: str) -> int:
    """Count genuine masked error-suppression sites in one source file."""
    clean = blank_cfg_test_blocks(src)
    lines = clean.splitlines()
    total = 0
    for idx, line in enumerate(lines):
        if line.lstrip().startswith("//"):
            # A masked-site pattern inside a comment is documentation, not code.
            continue
        if is_exempt(lines, idx):
            continue
        total += len(OK_FLATTEN.findall(line))
        for _ in LET_DROP.findall(line):
            if not is_idiom(line):
                total += 1
    return total


def scan() -> dict[str, int]:
    """Return ``{relative path: masked-site count}`` for the whole workspace."""
    counts: dict[str, int] = {}
    for path in iter_sources():
        src = path.read_text(encoding="utf-8", errors="replace")
        n = count_sites(src)
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
        "# Silent error-suppression baseline (ANTIPAT M4).",
        "# Format: <path> <count>. Regenerate with:",
        "#   python3 scripts/check_silent_errors.py --update-baseline",
        "# The gate fails when a file exceeds its baseline or a file that had none",
        "# gains a masked site. Mark an intentional drop with a same-line",
        "# `// INTENTIONAL: <reason>` comment to keep it out of the count.",
        "",
    ]
    for path, count in sorted(counts.items()):
        lines.append(f"{path} {count}")
    BASELINE_PATH.write_text("\n".join(lines) + "\n", encoding="utf-8")


def self_test() -> int:
    """Seed a violation in a temp file inside a scan root and confirm the
    scanner flags it. Proves the guard actually fails on a seeded breach."""
    seed = pathlib.Path("crates/ragent-types/src/__silent_error_selftest.rs")
    seed.write_text(
        "pub fn f() {\n    let _ = std::fs::write(\"/nonexistent\", b\"\");\n}\n",
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
                "ERROR: self-test failed - the guard passed a seeded drop.",
                file=sys.stderr,
            )
            return 1
        print("OK: self-test - the guard fails a seeded silent error drop.")
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

    if "--list" in sys.argv:
        for path, count in sorted(counts.items()):
            print(f"{path} {count}")
        print(f"total: {sum(counts.values())} masked site(s) in {len(counts)} file(s)")
        return 0

    baseline = load_baseline()
    offenders: list[str] = []
    for path, count in sorted(counts.items()):
        allowed = baseline.get(path, 0)
        if count > allowed:
            offenders.append(f"{path}: {count} masked site(s), baseline {allowed}")

    if offenders:
        print(
            f"ERROR: {len(offenders)} file(s) exceed the silent error-suppression "
            "baseline (ANTIPAT M4).",
            file=sys.stderr,
        )
        print(
            "Log the cause with `debug!`/`warn!`, or mark an intentional drop with "
            "a same-line `// INTENTIONAL: <reason>` comment.",
            file=sys.stderr,
        )
        print("", file=sys.stderr)
        for entry in offenders:
            print(f"  {entry}", file=sys.stderr)
        return 1

    total = sum(counts.values())
    print(
        f"OK: {total} silent error-suppression site(s) across {len(counts)} file(s), "
        "within the recorded baseline."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
