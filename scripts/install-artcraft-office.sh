#!/usr/bin/env bash
#
# install-artcraft-office.sh - download and install the ArtCraft Crafting Apps.
#
# Installs the free, open-source clean-room reimplementations of proprietary
# creative and productivity applications maintained by the ArtCraft team
# (https://getartcraft.com). Every app is published under the GitHub org
# "storytold".
#
# Office-style apps:
#   wordcraft     -> Microsoft Word        (storytold/wordcraft)
#   gridcraft     -> Microsoft Excel       (storytold/gridcraft)
#   deckcraft     -> Microsoft PowerPoint  (storytold/deckcraft)
#
# Creative apps:
#   vectorcraft   -> Adobe Illustrator     (storytold/vectorcraft)
#   photocraft    -> Adobe Photoshop       (storytold/photocraft)
#   figcraft      -> Figma / interface design (storytold/figcraft)
#   pdfcraft      -> Adobe Acrobat         (storytold/pdfcraft)
#   cadcraft      -> AutoCAD               (storytold/cadcraft)
#   soundcraft    -> Avid Pro Tools        (storytold/soundcraft)
#   effectcraft   -> Adobe After Effects   (storytold/effectcraft)
#   designcraft   -> page / layout design  (storytold/designcraft)
#   filmcraft     -> Adobe Premiere Pro    (storytold/filmcraft)
#   lightcraft    -> Adobe Lightroom       (storytold/lightcraft)
#
# Tooling:
#   craft-launcher -> the ArtCraft app launcher (storytold/craft-launcher;
#                     release assets are prefixed "artcraft-launcher-")
#   craft-fonts    -> shared fonts and font crate (storytold/craft-fonts);
#                     publishes no releases, installed as a source checkout of
#                     the default branch (no checksum available)
#
# For each release app the script resolves the latest GitHub release, downloads
# the native package for the host OS/architecture, verifies it against the
# release SHA256SUMS.txt, and installs it with the platform package manager.
# Source apps (craft-fonts) are fetched from their default branch instead.
#
# Dependencies: curl (or wget), git, sha256sum (or shasum), and tar/unzip for
# tarball and portable installs. Release metadata is read from the public
# GitHub release pages and repository refs, so no API token is needed and the
# REST API rate limit does not apply. GITHUB_TOKEN / GH_TOKEN are ignored.
#
# Usage:
#   scripts/install-artcraft-office.sh [options] [app ...]
#
# Options:
#   -a, --app NAME        Install only this app (repeatable): any name listed
#                         above, or "all". Default: every app.
#   -v, --version VER     Pin a release version (e.g. 0.6.1) instead of latest.
#   -m, --method METHOD   Install method: auto, rpm, deb, flatpak, appimage,
#                         tar, dmg, msi, portable, download.
#   -s, --source          Install from a git checkout of the app's default
#                         branch instead of a release package (skips checksums).
#   -d, --dest DIR        Destination for appimage/portable/source installs
#                         (default: ~/.local/bin).
#   -k, --keep            Keep the downloaded package file.
#   -n, --dry-run         Resolve and print the plan without downloading.
#   -y, --yes             Do not prompt before installing.
#   -h, --help            Show this help.
#
# Environment:
#   RAGCRAFT_VERSION         Same as --version.
#   RAGCRAFT_METHOD          Same as --method.
#
# Examples:
#   scripts/install-artcraft-office.sh                 # every app, auto method
#   scripts/install-artcraft-office.sh -a wordcraft    # just WordCraft
#   scripts/install-artcraft-office.sh -a wordcraft pdfcraft
#   scripts/install-artcraft-office.sh -m flatpak      # force Flatpak bundles
#   scripts/install-artcraft-office.sh -m appimage -d ~/Applications
#   scripts/install-artcraft-office.sh --dry-run       # show the plan only
#   scripts/install-artcraft-office.sh --source -a craft-fonts
#
set -u

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

readonly SCRIPT_NAME="${0##*/}"
readonly REPO_OWNER="storytold"

# Every installable app, in display order (office, creative, tooling).
readonly ALL_APPS="wordcraft gridcraft deckcraft vectorcraft photocraft \
figcraft pdfcraft cadcraft soundcraft effectcraft designcraft filmcraft \
lightcraft craft-launcher craft-fonts"

