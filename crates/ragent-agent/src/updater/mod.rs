//! Auto-update support for ragent.
//!
//! Checks GitHub releases API for a newer version and optionally downloads
//! and replaces the running binary.

use anyhow::{Context, Result, bail};
use serde_json::Value;
use sha2::Digest;

/// The GitHub repo to check for releases.
const GITHUB_REPO: &str = "thawkins/ragent";

/// Current version from Cargo.toml (e.g. "0.1.0-alpha.21").
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Information about a GitHub release.
#[derive(Debug, Clone)]
pub struct ReleaseInfo {
    /// Tag name (e.g. "v0.1.0-alpha.22").
    pub tag_name: String,
    /// Human-readable version without the leading `v`.
    pub version: String,
    /// Release notes body.
    pub body: String,
    /// Download URL for the platform-appropriate binary (if found).
    pub download_url: Option<String>,
    /// SHA-256 of the selected asset, when the release metadata published one
    /// (GitHub release assets expose a `digest` field of the form
    /// `sha256:<hex>`). `None` when the release carries no digest.
    pub sha256: Option<String>,
}

/// Check GitHub releases for a version newer than the running binary.
/// Returns `None` if already up-to-date or check fails silently.
pub async fn check_for_update() -> Option<ReleaseInfo> {
    match fetch_latest_release().await {
        Ok(info) => {
            if is_newer(&info.version, CURRENT_VERSION) {
                Some(info)
            } else {
                None
            }
        }
        Err(e) => {
            tracing::debug!("Update check failed (non-fatal): {e}");
            None
        }
    }
}

async fn fetch_latest_release() -> Result<ReleaseInfo> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("ragent-updater/1.0")
        .build()?;

    let url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest");
    let resp = client
        .get(&url)
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await
        .context("Failed to reach GitHub API")?;

    if !resp.status().is_success() {
        bail!("GitHub API returned {}", resp.status());
    }

    let json: Value = resp.json().await?;
    let tag_name = json["tag_name"].as_str().unwrap_or("").to_string();
    let version = tag_name.trim_start_matches('v').to_string();
    let body = json["body"].as_str().unwrap_or("").to_string();

    // Find the appropriate binary asset for the current platform
    let target = current_platform_target();
    let asset = json["assets"].as_array().and_then(|assets| {
        assets
            .iter()
            .find(|a| a["name"].as_str().is_some_and(|n| n.contains(&target)))
    });
    let download_url = asset
        .and_then(|a| a["browser_download_url"].as_str())
        .map(std::string::ToString::to_string);
    // GitHub exposes the asset digest as `sha256:<hex>` (SEC-043). Normalise to
    // the bare hex digest; `None` when the release published none.
    let sha256 = asset
        .and_then(|a| a["digest"].as_str())
        .and_then(|d| d.strip_prefix("sha256:"))
        .map(|hex| hex.to_ascii_lowercase());

    Ok(ReleaseInfo {
        tag_name,
        version,
        body,
        download_url,
        sha256,
    })
}

/// Simple semver-ish comparison: is `latest` newer than `current`?
/// Handles alpha/beta suffixes conservatively.
#[must_use]
pub fn is_newer(latest: &str, current: &str) -> bool {
    // Strip leading 'v'
    let latest = latest.trim_start_matches('v');
    let current = current.trim_start_matches('v');
    latest != current && parse_version(latest) > parse_version(current)
}

fn parse_version(v: &str) -> (u64, u64, u64, String) {
    // Split "0.1.0-alpha.21" -> ("0.1.0", "alpha.21")
    let (base, pre) = v.split_once('-').unwrap_or((v, ""));
    let mut parts = base.splitn(3, '.');
    let major = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let minor = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let patch = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    (major, minor, patch, pre.to_string())
}

