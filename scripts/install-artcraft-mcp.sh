#!/usr/bin/env bash
#
# install-artcraft-mcp.sh - generate MCP server definitions for the ArtCraft apps.
#
# The ArtCraft "Crafting Apps" (https://getartcraft.com, GitHub org "storytold")
# each ship a headless command-line companion `<app>-cli` that exposes an MCP
# (Model Context Protocol) server over stdio via its `mcp` subcommand:
#
#   wordcraft-cli mcp      -> WordCraft        (storytold/wordcraft)
#   gridcraft-cli mcp      -> GridCraft        (storytold/gridcraft)
#   deckcraft-cli mcp      -> DeckCraft        (storytold/deckcraft)
#   vectorcraft-cli mcp    -> VectorCraft      (storytold/vectorcraft)
#   photocraft-cli mcp     -> PhotoCraft       (storytold/photocraft)
#   figcraft-cli mcp       -> FigCraft         (storytold/figcraft)
#   pdfcraft-cli mcp       -> PdfCraft         (storytold/pdfcraft)
#   cadcraft-cli mcp       -> CadCraft         (storytold/cadcraft)
#   soundcraft-cli mcp     -> SoundCraft       (storytold/soundcraft)
#   effectcraft-cli mcp    -> EffectCraft      (storytold/effectcraft)
#   designcraft-cli mcp    -> DesignCraft      (storytold/designcraft)
#   filmcraft-cli mcp      -> FilmCraft        (storytold/filmcraft)
#   lightcraft-cli mcp     -> LightCraft       (storytold/lightcraft)
#
# This script writes one standalone MCP config file per app into ragent's MCP
# discovery directory, `~/.config/ragent/servers/<app>.json`. ragent does NOT
# auto-load that directory as session config: it is read only when the user runs
# `/mcp discover` in the TUI, which offers each server for opt-in. Nothing is
# connected, and no child process is spawned, until the user enables it there
# (or in `mcp_state.json`).
#
# The two tooling apps are intentionally skipped: craft-launcher is the app
# launcher and craft-fonts a shared font crate, and neither ships an MCP server.
#
# Assumes the apps are already installed and their `<app>-cli` companions are on
# PATH; a missing companion is reported and skipped.
#
# Usage:
#   scripts/install-artcraft-mcp.sh [options] [app ...]
#
# Options:
#   -a, --app NAME     Generate only this app (repeatable): any name listed above,
#                      or "all". Default: every app.
#   -d, --dir DIR      Output directory (default: ~/.config/ragent/servers).
#   -f, --force        Overwrite an existing <app>.json (default: leave it).
#   -n, --dry-run      Print the files that would be written, write nothing.
#   -h, --help         Show this help.
#
# Examples:
#   scripts/install-artcraft-mcp.sh                  # every app
#   scripts/install-artcraft-mcp.sh -a wordcraft     # just WordCraft
#   scripts/install-artcraft-mcp.sh wordcraft pdfcraft
#   scripts/install-artcraft-mcp.sh --dry-run        # show the plan only
#   scripts/install-artcraft-mcp.sh --force          # refresh every file
#
set -u

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

readonly SCRIPT_NAME="${0##*/}"

# ragent consolidates its global state under the XDG config dir; the MCP
# discovery directory is the "servers" subdirectory of that root. Mirror
# ragent_config::user_dirs::global_mcp_servers_dir() ($XDG_CONFIG_HOME aware).
readonly CONFIG_ROOT="${XDG_CONFIG_HOME:-${HOME}/.config}/ragent"
readonly DEFAULT_DIR="${CONFIG_ROOT}/servers"

# Every app that ships an MCP server, in display order:
#   id | FriendlyName | short description
readonly APPS_DATA='
wordcraft|WordCraft|word processor (Microsoft Word-class)
gridcraft|GridCraft|spreadsheet (Microsoft Excel-class)
deckcraft|DeckCraft|presentations (Microsoft PowerPoint-class)
vectorcraft|VectorCraft|vector illustration (Adobe Illustrator-class)
photocraft|PhotoCraft|image editor (Adobe Photoshop-class)
figcraft|FigCraft|interface design (Figma-class)
pdfcraft|PdfCraft|PDF editing (Adobe Acrobat-class)
cadcraft|CadCraft|2D/3D drafting (AutoCAD-class)
soundcraft|SoundCraft|digital audio workstation (Pro Tools-class)
effectcraft|EffectCraft|motion graphics compositing (After Effects-class)
designcraft|DesignCraft|page layout (Adobe InDesign-class)
filmcraft|FilmCraft|video editor (Adobe Premiere Pro-class)
lightcraft|LightCraft|photo library and raw developer (Lightroom-class)'

readonly ALL_APPS="$(printf '%s\n' "$APPS_DATA" | sed -n 's/^\([^|]*\)|.*/\1/p' | tr '\n' ' ')"

