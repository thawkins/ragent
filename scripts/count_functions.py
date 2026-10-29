#!/usr/bin/env python3
"""count_functions.py - count named function *definitions* in a source tree.

The primary target is Rust, where the counting is done over a masked copy of the
source (comments, string literals and char literals blanked out) so that ``fn``
text inside a comment or a doc-example string is never mistaken for a
definition.  Python (stdlib ``ast``) and a generic, explicitly approximate regex
mode for other languages are also supported.

Quick start::

    scripts/count_functions.py crates/ragent-config/src
    scripts/count_functions.py crates/ragent-tui/src --json
    scripts/count_functions.py src --lang rust --tests exclude --list
    scripts/count_functions.py . --lang generic --ext .go --ext .ts

See ``scripts/count_functions.py --help`` for every option and
``docs/howtos/count-functions.md`` for the counting rules, the categories and
the known limits of each mode.

Exit status is 0 on success and 2 on a usage or I/O error.
"""

from __future__ import annotations

import argparse
import ast
import bisect
import json
import os
import re
import sys
from dataclasses import dataclass
from pathlib import Path

# --------------------------------------------------------------------------- #
# Language tables
# --------------------------------------------------------------------------- #

RUST_EXTS = {".rs"}
PYTHON_EXTS = {".py", ".pyi"}
# Approximate mode: languages whose function definitions start with a keyword.
GENERIC_EXTS = {
    ".go", ".js", ".jsx", ".mjs", ".cjs", ".ts", ".tsx", ".java", ".kt", ".kts",
    ".c", ".h", ".cc", ".cpp", ".cxx", ".hpp", ".cs", ".swift", ".scala", ".php",
    ".rb", ".lua", ".zig", ".dart", ".m", ".mm", ".sol", ".groovy",
}

EXT_LANG = {}
for _ext in RUST_EXTS:
    EXT_LANG[_ext] = "rust"
for _ext in PYTHON_EXTS:
    EXT_LANG[_ext] = "python"
for _ext in GENERIC_EXTS:
    EXT_LANG[_ext] = "generic"

# Directories skipped unless the caller passes --no-default-excludes.
DEFAULT_SKIP_DIRS = {".git", "target", "node_modules", "__pycache__", ".venv"}

# Categories a record can have.
CAT_PROD = "production"
CAT_TEST_ATTR = "test-attribute"
CAT_TEST_MOD = "test-module"
CAT_DECL = "declaration"
CAT_MACRO = "macro-body"

IDENT_RE = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
FN_RE = re.compile(r"(?<![A-Za-z0-9_])fn(?![A-Za-z0-9_])")
MOD_RE = re.compile(r"(?<![A-Za-z0-9_])mod(?![A-Za-z0-9_])")
MACRO_RULES_RE = re.compile(r"(?<![A-Za-z0-9_])macro_rules(?![A-Za-z0-9_])")

# Tokens that may sit between a function's attributes and its ``fn`` keyword.
ITEM_MODIFIERS = {"pub", "async", "const", "unsafe", "extern", "default", "crate",
                  "super", "self", "impl"}

# #[test] and the common test-runner spellings; matched on the attribute path
# with any argument list stripped (so cfg(test) is *not* matched here).
TEST_ATTR_RE = re.compile(r"(?:^|::)(test|rstest|quickcheck|test_case|proptest)$")
# A ``cfg`` argument list that gates on the bare ``test`` predicate, allowing the
# usual ``all(...)`` / ``any(...)`` wrappers: ``cfg(test)``,
# ``cfg(all(test, feature = "x"))``.  ``cfg(not(test))`` is a non-test build.
CFG_RE = re.compile(r"^cfg\s*(\((?:.|\n)*\))?$")
BARE_TEST_RE = re.compile(r"(?<![A-Za-z0-9_])(test)(?![A-Za-z0-9_])")


# --------------------------------------------------------------------------- #
# Result model
# --------------------------------------------------------------------------- #

@dataclass
class FnRecord:
    """One named function found in one file."""

    name: str
    line: int
    category: str = CAT_PROD