# Where appimage/portable artifacts are placed by default.
DEFAULT_DEST="${HOME}/.local/bin"

# Download cache (kept out of /tmp per project convention).
CACHE_DIR="${RAGCRAFT_CACHE_DIR:-${TMPDIR:-/tmp}/artcraft-office}"

# ---------------------------------------------------------------------------
# Logging
# ---------------------------------------------------------------------------

log()  { printf '[artcraft] %s\n' "$*"; }
warn() { printf '[artcraft] WARNING: %s\n' "$*" >&2; }
die()  { printf '[artcraft] ERROR: %s\n' "$*" >&2; exit 1; }

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
VERSION=""
METHOD=""
DEST="$DEFAULT_DEST"
KEEP=0
DRY_RUN=0
ASSUME_YES=0
SOURCE=0

while [ $# -gt 0 ]; do
    case "$1" in
        -a|--app)
            [ $# -ge 2 ] || die "--app requires a value"
            APPS="${APPS} $2"; shift 2 ;;
        -v|--version)
            [ $# -ge 2 ] || die "--version requires a value"
            VERSION="$2"; shift 2 ;;
        -m|--method)
            [ $# -ge 2 ] || die "--method requires a value"
            METHOD="$2"; shift 2 ;;
        -d|--dest)
            [ $# -ge 2 ] || die "--dest requires a value"
            DEST="$2"; shift 2 ;;
        -s|--source)
            SOURCE=1; shift ;;
        -k|--keep)    KEEP=1; shift ;;
        -n|--dry-run) DRY_RUN=1; shift ;;
        -y|--yes)     ASSUME_YES=1; shift ;;
        -h|--help)    usage 0 ;;
        -*)           die "unknown option: $1 (try --help)" ;;
        *)            APPS="${APPS} $1"; shift ;;
    esac
done

# Environment fallbacks.
[ -z "$VERSION" ] && VERSION="${RAGCRAFT_VERSION:-}"
[ -z "$METHOD" ]  && METHOD="${RAGCRAFT_METHOD:-}"
[ -z "$METHOD" ]  && METHOD="auto"

# Expand the app list.
if [ -z "$APPS" ]; then
    APPS="$ALL_APPS"
fi
RESOLVED_APPS=""
for app in $APPS; do
    case "$app" in
        all) RESOLVED_APPS="$RESOLVED_APPS $ALL_APPS" ;;
        wordcraft|gridcraft|deckcraft|vectorcraft|photocraft|figcraft|pdfcraft|\
        cadcraft|soundcraft|effectcraft|designcraft|filmcraft|lightcraft|\
        craft-launcher|craft-fonts)
            RESOLVED_APPS="$RESOLVED_APPS $app" ;;
        *) die "unknown app: '$app' (run with --help for the list)" ;;
    esac
done
# Deduplicate while preserving order.
APPS=""
for app in $RESOLVED_APPS; do
    case " $APPS " in *" $app "*) ;; *) APPS="$APPS $app" ;; esac
done

# Validate the requested method.
case "$METHOD" in
    auto|rpm|deb|flatpak|appimage|tar|dmg|msi|portable|download) ;;
    *) die "unknown method: '$METHOD'" ;;
esac

# ---------------------------------------------------------------------------
# Tool discovery
# ---------------------------------------------------------------------------

have() { command -v "$1" >/dev/null 2>&1; }

FETCH=""
if have curl; then
    FETCH="curl"
elif have wget; then
    FETCH="wget"
else
    die "need curl or wget on PATH"
fi

SHASUM=""
if have sha256sum; then
    SHASUM="sha256sum"
elif have shasum; then
    SHASUM="shasum -a 256"
else
    die "need sha256sum or shasum on PATH"
fi

# ---------------------------------------------------------------------------
# Platform detection
# ---------------------------------------------------------------------------

OS=""
KERNEL="$(uname -s)"
case "$KERNEL" in
    Linux)  OS="linux" ;;
    Darwin) OS="macos" ;;
    FreeBSD) OS="freebsd" ;;
    MINGW*|MSYS*|CYGWIN*) OS="windows" ;;
    *)      OS="linux" ;;
