#!/usr/bin/env python3
"""CI guard (ANTIPAT M2.17): reject *genuine* inline ``#[cfg(test)]`` modules in
library and binary sources.

AGENTS-RUST.md requires tests to live in each crate's ``tests/`` directory. The
workspace migration (REMPLAN Milestone 5) relocated every test body out of the
source tree, leaving only ``#[cfg(test)] #[path = "../tests/inline/.."] mod x;``
hooks behind -- an idiomatic, allowlisted pattern that names an external file.

The pre-M2.17 guard counted any file containing the substring ``mod tests``,
which (a) also counted the ``#[path]`` hooks, forcing an inflated 129-file
baseline, and (b) never scanned the root ``src/`` tree at all. This scanner
counts only modules that carry their test *body* inline (a ``mod x {`` block
directly under ``#[cfg(test)]`` with no ``#[path]`` attribute), across both
``crates/*/src`` and the root ``src/``.

Scope: ``crates/*/src/**/*.rs`` and ``src/**/*.rs``. The baseline is shrink-only
and currently zero, so any new inline body fails immediately.

Usage:
    python3 scripts/check_inline_tests.py
    python3 scripts/check_inline_tests.py --list
    python3 scripts/check_inline_tests.py --self-test

Exit code: 0 = clean, 1 = at least one genuine inline module was found.
"""

from __future__ import annotations

import pathlib
import re
import sys

# Shrink-only baseline: genuine inline `#[cfg(test)] mod x { .. }` bodies that
# remain in production sources. Every M2 relocation moved a body to `tests/`,
# so the correct value is zero; lower it further only by migrating, never raise.
INLINE_BASELINE = 0

CFG_TEST = "#[cfg(test)]"
PATH_HOOK = "#[path"
INLINE_MOD_DECL = re.compile(r"^\s*mod\s+[A-Za-z_][A-Za-z0-9_]*\s*\{")


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
    return files


def has_inline_test_body(src: str) -> bool:
    """True when any ``#[cfg(test)]`` attribute is directly followed by an
    inline ``mod x {`` body rather than a ``#[path]`` external hook."""
    lines = src.splitlines()
    for idx, line in enumerate(lines):
        if CFG_TEST not in line:
            continue
        window = lines[idx + 1 : idx + 4]
        joined = "\n".join(window)
        if PATH_HOOK in joined:
            # External hook: the module body lives in tests/.
            continue
        if any(INLINE_MOD_DECL.match(candidate) for candidate in window):
            return True
    return False


def find_inline_modules() -> list[str]:
    """Return the relative paths of files holding a genuine inline test body."""
    offenders: list[str] = []
    for path in iter_sources():
        try:
            src = path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        if has_inline_test_body(src):
            offenders.append(str(path))
    return offenders


def self_test() -> int:
    """Seed a genuine inline module in a throwaway source file and confirm the
    guard flags it, then confirm a ``#[path]`` hook is *not* flagged.

    Covers both ``crates/*/src`` and the root ``src/`` tree (R-01): a guard
    that only scanned the crates would pass the old root-``src`` blind spot."""
    seed_dirs = [pathlib.Path("crates/ragent-types/src"), pathlib.Path("src")]
    for seed_dir in seed_dirs:
        if not seed_dir.is_dir():
            print(
                f"ERROR: self-test could not locate {seed_dir}",
                file=sys.stderr,
            )
            return 1

    seeds: list[tuple[pathlib.Path, str]] = []
    for seed_dir in seed_dirs:
        inline_seed = seed_dir / "__inline_tests_guard_selftest.rs"
        hook_seed = seed_dir / "__inline_tests_guard_selftest_hook.rs"
        inline_seed.write_text(
            "pub fn f() -> u32 { 1 }\n"
            "\n"
            "#[cfg(test)]\n"
            "mod tests {\n"
            "    #[test]\n"
            "    fn works() { assert_eq!(super::f(), 1); }\n"
            "}\n",
            encoding="utf-8",
        )
        hook_seed.write_text(
            "pub fn f() -> u32 { 1 }\n"
            "\n"
            "#[cfg(test)]\n"
            '#[path = "../tests/inline/selftest_hook.rs"]\n'
            "mod tests;\n",
            encoding="utf-8",
        )
        seeds.append((inline_seed, "inline"))
        seeds.append((hook_seed, "hook"))

    try:
        for seed, kind in seeds:
            flagged = has_inline_test_body(seed.read_text(encoding="utf-8"))
            if kind == "inline" and not flagged:
                print(
                    f"ERROR: self-test failed - the guard missed a seeded inline module in {seed}.",
                    file=sys.stderr,
                )
                return 1
            if kind == "hook" and flagged:
                print(
                    f"ERROR: self-test failed - the guard flagged a #[path] hook in {seed}.",
                    file=sys.stderr,
                )
                return 1
        # The scanner's file walk must also reach a seeded root-src file.
        root_offenders = [
            str(p)
            for p in iter_sources()
            if "__inline_tests_guard_selftest.rs" in str(p)
            and has_inline_test_body(p.read_text(encoding="utf-8"))
        ]
        if not any(path.startswith("src/") for path in root_offenders):
            print(
                "ERROR: self-test failed - the guard does not scan the root src/ tree.",
                file=sys.stderr,
            )
            return 1
        print(
            "OK: self-test - an inline module fails the guard and a #[path] hook passes,"
            " in both crates/*/src and root src/."
        )
        return 0
    finally:
        for seed, _ in seeds:
            seed.unlink(missing_ok=True)


def main() -> int:
    if "--self-test" in sys.argv:
        return self_test()

    offenders = find_inline_modules()

    if "--list" in sys.argv:
        for path in offenders:
            print(path)
        print(f"Total: {len(offenders)} file(s) with an inline test body.")
        return 0

    if len(offenders) > INLINE_BASELINE:
        print(
            f"ERROR: {len(offenders)} file(s) declare a genuine inline "
            f"#[cfg(test)] module (baseline: {INLINE_BASELINE})"
        )
        print("New inline test blocks are not allowed in src/ files (AGENTS-RUST.md).")
        print("Move the test body to the crate's tests/ directory and keep only a")
        print('#[cfg(test)] #[path = "../tests/inline/<name>.rs"] mod tests; hook.')
        print("")
        print("Offending files:")
        for path in offenders:
            print(f"  {path}")
        return 1

    print(
        f"OK: {len(offenders)} file(s) with an inline test body "
        f"(baseline: {INLINE_BASELINE})"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