fn current_platform_target() -> String {
    // Match common CI artifact naming conventions
    if cfg!(target_os = "linux") && cfg!(target_arch = "x86_64") {
        "x86_64-unknown-linux-musl".to_string()
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        "aarch64-apple-darwin".to_string()
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "x86_64") {
        "x86_64-apple-darwin".to_string()
    } else if cfg!(target_os = "windows") {
        "x86_64-pc-windows-msvc".to_string()
    } else {
        std::env::consts::ARCH.to_string()
    }
}

/// Download and replace the current binary with the latest release.
///
/// # Integrity (SEC-043)
///
/// The downloaded bytes must be verified against a SHA-256 digest before
/// anything is written or replaced. This entry point resolves that digest from
/// the `RAGENT_UPDATE_SHA256` environment variable (see
/// [`download_and_replace_verified`] to pass one explicitly, e.g. the
/// [`ReleaseInfo::sha256`] the release metadata published). If no digest is
/// available the download is **refused**: replacing the running binary with an
/// unverified artifact fetched over the network is not permitted.
pub async fn download_and_replace(download_url: &str) -> Result<()> {
    download_and_replace_verified(download_url, None).await
}

/// As [`download_and_replace`], but with the expected SHA-256 digest passed in.
///
/// `expected_sha256` takes precedence over `RAGENT_UPDATE_SHA256`; when neither
/// is available the download is refused (SEC-043). The digest may be a bare hex
/// string or carry a `sha256:` prefix, and the comparison is case-insensitive.
pub async fn download_and_replace_verified(
    download_url: &str,
    expected_sha256: Option<&str>,
) -> Result<()> {
    let expected = resolve_expected_sha256(expected_sha256)?;

    let current_exe = std::env::current_exe().context("Cannot determine current binary path")?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_mins(5))
        .user_agent("ragent-updater/1.0")
        .build()?;

    let resp = client.get(download_url).send().await?;
    if !resp.status().is_success() {
        bail!("Download failed: HTTP {}", resp.status());
    }

    let bytes = resp.bytes().await?;

    // Verify the artifact before touching the filesystem (SEC-043).
    let actual = hex::encode(sha2::Sha256::digest(&bytes));
    if actual != expected {
        bail!(
            "Downloaded update failed SHA-256 verification (expected {expected}, got {actual}); \
             refusing to replace the binary"
        );
    }
    tracing::info!(
        download_url = %crate::sanitize::redact_secrets(download_url),
        sha256 = %actual,
        "Update artifact verified; replacing binary"
    );

    // Write to a temp file alongside the current binary
    let tmp_path = current_exe.with_extension("tmp_update");
    std::fs::write(&tmp_path, &bytes)?;

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o755))?;
    }

    // Atomically replace current binary
    std::fs::rename(&tmp_path, &current_exe)
        .context("Failed to replace binary - may need elevated permissions")?;

    Ok(())
}

/// Resolve the SHA-256 digest an update download must match.
///
/// Prefers the caller-supplied `expected_sha256` (e.g. the release asset's
/// published `digest`), falling back to the `RAGENT_UPDATE_SHA256` environment
/// variable. Returns an error - refusing the update - when neither is present
/// or the supplied value is not valid hex (SEC-043). A `sha256:` prefix is
/// stripped and the result lowercased for a case-insensitive comparison.
fn resolve_expected_sha256(expected: Option<&str>) -> Result<String> {
    let raw = expected
        .map(str::to_string)
        .or_else(|| std::env::var("RAGENT_UPDATE_SHA256").ok())
        .filter(|s| !s.trim().is_empty())
        .context(
            "no expected SHA-256 digest available for the update; refusing to replace the binary \
             (pass one from the release metadata or set RAGENT_UPDATE_SHA256)",
        )?;

    let digest = raw
        .trim()
        .strip_prefix("sha256:")
        .unwrap_or(raw.trim())
        .to_ascii_lowercase();

    if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("expected SHA-256 digest '{digest}' is not a valid 64-character hex digest");
    }
    Ok(digest)
}