@dataclass
class FileResult:
    """Per-file count plus the records behind it."""

    path: str
    language: str
    records: list[FnRecord]

    def count(self, categories: set[str]) -> int:
        return sum(1 for r in self.records if r.category in categories)

    def summary(self) -> dict:
        out = {
            "file": self.path,
            "language": self.language,
            "functions": len(self.records),
        }
        for cat in (CAT_PROD, CAT_TEST_ATTR, CAT_TEST_MOD, CAT_DECL, CAT_MACRO):
            out[cat] = sum(1 for r in self.records if r.category == cat)
        return out


# --------------------------------------------------------------------------- #
# Rust masking: blank comments / strings / char literals, keep line structure
# --------------------------------------------------------------------------- #

def _blank(buf: list[str], start: int, end: int) -> None:
    """Blank ``buf[start:end]`` (keeping newlines so line numbers survive)."""
    for k in range(start, min(end, len(buf))):
        if buf[k] != "\n":
            buf[k] = " "


def _raw_string_end(src: str, i: int) -> int | None:
    """Return the index just past a raw string starting at ``i``, else None.

    Handles ``r".."``, ``r#".."#`` (any hash count) and the ``b``/``c`` prefixed
    forms ``br".."`` / ``cr#".."#``.
    """
    n = len(src)
    if i > 0 and (src[i - 1].isalnum() or src[i - 1] == "_"):
        return None  # part of a longer identifier, e.g. ``var"`` is impossible
    j = i
    if src[j] in "bc":
        j += 1
    if j >= n or src[j] != "r":
        return None
    j += 1
    hashes = 0
    while j < n and src[j] == "#":
        hashes += 1
        j += 1
    if j >= n or src[j] != '"':
        return None
    close = '"' + "#" * hashes
    end = src.find(close, j + 1)
    if end == -1:
        return None  # unterminated: leave the text alone
    return end + len(close)


def _string_end(src: str, i: int) -> int:
    """Return the index just past a normal ``"..."`` string starting at ``i``."""
    n = len(src)
    j = i + 1
    while j < n:
        c = src[j]
        if c == "\\":
            j += 2
            continue
        if c == '"':
            return j + 1
        j += 1
    return n


def _char_literal_end(src: str, i: int) -> int | None:
    """Return the index just past a char literal at ``i``, or None for a lifetime."""
    n = len(src)
    if i + 1 >= n:
        return None
    if src[i + 1] == "\\":
        # '\n', '\'', '\u{1F600}', ... : an escape never contains a bare quote.
        j = src.find("'", i + 2)
        if j != -1 and j - i <= 12:
            return j + 1
        return None
    if i + 2 < n and src[i + 2] == "'":
        return i + 3
    return None  # lifetime such as 'a or 'static


def mask_rust(src: str) -> str:
    """Return ``src`` with comments, strings and char literals blanked out.

    The returned string has exactly the same length and newline positions as
    ``src``, so an offset found in the masked text maps onto the original file.
    Raw strings, nested block comments, doc comments and lifetimes are handled.
    """
    buf = list(src)
    n = len(src)
    i = 0
    while i < n:
        c = src[i]
        if c == "/" and i + 1 < n and src[i + 1] == "/":
            j = src.find("\n", i)
            j = n if j == -1 else j
            _blank(buf, i, j)
            i = j
            continue
        if c == "/" and i + 1 < n and src[i + 1] == "*":
            depth = 1
            j = i + 2
            while j < n and depth:
                if src.startswith("/*", j):
                    depth += 1
                    j += 2
                elif src.startswith("*/", j):
                    depth -= 1
                    j += 2
                else:
                    j += 1
            _blank(buf, i, j)
            i = j
            continue
        if c in "rbc":
            end = _raw_string_end(src, i)
            if end is not None:
                _blank(buf, i, end)
                i = end
                continue
        if c == '"':
            end = _string_end(src, i)
            _blank(buf, i, end)
            i = end
            continue
        if c == "'":
            end = _char_literal_end(src, i)
            if end is not None:
                _blank(buf, i, end)
                i = end
                continue
        i += 1
    return "".join(buf)


