#!/bin/bash
# Build a flat .pkg installer for ragent on Apple Silicon (arm64) macOS.
#
# Usage:
#   ./scripts/macos-pkg.sh <version> <binary_path> <output_dir>
#
# Arguments:
#   version      - release version (e.g. 1.0.6)
#   binary_path  - path to the aarch64-apple-darwin release binary
#   output_dir   - directory where the .pkg will be written
#
# The produced installer installs the binary to /usr/local/lib/ragent/ragent
# and creates a symlink at /usr/local/bin/ragent.

set -euo pipefail

VERSION="${1:?usage: $0 <version> <binary_path> <output_dir>}"
BINARY_PATH="${2:?usage: $0 <version> <binary_path> <output_dir>}"
OUTPUT_DIR="${3:?usage: $0 <version> <binary_path> <output_dir>}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
PKG_ROOT="$(mktemp -d)"
trap 'rm -rf "${PKG_ROOT}"' EXIT

# Destination inside the package payload.
mkdir -p "${PKG_ROOT}/usr/local/lib/ragent"
cp "${BINARY_PATH}" "${PKG_ROOT}/usr/local/lib/ragent/ragent"
chmod 755 "${PKG_ROOT}/usr/local/lib/ragent/ragent"

# Verify we are actually packaging an arm64 binary.
ARCH="$(file "${PKG_ROOT}/usr/local/lib/ragent/ragent" | grep -o 'arm64' || true)"
if [ -z "${ARCH}" ]; then
    echo "ERROR: binary is not arm64: ${BINARY_PATH}" >&2
    exit 1
fi

mkdir -p "${OUTPUT_DIR}"
OUTPUT_PKG="${OUTPUT_DIR}/ragent-${VERSION}-macos-arm64.pkg"

pkgbuild \
    --root "${PKG_ROOT}" \
    --identifier "com.timhawkins.ragent" \
    --version "${VERSION}" \
    --install-location "/" \
    --scripts "${ROOT_DIR}/packaging/macos/scripts" \
    --filter '^.git.*' \
    "${OUTPUT_PKG}"

# Print the package info for verification.
ls -lh "${OUTPUT_PKG}"
pkgutil --check-signature "${OUTPUT_PKG}" || true
echo "Created ${OUTPUT_PKG}"
