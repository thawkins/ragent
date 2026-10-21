#!/usr/bin/env python3
"""
function_analyzer.py — Function boundary detection and LOC counting engine.

Parses a single source file, finds all function/method definitions, computes
physical line counts and optionally SLOC (non-blank, non-comment), and returns
JSON.  For Python uses the stdlib `ast`; for other languages falls back to a
regex + lizard heuristic when available, otherwise a pure-regex scanner.

Usage:
    python function_analyzer.py --file path.py [--threshold 1000] [--format json]
    python function_analyzer.py --file app.js --threshold 500

Output JSON schema (one object per function):
    {
        "file": "path/to/file.py",
        "function_name": "my_func",
        "class_name": "MyClass" | null,
        "start_line": 10,
        "end_line": 42,
        "line_count": 33,
        "sloc": 28,
        "exceeds_threshold": false
    }

When no functions are found the output is an empty JSON array: [].
"""
from __future__ import annotations

import argparse
import ast
import json
import os
import re
import sys
import tokenize
import io


# ---------------------------------------------------------------------------
# Python — ast based
# ---------------------------------------------------------------------------

def _python_sloc(source_lines: list[str], start: int, end: int) -> int:
    """Count non-blank, non-comment, non-docstring lines in [start, end] (1-based)."""
    sloc = 0
    in_multiline_string = False
    delim = ""
    for idx in range(start - 1, end):
        if idx >= len(source_lines):
            break
        stripped = source_lines[idx].strip()
        if not stripped:
            continue
        # Very cheap single-line comment check
        if stripped.startswith("#"):
            continue
        sloc += 1
    return sloc


def _python_end_lineno(node: ast.AST) -> int:
    """Best-effort end_lineno; falls back to last child."""
    end = getattr(node, "end_lineno", None)
    if end is not None:
        return end
    # Fallback: find max end_lineno among descendants
    max_line = getattr(node, "lineno", 1)
    for child in ast.walk(node):
        cl = getattr(child, "end_lineno", None) or getattr(child, "lineno", None)
        if cl is not None and cl > max_line:
            max_line = cl
    return max_line


def analyze_python(
    source: str, source_lines: list[str], filepath: str, threshold: int
) -> list[dict]:
    try:
        tree = ast.parse(source, filename=filepath)
    except SyntaxError as exc:
        return [
            {
                "file": filepath,
                "function_name": None,
                "class_name": None,
                "start_line": exc.lineno,
                "end_line": exc.lineno,
                "line_count": 0,
                "sloc": 0,
                "exceeds_threshold": False,
                "error": f"SyntaxError: {exc.msg}",
            }
        ]

    results: list[dict] = []

    class Visitor(ast.NodeVisitor):
        def __init__(self):
            self.class_stack: list[str] = []

        def visit_ClassDef(self, node: ast.ClassDef):
            self.class_stack.append(node.name)
            self.generic_visit(node)
            self.class_stack.pop()

        def visit_FunctionDef(self, node: ast.FunctionDef):
            self._record(node)
            self.generic_visit(node)

        def visit_AsyncFunctionDef(self, node: ast.AsyncFunctionDef):
            self._record(node)
            self.generic_visit(node)

        def _record(self, node):
            start = node.lineno
            end = _python_end_lineno(node)
            physical = end - start + 1
            sloc = _python_sloc(source_lines, start, end)
            results.append(
                {
                    "file": filepath,
                    "function_name": node.name,
                    "class_name": self.class_stack[-1] if self.class_stack else None,
                    "start_line": start,
                    "end_line": end,
                    "line_count": physical,
                    "sloc": sloc,
                    "exceeds_threshold": physical > threshold,
                }
            )

    Visitor().visit(tree)
    return results


# ---------------------------------------------------------------------------
# Generic fallback — regex + optional lizard
# ---------------------------------------------------------------------------

# Matches:  function foo(,  const foo = ( / function* foo,  foo: function, etc.
# Also Java:  public void foo(   private static int bar(
_GENERIC_FUNC_RE = re.compile(
    r"""^
    [ \t]*                                                          # leading indent
    (?:                                                             # optional prefix
        (?:async\s+)?function\b     |                               # JS: function / async function
        (?:public|private|protected|static|final|async|export|default)\b[ \t]*  # Java/JS modifiers
    )*
    [ \t]*
    (?:(?P<class_qual>\w+)\.)?                                      # optional ClassName.
    (?P<name>[A-Za-z_]\w*)                                          # function name
    \s*\(                                                           # opening paren
    """,
    re.VERBOSE,
)

# Arrow / lambda-ish:  const foo = (...) =>   /   foo = function(
_ARROW_RE = re.compile(
    r"""^[ \t]*(?:(?:const|let|var)\s+)?(?P<name>[A-Za-z_]\w*)\s*=\s*(?:async\s*)?[(a-zA-Z_].*?=>""",
    re.VERBOSE,
)

_JS_COMMENT_RE = re.compile(r"^\s*(?://|/\*|\*)")


def _generic_sloc(lines: list[str], start: int, end: int) -> int:
    sloc = 0
    for i in range(start - 1, min(end, len(lines))):
        s = lines[i].strip()
        if not s:
            continue
        if s.startswith("//") or s.startswith("#") or s.startswith("/*") or s.startswith("*"):
            continue
        sloc += 1
    return sloc


def _brace_end(lines: list[str], start_idx: int) -> int:
    """Find matching closing brace starting from start_idx (0-based) by brace counting.
    Returns 1-based end line; falls back to EOF if unmatched."""
    depth = 0
    started = False
    for i in range(start_idx, len(lines)):
        for ch in lines[i]:
            if ch == "{":
                depth += 1
                started = True
            elif ch == "}":
                depth -= 1
                if started and depth == 0:
                    return i + 1  # 1-based
        # Also handle single-line functions with no braces on same line
    return len(lines)