# ---------------------------------------------------------------------------
# Logging
# ---------------------------------------------------------------------------

log()  { printf '[artcraft-mcp] %s\n' "$*"; }
warn() { printf '[artcraft-mcp] WARNING: %s\n' "$*" >&2; }
die()  { printf '[artcraft-mcp] ERROR: %s\n' "$*" >&2; exit 1; }

# ---------------------------------------------------------------------------
# Usage
# ---------------------------------------------------------------------------

usage() {
    sed -n '2,/^set -u/p' "$0" | sed -e 's/^# \{0,1\}//' -e '/^set -u/d'
    exit "${1:-0}"
}

# ---------------------------------------------------------------------------
# Argument parsing
# ---------------------------------------------------------------------------

APPS=""
OUT_DIR="$DEFAULT_DIR"
FORCE=0
DRY_RUN=0

while [ $# -gt 0 ]; do
    case "$1" in
        -a|--app)
            [ $# -ge 2 ] || die "--app requires a value"
            APPS="${APPS} $2"; shift 2 ;;
        -d|--dir)
            [ $# -ge 2 ] || die "--dir requires a value"
            OUT_DIR="$2"; shift 2 ;;
        -f|--force)   FORCE=1; shift ;;
        -n|--dry-run) DRY_RUN=1; shift ;;
        -h|--help)    usage 0 ;;
        -*)           die "unknown option: $1 (try --help)" ;;
        *)            APPS="${APPS} $1"; shift ;;
    esac
done

# Expand the app list.
if [ -z "$APPS" ]; then
    APPS="$ALL_APPS"
fi
RESOLVED_APPS=""
for app in $APPS; do
    case "$app" in
        all) RESOLVED_APPS="$RESOLVED_APPS $ALL_APPS" ;;
        wordcraft|gridcraft|deckcraft|vectorcraft|photocraft|figcraft|pdfcraft|\
        cadcraft|soundcraft|effectcraft|designcraft|filmcraft|lightcraft)
            RESOLVED_APPS="$RESOLVED_APPS $app" ;;
        *) die "unknown app: '$app' (run with --help for the list)" ;;
    esac
done
# Deduplicate while preserving order.
APPS=""
for app in $RESOLVED_APPS; do
    case " $APPS " in *" $app "*) ;; *) APPS="$APPS $app" ;; esac
done

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

have() { command -v "$1" >/dev/null 2>&1; }

# field ID FIELD_NUM - print field FIELD_NUM (1-based) of the APPS_DATA row for ID.
field() {
    printf '%s\n' "$APPS_DATA" | sed -n "/^${1}|/p" \
        | awk -F'|' -v n="$2" '{print $n}'
}

# json_escape STRING - escape a value for a JSON string literal.
json_escape() {
    printf '%s' "$1" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g'
}

# write_config ID NAME CMD DEST - write the MCP config, atomically.
write_config() {
    local id="$1" name="$2" cmd="$3" dest="$4" tmp
    tmp="${dest}.tmp.$$"
    cat > "$tmp" <<EOF
{
  "type": "stdio",
  "name": "$(json_escape "${name} MCP Server")",
  "command": "$(json_escape "$cmd")",
  "args": ["mcp"],
  "env": {}
}
EOF
    mv -f "$tmp" "$dest"
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

main() {
    log "output dir: ${OUT_DIR}"

    local wrote=0 skipped=0 missing=0 app id name cli cmd dest
    for app in $APPS; do
        id="$app"
        name="$(field "$app" 2)"
        cli="${app}-cli"

        if ! have "$cli"; then
            warn "'${cli}' not found on PATH; skipping ${id} (install the app first)"
            missing=$((missing + 1))
            continue
        fi
        cmd="$(command -v "$cli")"
        dest="${OUT_DIR}/${id}.json"

        # Re-running must not clobber a file the user has hand-edited.
        if [ -e "$dest" ] && [ "$FORCE" -ne 1 ]; then
            warn "already exists, leaving untouched: ${dest} (use --force to overwrite)"
            skipped=$((skipped + 1))
            continue
        fi

        if [ "$DRY_RUN" -eq 1 ]; then
            log "would write: ${dest}  (${cmd} mcp)"
            continue
        fi

        mkdir -p "$OUT_DIR"
        write_config "$id" "$name" "$cmd" "$dest" \
            || { warn "failed to write ${dest}"; missing=$((missing + 1)); continue; }
        log "wrote: ${dest}  (${cmd} mcp)"
        wrote=$((wrote + 1))
    done

    if [ "$DRY_RUN" -eq 1 ]; then
        log "dry run: nothing written"
        return
    fi

    log "done: ${wrote} written, ${skipped} left untouched, ${missing} unavailable"
    if [ "$wrote" -gt 0 ]; then
        log "run '/mcp discover' in ragent to review and enable the servers"
    fi
    [ "$missing" -eq 0 ] || exit 1
}

main "$@"
