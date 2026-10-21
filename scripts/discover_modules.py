#!/usr/bin/env python3
"""
discover_modules.py — Fast, language-aware file discovery and module mapper.

Recursively scans a repository root, applies exclusion filters, groups files
by language via extension, and maps each file to a module identifier.

Output JSON manifest: list of {module_id, file_path, language, file_line_count}.

Usage:
    python discover_modules.py --root . --out modules.json
    python discover_modules.py --root /path/to/repo --out modules.json --exclude target --exclude dist
    python discover_modules.py --root . --out modules.json --package-mode
    python discover_modules.py --root . --out modules.json --config exclusions.json

Config / defaults:
    Default exclusion dirs (always skipped): assembled from DEFAULT_EXCLUDE_DIRS
    and, when present, .gitignore patterns at the root. All exclusion names
    are configurable via --exclude flags and --config JSON.

    Default exclusion file patterns: DEFAULT_EXCLUDE_FILE_PATTERNS (e.g. *.pyc).

    Extension -> language mapping: EXTENSION_MAP (override via --config).

    Module ID modes:
        - "path"    (default): relative POSIX path from root is the module_id.
        - "package": nearest ancestor directory containing a package marker
                     (__init__.py, Cargo.toml, package.json, go.mod, etc.)
                     is used; module_id = "<package_path>/<relative_remainder>".
                     Falls back to relative path when no marker is found.
"""

from __future__ import annotations

import argparse
import fnmatch
import json
import os
import sys
from pathlib import Path

# ---------------------------------------------------------------------------
# Defaults (used when no --config is supplied; all configurable)
# ---------------------------------------------------------------------------

DEFAULT_EXCLUDE_DIRS: list[str] = [
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "__pycache__",
    ".mypy_cache",
    ".pytest_cache",
    ".tox",
    ".venv",
    "venv",
    "env",
    ".env",
    "target",
    "build",
    "dist",
    ".next",
    ".nuxt",
    "coverage",
    ".coverage",
    "vendor",
    ".idea",
    ".vscode",
    ".gradle",
    ".cargo",  # local cargo cache if vendored
]

DEFAULT_EXCLUDE_FILE_PATTERNS: list[str] = [
    "*.pyc",
    "*.pyo",
    "*.so",
    "*.o",
    "*.a",
    "*.class",
    "*.min.js",
    "*.min.css",
    "*.map",
    "*.lock",  # package-lock.json etc handled specially — keep generic .lock
    "*.log",
    "*.tmp",
    "*.swp",
    "*.swo",
]

# Extension (lowercase, with dot) -> language label
EXTENSION_MAP: dict[str, str] = {
    ".py": "python",
    ".pyi": "python",
    ".rs": "rust",
    ".js": "javascript",
    ".jsx": "javascript",
    ".mjs": "javascript",
    ".cjs": "javascript",
    ".ts": "typescript",
    ".tsx": "typescript",
    ".mts": "typescript",
    ".cts": "typescript",
    ".go": "go",
    ".java": "java",
    ".kt": "kotlin",
    ".kts": "kotlin",
    ".rb": "ruby",
    ".php": "php",
    ".cs": "csharp",
    ".cpp": "cpp",
    ".cc": "cpp",
    ".cxx": "cpp",
    ".c": "c",
    ".h": "c",
    ".hpp": "cpp",
    ".hh": "cpp",
    ".swift": "swift",
    ".scala": "scala",
    ".sh": "shell",
    ".bash": "shell",
    ".zsh": "shell",
    ".ps1": "powershell",
    ".r": "r",
    ".R": "r",
    ".lua": "lua",
    ".dart": "dart",
    ".elm": "elm",
    ".ex": "elixir",
    ".exs": "elixir",
    ".erl": "erlang",
    ".hs": "haskell",
    ".clj": "clojure",
    ".toml": "toml",
    ".yaml": "yaml",
    ".yml": "yaml",
    ".json": "json",
    ".xml": "xml",
    ".html": "html",
    ".htm": "html",
    ".css": "css",
    ".scss": "css",
    ".less": "css",
    ".sql": "sql",
    ".graphql": "graphql",
    ".gql": "graphql",
    ".proto": "protobuf",
    ".md": "markdown",
    ".rst": "rst",
    ".dockerfile": "dockerfile",
    ".tf": "terraform",
    ".vue": "vue",
    ".svelte": "svelte",
}