# --------------------------------------------------------------------------- #
# Small backward / forward helpers over masked text
# --------------------------------------------------------------------------- #

def _skip_ws_back(mask: str, i: int) -> int:
    while i > 0 and mask[i - 1].isspace():
        i -= 1
    return i


def _skip_ws_fwd(mask: str, i: int) -> int:
    n = len(mask)
    while i < n and mask[i].isspace():
        i += 1
    return i


def _match_back(mask: str, i: int, open_c: str, close_c: str) -> int:
    """Given the index just past a ``close_c``, return the matching ``open_c``."""
    depth = 0
    j = i - 1
    while j >= 0:
        if mask[j] == close_c:
            depth += 1
        elif mask[j] == open_c:
            depth -= 1
            if depth == 0:
                return j
        j -= 1
    return -1


def _match_fwd(mask: str, i: int, open_c: str, close_c: str) -> int:
    """Given the index of an ``open_c``, return the index of its matching close."""
    depth = 0
    n = len(mask)
    j = i
    while j < n:
        if mask[j] == open_c:
            depth += 1
        elif mask[j] == close_c:
            depth -= 1
            if depth == 0:
                return j
        j += 1
    return -1


def collect_attributes(mask: str, item_start: int) -> tuple[list[str], int]:
    """Collect the attributes directly above an item.

    Returns ``(inner_texts_in_source_order, index_of_first_attribute_or_item)``.
    Walks backwards over attributes (``#[...]``) and item modifiers
    (``pub``, ``pub(crate)``, ``async``, ``unsafe``, ``extern``, ...), stopping at
    the first token that is neither.
    """
    attrs: list[str] = []
    i = item_start
    while True:
        i = _skip_ws_back(mask, i)
        if i == 0:
            break
        c = mask[i - 1]
        if c == "]":
            open_idx = _match_back(mask, i, "[", "]")
            if open_idx > 0 and mask[open_idx - 1] == "#":
                attrs.append(mask[open_idx + 1 : i - 1])
                i = open_idx - 1
                continue
            break
        if c == ")":
            open_idx = _match_back(mask, i, "(", ")")
            if open_idx == -1:
                break
            i = open_idx
            continue
        if c.isalnum() or c == "_":
            j = i
            while j > 0 and (mask[j - 1].isalnum() or mask[j - 1] == "_"):
                j -= 1
            word = mask[j:i]
            if word in ITEM_MODIFIERS:
                i = j
                continue
            break
        break
    attrs.reverse()
    return attrs, i


def _attr_path(inner: str) -> str:
    """Return the attribute path with any argument list removed and spaces gone."""
    path = inner.split("(")[0]
    return path.replace(" ", "").replace("\n", "")


def _is_test_attr(inner: str) -> bool:
    return bool(TEST_ATTR_RE.search(_attr_path(inner)))


def _is_cfg_test(inner: str) -> bool:
    """True when an attribute is ``cfg(test)`` or an ``all``/``any`` wrapper of it.

    ``cfg(not(test))`` is the non-test build of the crate and is therefore not a
    test gate, so a leading ``not`` disqualifies the attribute.
    """
    text = " ".join(inner.split())
    if not CFG_RE.match(text):
        return False
    return bool(BARE_TEST_RE.search(text)) and not re.match(r"^cfg\s*\(\s*not\b", text)


def _skip_generics(mask: str, i: int) -> int:
    """Skip a ``<...>`` generic parameter list starting at ``i`` if present."""
    n = len(mask)
    i = _skip_ws_fwd(mask, i)
    if i >= n or mask[i] != "<":
        return i
    depth = 0
    while i < n:
        if mask[i] == "<":
            depth += 1
        elif mask[i] == ">":
            depth -= 1
            if depth == 0:
                return i + 1
        elif mask[i] == "{" and depth == 1:
            # const generic such as ``{ N }``
            close = _match_fwd(mask, i, "{", "}")
            if close == -1:
                return i
            i = close
        i += 1
    return i


