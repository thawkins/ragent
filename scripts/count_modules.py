#!/usr/bin/env python3
"""
count_modules.py — Aggregate long-function hits by module and generate reports.

Input : long_functions.json (array of {module_id, file_path, function_name,
        class_name, start_line, end_line, line_count, ...}) produced by
        filter_long_functions.py / extract_functions.py, or the full pipeline
        outputs when --orchestrate is used.

Logic : Group long functions by module_id (falling back to file_path when
        module_id is absent).  A module qualifies iff it contains at least one
        long function.  All qualifying modules are reported; the headline
        number is the count of distinct qualifying module_ids.

Output:
    1) The headline integer — printed to stdout and written to result.txt
       (or --result).  Machine-readable, one line, no decoration.
    2) A detailed report — report.json (or the path given by --out) and,
       when requested, report.md.  The JSON report carries the headline
       number, summary stats, and the per-module breakdown; the Markdown
       report is a human-readable rendering of the same data.

Summary stats (best-effort, sourced from sibling sidecars when available):
    total_modules_scanned  — distinct module_ids seen across the full scan
    total_functions_scanned — total functions before filtering
    total_long_functions   — len(long_functions.json)
    qualifying_modules     — headline number
    percentage_long        — total_long_functions / total_functions_scanned * 100
    percentage_modules     — qualifying_modules / total_modules_scanned * 100

CLI:
    python scripts/count_modules.py --input long_functions.json --out report.json
    python scripts/count_modules.py --input long_functions.json --out report.json --md report.md
    python scripts/count_modules.py --input long_functions.json --out report.json --result result.txt
    python scripts/count_modules.py --input long_functions.json --out report.json --threshold 1000 --root .
    python scripts/count_modules.py --orchestrate --root . --threshold 1000 --out report.json

Orchestration:
    --orchestrate runs the s2-s4 pipeline (discover -> extract -> filter)
    into a temp directory and then aggregates.  Reuses existing scripts when
    present (scripts/discover_modules.py, scripts/extract_functions.py,
    scripts/filter_long_functions.py); falls back to a pure-Python fallback
    when they are absent.  In non-orchestrated mode the script is fully
    standalone and needs only its --input file.
"""

from __future__ import annotations

import argparse
import collections
import datetime
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

# Resolve repo/script locations for orchestration.
SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent

DISCOVER_SCRIPT = SCRIPT_DIR / "discover_modules.py"
EXTRACT_SCRIPT = SCRIPT_DIR / "extract_functions.py"
FILTER_SCRIPT = SCRIPT_DIR / "filter_long_functions.py"


# ---------------------------------------------------------------------------
# Helpers — schema-tolerant field access
# ---------------------------------------------------------------------------

_FILE_KEYS = ("file_path", "file", "filepath", "module_path", "path")
_MODULE_KEYS = ("module_id", "module", "module_path")
_NAME_KEYS = ("function_name", "name", "qualname", "function", "func_name")
_CLASS_KEYS = ("class_name", "class", "className", "klass")


def _first(rec: dict[str, Any], keys: tuple[str, ...], default: Any = None) -> Any:
    for k in keys:
        if k in rec and rec[k] not in (None, ""):
            return rec[k]
    return default


def _normalise(rec: dict[str, Any]) -> dict[str, Any]:
    """Return a canonical view {module_id, file_path, function_name, class_name,
    start_line, end_line, line_count, ...} for one long-function record."""
    fpath = _first(rec, _FILE_KEYS, default="")
    mid = _first(rec, _MODULE_KEYS, default=None)
    if not mid:
        mid = fpath or "(unknown)"
    mid = str(mid)
    fpath = str(fpath) if fpath else mid
    fname = _first(rec, _NAME_KEYS, default=None)
    cname = _first(rec, _CLASS_KEYS, default=None)
    # Handle qualname "Cls.method" when class is not separately recorded.
    if fname and "." in str(fname) and cname is None:
        parts = str(fname).rsplit(".", 1)
        if len(parts) == 2 and parts[0] and parts[1]:
            cname = parts[0]
            fname = parts[1]
    # line_count may be pre-computed; otherwise derive from span.
    lc = rec.get("line_count")
    if lc is None:
        lc = rec.get("lines")
    if lc is None:
        lc = rec.get("loc")
    if lc is None:
        sl = rec.get("start_line")
        el = rec.get("end_line")
        if isinstance(sl, int) and isinstance(el, int) and el >= sl:
            lc = el - sl + 1
    # Coerce to int when possible (tolerate numeric strings).
    if isinstance(lc, str):
        try:
            lc = int(lc.strip())
        except ValueError:
            lc = None
    try:
        sl = int(rec.get("start_line")) if rec.get("start_line") is not None else None
    except (TypeError, ValueError):
        sl = None
    try:
        el = int(rec.get("end_line")) if rec.get("end_line") is not None else None
    except (TypeError, ValueError):
        el = None
    return {
        "module_id": mid,
        "file_path": fpath,
        "function_name": str(fname) if fname is not None else None,
        "class_name": str(cname) if cname not in (None, "") else None,
        "start_line": sl,
        "end_line": el,
        "line_count": int(lc) if isinstance(lc, int) else lc,
        "function_type": rec.get("function_type") or rec.get("type") or rec.get("kind"),
        "sloc": rec.get("sloc"),
        "_raw": rec,
    }


