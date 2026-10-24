#!/usr/bin/env python3
"""Function-length analyzer.

Scans a source tree and reports every function or method whose definition span
exceeds a configurable number of lines (default 1000).

Usage
-----
    python3 tools/analyze_function_lengths.py [options]

    --threshold N     Report functions whose definition spans MORE than N lines
                      (default: 1000).
    --root PATH       Directory to scan. Defaults to the repository root, i.e.
                      the parent directory of the folder holding this script.
    --output PATH     Write the JSON report to PATH instead of stdout.
    --verbose         Print one line per scanned file to stderr.

Examples
--------
    # Report every function over 1000 lines, JSON on stdout.
    python3 tools/analyze_function_lengths.py

    # Lower the threshold, keep a report file for review.
    python3 tools/analyze_function_lengths.py --threshold 200 \\
        --output target/temp/function_length_report.json

    # Scan one crate and show which files were walked.
    python3 tools/analyze_function_lengths.py --root crates/ragent-agent --verbose

Output
------
One JSON object on stdout (or written to --output):

    {
      "summary": {
        "threshold": 1000,
        "root": "/path/to/repo",
        "files_scanned": 42,
        "files_skipped": 1,
        "functions_analysed": 310,
        "functions_over_threshold": 2,
        "by_language": {
          "rust": {"files": 20, "functions": 200, "over_threshold": 2}
        }
      },
      "offenders": [
        {"file": "src/big.rs", "language": "rust", "function": "parse_all",
         "start_line": 10, "end_line": 1420, "line_count": 1411}
      ],
      "skipped_files": [{"file": "broken.py", "reason": "SyntaxError: ..."}]
    }

`offenders` is sorted by `line_count` descending (then file, then start line).
`line_count` is `end_line - start_line + 1`: the whole definition span,
including the signature line(s) and, for Python, any decorators above the def.

Skipped directories
-------------------
Anything named in SKIP_DIRS (node_modules, .git, vendor, dist, build, .venv,
__pycache__, target, .tox, .mypy_cache, .pytest_cache) is never walked, and
neither is any directory whose name starts with a dot. Unreadable or
undecodable files are skipped with a warning on stderr and listed under
`skipped_files`.

Supported languages
-------------------
* Python - parsed with the standard library ``ast`` module, so boundaries are
  exact, decorators included, nested defs reported separately.
* JavaScript, TypeScript, Java, Go, C, C++, Rust, PHP - a line-based signature
  scanner plus brace balancing that skips comments and string literals while
  counting. Only the outermost braced function of a nest is reported; a
  declaration ending in ``;`` (no body) is not a function.
* Ruby - ``def``/``end`` keyword balancing.

Functions whose body is not braced (arrow functions returning an expression,
lambdas, Rust trait method declarations) cannot span the threshold and are not
reported. All heuristics are deliberately conservative: a missed function is
better than a fabricated one.
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

# Directories that are never source: vendored code, build output, caches.
SKIP_DIRS = frozenset(
    {
        "node_modules",
        ".git",
        "vendor",
        "dist",
        "build",
        ".venv",
        "__pycache__",
        "target",
        ".tox",
        ".mypy_cache",
        ".pytest_cache",
    }
)

# Extension -> language label used by the analyzers.
EXT_LANG = {
    ".py": "python",
    ".js": "javascript",
    ".jsx": "javascript",
    ".mjs": "javascript",
    ".cjs": "javascript",
    ".ts": "typescript",
    ".tsx": "typescript",
    ".java": "java",
    ".go": "go",
    ".c": "c",
    ".h": "c",
    ".cc": "cpp",
    ".cpp": "cpp",
    ".cxx": "cpp",
    ".hpp": "cpp",
    ".hh": "cpp",
    ".rs": "rust",
    ".rb": "ruby",
    ".php": "php",
}

DEFAULT_THRESHOLD = 1000


@dataclass(frozen=True)
class FunctionSpan:
    """One function definition: its name and its inclusive line span."""

    name: str
    start_line: int
    end_line: int

    @property
    def line_count(self) -> int:
        """Number of lines in the definition span (inclusive)."""
        return self.end_line - self.start_line + 1


# ---------------------------------------------------------------------------
# Shared text scanning helpers (comments and string literals)
# ---------------------------------------------------------------------------

# Languages where a single quote opens a char literal, a rune literal, or a
# Rust lifetime rather than a string. For these, a quote only opens a literal
# when the closing quote follows almost immediately.
_CHAR_LITERAL_LANGS = frozenset({"rust", "c", "cpp", "java", "go"})


def _skip_quoted(text: str, i: int, quote: str) -> int | None:
    """Return the index just past the literal opening at ``text[i]``.

    ``None`` means the literal is unterminated (to end of file).
    """
    n = len(text)
    j = i + 1
    while j < n:
        ch = text[j]
        if ch == "\\":
            j += 2
            continue
        if ch == quote:
            return j + 1
        j += 1
    return None


def _advance_past_noncode(text: str, i: int, lang: str) -> int | None:
    """Skip a comment or string literal starting at ``text[i]``.

    Returns the index just past the region, ``i`` itself when nothing
    skippable starts here, or ``None`` when the region never terminates.
    """
    ch = text[i]
    n = len(text)

    if ch == "/" and i + 1 < n:
        nxt = text[i + 1]
        if nxt == "/":
            j = text.find("\n", i + 2)
            return n if j == -1 else j + 1
        if nxt == "*":
            j = text.find("*/", i + 2)
            return None if j == -1 else j + 2
        return i

    if ch == "#" and lang in ("ruby", "php"):
        j = text.find("\n", i + 1)
        return n if j == -1 else j + 1

    if ch in ("r", "b") and lang == "rust":
        raw = _RUST_RAW_PREFIX.match(text, i)
        if raw is not None:
            terminator = '"' + raw.group("hashes")
            j = text.find(terminator, raw.end())
            return None if j == -1 else j + len(terminator)

    if ch in ('"', "'", "`"):
        if ch == "`" and lang not in ("javascript", "typescript"):
            return i
        if ch == "'" and lang in _CHAR_LITERAL_LANGS:
            j = i + 1
            if j < n and text[j] == "\\":
                j += 2
            else:
                j += 1
            if j < n and text[j] == "'" and "\n" not in text[i:j]:
                return j + 1
            return i
        return _skip_quoted(text, i, ch)

    return i


def _find_open_brace(text: str, start: int, lang: str) -> int | None:
    """Find the ``{`` that opens a function body, scanning forwards.

    Returns ``None`` when a ``;`` is reached first (a declaration without a
    body) or when the scan runs out of text.
    """
    i = start
    n = len(text)
    parens = 0
    while i < n:
        nxt = _advance_past_noncode(text, i, lang)
        if nxt is None:
            return None
        if nxt != i:
            i = nxt
            continue
        ch = text[i]
        if ch == "(":
            parens += 1
        elif ch == ")":
            parens = max(0, parens - 1)
        elif ch == "{" and parens == 0:
            return i
        elif ch == ";" and parens == 0:
            return None
        i += 1
    return None


def _match_brace(text: str, open_index: int, lang: str) -> int | None:
    """Return the index of the ``}`` matching the ``{`` at ``open_index``."""
    i = open_index
    n = len(text)
    depth = 0
    while i < n:
        nxt = _advance_past_noncode(text, i, lang)
        if nxt is None:
            return None
        if nxt != i:
            i = nxt
            continue
        ch = text[i]
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                return i
        i += 1
    return None


class _LineIndex:
    """Map character offsets to 1-based line numbers."""

    def __init__(self, text: str) -> None:
        self._offsets = [0]
        for match in re.finditer("\n", text):
            self._offsets.append(match.end())

    def line_of(self, offset: int) -> int:
        """Return the 1-based line number containing ``offset``."""
        return bisect.bisect_right(self._offsets, offset)


# ---------------------------------------------------------------------------
# Python (ast-based, exact)
# ---------------------------------------------------------------------------


def _analyse_python(text: str) -> list[FunctionSpan]:
    """Return every function/method definition span in a Python source file."""
    tree = ast.parse(text)
    spans: list[FunctionSpan] = []
    for node in ast.walk(tree):
        if not isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            continue
        if node.end_lineno is None:
            continue
        start = node.lineno
        for decorator in node.decorator_list:
            start = min(start, decorator.lineno)
        spans.append(FunctionSpan(node.name, start, node.end_lineno))
    return spans


# ---------------------------------------------------------------------------
# Ruby (def/end balancing)
# ---------------------------------------------------------------------------

_RUBY_DEF = re.compile(r"^[ \t]*def[ \t]+(?:self\.)?(?P<name>[A-Za-z_]\w*[?!=]?)")
_RUBY_OPENERS = re.compile(
    r"\b(?:def|class|module|do|if|unless|case|while|until|begin|for)\b"
)
_RUBY_END = re.compile(r"\bend\b")


def _ruby_code_only(line: str) -> str:
    """Strip comments and string literals from one Ruby source line."""
    out: list[str] = []
    quote: str | None = None
    i = 0
    while i < len(line):
        ch = line[i]
        if quote is not None:
            if ch == "\\":
                i += 2
                continue
            if ch == quote:
                quote = None
        elif ch in "\"'":
            quote = ch
        elif ch == "#":
            break
        else:
            out.append(ch)
        i += 1
    return "".join(out)


def _analyse_ruby(text: str) -> list[FunctionSpan]:
    """Return ``def``/``end`` spans from a Ruby source file.

    Keyword balancing is textual: modifier forms (``x if y``) inflate the
    depth count, and an unterminated ``def`` is treated as no function so the
    tool never reports a span running to end of file.
    """
    lines = text.splitlines()
    spans: list[FunctionSpan] = []
    i = 0
    while i < len(lines):
        match = _RUBY_DEF.match(lines[i])
        if not match:
            i += 1
            continue
        depth = 0
        end_line = None
        for j in range(i, len(lines)):
            code = _ruby_code_only(lines[j])
            depth += len(_RUBY_OPENERS.findall(code))
            depth -= len(_RUBY_END.findall(code))
            if depth == 0:
                end_line = j + 1
                break
        if end_line is not None:
            spans.append(FunctionSpan(match.group("name"), i + 1, end_line))
        i += 1
    return spans


# ---------------------------------------------------------------------------
# Brace languages (line-based signatures + brace balancing)
# ---------------------------------------------------------------------------

# Statement/declaration keywords that a signature regex must never treat as a
# function name.
_CONTROL_WORDS = frozenset(
    {
        "if",
        "for",
        "while",
        "switch",
        "catch",
        "return",
        "else",
        "do",
        "new",
        "delete",
        "sizeof",
        "defined",
        "template",
        "typedef",
        "using",
        "namespace",
        "struct",
        "class",
        "union",
        "enum",
        "static_assert",
        "decltype",
        "alignof",
        "noexcept",
        "throw",
        "try",
        "finally",
        "function",
        "typeof",
        "await",
        "yield",
        "with",
        "match",
        "loop",
        "fn",
        "def",
        "end",
    }
)

_JS_FUNCTION = re.compile(
    r"^[ \t]*(?:export\s+)?(?:default\s+)?(?:async\s+)?function\s*\*?\s*"
    r"(?P<name>[A-Za-z_$][\w$]*)",
    re.MULTILINE,
)

_JS_ARROW = re.compile(
    r"^[ \t]*(?:export\s+)?(?:const|let|var)\s+(?P<name>[A-Za-z_$][\w$]*)\s*"
    r"(?::\s*[^=\n]+)?=\s*(?:async\s*)?"
    r"(?:\([^)\n]*\)|[A-Za-z_$][\w$]*)\s*(?::\s*[^=\n]+)?=>",
    re.MULTILINE,
)

_JS_METHOD = re.compile(
    r"^[ \t]*(?:(?:public|private|protected|static|async|get|set|readonly)\s+)*"
    r"(?P<name>[A-Za-z_$][\w$]*)\s*\(",
    re.MULTILINE,
)

_JAVA_SIG = re.compile(
    r"^[ \t]*(?:(?:public|private|protected|static|final|abstract|synchronized|"
    r"native|default|strictfp|transient|volatile)\s+)*"
    r"(?:[\w.$]+(?:<[^>]*>)?(?:\[\])*\s+)?"
    r"(?P<name>[A-Za-z_$]\w*)\s*\(",
    re.MULTILINE,
)

_GO_SIG = re.compile(
    r"^[ \t]*func[ \t]+(?:\([^)\n]*\)[ \t]*)?(?P<name>[A-Za-z_]\w*)[ \t]*[\(\[]",
    re.MULTILINE,
)

_CFAMILY_SIG = re.compile(
    r"^[ \t]*(?:[A-Za-z_][\w:<>,*&\s]*[\s*&]+)?"
    r"(?P<name>[A-Za-z_~]\w*(?:::\w+)*)[ \t]*\(",
    re.MULTILINE,
)

_RUST_SIG = re.compile(
    r"^[ \t]*(?:pub(?:\([^)]*\))?[ \t]+)?(?:const[ \t]+)?(?:async[ \t]+)?"
    r"(?:unsafe[ \t]+)?(?:extern[ \t]+\"[^\"]*\"[ \t]+)?fn[ \t]+"
    r"(?P<name>[A-Za-z_]\w*)",
    re.MULTILINE,
)

_PHP_SIG = re.compile(
    r"^[ \t]*(?:(?:public|private|protected|static|final|abstract)\s+)*"
    r"function\s+&?(?P<name>[A-Za-z_]\w*)\s*\(",
    re.MULTILINE,
)

_SIGNATURE_RULES = {
    "javascript": (_JS_FUNCTION, _JS_ARROW, _JS_METHOD),
    "typescript": (_JS_FUNCTION, _JS_ARROW, _JS_METHOD),
    "java": (_JAVA_SIG,),
    "go": (_GO_SIG,),
    "c": (_CFAMILY_SIG,),
    "cpp": (_CFAMILY_SIG,),
    "rust": (_RUST_SIG,),
    "php": (_PHP_SIG,),
}


def _analyse_braces(text: str, lang: str) -> list[FunctionSpan]:
    """Return outermost braced function spans for a brace-delimited language.

    Candidates are collected from every signature rule for the language, then
    accepted in source order. A candidate that starts inside an accepted span
    is skipped, so nested closures are reported once as part of their parent.
    """
    patterns = _SIGNATURE_RULES.get(lang, ())
    index = _LineIndex(text)

    candidates: list[tuple[int, int, str]] = []
    for pattern in patterns:
        for match in pattern.finditer(text):
            name = match.group("name")
            if name in _CONTROL_WORDS:
                continue
            candidates.append((match.start(), match.end(), name))
    candidates.sort()

    spans: list[FunctionSpan] = []
    accepted: list[tuple[int, int]] = []
    for start, sig_end, name in candidates:
        if any(lo < start < hi for lo, hi in accepted):
            continue
        open_index = _find_open_brace(text, sig_end, lang)
        if open_index is None:
            continue
        close_index = _match_brace(text, open_index, lang)
        if close_index is None:
            continue
        accepted.append((start, close_index))
        spans.append(
            FunctionSpan(name, index.line_of(start), index.line_of(close_index))
        )
    return spans


def analyse_text(text: str, language: str) -> list[FunctionSpan]:
    """Return every function span found in ``text`` for ``language``.

    Raises ``SyntaxError`` for Python sources that do not parse.
    """
    if language == "python":
        return _analyse_python(text)
    if language == "ruby":
        return _analyse_ruby(text)
    return _analyse_braces(text, language)


# ---------------------------------------------------------------------------
# Directory walk and report
# ---------------------------------------------------------------------------


def iter_source_files(root: Path):
    """Yield ``(path, language)`` for every analyzable file under ``root``."""
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = sorted(
            name
            for name in dirnames
            if name not in SKIP_DIRS and not name.startswith(".")
        )
        for filename in sorted(filenames):
            language = EXT_LANG.get(Path(filename).suffix.lower())
            if language is not None:
                yield Path(dirpath) / filename, language


def _read_source(path: Path) -> tuple[str | None, str | None]:
    """Read ``path`` as UTF-8. Returns ``(text, None)`` or ``(None, reason)``."""
    try:
        return path.read_text(encoding="utf-8"), None
    except OSError as exc:
        return None, f"{type(exc).__name__}: {exc}"
    except UnicodeDecodeError as exc:
        return None, f"UnicodeDecodeError: {exc}"


def _relative(path: Path, root: Path) -> str:
    """Render ``path`` relative to ``root`` when possible."""
    try:
        return str(path.relative_to(root))
    except ValueError:
        return str(path)


def scan(root: Path, threshold: int, verbose: bool = False) -> dict:
    """Analyze every source file under ``root`` and build the report dict."""
    root = root.resolve()
    offenders: list[dict] = []
    skipped: list[dict] = []
    by_language: dict[str, dict[str, int]] = {}
    files_scanned = 0
    functions_analysed = 0

    for path, language in iter_source_files(root):
        stats = by_language.setdefault(
            language, {"files": 0, "functions": 0, "over_threshold": 0}
        )
        relative = _relative(path, root)
        text, reason = _read_source(path)
        if text is None:
            print(
                f"warning: skipping unreadable file {relative}: {reason}",
                file=sys.stderr,
            )
            skipped.append({"file": relative, "reason": reason})
            continue
        try:
            spans = analyse_text(text, language)
        except SyntaxError as exc:
            reason = f"SyntaxError: {exc}"
            print(
                f"warning: skipping unparsable file {relative}: {reason}",
                file=sys.stderr,
            )
            skipped.append({"file": relative, "reason": reason})
            continue

        files_scanned += 1
        stats["files"] += 1
        functions_analysed += len(spans)
        stats["functions"] += len(spans)
        if verbose:
            print(f"scanned {relative} ({language}): {len(spans)} functions",
                  file=sys.stderr)

        for span in spans:
            if span.line_count <= threshold:
                continue
            stats["over_threshold"] += 1
            offenders.append(
                {
                    "file": relative,
                    "language": language,
                    "function": span.name,
                    "start_line": span.start_line,
                    "end_line": span.end_line,
                    "line_count": span.line_count,
                }
            )

    offenders.sort(key=lambda o: (-o["line_count"], o["file"], o["start_line"]))
    return {
        "summary": {
            "threshold": threshold,
            "root": str(root),
            "files_scanned": files_scanned,
            "files_skipped": len(skipped),
            "functions_analysed": functions_analysed,
            "functions_over_threshold": len(offenders),
            "by_language": dict(sorted(by_language.items())),
        },
        "offenders": offenders,
        "skipped_files": skipped,
    }


def _parse_args(argv: list[str] | None) -> argparse.Namespace:
    """Build the comand-line parser and parse ``argv``."""
    parser = argparse.ArgumentParser(
        description=(
            "Report every function or method whose definition span exceeds a "
            "line threshold."
        ),
        formatter_class=argparse.ArgumentDefaultsHelpFormatter,
    )
    parser.add_argument(
        "--threshold",
        type=int,
        default=DEFAULT_THRESHOLD,
        help="report functions spanning more than this many lines",
    )
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parent.parent,
        help="directory to scan",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help="write the JSON report here instead of stdout",
    )
    parser.add_argument(
        "--verbose",
        action="store_true",
        help="list each scanned file on stderr",
    )
    args = parser.parse_args(argv)
    if args.threshold < 1:
        parser.error("--threshold must be at least 1")
    return args


def main(argv: list[str] | None = None) -> int:
    """Run the analyzer. Returns a process exit code (0 unless args are bad)."""
    args = _parse_args(argv)
    if not args.root.is_dir():
        print(f"error: --root is not a directory: {args.root}", file=sys.stderr)
        return 2

    report = scan(args.root, args.threshold, args.verbose)
    payload = json.dumps(report, indent=2) + "\n"
    if args.output is None:
        sys.stdout.write(payload)
        return 0

    try:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(payload, encoding="utf-8")
    except OSError as exc:
        print(f"error: cannot write {args.output}: {exc}", file=sys.stderr)
        return 1
    print(
        f"{report['summary']['functions_over_threshold']} function(s) over "
        f"{args.threshold} lines written to {args.output}",
        file=sys.stderr,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
