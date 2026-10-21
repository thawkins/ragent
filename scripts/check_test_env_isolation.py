#!/usr/bin/env python3
"""CI guard: reject unisolated process-global access in integration tests.

Each crate builds ONE test binary from its whole ``tests/`` tree, so every
``tests/test_*.rs`` file shares a single process. A ``std::env::set_var`` /
``remove_var`` / ``set_current_dir`` in one child file is therefore observed by
every concurrently running test in the other child files - which is how
``test_experimental_flags::test_experimental_round_trips_through_config_load``
came to fail about once in 40 unsharded runs (it won the race against a
``#[serial]`` test in another child file, because ``serial_test``'s default lock
and a file-local mutex are different mutexes).

Every test that mutates a process global must show isolation on the spot:

``serial``
    ``#[serial]`` / ``#[serial_test::serial]`` directly above or below the
    ``#[test]`` attribute. ``serial_test``'s default lock is per test *target*,
    so under one binary per crate every ``#[serial]`` test in the crate contends
    on the same lock.

``mutex``
    A ``crate::global_env_lock()`` acquisition (the crate-wide lock declared in
    ``tests/all.rs``) or another ``MutexGuard`` acquisition in the test body.

``marker``
    A ``// env-isolation: <reason>`` comment, for the cases where the guard is
    taken by a shared helper the test calls. The reason is mandatory, mirroring
    ``check-dead-code-reasons.sh``: an unguarded mutation that cannot name its
    guard is the bug.

Two further rules, both about locks that do not actually exclude each other:

``mixed families``
    A crate that isolates with BOTH ``#[serial]`` and file-local mutexes while
    declaring no crate-wide ``global_env_lock`` in ``tests/all.rs``.

``re-acquisition``
    A test that takes ``global_env_lock()`` and then calls a same-crate helper
    that takes it too. ``std::sync::Mutex`` is not reentrant, so the second
    acquisition self-deadlocks and the whole test binary hangs - which is what a
    harness helper such as ``slash_support::enter_project`` would do.

Usage: python3 scripts/check_test_env_isolation.py
Exit code: 0 = clean, 1 = an isolation violation was found.
"""

from __future__ import annotations

import pathlib
import re
import sys

GLOBAL_WRITE = re.compile(r"\b(?:set_var|remove_var|set_current_dir)\s*\(")
TEST_ATTR = re.compile(r"^\s*#\[(?:tokio::test|test)\b")
ATTR_LINE = re.compile(r"^\s*#\[")
COMMENT_LINE = re.compile(r"^\s*//")
SERIAL_ATTR = re.compile(r"#\[(?:serial_test::serial|serial)\b")
MUTEX_GUARD = re.compile(
    r"global_env_lock\(\)"
    r"|\.lock\(\)"
    r"|\.read\(\)"
    r"|MutexGuard"
    r"|\b_lock\s*="
    r"|\b_guard\s*=\s*[A-Za-z_:]+\(\)"
)
MARKER = re.compile(r"//\s*env-isolation:\s*\S+")
LOCK = re.compile(r"global_env_lock\s*\(\s*\)")
PLAIN_FN = re.compile(r"\s*(?:pub(?:\s*\([^)]*\))?\s+)?(?:async\s+)?fn\s+(\w+)")
FUNCTION_NAME = re.compile(r"fn\s+(\w+)")
LOCK_ACCESSORS = frozenset({"global_env_lock", "global_env_mutex"})


def blank_literals(text: str) -> str:
    """Blank string/char literal contents and comments, preserving offsets.

    Brace counting must ignore ``{"a":1}`` inside a raw string and a ``}`` in a
    comment, or a test body runs past its own closing brace and swallows the
    following tests.
    """
    out = list(text)
    i = 0
    n = len(text)
    while i < n:
        ch = text[i]
        if ch == "/" and i + 1 < n and text[i + 1] == "/":
            while i < n and text[i] != "\n":
                out[i] = " "
                i += 1
        elif ch == '"':
            i += 1
            while i < n and text[i] != '"':
                if text[i] == "\\" and i + 1 < n:
                    out[i] = " "
                    i += 1
                # Keep newlines so the cleaned text has the SAME line count as
                # the raw text (a multi-line string literal must not merge the
                # lines around it, or every later line index shifts).
                if text[i] != "\n":
                    out[i] = " "
                i += 1
            i += 1
        else:
            i += 1
    return "".join(out)


