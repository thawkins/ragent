#!/usr/bin/env python3
"""CI guard (ANTIPAT M1.16): reject non-ASCII in production Rust source.

``AGENTS-RUST.md`` states: "Exclude emojis and non-ASCII characters from
comments and identifiers." The workspace still carried ~5,900 non-ASCII lines
across 491 files when ANTIPAT.md was written: em-dashes and ellipses in prose,
emoji in tool output, mojibake ``U+FFFD`` bytes committed into source, and a
large amount of box-drawing used as section rules.

This scanner enforces the rule with a documented allowlist so the sweep cannot
regress:

* ``ALLOWED_RANGES`` - box-drawing, block-element, and braille codepoints, which
  ``AGENTS-RUST.md`` permits in user-facing terminal output and which are used
  for progress bars, section rules, and spinners.
* ``ALLOWED_CODEPOINTS`` - individual glyphs that are deliberately kept:
  ``U+2550`` (double horizontal, the ``===`` rule used in ``message_widget``)
  and the StatusBar / tool-category icons in ``layout_statusbar.rs`` and
  ``widgets/message_widget.rs``.
* ``ALLOWED_FILES`` - whole files exempt because they exist to render icons
  (``theme.rs``, ``layout_statusbar.rs``, ``widgets/message_widget.rs``).
  ANTIPAT.md M1.1 says the StatusBar icons and the message-window tool-category
  icons must NOT be removed.

Scope: ``crates/*/src/**/*.rs`` and root ``src/**/*.rs``. Test and bench
directories are skipped, and ``#[cfg(test)]`` module bodies are blanked out -
fixtures such as ``"café"`` exist to exercise multi-byte handling and are not
production text.

Usage:
    python3 scripts/check_non_ascii.py
    python3 scripts/check_non_ascii.py --self-test

Exit code: 0 = clean, 1 = a disallowed non-ASCII codepoint was found.
"""

from __future__ import annotations

import pathlib
import subprocess
import sys

# Inclusive codepoint ranges that are always allowed.
ALLOWED_RANGES: tuple[tuple[int, int, str], ...] = (
    (0x2500, 0x257F, "box drawing"),
    (0x2580, 0x259F, "block elements"),
    (0x2800, 0x28FF, "braille patterns"),
)

# Individual codepoints that are deliberately kept.
ALLOWED_CODEPOINTS: dict[int, str] = {
    0x2550: "double horizontal rule",
}

# Whole-file exemptions: files whose purpose is to define the icon set.
#
# ANTIPAT.md M1.1: "Do Not remove icons in the StatusBar or the Icons used to
# represent tool categories in the messagewindow widgets".
ALLOWED_FILES: dict[str, str] = {
    "crates/ragent-tui/src/theme.rs": "icon registry",
    "crates/ragent-tui/src/layout_statusbar.rs": "StatusBar icon registry",
    "crates/ragent-tui/src/widgets/message_widget.rs": "tool-category icons",
    "crates/ragent-tui/src/logo.rs": "ASCII-art logo glyphs",
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


def is_allowed(codepoint: int) -> bool:
    """Return True when ``codepoint`` is on the documented allowlist."""
    if codepoint in ALLOWED_CODEPOINTS:
        return True
    return any(lo <= codepoint <= hi for lo, hi, _name in ALLOWED_RANGES)


def source_files() -> list[pathlib.Path]:
    """Return every production Rust source file in the scan scope."""
    files: list[pathlib.Path] = []
    crates = pathlib.Path("crates")
    if crates.is_dir():
        for path in sorted(crates.glob("*/src/**/*.rs")):
            parts = path.parts
            if "tests" in parts or "benches" in parts:
                continue
            files.append(path)
    root_src = pathlib.Path("src")
    if root_src.is_dir():
        files.extend(sorted(root_src.rglob("*.rs")))
    return files


def scan() -> dict[str, list[tuple[int, int, str]]]:
    """Return ``{path: [(line, column, char), ...]}`` for disallowed glyphs."""
    findings: dict[str, list[tuple[int, int, str]]] = {}
    for path in source_files():
        rel = path.as_posix()
        if rel in ALLOWED_FILES:
            continue
        text = blank_cfg_test_blocks(path.read_text(encoding="utf-8", errors="replace"))
        hits: list[tuple[int, int, str]] = []
        for lineno, line in enumerate(text.splitlines(), 1):
            for col, ch in enumerate(line, 1):
                if ord(ch) <= 127:
                    continue
                if is_allowed(ord(ch)):
                    continue
                hits.append((lineno, col, ch))
        if hits:
            findings[rel] = hits
    return findings


def self_test() -> int:
    """Seed a disallowed codepoint inside the scan root and confirm failure."""
    seed = pathlib.Path("crates/ragent-types/src/__non_ascii_guard_selftest.rs")
    seed.write_text(
        "//! Self-test fixture for scripts/check_non_ascii.py.\n"
        'pub const BAD: &str = "an em dash \u2014 must fail";\n',
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
        print("OK: self-test - the guard fails a seeded non-ASCII codepoint.")
        return 0
    finally:
        seed.unlink(missing_ok=True)


def main() -> int:
    if "--self-test" in sys.argv:
        return self_test()

    findings = scan()
    if findings:
        total = sum(len(hits) for hits in findings.values())
        print(
            f"ERROR: {total} non-ASCII character(s) in {len(findings)} production "
            "source file(s) (ANTIPAT M1.16).",
            file=sys.stderr,
        )
        print(
            "Replace em/en dashes with '-', ellipses with '...', arrows with "
            "'->' / '<-' / '^' / 'v', and emoji with ASCII tags such as "
            "'[ok]' / '[warn]' / '[err]'.",
            file=sys.stderr,
        )
        print("", file=sys.stderr)
        for rel, hits in sorted(findings.items()):
            sample = ", ".join(
                f"L{lineno}:U+{ord(ch):04X}" for lineno, _col, ch in hits[:4]
            )
            more = "" if len(hits) <= 4 else f" (+{len(hits) - 4} more)"
            print(f"  {rel}: {len(hits)} - {sample}{more}", file=sys.stderr)
        return 1

    print("OK: no disallowed non-ASCII codepoints in production Rust source.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