def _skip_args(mask: str, i: int) -> int:
    """Skip a ``(...)`` parameter list starting at ``i`` if present."""
    i = _skip_ws_fwd(mask, i)
    if i < len(mask) and mask[i] == "(":
        close = _match_fwd(mask, i, "(", ")")
        if close == -1:
            return i
        return close + 1
    return i


def _has_body(mask: str, name_end: int) -> bool:
    """True when the item at ``name_end`` opens a body rather than ending in ``;``."""
    i = _skip_generics(mask, name_end)
    i = _skip_args(mask, i)
    n = len(mask)
    while i < n:
        c = mask[i]
        if c == "{":
            prev = mask[:i].rstrip()
            if prev.endswith("->") or prev.endswith("]"):
                # a return type or a where-clause bound; keep scanning
                pass
            return True
        if c == ";":
            return False
        if c in "([":
            close = _match_fwd(mask, i, c, ")" if c == "(" else "]")
            if close == -1:
                return True
            i = close + 1
            continue
        i += 1
    return False


def _braced_item_ranges(mask: str, keyword_re: re.Pattern[str]) -> list[tuple[int, int]]:
    """Find ``keyword ... { ... }`` ranges for ``mod`` / ``macro_rules`` items."""
    ranges: list[tuple[int, int]] = []
    for m in keyword_re.finditer(mask):
        j = _skip_ws_fwd(mask, m.end())
        if keyword_re is MACRO_RULES_RE:
            if j >= len(mask) or mask[j] != "!":
                continue
            j = _skip_ws_fwd(mask, j + 1)
        ident = IDENT_RE.match(mask, j)
        if ident:
            j = _skip_ws_fwd(mask, ident.end())
        if j < len(mask) and mask[j] == "{":
            close = _match_fwd(mask, j, "{", "}")
            if close != -1:
                ranges.append((j, close))
    return ranges


def _in_ranges(pos: int, ranges: list[tuple[int, int]]) -> bool:
    return any(start < pos < end for start, end in ranges)


# --------------------------------------------------------------------------- #
# Rust counter
# --------------------------------------------------------------------------- #

def count_rust(src: str, path: str) -> list[FnRecord]:
    """Count named ``fn`` items in Rust source ``src``.

    A record is created for every ``fn`` keyword followed by an identifier, i.e.
    for named functions, inherent/trait methods, associated functions and
    nested functions.  ``fn`` in a type position (``fn(i32) -> i32``), closures
    (``|x| ...``) and function-pointer types are not counted.
    """
    mask = mask_rust(src)
    newlines = [i for i, c in enumerate(mask) if c == "\n"]

    def line_of(pos: int) -> int:
        return bisect.bisect_left(newlines, pos) + 1

    test_mod_ranges: list[tuple[int, int]] = []
    for m in MOD_RE.finditer(mask):
        j = _skip_ws_fwd(mask, m.end())
        ident = IDENT_RE.match(mask, j)
        if ident:
            j = _skip_ws_fwd(mask, ident.end())
        if j >= len(mask) or mask[j] != "{":
            continue
        attrs, _ = collect_attributes(mask, m.start())
        if any(_is_cfg_test(a) for a in attrs):
            close = _match_fwd(mask, j, "{", "}")
            if close != -1:
                test_mod_ranges.append((j, close))

    macro_ranges = _braced_item_ranges(mask, MACRO_RULES_RE)

    file_is_test = bool(
        re.search(r"#!\s*\[\s*cfg\s*\(\s*(?:all\s*\(\s*)?test\b", mask)
    )

    records: list[FnRecord] = []
    for m in FN_RE.finditer(mask):
        name_match = IDENT_RE.match(mask, _skip_ws_fwd(mask, m.end()))
        if not name_match:
            continue  # ``fn(`` / ``fn ()`` / extern "C" fn(...) in a type position
        name = name_match.group(0)
        if name == "fn":
            continue
        attrs, _ = collect_attributes(mask, m.start())
        if (
            file_is_test
            or _in_ranges(m.start(), test_mod_ranges)
            or any(_is_cfg_test(a) for a in attrs)
        ):
            category = CAT_TEST_MOD
        elif any(_is_test_attr(a) for a in attrs):
            category = CAT_TEST_ATTR
        elif not _has_body(mask, name_match.end()):
            category = CAT_DECL
        else:
            category = CAT_PROD
        if _in_ranges(m.start(), macro_ranges):
            category = CAT_MACRO
        records.append(FnRecord(name=name, line=line_of(m.start()), category=category))
    return records