esac

ARCH=""
case "$(uname -m)" in
    x86_64|amd64)  ARCH="x86_64" ;;
    aarch64|arm64) ARCH="aarch64" ;;
    riscv64)       ARCH="riscv64" ;;
    i386|i486|i586|i686) ARCH="x86" ;;
    *)             ARCH="$(uname -m)" ;;
esac

# ---------------------------------------------------------------------------
# HTTP helpers
# ---------------------------------------------------------------------------

# fetch_to URL DEST - download a URL to a file.
fetch_to() {
    if [ "$FETCH" = "curl" ]; then
        curl -fsSL --retry 3 --retry-delay 2 -o "$2" "$1"
    else
        wget -q -O "$2" "$1"
    fi
}

# fetch_stdout URL - print a URL to stdout.
fetch_stdout() {
    if [ "$FETCH" = "curl" ]; then
        curl -fsSL --retry 3 --retry-delay 2 "$1"
    else
        wget -q -O - "$1"
    fi
}

# latest_tag REPO - resolve the newest release tag (e.g. v0.6.1) for a repo.
# Reads the redirect target of the public releases/latest page (or the release
# atom feed for the wget path), so it needs no API token and is not subject to
# the REST API rate limit.
latest_tag() {
    local repo="$1" url
    if [ "$FETCH" = "curl" ]; then
        url="$(curl -fsS -o /dev/null -w '%{url_effective}' \
            -L "https://github.com/${REPO_OWNER}/${repo}/releases/latest")" || return 1
        printf '%s\n' "${url##*/}"
    else
        fetch_stdout "https://github.com/${REPO_OWNER}/${repo}/releases.atom" \
            | sed -n 's#.*/releases/tag/\([^"<]*\).*#\1#p' | head -n1
    fi
}

# default_branch REPO - resolve the repository's default branch (e.g. main).
# Reads the HEAD symref with git ls-remote; no API token or rate limit involved.
default_branch() {
    local repo="$1" branch
    have git || return 1
    branch="$(git ls-remote --symref "https://github.com/${REPO_OWNER}/${repo}" HEAD 2>/dev/null \
        | sed -n 's#^ref: refs/heads/\([^[:space:]]*\).*#\1#p' | head -n1)"
    [ -n "$branch" ] || return 1
    printf '%s\n' "$branch"
}

# ---------------------------------------------------------------------------
# Asset selection
# ---------------------------------------------------------------------------

# pick_method - choose the concrete install method when METHOD=auto.
pick_method() {
    if [ "$METHOD" != "auto" ]; then
        printf '%s\n' "$METHOD"
        return
    fi
    case "$OS" in
        linux)
            case "$ARCH" in
                riscv64) printf 'tar\n'; return ;;
            esac
            if have dnf || have rpm; then
                printf 'rpm\n'
            elif have apt-get || have dpkg; then
                printf 'deb\n'
            elif have flatpak; then
                printf 'flatpak\n'
            else
                printf 'appimage\n'
            fi
            ;;
        macos)  printf 'dmg\n' ;;
        windows) printf 'msi\n' ;;
        freebsd) printf 'tar\n' ;;
        *)      printf 'download\n' ;;
    esac
}

# release_asset APP VER METHOD - print the release asset filename for a version.
# Most apps name assets "<app>-<version>-...", but craft-launcher ships them as
# "artcraft-launcher-<version>-...", so the prefix is overridden for that app.
release_asset() {
    local app="$1" ver="$2" method="$3"
    local prefix="$app"
    [ "$app" = "craft-launcher" ] && prefix="artcraft-launcher"
    asset_name "$prefix" "$ver" "$method"
}