# Files whose exact basename maps to a language (no extension or special)
BASENAME_MAP: dict[str, str] = {
    "Dockerfile": "dockerfile",
    "Makefile": "makefile",
    "makefile": "makefile",
    "CMakeLists.txt": "cmake",
    "build.gradle": "gradle",
    "build.gradle.kts": "gradle",
}

# Package marker filenames — presence means "this dir is a package root"
PACKAGE_MARKERS: list[str] = [
    "__init__.py",
    "Cargo.toml",
    "package.json",
    "go.mod",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "setup.py",
    "setup.cfg",
    "pyproject.toml",
    ".csproj",  # glob suffix match
    "Gemfile",
    "mix.exs",
]

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _load_gitignore_patterns(root: Path) -> list[str]:
    """Parse .gitignore at root into glob-style patterns (best-effort)."""
    gi = root / ".gitignore"
    if not gi.is_file():
        return []
    patterns: list[str] = []
    try:
        for line in gi.read_text(encoding="utf-8", errors="replace").splitlines():
            stripped = line.strip()
            if not stripped or stripped.startswith("#"):
                continue
            # Remove leading slash, handle trailing slash (= dir)
            p = stripped.lstrip("/")
            if p.startswith("!"):
                continue  # negations not supported in this simple pass
            patterns.append(p)
    except OSError:
        pass
    return patterns


def _is_excluded_dir(dirname: str, exclude_dirs: set[str], gitignore_patterns: list[str]) -> bool:
    if dirname in exclude_dirs:
        return True
    for pat in gitignore_patterns:
        # Directory patterns like "target/" or "build"
        norm = pat.rstrip("/")
        if fnmatch.fnmatch(dirname, norm) or fnmatch.fnmatch(dirname, pat):
            return True
    return False


def _is_excluded_file(filename: str, exclude_file_patterns: list[str]) -> bool:
    for pat in exclude_file_patterns:
        if fnmatch.fnmatch(filename, pat):
            return True
    return False


def _detect_language(path: Path, ext_map: dict[str, str], basename_map: dict[str, str]) -> str:
    # Basename first (Dockerfile etc.)
    blang = basename_map.get(path.name)
    if blang:
        return blang
    ext = path.suffix.lower()
    # Handle compound extensions like .min.js already covered via full suffix check
    # For .d.ts etc., check double suffix
    if path.suffixes and len(path.suffixes) >= 2:
        compound = "".join(path.suffixes[-2:]).lower()
        if compound in ext_map:
            return ext_map[compound]
    if ext in ext_map:
        return ext_map[ext]
    if ext:
        # Unknown extension — return stripped ext as language hint
        return ext.lstrip(".")
    return "unknown"


def _count_lines(path: Path) -> int:
    """Fast line count — binary-safe, counts \\n occurrences."""
    try:
        with open(path, "rb") as f:
            # Read in chunks for large files
            count = 0
            while True:
                chunk = f.read(1024 * 1024)
                if not chunk:
                    break
                count += chunk.count(b"\n")
            # If file doesn't end with newline but has content, add 1
            # Check last byte
            f.seek(0, 2)
            size = f.tell()
            if size > 0:
                f.seek(size - 1)
                if f.read(1) != b"\n":
                    count += 1
            return count
    except OSError:
        return 0


def _find_package_root(file_parent: Path, repo_root: Path, markers: list[str]) -> Path | None:
    """Walk up from file_parent to repo_root, return nearest dir containing a marker."""
    cur = file_parent
    while True:
        for marker in markers:
            if marker.startswith("."):
                # suffix match: any file ending with marker in this dir
                try:
                    if any(p.name.endswith(marker) for p in cur.iterdir() if p.is_file()):
                        return cur
                except OSError:
                    continue
            else:
                if (cur / marker).exists():
                    return cur
        if cur == repo_root or cur.parent == cur:
            break
        cur = cur.parent
    return None