CLOSURE_RE = re.compile(r"(?<![A-Za-z0-9_|])(?:move\s*)?\|[^|\n]{0,120}?\|")


def count_rust_closures(src: str) -> int:
    """Approximate closure count for ``|x| ...`` / ``move |x| ...`` expressions.

    Bit-or operands (``a | b | c``) can be mistaken for a closure header, which is
    why this number is reported separately and never added to the total.
    """
    mask = mask_rust(src)
    return len(CLOSURE_RE.findall(mask))


# --------------------------------------------------------------------------- #
# Python counter (stdlib ast: exact)
# --------------------------------------------------------------------------- #

def count_python(src: str, path: str) -> list[FnRecord]:
    """Count ``def``/``async def`` definitions (methods and nested functions too)."""
    records: list[FnRecord] = []
    try:
        tree = ast.parse(src, filename=path)
    except SyntaxError as exc:
        print(f"warning: {path}: cannot parse ({exc.msg}); skipped", file=sys.stderr)
        return records
    for node in ast.walk(tree):
        if isinstance(node, ast.FunctionDef | ast.AsyncFunctionDef):
            category = CAT_TEST_ATTR if node.name.startswith("test") else CAT_PROD
            records.append(FnRecord(name=node.name, line=node.lineno, category=category))
    return records


# --------------------------------------------------------------------------- #
# Generic (approximate) counter
# --------------------------------------------------------------------------- #

GENERIC_FN_RE = re.compile(
    r"^[ \t]*(?:(?:pub|public|private|protected|static|final|export|inline|"
    r"async|const|unsafe|extern|suspend|open|override|virtual)\s+)*"
    r"(?:def|fn|func|function|fun|sub|proc)\s+"
    r"(?:\([^)\n]*\)\s*)?"  # Go method receiver: func (r *T) Name(
    r"([A-Za-z_$][A-Za-z0-9_$]*)",
    re.MULTILINE,
)


def count_generic(src: str, path: str) -> list[FnRecord]:
    """Approximate count for non-Rust languages: a regex over line starts.

    Comments and strings are *not* removed in this mode, so a definition shown
    in a comment block can be counted.  Treat the numbers as indicative.
    """
    records: list[FnRecord] = []
    for m in GENERIC_FN_RE.finditer(src):
        line = src.count("\n", 0, m.start()) + 1
        name = m.group(1)
        category = CAT_TEST_ATTR if name.lower().startswith("test") else CAT_PROD
        records.append(FnRecord(name=name, line=line, category=category))
    return records


# --------------------------------------------------------------------------- #
# File walking and aggregation
# --------------------------------------------------------------------------- #

def iter_source_files(
    root: Path,
    exts: set[str],
    skip_dirs: set[str],
) -> list[Path]:
    """Yield source files under ``root`` with an extension in ``exts``."""
    found: list[Path] = []
    if root.is_file():
        return [root]
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = sorted(d for d in dirnames if d not in skip_dirs)
        for name in sorted(filenames):
            if Path(name).suffix.lower() in exts:
                found.append(Path(dirpath) / name)
    return found


def analyse_file(path: Path, language: str) -> FileResult | None:
    """Count the functions in one file, returning None when it cannot be read."""
    try:
        src = path.read_text(encoding="utf-8", errors="replace")
    except OSError as exc:
        print(f"warning: {path}: {exc}; skipped", file=sys.stderr)
        return None
    if language == "rust":
        records = count_rust(src, str(path))
    elif language == "python":
        records = count_python(src, str(path))
    else:
        records = count_generic(src, str(path))
    return FileResult(path=str(path), language=language, records=records)


