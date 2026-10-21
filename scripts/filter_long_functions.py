#!/usr/bin/env python3
"""
filter_long_functions.py — 1000+ line counter and filter.

Input : functions.json — array of function records with at least
        start_line / end_line plus identifying fields.  Accepts
        flexible schemas produced by different extractors:

        Canonical s3 schema (this swarm):
            {file_path, module_id, function_name, class_name,
             start_line, end_line, function_type?}

        Aliases also accepted:
            file / filepath / module_path -> file_path
            module_id / module / module_path -> module_id (defaults to file_path)
            name / qualname / function -> function_name
            class / className -> class_name
            type / kind -> function_type
            raw_length / nloc / loc -> pre-computed count (re-validated)

Output: long_functions.json — array of
        {file_path, module_id, function_name, class_name,
         start_line, end_line, line_count, sloc?}

Filtering:
  * line_count = end_line - start_line + 1  (inclusive)
  * sloc       = non-blank / non-comment lines when source is readable
               (validation metric; not used for thresholding by default)
  * keep only line_count > threshold  (strictly greater; configurable)
  * de-duplicate by (file_path, function_name, class_name, start_line, end_line)
  * validation: drop single-line lambdas / arrow shorthands and
    minified / generated functions that falsely span 1000 lines.

CLI:
    python scripts/filter_long_functions.py --input functions.json --threshold 1000 --out long_functions.json
    python scripts/filter_long_functions.py --input functions.json --threshold 500 --mode sloc --out out.json
    python scripts/filter_long_functions.py --help
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from pathlib import Path
from typing import Any


# ---------------------------------------------------------------------------
# Helpers — schema normalisation
# ---------------------------------------------------------------------------

_FILE_KEYS = ("file_path", "file", "filepath", "module_path", "path")
_MODULE_KEYS = ("module_id", "module", "module_path")
_NAME_KEYS = ("function_name", "name", "qualname", "function", "func_name")
_CLASS_KEYS = ("class_name", "class", "className", "klass")
_TYPE_KEYS = ("function_type", "type", "kind", "func_type")


def _first(rec: dict[str, Any], keys: tuple[str, ...], default: Any = None) -> Any:
    for k in keys:
        if k in rec and rec[k] not in (None, ""):
            return rec[k]
    return default


def _extract_qualname(rec: dict[str, Any]) -> tuple[str | None, str | None]:
    """Return (function_name, class_name) handling qualname like 'Cls.method'."""
    fname = _first(rec, _NAME_KEYS)
    cname = _first(rec, _CLASS_KEYS)
    if fname and "." in str(fname) and cname is None:
        # e.g. qualname "MyClass.my_method" -> split
        parts = str(fname).rsplit(".", 1)
        if len(parts) == 2 and parts[0] and parts[1]:
            cname = parts[0]
            fname = parts[1]
    return fname, cname


# ---------------------------------------------------------------------------
# SLOC — second metric for validation / reporting
# ---------------------------------------------------------------------------

_COMMENT_PREFIXES = ("#", "//", "/*", "*", "<!--")

PY_TRIPLE_RE = re.compile(r'^\s*("""|\'\'\')')


def _count_sloc(file_path: str, start: int, end: int) -> int | None:
    """Return non-blank / non-comment line count for [start, end] inclusive.

    Returns None if file cannot be read.  Best-effort; not expected to be
    perfect for every language — it is a validation signal, not the gate.
    """
    if not file_path or not os.path.isfile(file_path):
        return None
    try:
        with open(file_path, "r", encoding="utf-8", errors="replace") as fh:
            lines = fh.readlines()
    except OSError:
        return None
    total = len(lines)
    if start < 1:
        start = 1
    if end > total:
        end = total
    sloc = 0
    in_block = False  # /* ... */ tracking
    for idx in range(start - 1, end):
        if idx < 0 or idx >= total:
            continue
        raw = lines[idx]
        stripped = raw.strip()
        if not stripped:
            continue
        # Block comment tracking (JS/Java/CSS style)
        if in_block:
            if "*/" in stripped:
                in_block = False
                # If line is only the closing marker, skip it
                after = stripped.split("*/", 1)[1].strip()
                if not after:
                    continue
            else:
                continue
        if stripped.startswith("/*"):
            if "*/" not in stripped:
                in_block = True
            continue
        # Single-line comment prefixes
        is_comment = False
        for pref in _COMMENT_PREFIXES:
            if stripped.startswith(pref):
                is_comment = True
                break
        if is_comment:
            continue
        sloc += 1
    return sloc


