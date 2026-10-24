#!/usr/bin/env python3
"""Report source functions whose body length exceeds a configurable threshold.

Standard library only (Python 3.9+). No network access, no third-party imports.

Usage
-----
    python tools/func_length.py --root <dir> [--threshold 1000] \
        [--exclude <glob> ...] [--format json|text] [--out <path>]

Definitions
-----------
``start_line``
    First line of the definition proper: the ``def`` line for Python, the line
    holding the function *name* for every other language (so attributes,
    decorators or a return type wrapped onto an earlier line are not counted).
``end_line``
    Last line of the body: the closing ``}`` / ``end`` line, or - for Python,
    which has no closing line - the last line of the last body statement.
``body_lines``
    Number of source lines strictly *inside* the body. The signature line(s)
    and the closing ``}`` / ``end`` line are excluded, blank lines inside the
    body are included.
``total_lines``
    ``end_line - start_line + 1``: the whole definition span, signature
    included.

A function is reported when ``body_lines > threshold`` (strictly greater).

Output
------
``--format json`` (default) emits one object:

    {
      "threshold": 1000,
      "root": "/abs/path/to/root",
      "scanned_files": 12,
      "functions_analyzed": 340,
      "findings": [
        {"path": "src/x.py", "language": "python", "function": "name",
         "start_line": 10, "end_line": 1200,
         "body_lines": 1200, "total_lines": 1192}
      ],
      "skipped": [{"path": "bad.py", "reason": "SyntaxError: ..."}]
    }

``path`` is relative to ``root`` and uses ``/`` separators. ``findings`` is
sorted by ``body_lines`` descending. ``skipped`` lists every file that could
not be read or parsed; such a file never aborts the scan.

``--format text`` prints the same information as a human-readable table.

Language support
----------------
Python
    Exact. Uses the stdlib :mod:`ast`; nested functions and methods are
    reported with a qualified name (``outer.inner``, ``Class.method``).
Ruby
    Exact for ``def ... end`` bodies. The ``end`` matcher understands the
    usual block keywords (``class``/``module``/``if``/``while``/``until``/
    ``case``/``begin``/``for``/``do``) and ignores ``end`` inside strings and
    comments. Best-effort for line-modifier forms (``x if y``) and for
    ``do |a|`` blocks written on one line.
JavaScript / TypeScript, Go, Rust, Java, C#, C, C++
    Brace matching over a *masked* copy of the file (string literals, character
    literals, raw strings, and comments replaced by spaces with newlines kept),
    so braces inside strings or comments cannot shift a body boundary. Function
    declarations, methods, Go ``func`` forms and Rust ``fn`` forms are found by
    locating a `{` whose preceding header ends in ``name(...)``. This is a
    heuristic: it will not report a function whose body is not a ``{}`` block,
    it may miss signatures whose parameter list is split by a
    ``;``/``{``/``}`` character or a template/generic clause nested more than
    one level, and it can mistake a same-line control-flow block for a function
    if that block's header happens to end in ``name(...)``. Treat these
    languages as best-effort.
Expression-bodied arrow functions (``const f = (x) => x + 1``) are reported
with ``body_lines`` measured from the ``=>`` line to the terminating ``;``; a
single-line arrow therefore reports 0.
"""

from __future__ import annotations

import argparse
import ast
import bisect
import fnmatch
import json
import os
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterator, Sequence

DEFAULT_THRESHOLD = 1000

#: Directories that are never walked: VCS metadata, dependencies, build output,
#: caches. Compared against the directory's own name.
DEFAULT_SKIP_DIRS = frozenset(
    {
        ".git",
        ".hg",
        ".svn",
        ".idea",
        ".mypy_cache",
        ".next",
        ".nuxt",
        ".pytest_cache",
        ".ruff_cache",
        ".tox",
        ".venv",
        ".vscode",
        "__pycache__",
        "build",
        "coverage",
        "dist",
        "node_modules",
        "target",
        "vendor",
        "venv",
    }
)