def selected_categories(tests: str, include_declarations: bool, include_macros: bool) -> set[str]:
    """Return the categories that make up the headline total."""
    cats: set[str] = set()
    if tests == "only":
        cats |= {CAT_TEST_ATTR, CAT_TEST_MOD}
    else:
        cats.add(CAT_PROD)
        if tests == "include":
            cats |= {CAT_TEST_ATTR, CAT_TEST_MOD}
    if include_declarations:
        cats.add(CAT_DECL)
    if include_macros:
        cats.add(CAT_MACRO)
    return cats


def build_report(
    root: Path,
    results: list[FileResult],
    cats: set[str],
    language: str,
    exts: set[str],
    tests: str,
    include_declarations: bool,
    include_macros: bool,
    closures: int | None,
) -> dict:
    """Assemble the machine-readable report."""
    by_cat = {c: 0 for c in (CAT_PROD, CAT_TEST_ATTR, CAT_TEST_MOD, CAT_DECL, CAT_MACRO)}
    for res in results:
        for rec in res.records:
            by_cat[rec.category] = by_cat.get(rec.category, 0) + 1
    per_file = [res.summary() for res in results]
    per_file.sort(key=lambda d: (-d["functions"], d["file"]))
    report = {
        "root": str(root),
        "language": language,
        "extensions": sorted(exts),
        "files_scanned": len(results),
        "total": sum(res.count(cats) for res in results),
        "categories": {
            "production": by_cat[CAT_PROD],
            "test_attribute": by_cat[CAT_TEST_ATTR],
            "test_module": by_cat[CAT_TEST_MOD],
            "declaration": by_cat[CAT_DECL],
            "macro_body": by_cat[CAT_MACRO],
        },
        "counted_categories": sorted(cats),
        "tests": tests,
        "include_declarations": include_declarations,
        "include_macros": include_macros,
        "per_file": per_file,
    }
    if closures is not None:
        report["closures_approximate"] = closures
    return report


def print_report(report: dict, show_list: bool, show_names: bool, results: list[FileResult],
                 cats: set[str]) -> None:
    """Render the human-readable report."""
    cats_meta = report["categories"]
    print(
        f"{report['total']} function definition(s) in "
        f"{report['files_scanned']} file(s) under '{report['root']}' "
        f"[{report['language']}]"
    )
    print(f"  production           : {cats_meta['production']}")
    print(f"  #[test] attribute    : {cats_meta['test_attribute']}")
    print(f"  #[cfg(test)] module  : {cats_meta['test_module']}")
    if cats_meta["declaration"]:
        mark = "counted" if CAT_DECL in cats else "excluded"
        print(f"  trait/impl declaration: {cats_meta['declaration']} ({mark})")
    if cats_meta["macro_body"]:
        mark = "counted" if CAT_MACRO in cats else "excluded"
        print(f"  macro_rules! body    : {cats_meta['macro_body']} ({mark})")
    if "closures_approximate" in report:
        print(f"  closures (approx)    : {report['closures_approximate']}")
    if show_list:
        print()
        width = max((len(r["file"]) for r in report["per_file"]), default=4)
        for row in report["per_file"]:
            counted = sum(
                1
                for rec in next(x for x in results if x.path == row["file"]).records
                if rec.category in cats
            )
            print(f"  {row['file']:<{width}}  {counted}")
    if show_names:
        print()
        for res in results:
            counted = [r for r in res.records if r.category in cats]
            if not counted:
                continue
            print(f"  {res.path}")
            for rec in sorted(counted, key=lambda r: r.line):
                print(f"    {rec.line:>6}  {rec.category:<14} {rec.name}")


# --------------------------------------------------------------------------- #
# Self-test: an embedded fixture that pins the counting rules
# --------------------------------------------------------------------------- #