def _load_json_list(path: Path) -> list[dict[str, Any]]:
    """Load a JSON file that may be a list or a {key: [list]} wrapper."""
    text = path.read_text(encoding="utf-8")
    data = json.loads(text)
    if isinstance(data, list):
        return data
    if isinstance(data, dict):
        for key in ("long_functions", "functions", "items", "data", "results", "modules"):
            if key in data and isinstance(data[key], list):
                return data[key]
        return [data]
    raise ValueError(f"Unexpected JSON top-level type in {path}: {type(data).__name__}")


def _try_load_json(path: Path | None) -> Any | None:
    if path is None or not path.is_file():
        return None
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return None


# ---------------------------------------------------------------------------
# Aggregation
# ---------------------------------------------------------------------------

def aggregate(
    long_functions: list[dict[str, Any]],
    modules_manifest: list[dict[str, Any]] | None = None,
    all_functions: list[dict[str, Any]] | None = None,
    threshold: int | None = None,
) -> dict[str, Any]:
    """Group long functions by module_id and build the full report payload.

    Returns a dict ready to serialise as report.json (and to render as markdown).
    """
    normalised = [_normalise(r) for r in long_functions]

    # Group by module_id.
    by_module: dict[str, list[dict[str, Any]]] = collections.defaultdict(list)
    for rec in normalised:
        by_module[rec["module_id"]].append(rec)

    # Sort functions within each module: largest first, then by start_line.
    for mid in by_module:
        by_module[mid].sort(
            key=lambda r: (-(r["line_count"] or 0), r["start_line"] or 0, r["function_name"] or ""),
        )

    # Build per-module entries (sorted by module_id for determinism).
    modules: list[dict[str, Any]] = []
    for mid in sorted(by_module):
        funcs = by_module[mid]
        # Representative file_path: most common file_path in the group, else module_id.
        file_paths = [f["file_path"] for f in funcs if f["file_path"]]
        top_file = collections.Counter(file_paths).most_common(1)[0][0] if file_paths else mid
        modules.append(
            {
                "module_id": mid,
                "file_path": top_file,
                "long_function_count": len(funcs),
                "functions": [
                    {
                        "name": f["function_name"],
                        "qualified_name": f"{f['class_name']}.{f['function_name']}"
                        if f["class_name"]
                        else f["function_name"],
                        "class_name": f["class_name"],
                        "lines": f["line_count"],
                        "sloc": f["sloc"],
                        "location": f"{f['file_path']}:{f['start_line']}"
                        if f["start_line"] is not None
                        else f["file_path"],
                        "file_path": f["file_path"],
                        "start_line": f["start_line"],
                        "end_line": f["end_line"],
                        "function_type": f["function_type"],
                    }
                    for f in funcs
                ],
            }
        )

    qualifying_modules = len(modules)
    total_long_functions = len(long_functions)

    # Summary stats — best-effort from sidecars, else derived.
    total_modules_scanned: int | None = None
    total_functions_scanned: int | None = None

    if modules_manifest is not None:
        # modules_manifest is a list of {module_id, file_path, ...}
        mids = set()
        for m in modules_manifest:
            mid = m.get("module_id") or m.get("module") or m.get("file_path") or m.get("path")
            if mid:
                mids.add(str(mid))
        if mids:
            total_modules_scanned = len(mids)

    if all_functions is not None:
        total_functions_scanned = len(all_functions)

    # Fallback: if we have no sidecars, total_modules is at least the qualifying count;
    # report it as such but mark it derived.
    derived_modules = False
    if total_modules_scanned is None:
        # Try to infer from long_functions alone? No — that would equal qualifying count
        # and hide coverage. Leave as None and let the report note it.
        # But for percentage we need a denominator; treat None as "unknown".
        derived_modules = True
    derived_functions = total_functions_scanned is None

    pct_long: float | None = None
    if total_functions_scanned and total_functions_scanned > 0:
        pct_long = round(total_long_functions / total_functions_scanned * 100, 2)

    pct_modules: float | None = None
    if total_modules_scanned and total_modules_scanned > 0:
        pct_modules = round(qualifying_modules / total_modules_scanned * 100, 2)

    return {
        "generated_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "threshold": threshold,
        "qualifying_modules": qualifying_modules,
        "total_long_functions": total_long_functions,
        "total_modules_scanned": total_modules_scanned,
        "total_functions_scanned": total_functions_scanned,
        "percentage_long_functions": pct_long,
        "percentage_modules_with_long_functions": pct_modules,
        "derived_modules_count": derived_modules,
        "derived_functions_count": derived_functions,
        "modules": modules,
    }