def body_bounds(clean_lines: list[str], start: int) -> int:
    """Index of the last line of the item starting at ``start``."""
    j = start
    depth = 0
    started = False
    while j < len(clean_lines):
        depth += clean_lines[j].count("{") - clean_lines[j].count("}")
        started = started or "{" in clean_lines[j]
        if started and depth <= 0:
            return j
        j += 1
    return len(clean_lines) - 1


def attribute_run(clean_lines: list[str], raw_lines: list[str], index: int) -> tuple[int, int]:
    """Line span covering the attributes attached to the test at ``index``."""
    start = index
    while start > 0 and (
        ATTR_LINE.match(clean_lines[start - 1])
        or COMMENT_LINE.match(raw_lines[start - 1])
        or not clean_lines[start - 1].strip()
    ):
        start -= 1
    j = index
    while j < len(clean_lines) and (
        ATTR_LINE.match(clean_lines[j]) or COMMENT_LINE.match(raw_lines[j]) or not clean_lines[j].strip()
    ):
        j += 1
    return start, j


def split_tests(raw_lines: list[str], clean_lines: list[str]):
    """Yield ``(line_index, attributes_text, body_text, name)`` per test."""
    i = 0
    while i < len(clean_lines):
        if not TEST_ATTR.match(clean_lines[i]):
            i += 1
            continue
        attrs_start, body_start = attribute_run(clean_lines, raw_lines, i)
        end = body_bounds(clean_lines, body_start)
        attrs = "\n".join(raw_lines[attrs_start:body_start])
        body = "\n".join(raw_lines[body_start : end + 1])
        name_match = FUNCTION_NAME.search(body)
        yield i, attrs, body, name_match.group(1) if name_match else "<anonymous>"
        i = end + 1


def plain_functions(raw_lines: list[str], clean_lines: list[str]):
    """Yield ``(name, body_text)`` for every non-test ``fn`` in a file."""
    i = 0
    while i < len(clean_lines):
        if TEST_ATTR.match(clean_lines[i]):
            i = body_bounds(clean_lines, i) + 1
            continue
        match = PLAIN_FN.match(clean_lines[i])
        if match is None or "{" not in "".join(clean_lines[i : i + 8]):
            i += 1
            continue
        brace_line = i
        while brace_line < len(clean_lines) and "{" not in clean_lines[brace_line]:
            brace_line += 1
        end = body_bounds(clean_lines, brace_line)
        yield match.group(1), "\n".join(raw_lines[i : end + 1])
        i = end + 1


def isolation(attrs: str, body: str, harnesses: frozenset[str]) -> str | None:
    """The isolation family the test holds, or ``None`` when unisolated.

    ``harnesses`` names the same-crate helper functions whose body acquires the
    crate-wide lock: calling one hands the caller that lock for the whole test
    body (the guard is returned and bound by the caller), so it counts as
    ``mutex``.
    """
    if SERIAL_ATTR.search(attrs):
        return "serial"
    if MUTEX_GUARD.search(body) or MUTEX_GUARD.search(attrs):
        return "mutex"
    for helper in harnesses:
        if re.search(rf"\b{re.escape(helper)}\s*\(", body):
            return "mutex"
    if MARKER.search(attrs) or MARKER.search(body):
        return "marker"
    return None


def load_tests(path: pathlib.Path) -> tuple[list[str], list[str]] | None:
    try:
        raw = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return None
    return raw.splitlines(), blank_literals(raw).splitlines()