def discover(
    root: Path,
    exclude_dirs: set[str],
    exclude_file_patterns: list[str],
    ext_map: dict[str, str],
    basename_map: dict[str, str],
    package_mode: bool = False,
    package_markers: list[str] | None = None,
    include_unknown: bool = False,
    follow_symlinks: bool = False,
    use_gitignore: bool = True,
) -> list[dict]:
    """Scan root and return manifest entries."""
    root = root.resolve()
    gitignore_patterns: list[str] = []
    if use_gitignore:
        gitignore_patterns = _load_gitignore_patterns(root)

    markers = package_markers or PACKAGE_MARKERS
    entries: list[dict] = []

    for dirpath, dirnames, filenames in os.walk(root, followlinks=follow_symlinks):
        # Prune excluded directories in-place so os.walk doesn't descend
        dirnames[:] = [
            d for d in dirnames
            if not _is_excluded_dir(d, exclude_dirs, gitignore_patterns)
            # Also skip hidden dirs that look like caches if not explicitly listed
            # (but keep .config etc. if user wants — only skip dotfiles that are typical junk)
        ]
        # Sort for deterministic output
        dirnames.sort()
        filenames.sort()

        dir_path = Path(dirpath)
        for fname in filenames:
            if _is_excluded_file(fname, exclude_file_patterns):
                continue
            # Also check gitignore patterns against filename
            if use_gitignore:
                skip = False
                for pat in gitignore_patterns:
                    if fnmatch.fnmatch(fname, pat) or fnmatch.fnmatch(fname, pat.lstrip("/")):
                        skip = True
                        break
                if skip:
                    continue

            fpath = dir_path / fname
            # Skip symlinked files if not following symlinks
            if not follow_symlinks and fpath.is_symlink():
                continue

            lang = _detect_language(fpath, ext_map, basename_map)
            if lang == "unknown" and not include_unknown:
                continue

            rel = fpath.relative_to(root).as_posix()

            if package_mode:
                pkg_root = _find_package_root(fpath.parent, root, markers)
                if pkg_root is not None:
                    pkg_rel = pkg_root.relative_to(root).as_posix()
                    # module_id = package prefix + "/" + remainder (or just package dir if at root of package)
                    if pkg_rel == ".":
                        module_id = rel
                    else:
                        module_id = rel  # keep full path; package grouping via separate field
                        # Also store package info
                    # For now module_id stays as relative path; package field added
                else:
                    module_id = rel
            else:
                module_id = rel

            entry: dict = {
                "module_id": module_id,
                "file_path": rel,
                "language": lang,
                "file_line_count": _count_lines(fpath),
            }
            if package_mode:
                pkg_root_found = _find_package_root(fpath.parent, root, markers)
                if pkg_root_found is not None:
                    pr = pkg_root_found.relative_to(root).as_posix()
                    entry["package"] = pr if pr != "." else "."
                else:
                    entry["package"] = None
            entries.append(entry)

    # Deterministic sort by file_path
    entries.sort(key=lambda e: e["file_path"])
    return entries