SELF_TEST_RUST = r'''
// comment: fn in_line_comment() {} must not count
/* outer /* nested */ fn in_nested_comment() {} */
pub const DOC: &str = "fn in_string() {}";
pub const RAW: &str = r#"fn in_raw_string() {}"#;

impl Holder {
    pub fn new(v: usize) -> Self { Self { v } }
    pub(crate) async fn run<T: Clone>(&self, t: T) -> String
    where
        T: Send,
    {
        format!("{:?}", t)
    }
    const fn limit() -> usize { 8 }
    pub unsafe extern "C" fn ffi() {}
    fn with_closure(&self) -> i32 {
        let f = |x: i32| x + 1;
        let g = move || 2;
        f(1) + g()
    }
    fn with_fn_ptr(cb: fn(i32) -> i32) -> i32 { cb(2) }
}

pub const fn const_generic<const N: usize>() -> usize { N }

pub trait Service {
    fn declared(&self) -> bool;
    fn provided(&self) -> u8 { 1 }
}

fn lifetime_ok<'a>(v: &'a str) -> &'a str { v }

fn char_literals() -> char {
    let a = '\'';
    let b = '"';
    let _ = (a, b);
    'z'
}

macro_rules! gen {
    ($n:ident) => { fn $n() -> u8 { 0 } };
}

macro_rules! stamp {
    () => { fn generated() -> u8 { 1 } };
}

extern "C" {
    fn c_binding(x: i32) -> i32;
}

#[test]
fn test_plain() {}

#[tokio::test]
async fn test_async() {}

#[cfg(test)]
mod tests {
    fn helper_in_test_mod() -> u8 { 1 }
    mod nested { fn deep_helper() {} }
}

#[cfg(all(test, feature = "x"))]
mod gated { fn gated_helper() {} }
'''

# (production, test-attribute, test-module, declaration, macro-body, closures)
SELF_TEST_RUST_EXPECTED = (10, 2, 3, 2, 1, 2)

SELF_TEST_PYTHON = r'''
class A:
    def method(self):
        def nested():
            pass
        return nested

    async def amethod(self):
        pass

def helper(a, b):
    return a
'''

SELF_TEST_PYTHON_EXPECTED = (4, 0, 0)


def run_self_test() -> int:
    """Check the counters against the embedded fixtures; return an exit status."""
    failures: list[str] = []

    rust = count_rust(SELF_TEST_RUST, "<self-test.rs>")
    got = tuple(
        sum(1 for r in rust if r.category == c)
        for c in (CAT_PROD, CAT_TEST_ATTR, CAT_TEST_MOD, CAT_DECL, CAT_MACRO)
    ) + (count_rust_closures(SELF_TEST_RUST),)
    if got != SELF_TEST_RUST_EXPECTED:
        failures.append(
            f"rust categories: expected {SELF_TEST_RUST_EXPECTED}, got {got} "
            f"-> {[(r.category, r.name) for r in rust]}"
        )

    names = {r.name for r in rust}
    leaked = names & {
        "in_line_comment", "in_nested_comment", "in_string", "in_raw_string",
        "generated_plain", "$n",
    }
    if leaked:
        failures.append(f"rust: false positives counted: {sorted(leaked)}")
    missing = {
        "new", "run", "limit", "ffi", "with_closure", "with_fn_ptr",
        "const_generic", "provided", "lifetime_ok", "char_literals",
    } - names
    if missing:
        failures.append(f"rust: expected definitions not counted: {sorted(missing)}")

    python = count_python(SELF_TEST_PYTHON, "<self-test.py>")
    py_got = tuple(
        sum(1 for r in python if r.category == c)
        for c in (CAT_PROD, CAT_TEST_ATTR, CAT_TEST_MOD)
    )
    if py_got != SELF_TEST_PYTHON_EXPECTED:
        failures.append(
            f"python categories: expected {SELF_TEST_PYTHON_EXPECTED}, got {py_got}"
        )
    py_names = {r.name for r in python}
    if py_names != {"method", "nested", "amethod", "helper"}:
        failures.append(f"python: unexpected names {sorted(py_names)}")

    if failures:
        for line in failures:
            print(f"FAIL: {line}", file=sys.stderr)
        return 1
    print("count_functions.py self-test: OK (rust + python fixtures)")
    return 0


# --------------------------------------------------------------------------- #
# CLI
# --------------------------------------------------------------------------- #

