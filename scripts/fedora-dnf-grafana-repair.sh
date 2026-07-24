#!/usr/bin/env bash
# Fedora DNF repair script for Grafana repository SSL certificate issues.
# Also handles slow/timing-out mirrors by refreshing metadata and cache.
# Run with: sudo bash scripts/fedora-dnf-grafana-repair.sh
set -euo pipefail

readonly REPO_FILE="/etc/yum.repos.d/grafana.repo"
readonly DNF_CONF="/etc/dnf/dnf.conf"
readonly BACKUP_DIR="/var/tmp/ragent-dnf-repair-$(date +%Y%m%d-%H%M%S)"
readonly LOG="${BACKUP_DIR}/repair.log"

mkdir -p "$BACKUP_DIR"
exec > >(tee -a "$LOG")
exec 2> >(tee -a "$LOG" >&2)

echo "=== Fedora DNF repair started: $(date) ==="
echo "Log: $LOG"
echo "Backup directory: $BACKUP_DIR"
echo ""

if [[ $EUID -ne 0 ]]; then
    echo "ERROR: This script must be run as root. Use: sudo bash $0"
    exit 1
fi

# ---- 1. Backup current repo configuration ----
echo "[1/7] Backing up current configuration..."
cp -a /etc/yum.repos.d "$BACKUP_DIR/yum.repos.d" || true
cp -a "$DNF_CONF" "$BACKUP_DIR/dnf.conf.bak" || true

# ---- 2. Update system CA certificates ----
echo "[2/7] Updating CA certificate store..."
update-ca-trust extract

# ---- 3. Clean DNF cache ----
echo "[3/7] Cleaning DNF cache..."
dnf clean all

# ---- 4. Refresh repository metadata ----
echo "[4/7] Refreshing DNF metadata..."
if dnf makecache --refresh; then
    echo "Metadata refresh succeeded."
else
    echo "WARN: Full metadata refresh failed; will try with Grafana disabled."
fi

# ---- 5. Inspect Grafana repo file ----
echo "[5/7] Inspecting Grafana repository configuration..."
if [[ -f "$REPO_FILE" ]]; then
    echo "Current $REPO_FILE contents:"
    cat "$REPO_FILE"
    echo ""
else
    echo "WARN: $REPO_FILE not found. Skipping Grafana-specific fixes."
fi

# ---- 6. Attempt a small test update (skip packages, just verify repo loading) ----
echo "[6/7] Testing DNF repository loading..."
if dnf repolist --all 2>&1 | tee -a "$LOG"; then
    echo "Repository list OK."
else
    echo "WARN: Repository list still failing."
fi

if dnf check-update --refresh 2>&1 | tee -a "$LOG"; then
    echo "Check-update completed (exit 0 means no updates, exit 100 means updates available)."
else
    rc=$?
    if [[ $rc -eq 100 ]]; then
        echo "Check-update: updates are available."
    else
        echo "ERROR: check-update failed with exit code $rc."
    fi
fi

# ---- 7. Offer remediation if Grafana is still broken ----
echo ""
echo "[7/7] Final remediation options"

if [[ -f "$REPO_FILE" ]]; then
    echo "If the Grafana repository is still failing, choose an action:"
    echo "  1) Temporarily disable SSL verification for Grafana only (NOT recommended for production)"
    echo "  2) Disable the Grafana repository entirely"
    echo "  3) Make no further changes"
    read -rp "Choice [1/2/3]: " choice || true

    case "$choice" in
        1)
            echo "Setting sslverify=0 in $REPO_FILE (temporary workaround)..."
            sed -i 's/^sslverify=.*/sslverify=0/' "$REPO_FILE"
            # Ensure every section has sslverify explicitly set
            if ! grep -q "^sslverify=" "$REPO_FILE"; then
                # Append to every repo section that doesn't already have it
                awk '
                    /^\[.*\]/ { repo=1 }
                    /^sslverify=/ { set=1 }
                    repo && /^\[/ && set==0 && prev_repo {
                        print "sslverify=0"
                    }
                    { print; prev_repo=repo; if (/^sslverify=/) { set=0 } }
                ' "$REPO_FILE" > "${REPO_FILE}.tmp" && mv "${REPO_FILE}.tmp" "$REPO_FILE"
            fi
            echo "Grafana SSL verification disabled. To revert, restore from $BACKUP_DIR"
            ;;
        2)
            echo "Disabling Grafana repository..."
            dnf config-manager --disable grafana || true
            ;;
        *)
            echo "No further changes made."
            ;;
    esac
fi

echo ""
echo "Applying conservative DNF timeout settings (if not already present)..."
if [[ -f "$DNF_CONF" ]]; then
    grep -q "^timeout=" "$DNF_CONF" || echo "timeout=120" >> "$DNF_CONF"
    grep -q "^minrate=" "$DNF_CONF" || echo "minrate=1000" >> "$DNF_CONF"
    grep -q "^retries=" "$DNF_CONF" || echo "retries=3" >> "$DNF_CONF"
else
    echo "WARN: $DNF_CONF not found. Skipping timeout settings."
fi

echo ""
echo "=== Repair finished: $(date) ==="
echo "Backups saved in: $BACKUP_DIR"
echo "To restore the original repo files, run:"
echo "  sudo cp -a $BACKUP_DIR/yum.repos.d/* /etc/yum.repos.d/"
echo ""
echo "Recommended next steps:"
echo "  1. sudo dnf update --refresh"
echo "  2. If Grafana remains broken, consider switching to the official package:"
echo "     https://grafana.com/docs/grafana/latest/setup-grafana/installation/fedora/"
