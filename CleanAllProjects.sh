#!/usr/bin/env bash
#
# CleanAllProjects.sh - reclaim disk space used by Rust build artifacts.
#
# Walks every immediate subdirectory of the projects root (default
# ~/Projects) and runs `cargo clean` in each one whose root holds a
# Cargo.toml. Nothing outside the projects root is touched.
#
# Usage:
#   ./CleanAllProjects.sh            # clean every Rust project
#   ./CleanAllProjects.sh -n         # dry run: list what would be cleaned
#   ./CleanAllProjects.sh -y         # do not prompt for confirmation
#   ./CleanAllProjects.sh -d DIR     # use DIR as the projects root
#   ./CleanAllProjects.sh -h         # show this help
#
# Exit status: 0 on success, 1 if any project's cargo clean failed.

set -uo pipefail

PROJECTS_ROOT="${HOME}/Projects"
DRY_RUN=0
ASSUME_YES=0

usage() {
    sed -n '3,17p' "$0" | sed 's/^# \{0,1\}//'
}

while getopts ':nyd:h' opt; do
    case "$opt" in
        n) DRY_RUN=1 ;;
        y) ASSUME_YES=1 ;;
        d) PROJECTS_ROOT="$OPTARG" ;;
        h) usage; exit 0 ;;
        :) echo "error: -${OPTARG} requires an argument" >&2; exit 2 ;;
        *) echo "error: unknown option -${OPTARG}" >&2; usage >&2; exit 2 ;;
    esac
done
shift $((OPTIND - 1))

if [ ! -d "$PROJECTS_ROOT" ]; then
    echo "error: projects root not found: $PROJECTS_ROOT" >&2
    exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
    echo "error: cargo not found on PATH" >&2
    exit 1
fi

# Collect every immediate child directory that has a Cargo.toml in its root.
projects=()
for dir in "$PROJECTS_ROOT"/*/; do
    [ -d "$dir" ] || continue
    dir="${dir%/}"
    [ -f "$dir/Cargo.toml" ] && projects+=("$dir")
done

if [ ${#projects[@]} -eq 0 ]; then
    echo "No Rust projects (Cargo.toml) found under $PROJECTS_ROOT"
    exit 0
fi

echo "Projects root: $PROJECTS_ROOT"
echo "Found ${#projects[@]} Rust project(s):"
for dir in "${projects[@]}"; do
    size=$(du -sh "$dir/target" 2>/dev/null | cut -f1)
    printf '  %-40s target: %s\n' "$(basename "$dir")" "${size:-none}"
done
echo

if [ "$DRY_RUN" -eq 1 ]; then
    echo "Dry run - nothing cleaned."
    exit 0
fi

if [ "$ASSUME_YES" -eq 0 ]; then
    read -r -p "Run 'cargo clean' in all of them? [y/N] " reply
    case "$reply" in
        [yY]|[yY][eE][sS]) ;;
        *) echo "Aborted."; exit 0 ;;
    esac
fi

failed=0
for dir in "${projects[@]}"; do
    name=$(basename "$dir")
    printf 'Cleaning %s ... ' "$name"
    if ( cd "$dir" && cargo clean >/dev/null 2>&1 ); then
        echo "ok"
    else
        echo "FAILED"
        failed=$((failed + 1))
    fi
done

echo
if [ "$failed" -ne 0 ]; then
    echo "Done with $failed failure(s)."
    exit 1
fi
echo "Done. All ${#projects[@]} project(s) cleaned."