def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        prog="count_functions.py",
        description=(
            "Count named function definitions in a source tree. Rust and Python "
            "are counted accurately; other languages use an approximate regex mode."
        ),
        epilog=(
            "Examples:\n"
            "  scripts/count_functions.py crates/ragent-config/src\n"
            "  scripts/count_functions.py crates/ragent-tui/src --json\n"
            "  scripts/count_functions.py src --lang rust --tests exclude --list\n"
            "  scripts/count_functions.py . --lang generic --ext .go\n"
        ),
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("path", nargs="?", default=".", help="directory or file to scan")
    parser.add_argument(
        "--lang",
        choices=("auto", "rust", "python", "generic"),
        default="auto",
        help="language mode (default: auto, from file extensions)",
    )
    parser.add_argument(
        "--ext",
        action="append",
        default=None,
        metavar="EXT",
        help="restrict to this extension (repeatable, e.g. --ext .rs)",
    )
    parser.add_argument(
        "--tests",
        choices=("include", "exclude", "only"),
        default="include",
        help=(
            "how to treat test functions in the headline total: include (default), "
            "exclude, or only"
        ),
    )
    parser.add_argument(
        "--include-declarations",
        action="store_true",
        help="also count body-less trait/impl method declarations",
    )
    parser.add_argument(
        "--include-macros",
        action="store_true",
        help="also count fn items written inside macro_rules! bodies",
    )
    parser.add_argument(
        "--closures",
        action="store_true",
        help="also report an approximate closure count (Rust only, never in the total)",
    )
    parser.add_argument("--list", action="store_true", help="print per-file counts")
    parser.add_argument(
        "--names", action="store_true", help="print every counted function name"
    )
    parser.add_argument("--json", action="store_true", help="print the report as JSON")
    parser.add_argument(
        "--no-default-excludes",
        action="store_true",
        help="do not skip target/, node_modules/, .git/, __pycache__/, .venv/",
    )
    parser.add_argument(
        "--exclude-dir",
        action="append",
        default=None,
        metavar="NAME",
        help="additionally skip this directory name (repeatable)",
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="run the embedded fixture checks and exit (no directory is scanned)",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    if args.self_test:
        return run_self_test()
    root = Path(args.path)
    if not root.exists():
        print(f"error: no such path: {root}", file=sys.stderr)
        return 2

    language = args.lang
    if language == "auto":
        if root.is_file():
            language = EXT_LANG.get(root.suffix.lower(), "generic")
        else:
            language = "rust"  # the workspace default

    if args.ext:
        exts = {e if e.startswith(".") else "." + e for e in args.ext}
        exts = {e.lower() for e in exts}
    elif language == "rust":
        exts = set(RUST_EXTS)
    elif language == "python":
        exts = set(PYTHON_EXTS)
    else:
        exts = set(GENERIC_EXTS)

    skip_dirs: set[str] = set()
    if not args.no_default_excludes:
        skip_dirs |= DEFAULT_SKIP_DIRS
    if args.exclude_dir:
        skip_dirs |= set(args.exclude_dir)

    files = iter_source_files(root, exts, skip_dirs)
    if not files:
        print(
            f"warning: no files with extensions {sorted(exts)} under {root}",
            file=sys.stderr,
        )

    results: list[FileResult] = []
    for path in files:
        file_lang = language
        if args.lang == "auto" and language == "rust" and root.is_dir():
            file_lang = EXT_LANG.get(path.suffix.lower(), language)
        res = analyse_file(path, file_lang)
        if res is not None:
            results.append(res)

    cats = selected_categories(args.tests, args.include_declarations, args.include_macros)
    closures = None
    if args.closures and language == "rust":
        closures = 0
        for path in files:
            try:
                closures += count_rust_closures(
                    path.read_text(encoding="utf-8", errors="replace")
                )
            except OSError:
                continue

    report = build_report(
        root=root,
        results=results,
        cats=cats,
        language=language,
        exts=exts,
        tests=args.tests,
        include_declarations=args.include_declarations,
        include_macros=args.include_macros,
        closures=closures,
    )

    if args.json:
        print(json.dumps(report, indent=2, sort_keys=False))
    else:
        print_report(report, args.list, args.names, results, cats)
    return 0


if __name__ == "__main__":
    sys.exit(main())
