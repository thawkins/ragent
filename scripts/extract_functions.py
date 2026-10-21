#!/usr/bin/env python3
"""
extract_functions.py — Function boundary extractor with AST / tree-sitter parsing.

For each file in the codebase, outputs:
    file_path, function_name, class_name (if method), start_line, end_line, function_type

Supported languages (primary):
    Python — stdlib ast (exact boundaries including nested, async, methods)
    JavaScript / TypeScript / TSX / JSX — tree-sitter

Extensible: add a new language by registering its tree-sitter Language and
node-type mapping in LANGUAGE_CONFIGS.

CLI:
    python extract_functions.py --input modules.json --out functions.json
    python extract_functions.py --out functions.json              # scan repo directly
    python extract_functions.py --input modules.json              # stdout
    python extract_functions.py --root /path/to/repo --out functions.json

Parse failures are logged to <out>.errors.json (or stderr when writing to stdout).
"""
from __future__ import annotations

import argparse
import ast
import json
import os
import sys
import pathlib
from dataclasses import dataclass, asdict

# ---------------------------------------------------------------------------
# Data model
# ---------------------------------------------------------------------------

@dataclass
class FunctionRecord:
    file_path: str
    function_name: str
    class_name: str | None
    start_line: int
    end_line: int
    function_type: str  # function | method | async_function | async_method | arrow | constructor | nested

    def to_dict(self) -> dict:
        return asdict(self)


# ---------------------------------------------------------------------------
# Python extractor (stdlib ast — authoritative)
# ---------------------------------------------------------------------------

def _python_end_lineno(node: ast.AST) -> int:
    end = getattr(node, "end_lineno", None)
    if end is not None:
        return end
    max_line = getattr(node, "lineno", 1)
    for child in ast.walk(node):
        cl = getattr(child, "end_lineno", None) or getattr(child, "lineno", None)
        if cl is not None and cl > max_line:
            max_line = cl
    return max_line


def extract_python(source: str, file_path: str) -> tuple[list[FunctionRecord], list[dict]]:
    try:
        tree = ast.parse(source, filename=file_path)
    except SyntaxError as exc:
        return [], [{"file": file_path, "error": f"SyntaxError: {exc.msg}", "line": exc.lineno}]

    records: list[FunctionRecord] = []

    class Visitor(ast.NodeVisitor):
        def __init__(self):
            self.class_stack: list[str] = []
            self.func_depth = 0  # >0 means inside a function (for nested detection)

        def visit_ClassDef(self, node: ast.ClassDef):
            self.class_stack.append(node.name)
            self.generic_visit(node)
            self.class_stack.pop()

        def visit_FunctionDef(self, node: ast.FunctionDef):
            self._record(node, is_async=False)
            self.func_depth += 1
            self.generic_visit(node)
            self.func_depth -= 1

        def visit_AsyncFunctionDef(self, node: ast.AsyncFunctionDef):
            self._record(node, is_async=True)
            self.func_depth += 1
            self.generic_visit(node)
            self.func_depth -= 1

        def _record(self, node, is_async: bool):
            in_class = bool(self.class_stack)
            nested = self.func_depth > 0
            if is_async and in_class:
                ftype = "async_method"
            elif is_async and not in_class:
                ftype = "async_function" if not nested else "nested_async"
            elif not is_async and in_class:
                ftype = "method"
            elif nested:
                ftype = "nested"
            else:
                ftype = "function"
            # constructors in Python are __init__
            if node.name == "__init__":
                ftype = "constructor"
            records.append(FunctionRecord(
                file_path=file_path,
                function_name=node.name,
                class_name=self.class_stack[-1] if self.class_stack else None,
                start_line=node.lineno,
                end_line=_python_end_lineno(node),
                function_type=ftype,
            ))

    Visitor().visit(tree)
    return records, []


# ---------------------------------------------------------------------------
# Tree-sitter extractors (JS / TS / TSX)
# ---------------------------------------------------------------------------

_TS_LANGUAGES: dict[str, object] = {}  # ext -> Language
_TS_AVAILABLE = False