#: File extension -> language label.
EXT_TO_LANGUAGE = {
    ".py": "python",
    ".pyi": "python",
    ".rb": "ruby",
    ".js": "javascript",
    ".jsx": "javascript",
    ".mjs": "javascript",
    ".cjs": "javascript",
    ".ts": "typescript",
    ".tsx": "typescript",
    ".mts": "typescript",
    ".cts": "typescript",
    ".go": "go",
    ".rs": "rust",
    ".java": "java",
    ".cs": "csharp",
    ".c": "c",
    ".h": "c",
    ".cc": "cpp",
    ".cpp": "cpp",
    ".cxx": "cpp",
    ".hpp": "cpp",
    ".hh": "cpp",
    ".hxx": "cpp",
}

#: Languages whose bodies are delimited by braces.
BRACE_LANGUAGES = frozenset(
    {"javascript", "typescript", "go", "rust", "java", "csharp", "c", "cpp"}
)

#: Identifiers that end in "(" but open a control-flow or declaration block
#: rather than a function body.
_NOT_A_FUNCTION_NAME = frozenset(
    {
        "alignof",
        "assert",
        "catch",
        "decltype",
        "delete",
        "do",
        "else",
        "for",
        "foreach",
        "if",
        "lock",
        "new",
        "operator",
        "return",
        "sizeof",
        "static_assert",
        "switch",
        "throw",
        "throws",
        "typeof",
        "using",
        "when",
        "while",
    }
)

#: Words that may open a header without that header being a function.
_NOT_A_FUNCTION_HEADER = frozenset(
    {
        "begin",
        "case",
        "catch",
        "class",
        "default",
        "do",
        "else",
        "enum",
        "extern",
        "for",
        "foreach",
        "if",
        "impl",
        "interface",
        "loop",
        "match",
        "module",
        "namespace",
        "new",
        "record",
        "return",
        "select",
        "struct",
        "switch",
        "throw",
        "trait",
        "try",
        "type",
        "typedef",
        "union",
        "unless",
        "until",
        "while",
        "with",
    }
)

#: Names that stand for an anonymous function.
_ANONYMOUS_NAMES = {"func": "<anonymous>", "function": "<anonymous>"}

#: Ruby keywords that open a block closed by ``end``.
_RUBY_BLOCK_KEYWORDS = frozenset(
    {"begin", "case", "class", "def", "for", "if", "module", "unless", "until", "while"}
)

#: `name(` optionally preceded by a single-level generic argument list.
_NAME_CALL_RE = re.compile(r"([A-Za-z_]\w*)\s*(?:<[^<>()]*>\s*)?\(")

#: `const f = (a) => ...` / `const f = a => ...` (JavaScript / TypeScript).
_JS_ARROW_RE = re.compile(
    r"\b(?:const|let|var)\s+([A-Za-z_$][\w$]*)\s*(?::\s*[^=;]+?)?=\s*"
    r"(?:async\s+)?"
    r"(?:\([^()]*\)|[A-Za-z_$][\w$]*)\s*"
    r"(?::\s*[^=;]+?)?"
    r"=>"
)

#: Ruby `def name` / `def self.name`.
_RUBY_DEF_RE = re.compile(r"^\s*def\s+(?:self\.)?([A-Za-z_]\w*[!?=]?)")


@dataclass(frozen=True)
class FunctionSpan:
    """One detected function definition and its measured line counts."""

    name: str
    start_line: int
    end_line: int
    body_lines: int
    total_lines: int


def _brace_span(name: str, start_line: int, open_line: int, end_line: int) -> FunctionSpan:
    """Measure a brace/``end``-delimited body.

    ``open_line`` is the line holding the opening ``{`` (or the ``def`` line for
    Ruby); ``end_line`` is the line holding the matching ``}``/``end``.
    """
    body_lines = max(0, end_line - open_line - 1)
    total_lines = max(1, end_line - start_line + 1)
    return FunctionSpan(name, start_line, end_line, body_lines, total_lines)