# ---------------------------------------------------------------------------
# Renderers
# ---------------------------------------------------------------------------

def render_markdown(report: dict[str, Any]) -> str:
    """Render a Markdown report from the JSON report payload."""
    lines: list[str] = []
    lines.append("# Module Aggregation Report — Long Functions")
    lines.append("")
    lines.append(f"_Generated: {report['generated_at']}_")
    if report.get("threshold") is not None:
        lines.append(f"_Threshold: &gt; {report['threshold']} lines_")
    lines.append("")

    # Summary table
    lines.append("## Summary")
    lines.append("")
    lines.append("| Metric | Value |")
    lines.append("|---|---|")
    lines.append(f"| Qualifying modules (≥1 long function) | **{report['qualifying_modules']}** |")
    lines.append(f"| Total long functions | {report['total_long_functions']} |")
    tms = report.get("total_modules_scanned")
    tfs = report.get("total_functions_scanned")
    lines.append(f"| Total modules scanned | {tms if tms is not None else 'unknown'} |")
    lines.append(f"| Total functions scanned | {tfs if tfs is not None else 'unknown'} |")
    pl = report.get("percentage_long_functions")
    pm = report.get("percentage_modules_with_long_functions")
    lines.append(f"| % long functions | {f'{pl:.2f}%' if pl is not None else 'n/a'} |")
    lines.append(f"| % modules with long functions | {f'{pm:.2f}%' if pm is not None else 'n/a'} |")
    lines.append("")

    if report.get("derived_modules_count") or report.get("derived_functions_count"):
        notes: list[str] = []
        if report.get("derived_modules_count"):
            notes.append("total modules scanned could not be determined (no modules manifest supplied)")
        if report.get("derived_functions_count"):
            notes.append("total functions scanned could not be determined (no functions manifest supplied)")
        lines.append(f"> **Note:** {'; '.join(notes)}. Pass --modules/--functions or run with --orchestrate for complete stats.")
        lines.append("")

    if not report["modules"]:
        lines.append("_No modules contain long functions above the threshold._")
        lines.append("")
        return "\n".join(lines)

    lines.append("## Modules")
    lines.append("")
    # Overview table
    lines.append("| # | Module | File | Long fns |")
    lines.append("|---|---|---|---|")
    for idx, m in enumerate(report["modules"], 1):
        lines.append(f"| {idx} | `{m['module_id']}` | `{m['file_path']}` | {m['long_function_count']} |")
    lines.append("")

    # Per-module detail
    for m in report["modules"]:
        lines.append(f"### `{m['module_id']}`")
        lines.append("")
        lines.append(f"- **File:** `{m['file_path']}`")
        lines.append(f"- **Long functions:** {m['long_function_count']}")
        lines.append("")
        lines.append(f"| Function | Lines | Location |")
        lines.append(f"|---|---|---|")
        for fn in m["functions"]:
            qn = fn["qualified_name"] or "(anonymous)"
            loc = fn["location"]
            lns = fn["lines"] if fn["lines"] is not None else "?"
            extra = f" ({fn['function_type']})" if fn.get("function_type") else ""
            lines.append(f"| `{qn}`{extra} | {lns} | `{loc}` |")
        lines.append("")

    return "\n".join(lines)


# ---------------------------------------------------------------------------
# Orchestration — run the s2-s4 pipeline into a temp dir
# ---------------------------------------------------------------------------

def _run_step(cmd: list[str], label: str) -> None:
    print(f"[orchestrate] {label}: {' '.join(cmd)}", file=sys.stderr)
    result = subprocess.run(cmd, capture_output=True, text=True)
    if result.returncode != 0:
        print(result.stdout, file=sys.stderr)
        print(result.stderr, file=sys.stderr)
        raise RuntimeError(f"Step '{label}' failed (exit {result.returncode})")