def _load_config(config_path: Path) -> dict:
    try:
        return json.loads(config_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        print(f"error: failed to load config {config_path}: {exc}", file=sys.stderr)
        sys.exit(2)


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Discover codebase files and emit a JSON module manifest.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=(
            "Examples:\n"
            "  python discover_modules.py --root . --out modules.json\n"
            "  python discover_modules.py --root . --out modules.json --package-mode\n"
            "  python discover_modules.py --root . --out modules.json --exclude target --exclude .next\n"
            "  python discover_modules.py --root . --out modules.json --config exclusions.json\n"
            "  python discover_modules.py --root . --out modules.json --include-unknown\n"
        ),
    )
    parser.add_argument("--root", default=".", help="Repository root to scan (default: .).")
    parser.add_argument("--out", required=True, help="Output JSON file path for the manifest.")
    parser.add_argument(
        "--exclude",
        action="append",
        default=[],
        dest="extra_excludes",
        help="Additional directory name to exclude (repeatable).",
    )
    parser.add_argument(
        "--exclude-pattern",
        action="append",
        default=[],
        dest="extra_exclude_patterns",
        help="Additional file glob to exclude (repeatable, e.g. '*.generated.*').",
    )
    parser.add_argument(
        "--config",
        default=None,
        help="Optional JSON config file with keys: exclude_dirs, exclude_file_patterns, "
             "extension_map, basename_map, package_markers.",
    )
    parser.add_argument(
        "--package-mode",
        action="store_true",
        help="Enable package aggregation: each entry gets a 'package' field (nearest marker dir).",
    )
    parser.add_argument(
        "--include-unknown",
        action="store_true",
        help="Include files with unknown extensions (default: skip them).",
    )
    parser.add_argument(
        "--follow-symlinks",
        action="store_true",
        help="Follow symlinked dirs/files (default: skip symlinks).",
    )
    parser.add_argument(
        "--no-gitignore",
        action="store_true",
        help="Do not apply .gitignore exclusions.",
    )
    parser.add_argument(
        "--format",
        choices=["array", "object"],
        default="array",
        help="Output format: 'array' (JSON array, default) or 'object' ({modules: [...], summary: {...}}).",
    )
    parser.add_argument(
        "--print-summary",
        action="store_true",
        help="Print a summary table to stdout after writing the manifest.",
    )

    args = parser.parse_args(argv)

    root = Path(args.root)
    if not root.exists():
        print(f"error: root does not exist: {root}", file=sys.stderr)
        return 2
    if not root.is_dir():
        print(f"error: root is not a directory: {root}", file=sys.stderr)
        return 2

    # Start from defaults, layer config + CLI extras
    exclude_dirs: set[str] = set(DEFAULT_EXCLUDE_DIRS)
    exclude_file_patterns: list[str] = list(DEFAULT_EXCLUDE_FILE_PATTERNS)
    ext_map: dict[str, str] = dict(EXTENSION_MAP)
    basename_map: dict[str, str] = dict(BASENAME_MAP)
    package_markers: list[str] = list(PACKAGE_MARKERS)

    if args.config:
        cfg = _load_config(Path(args.config))
        if "exclude_dirs" in cfg:
            exclude_dirs = set(cfg["exclude_dirs"])
        if "exclude_file_patterns" in cfg:
            exclude_file_patterns = list(cfg["exclude_file_patterns"])
        if "extension_map" in cfg:
            ext_map.update(cfg["extension_map"])
        if "basename_map" in cfg:
            basename_map.update(cfg["basename_map"])
        if "package_markers" in cfg:
            package_markers = list(cfg["package_markers"])

    # CLI extras are additive
    exclude_dirs.update(args.extra_excludes)
    exclude_file_patterns.extend(args.extra_exclude_patterns)

    entries = discover(
        root=root,
        exclude_dirs=exclude_dirs,
        exclude_file_patterns=exclude_file_patterns,
        ext_map=ext_map,
        basename_map=basename_map,
        package_mode=args.package_mode,
        package_markers=package_markers,
        include_unknown=args.include_unknown,
        follow_symlinks=args.follow_symlinks,
        use_gitignore=not args.no_gitignore,
    )

    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)

    if args.format == "object":
        # Summary counts
        by_lang: dict[str, int] = {}
        total_lines = 0
        for e in entries:
            by_lang[e["language"]] = by_lang.get(e["language"], 0) + 1
            total_lines += e["file_line_count"]
        payload = {
            "root": str(root.resolve()),
            "summary": {
                "total_files": len(entries),
                "total_lines": total_lines,
                "by_language": by_lang,
            },
            "modules": entries,
        }
        out_path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    else:
        out_path.write_text(json.dumps(entries, indent=2) + "\n", encoding="utf-8")

    print(f"Wrote {len(entries)} modules to {out_path}", file=sys.stderr)

    if args.print_summary:
        by_lang2: dict[str, int] = {}
        for e in entries:
            by_lang2[e["language"]] = by_lang2.get(e["language"], 0) + 1
        print(f"\nTotal files: {len(entries)}", file=sys.stderr)
        for lang, cnt in sorted(by_lang2.items(), key=lambda x: (-x[1], x[0])):
            print(f"  {lang}: {cnt}", file=sys.stderr)

    return 0


if __name__ == "__main__":
    sys.exit(main())