# ---------------------------------------------------------------------------
# Validation — lambdas / generated / minified
# ---------------------------------------------------------------------------

_LAMBDA_NAME_RE = re.compile(
    r"^(<lambda>|lambda\b|__lambda|arrow|=>|anonymous)", re.IGNORECASE
)
_GENERATED_MARKERS = (
    "generated",
    "auto-generated",
    "autogenerated",
    "do not edit",
    "do not modify",
    "codegen",
    "protobuf",
    "minified",
)

_MINIFIED_AVG_LINE_LEN = 400  # heuristic: avg chars/line above this looks minified
_MINIFIED_MAX_NEWLINES_FOR_1K = 3  # 1000 "lines" but only ~1-3 physical newlines


def is_single_line_lambda(rec: dict[str, Any], fname: str | None, ftype: str | None) -> bool:
    """True if this record is a single-line lambda / arrow shorthand that
    should not count toward the 1000-line gate."""
    # Explicit type tag
    if ftype and ftype.lower() in {"lambda", "arrow", "arrow_function", "anonymous"}:
        # Lambdas are single-expression; if somehow reported spanning 1000 lines
        # that is a parser error — always exclude by type.
        # Exception: if caller used a real multi-line lambda assignment name,
        # ftype would not be bare "lambda".
        # We still exclude when start==end (true single line) regardless.
        start = rec.get("start_line")
        end = rec.get("end_line")
        if start is not None and end is not None and start == end:
            return True
        # Bare lambda type always excluded (Python lambdas cannot span 1000 lines)
        if ftype.lower() == "lambda":
            return True

    if fname and _LAMBDA_NAME_RE.match(str(fname).strip()):
        return True

    # Heuristic: single physical line reported (start==end) and name looks synthetic
    start = rec.get("start_line")
    end = rec.get("end_line")
    if start is not None and end is not None and start == end:
        # Single-line function cannot be >threshold anyway, but guard explicitly
        return True

    return False


def is_generated_or_minified(rec: dict[str, Any], file_path: str | None) -> bool:
    """Heuristics for generated / minified / vendored files that falsely
    inflate line spans."""
    fpath = (file_path or rec.get("file_path") or rec.get("file") or "").lower()

    # Path-based exclusions (also enforced by discovery, but re-checked here)
    vendored_fragments = (
        "node_modules", ".venv", "venv/", "__pycache__", "build/", "dist/",
        ".git/", "vendor/", ".next/", ".nuxt/", "coverage/",
    )
    for frag in vendored_fragments:
        if frag in fpath:
            return True
    if fpath.endswith(".min.js") or fpath.endswith(".min.css") or ".min." in fpath:
        return True
    if fpath.endswith(".pb.go") or fpath.endswith("_pb2.py") or fpath.endswith(".generated."):
        return True

    # Content-based minified check: if the underlying file has very long lines
    # (avg line length huge) yet the function claims 1000 lines, the parser's
    # brace matcher likely consumed the whole file.
    if file_path and os.path.isfile(file_path):
        try:
            with open(file_path, "r", encoding="utf-8", errors="replace") as fh:
                content = fh.read()
            if content:
                lines = content.splitlines()
                if lines:
                    avg_len = len(content) / max(len(lines), 1)
                    # File has very few physical lines but function claims huge span
                    start = rec.get("start_line")
                    end = rec.get("end_line")
                    if start is not None and end is not None:
                        claimed = end - start + 1
                        if len(lines) <= _MINIFIED_MAX_NEWLINES_FOR_1K and claimed > 500:
                            return True
                        # Average line length monstrous -> minified bundle
                        if avg_len > _MINIFIED_AVG_LINE_LEN and len(lines) < claimed * 0.5:
                            return True
        except OSError:
            pass

    # Marker in name/type
    fname = str(_first(rec, _NAME_KEYS) or "").lower()
    ftype = str(_first(rec, _TYPE_KEYS) or "").lower()
    for marker in _GENERATED_MARKERS:
        if marker in fname or marker in ftype:
            return True

    return False