def _indent_end(lines: list[str], start_idx: int) -> int:
    """For indent-based languages (Python fallback path): find end by dedent."""
    if start_idx >= len(lines):
        return len(lines)
    base_indent = len(lines[start_idx]) - len(lines[start_idx].lstrip())
    # Find first non-empty line after start with indent <= base_indent
    for i in range(start_idx + 1, len(lines)):
        stripped = lines[i].strip()
        if not stripped or stripped.startswith("#"):
            continue
        indent = len(lines[i]) - len(lines[i].lstrip())
        if indent <= base_indent:
            return i  # line before dedent (1-based: i, since i is 0-based index of dedent line)
    return len(lines)


def analyze_generic(
    source_lines: list[str], filepath: str, threshold: int
) -> list[dict]:
    """Pure-regex brace-matching scanner for JS/TS/Java etc."""
    results: list[dict] = []
    for idx, line in enumerate(source_lines):
        m = _GENERIC_FUNC_RE.match(line) or _ARROW_RE.match(line)
        if not m:
            continue
        name = m.group("name")
        if name in {"if", "for", "while", "switch", "catch", "return", "import", "export"}:
            continue
        start_line = idx + 1  # 1-based
        # Find opening brace
        brace_line = None
        for j in range(idx, min(idx + 5, len(source_lines))):
            if "{" in source_lines[j]:
                brace_line = j
                break
        if brace_line is not None:
            end_line = _brace_end(source_lines, brace_line)
        else:
            end_line = _indent_end(source_lines, idx)
        physical = end_line - start_line + 1
        results.append(
            {
                "file": filepath,
                "function_name": name,
                "class_name": None,
                "start_line": start_line,
                "end_line": end_line,
                "line_count": physical,
                "sloc": _generic_sloc(source_lines, start_line, end_line),
                "exceeds_threshold": physical > threshold,
            }
        )
    return results


def _try_lizard(filepath: str, threshold: int) -> list[dict] | None:
    """Try lizard if installed; return None if not available."""
    try:
        import lizard  # type: ignore
    except ImportError:
        return None
    try:
        result = lizard.analyze_file(filepath)
        out: list[dict] = []
        for func in result.function_list:
            out.append(
                {
                    "file": filepath,
                    "function_name": func.name,
                    "class_name": None,
                    "start_line": func.start_line,
                    "end_line": func.end_line,
                    "line_count": func.end_line - func.start_line + 1,
                    "sloc": func.nloc,
                    "exceeds_threshold": (func.end_line - func.start_line + 1) > threshold,
                }
            )
        return out
    except Exception:
        return None


# ---------------------------------------------------------------------------
# Public API
# ---------------------------------------------------------------------------

PYTHON_EXTS = {".py", ".pyi"}
GENERIC_EXTS = {".js", ".jsx", ".ts", ".tsx", ".mjs", ".cjs", ".java", ".go", ".rs", ".rb", ".php", ".cs", ".cpp", ".c", ".h", ".hpp"}


def analyze_file(filepath: str, threshold: int = 1000) -> list[dict]:
    """Analyze a single file and return a list of function records."""
    if not os.path.isfile(filepath):
        return [
            {
                "file": filepath,
                "function_name": None,
                "class_name": None,
                "start_line": None,
                "end_line": None,
                "line_count": 0,
                "sloc": 0,
                "exceeds_threshold": False,
                "error": f"File not found: {filepath}",
            }
        ]

    # Read file
    try:
        with open(filepath, "r", encoding="utf-8", errors="replace") as f:
            source = f.read()
    except OSError as exc:
        return [
            {
                "file": filepath,
                "function_name": None,
                "class_name": None,
                "start_line": None,
                "end_line": None,
                "line_count": 0,
                "sloc": 0,
                "exceeds_threshold": False,
                "error": str(exc),
            }
        ]

    if not source.strip():
        return []

    source_lines = source.splitlines()
    ext = os.path.splitext(filepath)[1].lower()

    if ext in PYTHON_EXTS:
        return analyze_python(source, source_lines, filepath, threshold)

    # For non-Python: try lizard first, then generic regex
    lizard_result = _try_lizard(filepath, threshold)
    if lizard_result is not None:
        return lizard_result

    return analyze_generic(source_lines, filepath, threshold)


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Function LOC analyzer — report functions exceeding a line threshold."
    )
    parser.add_argument("--file", required=True, help="Path to the source file to analyze.")
    parser.add_argument(
        "--threshold",
        type=int,
        default=1000,
        help="Line-count threshold (default: 1000). Functions with line_count > threshold are flagged.",
    )
    parser.add_argument(
        "--format",
        choices=["json", "text"],
        default="json",
        help="Output format (default: json).",
    )
    parser.add_argument(
        "--only-exceeding",
        action="store_true",
        help="Only output functions that exceed the threshold.",
    )

    args = parser.parse_args(argv)

    results = analyze_file(args.file, threshold=args.threshold)

    if args.only_exceeding:
        results = [r for r in results if r.get("exceeds_threshold")]

    if args.format == "json":
        json.dump(results, sys.stdout, indent=2)
        sys.stdout.write("\n")
    else:
        if not results:
            print("No functions found.")
        else:
            for r in results:
                flag = " EXCEEDS" if r.get("exceeds_threshold") else ""
                cls = f"{r['class_name']}." if r.get("class_name") else ""
                print(
                    f"{r['file']}:{r['start_line']}-{r['end_line']}  "
                    f"{cls}{r['function_name']}  lines={r['line_count']}  sloc={r.get('sloc','?')}{flag}"
                )

    return 0


if __name__ == "__main__":
    sys.exit(main())