def main() -> int:
    offenders: list[str] = []
    families_seen: dict[str, dict[str, list[str]]] = {}

    crate_dirs = sorted(p for p in pathlib.Path("crates").iterdir() if p.is_dir())

    for crate_dir in crate_dirs:
        crate = crate_dir.name
        loaded: dict[pathlib.Path, tuple[list[str], list[str]]] = {}
        for path in sorted((crate_dir / "tests").rglob("*.rs")):
            data = load_tests(path)
            if data is not None:
                loaded[path] = data

        # A helper "holds the lock" only when its OWN body takes it. Do not
        # close transitively: that drags in every unrelated helper that happens
        # to reach a locking fn through several layers (`make_app`, `new`,
        # `default`) and produces false positives.
        #
        # `harnesses` are the subset whose signature RETURNS the guard (e.g.
        # `fn isolated_app() -> (MutexGuard<'static, ()>, ...)`): calling one
        # hands the caller the lock for the whole body, so it satisfies rule 1.
        # Every direct locker still trips rule 2 when a test holds the lock and
        # calls it - that is the self-deadlock.
        helper_bodies = {
            name: body
            for raw_lines, clean_lines in loaded.values()
            for name, body in plain_functions(raw_lines, clean_lines)
            if name not in LOCK_ACCESSORS
        }
        locking_helpers = {name for name, body in helper_bodies.items() if LOCK.search(body)}
        # Guard-bearing aliases make a helper a harness without naming
        # `MutexGuard` in its own signature (`type Harness = (MutexGuard<..>,
        # ..);`), so collect them across the whole crate's test tree.
        crate_text = "\n".join(
            "\n".join(raw_lines) for raw_lines, _clean_lines in loaded.values()
        )
        guard_aliases = {
            m.group(1)
            for m in re.finditer(
                r"\btype\s+(\w+)\s*=[^;]*MutexGuard", crate_text, re.DOTALL
            )
        }

        def returns_guard(name: str, body: str) -> bool:
            signature = body.split("{", 1)[0]
            if "MutexGuard" in signature:
                return True
            return any(
                re.search(rf"\b{re.escape(alias)}\b", signature)
                for alias in guard_aliases
            )

        # A helper "holds the lock for its caller" when it returns the guard AND
        # (directly, or through another harness) takes the lock. Only these
        # satisfy rule 1; every direct locker still trips rule 2 when a test
        # holds the lock and calls it - that is the self-deadlock.
        harnesses = {
            name
            for name, body in helper_bodies.items()
            if returns_guard(name, body) and name in locking_helpers
        }
        changed = True
        while changed:
            changed = False
            for name, body in helper_bodies.items():
                if name in harnesses or not returns_guard(name, body):
                    continue
                if any(
                    helper != name
                    and re.search(rf"\b{re.escape(helper)}\s*\(", body)
                    for helper in harnesses
                ):
                    harnesses.add(name)
                    changed = True

        # -- rule 1: every mutating test carries isolation ---------------------
        for path, (raw_lines, clean_lines) in loaded.items():
            if path.name == "all.rs":
                continue
            for start, attrs, body, name in split_tests(raw_lines, clean_lines):
                if not GLOBAL_WRITE.search(body):
                    continue
                family = isolation(attrs, body, frozenset(harnesses))
                where = f"{path}:{start + 1}: test `{name}`"
                if family is None:
                    offenders.append(
                        f"{where} mutates a process global without `#[serial]`, a "
                        "crate-wide env lock, or a `// env-isolation: <reason>` marker"
                    )
                    continue
                families_seen.setdefault(crate, {}).setdefault(family, []).append(where)

        # -- rule 2: no non-reentrant re-acquisition --------------------------
        for path, (raw_lines, clean_lines) in loaded.items():
            if path.name == "all.rs":
                continue
            for start, _attrs, body, name in split_tests(raw_lines, clean_lines):
                if not LOCK.search(body):
                    continue
                for helper in sorted(locking_helpers):
                    if helper == name or not re.search(rf"\b{re.escape(helper)}\s*\(", body):
                        continue
                    offenders.append(
                        f"{path}:{start + 1}: test `{name}` takes `global_env_lock()` and "
                        f"also calls `{helper}()`, which takes it too - "
                        "`std::sync::Mutex` is not reentrant, so this self-deadlocks"
                    )

    # -- rule 3: a crate must not mix families without a shared lock ----------
    shared_lock: dict[str, bool] = {}
    for all_rs in pathlib.Path("crates").glob("*/tests/all.rs"):
        shared_lock[all_rs.parts[1]] = (
            "global_env_lock"
            in all_rs.read_text(encoding="utf-8", errors="replace")
        )

    for crate, families in sorted(families_seen.items()):
        if {"serial", "mutex"} <= set(families) and not shared_lock.get(crate):
            offenders.append(
                f"crate `{crate}` isolates process-global mutation with BOTH `#[serial]` "
                "and file-local mutexes but declares no crate-wide `global_env_lock` in "
                f"crates/{crate}/tests/all.rs, so the two groups do not exclude each "
                "other. Mutex-only tests:\n    " + "\n    ".join(families["mutex"][:5])
            )

    if offenders:
        print("ERROR: test process-global isolation violations:\n")
        for entry in sorted(set(offenders)):
            print(f"  {entry}")
        print(
            "\nFix: give every mutating test of a crate the SAME isolation - "
            "`#[serial_test::serial]` everywhere, or `let _guard = crate::global_env_lock();` "
            "everywhere (`tests/all.rs` declares it). When a shared helper already holds the "
            "lock, do NOT take it again (it is not reentrant) and say so with "
            "`// env-isolation: <reason>`. See AGENTS.md (\"Test Layout\")."
        )
        return 1

    print("OK: every process-global-mutating test takes shared, non-reentrant isolation.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