def orchestrate(
    root: Path,
    threshold: int,
    workdir: Path,
    keep_temp: bool = False,
) -> tuple[Path, Path | None, Path | None]:
    """Run discover -> extract -> filter into workdir.

    Returns (long_functions_json, modules_json_or_None, functions_json_or_None).
    Exits non-zero if the pipeline cannot be executed.
    """
    root = root.resolve()
    workdir.mkdir(parents=True, exist_ok=True)

    modules_json = workdir / "modules.json"
    functions_json = workdir / "functions.json"
    long_json = workdir / "long_functions.json"

    # Prefer the real scripts when they exist; otherwise fall back.
    if DISCOVER_SCRIPT.is_file() and EXTRACT_SCRIPT.is_file() and FILTER_SCRIPT.is_file():
        _run_step(
            [sys.executable, str(DISCOVER_SCRIPT), "--root", str(root), "--out", str(modules_json)],
            "discover_modules",
        )
        _run_step(
            [sys.executable, str(EXTRACT_SCRIPT), "--root", str(root), "--input", str(modules_json), "--out", str(functions_json)],
            "extract_functions",
        )
        _run_step(
            [
                sys.executable, str(FILTER_SCRIPT),
                "--input", str(functions_json),
                "--threshold", str(threshold),
                "--out", str(long_json),
            ],
            "filter_long_functions",
        )
    else:
        # Minimal fallback — reuse whatever is available, otherwise scan directly.
        from discover_modules import discover, DEFAULT_EXCLUDE_DIRS, DEFAULT_EXCLUDE_FILE_PATTERNS, EXTENSION_MAP, BASENAME_MAP  # type: ignore[import-not-found]
        from extract_functions import run as extract_run  # type: ignore[import-not-found]
        from filter_long_functions import filter_long_functions, load_functions  # type: ignore[import-not-found]

        entries = discover(root, set(DEFAULT_EXCLUDE_DIRS), DEFAULT_EXCLUDE_FILE_PATTERNS, EXTENSION_MAP, BASENAME_MAP)
        modules_json.write_text(json.dumps(entries, indent=2), encoding="utf-8")
        extract_run(input_path=modules_json, output_path=functions_json, root=root)
        funcs = load_functions(functions_json)
        kept, _stats = filter_long_functions(funcs, threshold=threshold)
        long_json.write_text(json.dumps(kept, indent=2), encoding="utf-8")

    if not long_json.is_file():
        raise RuntimeError(f"Orchestration did not produce {long_json}")
    return long_json, (modules_json if modules_json.is_file() else None), (functions_json if functions_json.is_file() else None)


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        description="Aggregate long functions by module and generate reports.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=(
            "Examples:\n"
            "  python scripts/count_modules.py --input long_functions.json --out report.json\n"
            "  python scripts/count_modules.py --input long_functions.json --out report.json --md report.md --result result.txt\n"
            "  python scripts/count_modules.py --input long_functions.json --out report.json --modules modules.json --functions functions.json\n"
            "  python scripts/count_modules.py --orchestrate --root . --threshold 1000 --out report.json\n"
        ),
    )
    p.add_argument("--input", dest="input_path", type=str, default=None,
                   help="Path to long_functions.json (array of long-function records).")
    p.add_argument("--out", dest="out_path", type=str, default=None,
                   help="Path for the JSON report (report.json). When --orchestrate is used this defaults to ./report.json.")
    p.add_argument("--md", dest="md_path", type=str, default=None,
                   help="Also write a Markdown report to this path (report.md). Defaults to <out> with .json -> .md when --md is set without a value? Use --md report.md explicitly.")
    p.add_argument("--result", dest="result_path", type=str, default=None,
                   help="Path for the headline integer file (default: result.txt next to --out, or ./result.txt).")
    p.add_argument("--modules", dest="modules_path", type=str, default=None,
                   help="Optional modules manifest (modules.json) for total_modules_scanned.")
    p.add_argument("--functions", dest="functions_path", type=str, default=None,
                   help="Optional full functions manifest (functions.json) for total_functions_scanned.")
    p.add_argument("--threshold", type=int, default=None,
                   help="Threshold that was used to produce long_functions.json (recorded in the report; does not re-filter).")
    p.add_argument("--root", type=str, default=None,
                   help="Repository root (used with --orchestrate; also resolves relative file_path entries).")
    p.add_argument("--orchestrate", action="store_true",
                   help="Run the full discover->extract->filter pipeline before aggregating.")
    p.add_argument("--keep-temp", action="store_true",
                   help="With --orchestrate, keep the temporary workdir instead of removing it (path printed to stderr).")
    p.add_argument("--workdir", type=str, default=None,
                   help="With --orchestrate, use this directory as the pipeline workdir instead of a temp dir.")
    return p


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)

    tmpdir: tempfile.TemporaryDirectory[str] | None = None
    modules_manifest: list[dict[str, Any]] | None = None
    all_functions: list[dict[str, Any]] | None = None
    input_path: Path | None = None

    # Resolve --out / --result defaults sensibly in both modes.
    out_path: Path | None = Path(args.out_path) if args.out_path else None
    result_path: Path | None = Path(args.result_path) if args.result_path else None

    # Orchestrated mode — produce long_functions.json ourselves.
    if args.orchestrate:
        root = Path(args.root) if args.root else REPO_ROOT
        threshold = args.threshold if args.threshold is not None else 1000
        if args.workdir:
            workdir = Path(args.workdir)
            workdir.mkdir(parents=True, exist_ok=True)
            long_path, mod_sidecar, fn_sidecar = orchestrate(root, threshold, workdir)
        else:
            tmpdir = tempfile.TemporaryDirectory(prefix="count_modules_")
            workdir = Path(tmpdir.name)
            long_path, mod_sidecar, fn_sidecar = orchestrate(root, threshold, workdir)
            if args.keep_temp:
                print(f"[orchestrate] keeping workdir: {workdir}", file=sys.stderr)
                # Prevent cleanup — detach by stashing the path and not calling cleanup.
                tmpdir_keep = workdir
                _ = tmpdir_keep  # noqa: F841
                # Leak tmpdir intentionally; do not call cleanup.
                tmpdir = None  # type: ignore[assignment]
        input_path = long_path
        # Use sidecars from orchestration for stats if the user didn't supply their own.
        if args.modules_path:
            modules_manifest = _load_json_list(Path(args.modules_path))
        elif mod_sidecar is not None:
            try:
                modules_manifest = _load_json_list(mod_sidecar)
            except Exception:
                pass
        if args.functions_path:
            all_functions = _load_json_list(Path(args.functions_path))
        elif fn_sidecar is not None:
            try:
                all_functions = _load_json_list(fn_sidecar)
            except Exception:
                pass
        if out_path is None:
            out_path = Path("report.json")
        if result_path is None:
            result_path = out_path.parent / "result.txt"
        # threshold for the report
        report_threshold = threshold
    else:
        # Standalone mode — --input is required.
        if not args.input_path:
            parser.error("--input is required (or use --orchestrate to generate it)")
        input_path = Path(args.input_path)
        if not input_path.is_file():
            print(f"error: input not found: {input_path}", file=sys.stderr)
            return 2
        if args.modules_path:
            try:
                modules_manifest = _load_json_list(Path(args.modules_path))
            except Exception as exc:
                print(f"warning: could not load --modules {args.modules_path}: {exc}", file=sys.stderr)
        if args.functions_path:
            try:
                all_functions = _load_json_list(Path(args.functions_path))
            except Exception as exc:
                print(f"warning: could not load --functions {args.functions_path}: {exc}", file=sys.stderr)
        if out_path is None:
            parser.error("--out is required (path for report.json)")
        if result_path is None:
            result_path = out_path.parent / "result.txt"
        report_threshold = args.threshold

    assert input_path is not None and out_path is not None and result_path is not None

    long_functions = _load_json_list(input_path)

    report = aggregate(
        long_functions,
        modules_manifest=modules_manifest,
        all_functions=all_functions,
        threshold=report_threshold,
    )
    # Record provenance for reproducibility.
    report["input"] = str(input_path)
    if args.orchestrate:
        report["orchestrated"] = True
        report["root"] = str((Path(args.root) if args.root else REPO_ROOT).resolve())

    headline = report["qualifying_modules"]

    # 1) stdout — the single integer (bare, for piping).
    print(headline)

    # 2) result.txt — same integer + newline.
    result_path.parent.mkdir(parents=True, exist_ok=True)
    result_path.write_text(f"{headline}\n", encoding="utf-8")

    # 3) report.json — full payload.
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")

    # 4) report.md — when requested.
    if args.md_path:
        md_path = Path(args.md_path)
        md_path.parent.mkdir(parents=True, exist_ok=True)
        md_path.write_text(render_markdown(report), encoding="utf-8")
        print(f"wrote {result_path} ({headline}), {out_path}, {md_path}", file=sys.stderr)
    else:
        print(f"wrote {result_path} ({headline}), {out_path}", file=sys.stderr)

    if tmpdir is not None:
        tmpdir.cleanup()

    return 0


if __name__ == "__main__":
    sys.exit(main())
