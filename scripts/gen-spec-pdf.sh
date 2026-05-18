#!/usr/bin/env bash
set -euo pipefail

# gen-spec-pdf.sh — Convert SPEC.md (with Mermaid diagrams) to PDF
#
# Usage:
#   ./scripts/gen-spec-pdf.sh          # Convert SPEC.md → SPEC.pdf
#   ./scripts/gen-spec-pdf.sh -o out.pdf   # Custom output path (renamed after generation)
#   ./scripts/gen-spec-pdf.sh --watch      # Watch SPEC.md for changes

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

INPUT="$PROJECT_ROOT/SPEC.md"
OUTPUT="$PROJECT_ROOT/SPEC.pdf"
WATCH=false

# ── Parse arguments ─────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        -o|--output)
            OUTPUT="$2"
            shift 2
            ;;
        -w|--watch)
            WATCH=true
            shift
            ;;
        -h|--help)
            cat <<'EOF'
Usage: gen-spec-pdf.sh [OPTIONS]

Convert SPEC.md (with embedded Mermaid diagrams) to a PDF document.

Options:
  -o, --output PATH   Output PDF path (default: ./SPEC.pdf)
  -w, --watch         Watch SPEC.md for changes and auto-regenerate
  -h, --help          Show this help message

Dependencies:
  • md-to-pdf   (npm install -g md-to-pdf)
  • mmdc        (npm install -g @mermaid-js/mermaid-cli)
  • Chromium/Chrome (for Puppeteer rendering)

EOF
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            exit 1
            ;;
    esac
done

# ── Dependency checks ──────────────────────────────────────────────────────
check_dep() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "❌ Error: '$1' is not installed." >&2
        echo "   Install it with: npm install -g $2" >&2
        exit 1
    fi
}

check_dep "md-to-pdf" "md-to-pdf"
check_dep "mmdc" "@mermaid-js/mermaid-cli"

# ── Generate PDF ───────────────────────────────────────────────────────────
generate() {
    echo "📄 Converting $(basename "$INPUT") → $(basename "$OUTPUT") ..."

    cd "$PROJECT_ROOT"

    # md-to-pdf derives output name from input (SPEC.md → SPEC.pdf).
    # We generate into the project root then optionally rename.
    local tmp_out="$PROJECT_ROOT/SPEC.pdf"

    rm -f "$tmp_out"

    md-to-pdf "$INPUT" \
        --pdf-options '{
            "format": "A4",
            "margin": {
                "top": "20mm",
                "bottom": "20mm",
                "left": "20mm",
                "right": "20mm"
            },
            "printBackground": true
        }' \
        --launch-options '{
            "args": ["--no-sandbox", "--disable-setuid-sandbox"]
        }'

    if [[ -f "$tmp_out" ]]; then
        # If user requested a different output path, move it there
        if [[ "$tmp_out" != "$OUTPUT" ]]; then
            mv "$tmp_out" "$OUTPUT"
        fi
        local size
        size=$(du -h "$OUTPUT" | cut -f1)
        echo "✅ Done! PDF generated: $OUTPUT ($size)"
    else
        echo "❌ Error: PDF was not generated." >&2
        exit 1
    fi
}

# ── Watch mode ─────────────────────────────────────────────────────────────
watch_mode() {
    if ! command -v inotifywait >/dev/null 2>&1; then
        echo "❌ 'inotifywait' not found. Install inotify-tools for --watch." >&2
        exit 1
    fi

    echo "👀 Watching $(basename "$INPUT") for changes... (Ctrl+C to stop)"
    generate

    while inotifywait -e modify,move,create,delete "$INPUT" >/dev/null 2>&1; do
        echo "📝 Change detected, regenerating..."
        generate
    done
}

# ── Main ───────────────────────────────────────────────────────────────────
if $WATCH; then
    watch_mode
else
    generate
fi