class _LineIndex:
    """Map a character offset to a 1-based line number."""

    __slots__ = ("_starts",)

    def __init__(self, text: str) -> None:
        self._starts = [0]
        self._starts.extend(match.end() for match in re.finditer("\n", text))

    def line_of(self, offset: int) -> int:
        """Return the 1-based line number containing ``offset``."""
        return bisect.bisect_right(self._starts, offset)


# ---------------------------------------------------------------------------
# Masking: blank out everything that is not code, keeping newlines in place so
# that offsets and line numbers stay valid.
# ---------------------------------------------------------------------------


def _blank(chars: list, start: int, stop: int, text: str) -> None:
    """Overwrite ``chars[start:stop]`` with spaces, preserving newlines."""
    for index in range(start, stop):
        if text[index] != "\n":
            chars[index] = " "


def mask_non_code(text: str, language: str) -> str:
    """Return ``text`` with string/char literals and comments replaced by spaces.

    Newlines are preserved, so line numbers and offsets computed on the masked
    text are identical to those of the original. Masking is what makes brace
    matching safe: a ``}`` inside ``"..."`` or ``// ...`` can no longer close a
    body early.
    """
    chars = list(text)
    length = len(text)
    index = 0
    while index < length:
        char = text[index]
        two = text[index : index + 2]

        if language == "ruby" and char == "#":
            end = text.find("\n", index)
            end = length if end < 0 else end
            _blank(chars, index, end, text)
            index = end
            continue

        if language != "ruby" and two == "//":
            end = text.find("\n", index)
            end = length if end < 0 else end
            _blank(chars, index, end, text)
            index = end
            continue

        if language != "ruby" and two == "/*":
            end = text.find("*/", index + 2)
            end = length if end < 0 else end + 2
            _blank(chars, index, end, text)
            index = end
            continue

        # Rust raw string: r"..." / r#"..."# / r##"..."##.
        if language == "rust" and char == "r":
            raw = re.match(r'r(#{0,16})"', text[index:])
            if raw is not None:
                closing = '"' + raw.group(1)
                end = text.find(closing, index + raw.end())
                end = length if end < 0 else end + len(closing)
                _blank(chars, index, end, text)
                index = end
                continue

        # C++ raw string: R"delim( ... )delim".
        if language == "cpp" and char == "R" and two == 'R"':
            opening = text.find("(", index + 2)
            if opening >= 0:
                delim = text[index + 2 : opening]
                closing = ")" + delim + '"'
                end = text.find(closing, opening + 1)
                end = length if end < 0 else end + len(closing)
                _blank(chars, index, end, text)
                index = end
                continue

        if language == "go" and char == "`":
            end = text.find("`", index + 1)
            end = length if end < 0 else end + 1
            _blank(chars, index, end, text)
            index = end
            continue

        if char == '"' or (char == "`" and language != "go"):
            end = _end_of_quoted(text, index, char)
            _blank(chars, index, end, text)
            index = end
            continue

        if char == "'":
            if language == "rust":
                end = _end_of_rust_char_or_lifetime(text, index)
            else:
                end = _end_of_quoted(text, index, "'")
            _blank(chars, index, end, text)
            index = end
            continue

        index += 1

    return "".join(chars)


def _end_of_quoted(text: str, start: int, quote: str) -> int:
    """Return the offset just past the quoted literal beginning at ``start``."""
    index = start + 1
    length = len(text)
    while index < length:
        char = text[index]
        if char == "\\":
            index += 2
            continue
        if char == quote:
            return index + 1
        if char == "\n":
            # Unterminated literal: stop at the end of the line rather than
            # masking the whole rest of the file.
            return index
        index += 1
    return length


def _end_of_rust_char_or_lifetime(text: str, start: int) -> int:
    """Distinguish a Rust char literal (``'{'``) from a lifetime (``'a``)."""
    if text[start + 1 : start + 2] == "\\":
        end = text.find("'", start + 3)
        return len(text) if end < 0 else end + 1
    if text[start + 2 : start + 3] == "'":
        return start + 3
    return start + 1