def _init_tree_sitter():
    global _TS_AVAILABLE, _TS_LANGUAGES
    try:
        from tree_sitter import Language as TSLanguage
        import tree_sitter_python as tspy  # noqa: F401 — validates install
        import tree_sitter_javascript as tsjs
        import tree_sitter_typescript as tsts
        _TS_LANGUAGES = {
            ".js": TSLanguage(tsjs.language()),
            ".jsx": TSLanguage(tsjs.language()),
            ".mjs": TSLanguage(tsjs.language()),
            ".cjs": TSLanguage(tsjs.language()),
            ".ts": TSLanguage(tsts.language_typescript()),
            ".tsx": TSLanguage(tsts.language_tsx()),
        }
        _TS_AVAILABLE = True
    except ImportError:
        _TS_AVAILABLE = False


_init_tree_sitter()

# Maps tree-sitter node types to logical extraction behaviour for JS/TS family
_JS_FUNC_NODE_TYPES = {
    "function_declaration",
    "function_definition",  # just in case
    "function",
    "generator_function_declaration",
    "method_definition",
    "arrow_function",
    "function_expression",
}


def _ts_text(node, source_bytes: bytes) -> str:
    return source_bytes[node.start_byte:node.end_byte].decode("utf-8", errors="replace")


def _extract_js_ts(source: str, file_path: str) -> tuple[list[FunctionRecord], list[dict]]:
    if not _TS_AVAILABLE:
        return [], [{"file": file_path, "error": "tree-sitter not available for JS/TS", "line": None}]
    from tree_sitter import Parser

    ext = pathlib.Path(file_path).suffix.lower()
    lang = _TS_LANGUAGES.get(ext)
    if lang is None:
        return [], [{"file": file_path, "error": f"No tree-sitter grammar for {ext}", "line": None}]

    source_bytes = source.encode("utf-8")
    parser = Parser(lang)
    tree = parser.parse(source_bytes)
    root = tree.root_node

    # Check for parse errors (tree-sitter produces ERROR nodes)
    has_error = any(True for _ in _iter_nodes(root) if _.type == "ERROR" or _.is_missing)

    records: list[FunctionRecord] = []
    errors: list[dict] = []
    if has_error:
        # Still extract what we can, but log
        errors.append({"file": file_path, "error": "tree-sitter parse contains ERROR nodes", "line": None})

    # Walk with class/method context
    _walk_js_ts(root, source_bytes, file_path, records, class_stack=[], func_depth=0)
    return records, errors


def _iter_nodes(node):
    yield node
    for child in node.children:
        # named children only for error check; but traverse all
        yield from _iter_nodes(child)


def _child_by_field(node, field: str):
    return node.child_by_field_name(field)


def _walk_js_ts(node, source_bytes: bytes, file_path: str,
                records: list[FunctionRecord], class_stack: list[str], func_depth: int):
    ntype = node.type

    # Class body — push name
    if ntype in ("class_declaration", "class", "class_definition"):
        name_node = _child_by_field(node, "name")
        cls_name = _ts_text(name_node, source_bytes) if name_node else None
        if cls_name:
            class_stack.append(cls_name)
        for child in node.children:
            _walk_js_ts(child, source_bytes, file_path, records, class_stack, func_depth)
        if cls_name:
            class_stack.pop()
        return

    # Function-like nodes
    func_name, func_type = _classify_js_ts_node(node, source_bytes, class_stack, func_depth)

    if func_name is not None:
        start_line = node.start_point.row + 1  # 1-based
        end_line = node.end_point.row + 1
        # end_point can be off by one for trailing newline; use end_point directly
        records.append(FunctionRecord(
            file_path=file_path,
            function_name=func_name,
            class_name=class_stack[-1] if class_stack else None,
            start_line=start_line,
            end_line=end_line,
            function_type=func_type,
        ))
        # Recurse with increased depth for nested detection
        for child in node.children:
            _walk_js_ts(child, source_bytes, file_path, records, class_stack, func_depth + 1)
        return

    # Variable declarator with arrow/function: name is the variable, handled above
    # but lexically we also need to handle: const foo = () => {} — the arrow is inside declarator
    # Already covered if we recurse generically plus classify arrow
    for child in node.children:
        _walk_js_ts(child, source_bytes, file_path, records, class_stack, func_depth)