# asset_name APP VERSION METHOD - print the release asset filename.
asset_name() {
    local app="$1" ver="$2" method="$3"
    case "$method" in
        rpm)      printf '%s-%s-linux-%s.rpm\n'    "$app" "$ver" "$ARCH" ;;
        deb)      printf '%s-%s-linux-%s.deb\n'    "$app" "$ver" "$ARCH" ;;
        flatpak)  printf '%s-%s-linux-%s.flatpak\n' "$app" "$ver" "$ARCH" ;;
        appimage) printf '%s-%s-linux-%s.AppImage\n' "$app" "$ver" "$ARCH" ;;
        tar)
            if [ "$OS" = "freebsd" ]; then
                printf '%s-%s-freebsd-%s.tar.gz\n' "$app" "$ver" "$ARCH"
            else
                printf '%s-%s-linux-%s.tar.gz\n' "$app" "$ver" "$ARCH"
            fi
            ;;
        dmg)      printf '%s-%s-macos-universal.dmg\n' "$app" "$ver" ;;
        msi)      printf '%s-%s-windows-%s.msi\n' "$app" "$ver" "$ARCH" ;;
        portable) printf '%s-%s-windows-%s-portable.zip\n' "$app" "$ver" "$ARCH" ;;
        download)
            # Unknown platform: take the generic Linux tarball when it exists.
            printf '%s-%s-linux-%s.tar.gz\n' "$app" "$ver" "$ARCH" ;;
        *)        die "no asset mapping for method '$method'" ;;
    esac
}

# ---------------------------------------------------------------------------
# Integrity
# ---------------------------------------------------------------------------

# verify_sha FILE SUMS_FILE ASSET - verify FILE against SHA256SUMS.txt.
verify_sha() {
    local file="$1" sums="$2" asset="$3" want got
    want="$(sed -n "s/^\([0-9a-f]*\) [ *]${asset}\$/\1/p" "$sums" | head -n1)"
    if [ -z "$want" ]; then
        warn "no checksum entry for ${asset}; skipping verification"
        return 0
    fi
    got="$($SHASUM "$file" | awk '{print $1}')"
    if [ "$got" != "$want" ]; then
        die "checksum mismatch for ${asset}: expected ${want}, got ${got}"
    fi
    log "checksum OK: ${asset}"
}

# ---------------------------------------------------------------------------
# Installers
# ---------------------------------------------------------------------------

install_rpm() {
    local file="$1"
    if have dnf; then
        sudo dnf install -y "$file"
    elif have rpm; then
        sudo rpm -Uvh --replacepkgs "$file"
    else
        die "no rpm/dnf available"
    fi
}

install_deb() {
    local file="$1"
    if have apt-get; then
        sudo apt-get install -y "$file"
    elif have dpkg; then
        sudo dpkg -i "$file" || sudo apt-get -f install -y
    else
        die "no dpkg/apt available"
    fi
}

install_flatpak() {
    local file="$1"
    have flatpak || die "flatpak not installed"
    flatpak install --user --noninteractive -y "$file"
}

install_appimage() {
    local file="$1" target="$2" name="${3:-}"
    mkdir -p "$target"
    local out="${target}/${name}"
    cp "$file" "$out"
    chmod +x "$out"
    log "installed AppImage: ${out}"
}

install_tar() {
    local file="$1" target="$2"
    mkdir -p "$target"
    tar -xzf "$file" -C "$target"
    log "extracted tarball into: ${target}"
}

install_dmg() {
    local file="$1"
    have hdiutil || die "hdiutil not available (macOS only)"
    local mnt
    mnt="$(mktemp -d "${TMPDIR:-/tmp}/artcraft.XXXXXX")"
    hdiutil attach "$file" -mountpoint "$mnt" -nobrowse -quiet
    local app
    app="$(find "$mnt" -maxdepth 1 -name '*.app' | head -n1)"
    if [ -z "$app" ]; then
        hdiutil detach "$mnt" -quiet
        die "no .app bundle inside ${file}"
    fi
    cp -R "$app" /Applications/
    hdiutil detach "$mnt" -quiet
    log "installed to /Applications/$(basename "$app")"
}

install_msi() {
    local file="$1"
    have msiexec || die "msiexec not available (run from a Windows shell)"
    msiexec //i "$file" //qn
}

install_portable() {
    local file="$1" target="$2"
    mkdir -p "$target"
    unzip -q -o "$file" -d "$target"
    log "extracted portable build into: ${target}"
}