def _match_brace(masked: str, open_index: int) -> int | None:
    """Return the offset of the ``}`` matching the ``{`` at ``open_index``."""
    depth = 0
    for index in range(open_index, len(masked)):
        char = masked[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return index
    return None


# ---------------------------------------------------------------------------
# Python (exact, via ast)
# ---------------------------------------------------------------------------


def analyse_python(text: str) -> list[FunctionSpan]:
    """Return every function/method span in a Python source, exactly."""
    tree = ast.parse(text)
    spans: list[FunctionSpan] = []

    def visit(node: ast.AST, prefix: str) -> None:
        for child in ast.iter_child_nodes(node):
            if isinstance(child, (ast.FunctionDef, ast.AsyncFunctionDef)):
                name = f"{prefix}{child.name}"
                start_line = child.lineno
                end_line = child.end_lineno or start_line
                if child.body:
                    first_body = min(
                        getattr(statement, "lineno", start_line)
                        for statement in child.body
                    )
                else:
                    first_body = start_line
                spans.append(
                    FunctionSpan(
                        name=name,
                        start_line=start_line,
                        end_line=end_line,
                        body_lines=max(0, end_line - first_body + 1),
                        total_lines=max(1, end_line - start_line + 1),
                    )
                )
                visit(child, f"{name}.")
            elif isinstance(child, ast.ClassDef):
                visit(child, f"{prefix}{child.name}.")
            else:
                visit(child, prefix)

    visit(tree, "")
    return spans


# ---------------------------------------------------------------------------
# Ruby (`def ... end`)
# ---------------------------------------------------------------------------


def analyse_ruby(text: str) -> list[FunctionSpan]:
    """Return every ``def ... end`` span in a Ruby source (best-effort).

    The ``end`` counter understands the usual block keywords and ignores
    strings and comments (they are masked first). Line modifiers such as
    ``return 1 if x`` are treated as openers only when they start the line, so
    a modifier does not consume a later ``end``.
    """
    masked = mask_non_code(text, "ruby")
    spans: list[FunctionSpan] = []
    stack: list[tuple[str | None, int]] = []

    for lineno, raw_line in enumerate(masked.split("\n"), start=1):
        line = raw_line.strip()
        if not line:
            continue

        words = re.findall(r"[A-Za-z_]\w*", line)
        first = words[0] if words else ""

        def_match = _RUBY_DEF_RE.match(line)
        if def_match is not None:
            stack.append((def_match.group(1), lineno))
            opened = False
        elif first in _RUBY_BLOCK_KEYWORDS:
            stack.append((None, lineno))
            opened = True
        elif first and re.match(r"^(?:[A-Za-z_@$][\w@$.\]\[]*)\s*(?:<<|\.)?\s*\w*.*\bdo\b", line):
            # `items.each do |x|` style block.
            stack.append((None, lineno))
            opened = True
        else:
            opened = False

        end_count = len(re.findall(r"\bend\b", line))
        if not opened and first == "end":
            end_count = len(re.findall(r"\bend\b", line))
        for _ in range(end_count):
            if not stack:
                break
            name, start_line = stack.pop()
            if name is not None:
                spans.append(_brace_span(name, start_line, start_line, lineno))

    return spans


# ---------------------------------------------------------------------------
# Brace languages (heuristic)
# ---------------------------------------------------------------------------


def _header_start(masked: str, brace_index: int, max_chars: int = 400, max_lines: int = 12) -> int:
    """Return the offset where the header of the block opened at ``brace_index`` begins."""
    index = brace_index - 1
    newlines = 0
    limit = max(0, brace_index - max_chars)
    while index >= limit:
        char = masked[index]
        if char in "{};":
            return index + 1
        if char == "\n":
            newlines += 1
            if newlines > max_lines:
                return index + 1
        index -= 1
    return limit


def _strip_leading_noise(header: str) -> str:
    """Drop leading attributes/annotations/``template<...>`` from a header."""
    text = header.lstrip()
    changed = True
    while changed:
        changed = False
        if text.startswith("#["):
            close = text.find("]")
            if close > 0:
                text = text[close + 1 :].lstrip()
                changed = True
        elif text.startswith("[["):
            close = text.find("]]")
            if close > 0:
                text = text[close + 2 :].lstrip()
                changed = True
        elif text.startswith("@"):
            match = re.match(r"@[A-Za-z_]\w*(?:\([^()]*\))?\s*", text)
            if match is not None and match.end() > 0:
                text = text[match.end() :].lstrip()
                changed = True
        elif text.startswith("template"):
            match = re.match(r"template\s*<", text)
            if match is not None:
                depth = 0
                for offset in range(match.end() - 1, len(text)):
                    if text[offset] == "<":
                        depth += 1
                    elif text[offset] == ">":
                        depth -= 1
                        if depth == 0:
                            text = text[offset + 1 :].lstrip()
                            changed = True
                            break
                if not changed:
                    return text
    return text


def _has_top_level_assignment(header: str, stop: int) -> bool:
    """True when ``header[:stop]`` assigns at paren depth 0 (a lambda, not a signature)."""
    depth = 0
    index = 0
    while index < stop:
        char = header[index]
        if char in "([":
            depth += 1
        elif char in ")]":
            depth = max(0, depth - 1)
        elif char == "=" and depth == 0:
            previous = header[index - 1] if index else ""
            following = header[index + 1] if index + 1 < len(header) else ""
            if previous not in "=!<>+-*/%&|^" and following != ">":
                return True
        index += 1
    return False


def _classify_brace_header(header: str) -> tuple[str, int] | None:
    """Return ``(name, name_offset)`` when ``header`` looks like a signature."""
    if header.count("(") != header.count(")"):
        return None

    stripped = _strip_leading_noise(header)
    lead = re.match(r"[A-Za-z_]\w*", stripped)
    if lead is not None and lead.group(0) in _NOT_A_FUNCTION_HEADER:
        return None

    matches = list(_NAME_CALL_RE.finditer(header))
    if not matches:
        return None
    match = matches[-1]
    name = match.group(1)
    if name in _NOT_A_FUNCTION_NAME:
        return None

    before = header[: match.start()].rstrip()
    if before.endswith(".") or before.endswith("->"):
        return None
    if _has_top_level_assignment(header, match.start()):
        return None

    return _ANONYMOUS_NAMES.get(name, name), match.start(1)


def _find_block_body(masked: str, start: int, window: int = 4000) -> int | None:
    """Return the offset of the ``{`` opening the body, or None for a bare declaration."""
    for index in range(start, min(len(masked), start + window)):
        char = masked[index]
        if char == "{":
            return index
        if char == ";":
            return None
    return None


def _analyse_braces(masked: str, lines: _LineIndex) -> list[FunctionSpan]:
    """Return every brace-delimited function span found by heuristic header matching."""
    spans: list[FunctionSpan] = []
    seen: set[int] = set()
    for brace_index, char in enumerate(masked):
        if char != "{":
            continue
        header_start = _header_start(masked, brace_index)
        header = masked[header_start:brace_index]
        if "(" not in header:
            continue
        classified = _classify_brace_header(header)
        if classified is None:
            continue
        name, name_offset = classified
        close_index = _match_brace(masked, brace_index)
        if close_index is None or close_index in seen:
            continue
        seen.add(brace_index)
        spans.append(
            _brace_span(
                name,
                lines.line_of(header_start + name_offset),
                lines.line_of(brace_index),
                lines.line_of(close_index),
            )
        )
    return spans


def _analyse_arrows(masked: str, lines: _LineIndex) -> list[FunctionSpan]:
    """Return brace-bodied ``const f = (...) => {...}`` spans (JavaScript/TypeScript)."""
    spans: list[FunctionSpan] = []
    for match in _JS_ARROW_RE.finditer(masked):
        arrow_line = lines.line_of(match.end() - 2)
        body = _find_block_body(masked, match.end(), window=2000)
        if body is not None:
            close_index = _match_brace(masked, body)
            if close_index is None:
                continue
            spans.append(
                _brace_span(
                    match.group(1),
                    lines.line_of(match.start(1)),
                    lines.line_of(body),
                    lines.line_of(close_index),
                )
            )
            continue
        # Expression body: measure to the terminating `;`.
        end = None
        depth = 0
        for index in range(match.end(), len(masked)):
            char = masked[index]
            if char in "([{":
                depth += 1
            elif char in ")]}":
                depth = max(0, depth - 1)
            elif char == ";" and depth == 0:
                end = index
                break
        if end is None:
            continue
        end_line = lines.line_of(end)
        spans.append(
            FunctionSpan(
                name=match.group(1),
                start_line=lines.line_of(match.start(1)),
                end_line=end_line,
                body_lines=max(0, end_line - arrow_line),
                total_lines=max(1, end_line - lines.line_of(match.start(1)) + 1),
            )
        )
    return spans


def analyse_braces(text: str, language: str) -> list[FunctionSpan]:
    """Return every function span in a brace-delimited source (heuristic)."""
    masked = mask_non_code(text, language)
    lines = _LineIndex(text)
    spans = _analyse_braces(masked, lines)
    if language in ("javascript", "typescript"):
        spans.extend(_analyse_arrows(masked, lines))
    return _dedupe(spans)


def _dedupe(spans: Sequence[FunctionSpan]) -> list[FunctionSpan]:
    """Drop duplicate spans (same name and start line), keeping the first."""
    unique: dict[tuple[str, int], FunctionSpan] = {}
    for span in spans:
        unique.setdefault((span.name, span.start_line), span)
    return list(unique.values())


def analyse_text(text: str, language: str) -> list[FunctionSpan]:
    """Dispatch to the analyser for ``language`` and return its spans."""
    if language == "python":
        return analyse_python(text)
    if language == "ruby":
        return _dedupe(analyse_ruby(text))
    if language in BRACE_LANGUAGES:
        return analyse_braces(text, language)
    raise ValueError(f"unsupported language: {language}")


# ---------------------------------------------------------------------------
# Walking, scanning, reporting
# ---------------------------------------------------------------------------


def is_excluded(relative_path: str, patterns: Sequence[str]) -> bool:
    """True when ``relative_path`` matches any ``--exclude`` glob."""
    from pathlib import PurePosixPath

    name = relative_path.rsplit("/", 1)[-1]
    for pattern in patterns:
        candidate = pattern.strip()
        if not candidate:
            continue
        prefix = candidate.rstrip("/")
        if fnmatch.fnmatch(relative_path, candidate) or fnmatch.fnmatch(name, candidate):
            return True
        if relative_path == prefix or relative_path.startswith(prefix + "/"):
            return True
        if PurePosixPath(relative_path).match(candidate):
            return True
    return False


def iter_source_files(
    root: Path, exclude: Sequence[str], skip_dirs: frozenset = DEFAULT_SKIP_DIRS
) -> Iterator[tuple[Path, str]]:
    """Yield ``(absolute_path, language)`` for every analysable file under ``root``."""
    for directory, dirnames, filenames in os.walk(root):
        directory_path = Path(directory)
        relative_dir = directory_path.relative_to(root).as_posix()
        relative_dir = "" if relative_dir == "." else relative_dir + "/"
        dirnames[:] = sorted(
            name
            for name in dirnames
            if name not in skip_dirs and not is_excluded(relative_dir + name, exclude)
        )
        for filename in sorted(filenames):
            language = EXT_TO_LANGUAGE.get(Path(filename).suffix.lower())
            if language is None:
                continue
            relative = relative_dir + filename
            if is_excluded(relative, exclude):
                continue
            yield directory_path / filename, language


def scan(
    root: Path,
    threshold: int = DEFAULT_THRESHOLD,
    exclude: Sequence[str] = (),
    skip_dirs: frozenset = DEFAULT_SKIP_DIRS,
) -> dict:
    """Scan ``root`` and return the report dictionary described in the module docstring."""
    root = Path(root).resolve()
    findings: list[dict] = []
    skipped: list[dict] = []
    scanned_files = 0
    functions_analyzed = 0

    for path, language in iter_source_files(root, exclude, skip_dirs):
        relative = path.relative_to(root).as_posix()
        try:
            text = path.read_text(encoding="utf-8", errors="replace")
        except OSError as error:
            skipped.append({"path": relative, "reason": f"{type(error).__name__}: {error}"})
            continue
        try:
            spans = analyse_text(text, language)
        except (SyntaxError, ValueError, RecursionError) as error:
            skipped.append({"path": relative, "reason": f"{type(error).__name__}: {error}"})
            continue
        except Exception as error:  # noqa: BLE001 - one bad file must not abort the scan
            skipped.append({"path": relative, "reason": f"unexpected {type(error).__name__}: {error}"})
            continue

        scanned_files += 1
        functions_analyzed += len(spans)
        for span in spans:
            if span.body_lines > threshold:
                findings.append(
                    {
                        "path": relative,
                        "language": language,
                        "function": span.name,
                        "start_line": span.start_line,
                        "end_line": span.end_line,
                        "body_lines": span.body_lines,
                        "total_lines": span.total_lines,
                    }
                )

    findings.sort(key=lambda item: (-item["body_lines"], item["path"], item["start_line"]))
    return {
        "threshold": threshold,
        "root": str(root),
        "scanned_files": scanned_files,
        "functions_analyzed": functions_analyzed,
        "findings": findings,
        "skipped": skipped,
    }


def format_text(report: dict) -> str:
    """Render a report as a human-readable table."""
    lines = [
        f"threshold:       {report['threshold']}",
        f"root:            {report['root']}",
        f"scanned_files:   {report['scanned_files']}",
        f"functions:       {report['functions_analyzed']}",
        f"findings:        {len(report['findings'])}",
    ]
    if report["findings"]:
        lines.append("")
        lines.append(f"{'BODY':>6}  {'TOTAL':>6}  {'LOCATION':<44}  FUNCTION (LANGUAGE)")
        for finding in report["findings"]:
            location = f"{finding['path']}:{finding['start_line']}-{finding['end_line']}"
            lines.append(
                f"{finding['body_lines']:>6}  {finding['total_lines']:>6}  "
                f"{location:<44}  {finding['function']} ({finding['language']})"
            )
    if report["skipped"]:
        lines.append("")
        lines.append(f"skipped ({len(report['skipped'])}):")
        for entry in report["skipped"]:
            lines.append(f"  {entry['path']}: {entry['reason']}")
    return "\n".join(lines)


def build_parser() -> argparse.ArgumentParser:
    """Return the command-line parser."""
    parser = argparse.ArgumentParser(
        prog="func_length",
        description="Report functions whose body length exceeds a threshold.",
    )
    parser.add_argument("--root", default=".", help="directory to scan (default: .)")
    parser.add_argument(
        "--threshold",
        type=int,
        default=DEFAULT_THRESHOLD,
        help=f"report bodies longer than this many lines (default: {DEFAULT_THRESHOLD})",
    )
    parser.add_argument(
        "--exclude",
        action="append",
        default=[],
        metavar="GLOB",
        help="glob (relative to --root) to skip; repeatable",
    )
    parser.add_argument(
        "--format",
        choices=("json", "text"),
        default="json",
        help="output format (default: json)",
    )
    parser.add_argument("--out", default=None, help="write the report here instead of stdout")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    """Entry point. Returns a process exit code."""
    parser = build_parser()
    args = parser.parse_args(argv)
    root = Path(args.root)
    if not root.is_dir():
        print(f"func_length: not a directory: {root}", file=sys.stderr)
        return 2
    if args.threshold < 0:
        print("func_length: --threshold must be >= 0", file=sys.stderr)
        return 2

    report = scan(root, args.threshold, args.exclude)
    payload = (
        json.dumps(report, indent=2, sort_keys=False)
        if args.format == "json"
        else format_text(report)
    )

    if args.out:
        out_path = Path(args.out)
        if out_path.parent != Path(""):
            out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(payload + "\n", encoding="utf-8")
    else:
        print(payload)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