def _classify_js_ts_node(node, source_bytes: bytes, class_stack: list[str], func_depth: int):
    ntype = node.type
    nested = func_depth > 0
    in_class = bool(class_stack)

    # function_declaration / generator_function_declaration
    if ntype in ("function_declaration", "function_definition", "generator_function_declaration"):
        name_node = _child_by_field(node, "name")
        name = _ts_text(name_node, source_bytes) if name_node else "<anonymous>"
        is_async = _has_async_modifier(node, source_bytes)
        if nested:
            return name, "nested" if not is_async else "nested_async"
        if is_async:
            return name, "async_function"
        return name, "function"

    # method_definition  (covers class methods, object methods)
    if ntype == "method_definition":
        name_node = _child_by_field(node, "name")
        name = _ts_text(name_node, source_bytes) if name_node else "<anonymous>"
        if name == "constructor":
            return name, "constructor"
        is_async = _has_async_modifier(node, source_bytes)
        if is_async:
            return name, "async_method"
        if nested:
            return name, "nested"
        return name, "method"

    # arrow_function — try to recover variable name from parent declarator
    if ntype == "arrow_function":
        # Look at parent variable_declarator / assignment
        # We can't easily walk up without parent pointer; assign synthetic name
        # The caller will see arrow but we try to name it meaningfully by scanning
        # sibling variable name if available via looking at source — do best effort
        # tree-sitter arrow_function has no field name, so use anonymous with hint
        parent_hint = _arrow_name_hint(node, source_bytes)
        name = parent_hint if parent_hint else "<anonymous>"
        return name, "arrow"

    # function_expression (anonymous or named assigned)
    if ntype == "function_expression":
        name_node = _child_by_field(node, "name")
        name = _ts_text(name_node, source_bytes) if name_node else "<anonymous>"
        is_async = _has_async_modifier(node, source_bytes)
        if nested:
            return name, "nested" if not is_async else "nested_async"
        if is_async:
            return name, "async_function"
        return name, "function"

    # lexical_declaration / variable_declarator containing arrow — not a func node itself
    # handled via arrow_function branch above

    return None, None


def _has_async_modifier(node, source_bytes: bytes) -> bool:
    text = _ts_text(node, source_bytes)
    # first 20 chars heuristic plus field check
    if text.lstrip().startswith("async"):
        return True
    return False


def _arrow_name_hint(node, source_bytes: bytes) -> str | None:
    # No parent pointer in tree-sitter Python bindings without extra work;
    # return None and rely on <anonymous> — extensible hook for future parent-pointer pass
    return None


# ---------------------------------------------------------------------------
# Registry — add new languages here
# ---------------------------------------------------------------------------

PYTHON_EXTS = {".py", ".pyi"}
JS_TS_EXTS = {".js", ".jsx", ".mjs", ".cjs", ".ts", ".tsx"}

# For future: register new tree-sitter grammars in _init_tree_sitter and add exts here.
# Example:
#   REGISTERED_PARSERS[".rs"]  -> extract via tree-sitter-rust
#   REGISTERED_PARSERS[".go"]  -> extract via tree-sitter-go


def extract_file(source: str, file_path: str) -> tuple[list[FunctionRecord], list[dict]]:
    ext = pathlib.Path(file_path).suffix.lower()
    if ext in PYTHON_EXTS:
        return extract_python(source, file_path)
    if ext in JS_TS_EXTS:
        return _extract_js_ts(source, file_path)
    # Unknown extension: attempt tree-sitter if grammar registered, else error
    if ext in _TS_LANGUAGES:
        return _extract_js_ts(source, file_path)
    return [], [{"file": file_path, "error": f"Unsupported extension: {ext}", "line": None}]


# ---------------------------------------------------------------------------
# Batch / CLI
# ---------------------------------------------------------------------------

DEFAULT_EXTS = PYTHON_EXTS | JS_TS_EXTS

# Directories always skipped when scanning
SKIP_DIRS = {".git", "node_modules", "target", "dist", "build", ".venv", "venv",
             "__pycache__", ".next", ".ragent", "vendor", "assets"}


def collect_files_from_modules(modules_json: str, root: str) -> list[str]:
    with open(modules_json, "r", encoding="utf-8") as f:
        data = json.load(f)
    files: list[str] = []
    # Flexible: support common shapes
    if isinstance(data, list):
        for item in data:
            if isinstance(item, str):
                files.append(item)
            elif isinstance(item, dict):
                for k in ("path", "file", "filepath", "name"):
                    if k in item and isinstance(item[k], str):
                        files.append(item[k])
                        break
    elif isinstance(data, dict):
        # {"modules": [...]} or {"files": [...]}
        for k in ("modules", "files", "paths"):
            if k in data and isinstance(data[k], list):
                for item in data[k]:
                    if isinstance(item, str):
                        files.append(item)
                    elif isinstance(item, dict):
                        for kk in ("path", "file", "filepath"):
                            if kk in item:
                                files.append(item[kk])
                                break
                break
        if not files:
            # fallback: treat dict keys as paths if they look like files
            for k, v in data.items():
                if isinstance(k, str) and "." in k:
                    files.append(k)
    # Resolve relative to root
    resolved: list[str] = []
    for p in files:
        if os.path.isabs(p):
            resolved.append(p)
        else:
            resolved.append(os.path.join(root, p))
    return [p for p in resolved if os.path.isfile(p)]