# ---------------------------------------------------------------------------
# Core — filter
# ---------------------------------------------------------------------------

def filter_long_functions(
    functions: list[dict[str, Any]],
    threshold: int = 1000,
    mode: str = "total",
    dedup: bool = True,
    validate: bool = True,
    compute_sloc: bool = True,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    """Filter to functions with line count > threshold.

    Returns (kept_records, stats).

    mode:
        "total" — gate on physical line count (end-start+1)
        "sloc"  — gate on SLOC (requires file access; falls back to total)
        "either" — keep if either metric exceeds threshold
        "both"  — keep only if both exceed threshold
    """
    stats: dict[str, Any] = {
        "total_input": len(functions),
        "invalid_records": 0,
        "dedup_removed": 0,
        "validation_removed": 0,
        "below_threshold": 0,
        "kept": 0,
    }

    # Dedup key -> first occurrence
    seen: set[tuple[Any, ...]] = set()
    kept: list[dict[str, Any]] = []

    for rec in functions:
        start = rec.get("start_line")
        end = rec.get("end_line")

        # Coerce numeric strings
        try:
            if isinstance(start, str):
                start = int(start.strip())
            if isinstance(end, str):
                end = int(end.strip())
        except (ValueError, AttributeError):
            stats["invalid_records"] += 1
            continue

        if not isinstance(start, int) or not isinstance(end, int):
            stats["invalid_records"] += 1
            continue
        if end < start:
            stats["invalid_records"] += 1
            continue

        fname, cname = _extract_qualname(rec)
        fpath = _first(rec, _FILE_KEYS, default="")
        module_id = _first(rec, _MODULE_KEYS, default=fpath)
        if not module_id:
            module_id = fpath or ""
        ftype = _first(rec, _TYPE_KEYS)

        line_count = end - start + 1

        # Optional second metric
        sloc: int | None = None
        if compute_sloc:
            sloc = _count_sloc(str(fpath) if fpath else "", start, end)

        # Validation — lambdas / generated / minified
        if validate:
            probe = {**rec, "start_line": start, "end_line": end}
            if is_single_line_lambda(probe, fname, ftype):
                stats["validation_removed"] += 1
                continue
            if is_generated_or_minified(probe, str(fpath) if fpath else None):
                stats["validation_removed"] += 1
                continue
            # Additional guard: single-line function that somehow claims 1000 lines
            # can only happen via broken end_line; sloc would be tiny — use as signal
            if ftype and ftype.lower() == "lambda":
                stats["validation_removed"] += 1
                continue

        # Threshold gate
        passes: bool
        if mode == "total":
            passes = line_count > threshold
        elif mode == "sloc":
            gate = sloc if sloc is not None else line_count
            passes = gate > threshold
        elif mode == "either":
            gate_sloc = sloc if sloc is not None else -1
            passes = (line_count > threshold) or (gate_sloc > threshold)
        elif mode == "both":
            gate_sloc = sloc if sloc is not None else line_count
            passes = (line_count > threshold) and (gate_sloc > threshold)
        else:
            passes = line_count > threshold

        if not passes:
            stats["below_threshold"] += 1
            continue

        # Deduplication — same logical function reported twice (e.g. two extractors)
        dedup_key = (fpath, fname, cname, start, end)
        if dedup:
            if dedup_key in seen:
                stats["dedup_removed"] += 1
                continue
            seen.add(dedup_key)

        out: dict[str, Any] = {
            "file_path": fpath,
            "module_id": module_id,
            "function_name": fname,
            "class_name": cname,
            "start_line": start,
            "end_line": end,
            "line_count": line_count,
        }
        if sloc is not None:
            out["sloc"] = sloc
        # Preserve function_type if present (useful for downstream reporting)
        if ftype is not None:
            out["function_type"] = ftype

        kept.append(out)

    stats["kept"] = len(kept)
    return kept, stats


# ---------------------------------------------------------------------------
# I/O
# ---------------------------------------------------------------------------

def load_functions(path: str | Path) -> list[dict[str, Any]]:
    p = Path(path)
    if not p.is_file():
        raise FileNotFoundError(f"Input not found: {p}")
    text = p.read_text(encoding="utf-8")
    data = json.loads(text)
    # Accept either a bare list or {"functions": [...]} wrapper
    if isinstance(data, dict):
        for key in ("functions", "items", "data", "results"):
            if key in data and isinstance(data[key], list):
                return data[key]
        # Single-record dict (not a list) — wrap
        return [data]
    if isinstance(data, list):
        return data
    raise ValueError(f"Unexpected JSON top-level type: {type(data).__name__}")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Filter functions.json to long functions (> threshold lines).",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=(
            "Examples:\n"
            "  python scripts/filter_long_functions.py --input output/functions.json --out output/long_functions.json\n"
            "  python scripts/filter_long_functions.py --input functions.json --threshold 500 --mode sloc --out long.json\n"
            "  python scripts/filter_long_functions.py --input functions.json --threshold 1000 --no-validate --out out.json\n"
        ),
    )
    parser.add_argument("--input", dest="input_path", required=True, help="Path to functions.json")
    parser.add_argument("--threshold", type=int, default=1000, help="Keep functions with line_count > THRESHOLD (default: 1000)")
    parser.add_argument("--out", dest="out_path", required=True, help="Output path for long_functions.json")
    parser.add_argument(
        "--mode",
        choices=["total", "sloc", "either", "both"],
        default="total",
        help="Which metric drives the threshold gate (default: total). sloc requires file access.",
    )
    parser.add_argument("--no-dedup", action="store_true", help="Disable de-duplication")
    parser.add_argument("--no-validate", action="store_true", help="Disable lambda/generated/minified validation")
    parser.add_argument("--no-sloc", action="store_true", help="Skip computing the SLOC second metric")
    parser.add_argument("--stats", action="store_true", help="Print stats JSON to stderr")
    parser.add_argument("--pretty", action="store_true", default=True, help="Pretty-print output JSON (default: on)")
    parser.add_argument("--no-pretty", dest="pretty", action="store_false", help="Minified output JSON")

    args = parser.parse_args(argv)

    if args.threshold < 0:
        parser.error("--threshold must be >= 0")

    try:
        functions = load_functions(args.input_path)
    except (FileNotFoundError, ValueError, json.JSONDecodeError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2

    kept, stats = filter_long_functions(
        functions,
        threshold=args.threshold,
        mode=args.mode,
        dedup=not args.no_dedup,
        validate=not args.no_validate,
        compute_sloc=not args.no_sloc,
    )

    out_path = Path(args.out_path)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    with open(out_path, "w", encoding="utf-8") as fh:
        if args.pretty:
            json.dump(kept, fh, indent=2, ensure_ascii=False)
            fh.write("\n")
        else:
            json.dump(kept, fh, ensure_ascii=False)

    # Summary to stdout (and stats to stderr if requested)
    print(f"kept {len(kept)} / {len(functions)} functions (>{args.threshold} lines, mode={args.mode}) -> {out_path}")
    if args.stats or True:  # always emit a one-line stats summary to stderr for observability
        print(json.dumps({"threshold": args.threshold, "mode": args.mode, **stats}), file=sys.stderr)

    return 0


if __name__ == "__main__":
    sys.exit(main())
