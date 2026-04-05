//! Auto-update support for ragent.
//!
//! Checks GitHub releases API for a newer version and optionally downloads
//! and replaces the running binary.

use anyhow::{Context, Result, bail};
use serde_json::Value;

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
    let download_url = json["assets"]
        .as_array()
        .and_then(|assets| {
            assets
                .iter()
                .find(|a| a["name"].as_str().is_some_and(|n| n.contains(&target)))
        })
        .and_then(|a| a["browser_download_url"].as_str())
        .map(std::string::ToString::to_string);

    Ok(ReleaseInfo {
        tag_name,
        version,
        body,
        download_url,
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
    // Split "0.1.0-alpha.21" → ("0.1.0", "alpha.21")
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
pub async fn download_and_replace(download_url: &str) -> Result<()> {
    let current_exe = std::env::current_exe().context("Cannot determine current binary path")?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .user_agent("ragent-updater/1.0")
        .build()?;

    let resp = client.get(download_url).send().await?;
    if !resp.status().is_success() {
        bail!("Download failed: HTTP {}", resp.status());
    }

    let bytes = resp.bytes().await?;

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
        .context("Failed to replace binary — may need elevated permissions")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_newer_same() {
        assert!(!is_newer("0.1.0-alpha.21", "0.1.0-alpha.21"));
    }

    #[test]
    fn test_is_newer_higher_prerelease() {
        assert!(is_newer("0.1.0-alpha.22", "0.1.0-alpha.21"));
    }

    #[test]
    fn test_is_newer_higher_patch() {
        assert!(is_newer("0.1.1", "0.1.0"));
    }

    #[test]
    fn test_is_newer_lower() {
        assert!(!is_newer("0.1.0-alpha.20", "0.1.0-alpha.21"));
    }

    #[test]
    fn test_is_newer_with_v_prefix() {
        assert!(is_newer("v0.2.0", "0.1.0"));
    }
}