def scan_directory(root: str, exts: set[str] | None = None) -> list[str]:
    if exts is None:
        exts = DEFAULT_EXTS
    files: list[str] = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS and not d.startswith(".")]
        for fname in filenames:
            ext = pathlib.Path(fname).suffix.lower()
            if ext in exts:
                files.append(os.path.join(dirpath, fname))
    return sorted(files)


def run(input_path: str | None, out_path: str | None, root: str, exts: set[str] | None = None) -> int:
    if input_path and os.path.isfile(input_path):
        file_list = collect_files_from_modules(input_path, root)
        if not file_list:
            print(f"[extract_functions] modules.json parsed but yielded 0 files; falling back to directory scan.", file=sys.stderr)
            file_list = scan_directory(root, exts)
    else:
        if input_path:
            print(f"[extract_functions] --input '{input_path}' not found; scanning '{root}' directly.", file=sys.stderr)
        file_list = scan_directory(root, exts)

    all_records: list[dict] = []
    all_errors: list[dict] = []

    for fp in file_list:
        try:
            with open(fp, "r", encoding="utf-8", errors="replace") as f:
                source = f.read()
        except OSError as exc:
            all_errors.append({"file": fp, "error": str(exc), "line": None})
            continue
        if not source.strip():
            continue
        # Use relative path for output stability
        rel = os.path.relpath(fp, root) if os.path.isabs(fp) else fp
        records, errs = extract_file(source, rel)
        all_records.extend(r.to_dict() if hasattr(r, "to_dict") else r for r in records)
        for e in errs:
            if "Unsupported extension" not in e.get("error", ""):
                all_errors.append(e)

    # Deterministic output
    all_records.sort(key=lambda r: (r["file_path"], r["start_line"]))

    if out_path:
        os.makedirs(os.path.dirname(os.path.abspath(out_path)) or ".", exist_ok=True)
        with open(out_path, "w", encoding="utf-8") as out:
            json.dump(all_records, out, indent=2)
            out.write("\n")
        print(f"[extract_functions] wrote {len(all_records)} functions from {len(file_list)} files to {out_path}", file=sys.stderr)
        if all_errors:
            err_path = out_path + ".errors.json"
            # Also support convention: functions.json -> functions.errors.json
            if out_path.endswith(".json"):
                err_path = out_path[:-5] + ".errors.json"
            with open(err_path, "w", encoding="utf-8") as ef:
                json.dump(all_errors, ef, indent=2)
                ef.write("\n")
            print(f"[extract_functions] {len(all_errors)} parse failures written to {err_path}", file=sys.stderr)
    else:
        json.dump(all_records, sys.stdout, indent=2)
        sys.stdout.write("\n")
        if all_errors:
            print(f"[extract_functions] {len(all_errors)} parse failures (not written; use --out to persist):", file=sys.stderr)
            for e in all_errors:
                print(f"  {e}", file=sys.stderr)

    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Function boundary extractor — AST / tree-sitter based; handles nested, async, methods."
    )
    parser.add_argument("--input", dest="input_path", default=None,
                        help="Path to modules.json (if absent, scans --root directly).")
    parser.add_argument("--out", dest="out_path", default=None,
                        help="Output JSON path (default: stdout).")
    parser.add_argument("--root", default=".",
                        help="Repository root for scanning and path resolution (default: cwd).")
    parser.add_argument("--exts", nargs="*", default=None,
                        help="File extensions to include (e.g. --exts .py .js .ts). Default: py/js/ts family.")
    args = parser.parse_args(argv)

    exts = set(e if e.startswith(".") else f".{e}" for e in args.exts) if args.exts else None
    return run(args.input_path, args.out_path, args.root, exts)


if __name__ == "__main__":
    sys.exit(main())