# install_source APP DEST - fetch an app's repository tarball of its default
# branch and install it as a pinned source checkout under DEST/<app>.
install_source() {
    local app="$1" target="$2" branch tmpfile
    branch="$(default_branch "$app")" || branch=""
    [ -z "$branch" ] && branch="main"

    mkdir -p "$target"
    tmpfile="$(mktemp "${CACHE_DIR}/${app}.XXXXXX")"
    log "fetching ${branch} tarball for ${app}"
    fetch_to "https://github.com/${REPO_OWNER}/${app}/archive/refs/heads/${branch}.tar.gz" \
        "$tmpfile" || { rm -f "$tmpfile"; return 1; }
    tar -xzf "$tmpfile" -C "$target" --strip-components=1
    local rc=$?
    rm -f "$tmpfile"
    [ $rc -eq 0 ] || return 1
    log "installed source checkout: ${target}"
}

# ---------------------------------------------------------------------------
# Main loop
# ---------------------------------------------------------------------------

main() {
    log "host: ${OS}/${ARCH}   method: ${METHOD}   dest: ${DEST}"
    mkdir -p "$CACHE_DIR"

    local failures=0
    local app tag ver method asset url sums_url file sums

    for app in $APPS; do
        log "--- ${app} ---"

        # Source installs checkout the default branch, skipping release logic.
        if [ "$SOURCE" -eq 1 ] || [ "$app" = "craft-fonts" ]; then
            if [ "$app" = "craft-fonts" ]; then
                warn "craft-fonts publishes no releases and ships no .rpm; installing from source, not the package manager"
            fi
            if [ "$DRY_RUN" -eq 1 ]; then
                log "source:  ${REPO_OWNER}/${app} (default branch) -> ${DEST}/${app}"
                continue
            fi
            install_source "$app" "${DEST}/${app}" \
                || { warn "source install failed for ${app}"; failures=$((failures + 1)); continue; }
            log "installed ${app} (source)"
            continue
        fi

        # Resolve version.
        if [ -n "$VERSION" ]; then
            ver="$VERSION"
            tag="v${VERSION}"
        else
            tag="$(latest_tag "$app")" || tag=""
            if [ -z "$tag" ]; then
                warn "could not resolve latest release for ${app}; skipping"
                failures=$((failures + 1))
                continue
            fi
            ver="${tag#v}"
        fi

        method="$(pick_method)"
        asset="$(release_asset "$app" "$ver" "$method")"
        url="https://github.com/${REPO_OWNER}/${app}/releases/download/${tag}/${asset}"
        sums_url="https://github.com/${REPO_OWNER}/${app}/releases/download/${tag}/SHA256SUMS.txt"

        log "version: ${ver}   tag: ${tag}   method: ${method}"
        log "asset:   ${asset}"

        if [ "$DRY_RUN" -eq 1 ]; then
            log "url:     ${url}"
            continue
        fi

        if [ "$ASSUME_YES" -ne 1 ] && [ ! -t 0 ]; then
            log "non-interactive without --yes; installing ${app}"
        fi

        file="${CACHE_DIR}/${asset}"
        sums="${CACHE_DIR}/${app}-${ver}-SHA256SUMS.txt"

        if [ ! -f "$file" ]; then
            log "downloading ${url}"
            fetch_to "$url" "$file" || { warn "download failed for ${asset}"; failures=$((failures + 1)); continue; }
        fi

        fetch_to "$sums_url" "$sums" || warn "could not fetch SHA256SUMS.txt for ${app}"
        if [ -f "$sums" ]; then
            verify_sha "$file" "$sums" "$asset" || { failures=$((failures + 1)); continue; }
        fi

        case "$method" in
            rpm)      install_rpm "$file" ;;
            deb)      install_deb "$file" ;;
            flatpak)  install_flatpak "$file" ;;
            appimage) install_appimage "$file" "$DEST" "$app" ;;
            tar)      install_tar "$file" "$DEST/$app" ;;
            dmg)      install_dmg "$file" ;;
            msi)      install_msi "$file" ;;
            portable) install_portable "$file" "$DEST/$app" ;;
            download) install_tar "$file" "$DEST/$app" ;;
        esac || { warn "install failed for ${app}"; failures=$((failures + 1)); continue; }

        log "installed ${app} ${ver}"

        if [ "$KEEP" -ne 1 ]; then
            rm -f "$file"
        fi
    done

    if [ "$failures" -gt 0 ]; then
        die "${failures} app(s) failed"
    fi
    log "done"
}

main "$@"
