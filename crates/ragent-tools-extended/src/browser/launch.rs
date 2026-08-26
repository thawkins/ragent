//! Chrome/Chromium process launcher for the `browser` `setup` action.
//!
//! Launches a headless Chrome or Chromium instance with the `--remote-debugging-port`
//! flag so the CDP client can connect. The launcher searches for an installed
//! browser binary in a platform-specific order.
//!
//! # Platform search order
//!
//! - **Linux**: `google-chrome`, `google-chrome-stable`, `chromium`,
//!   `chromium-browser`, `headless-shell`
//! - **macOS**: `/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`,
//!   `/Applications/Chromium.app/Contents/MacOS/Chromium`
//! - **Windows**: `C:\Program Files\Google\Chrome\Application\chrome.exe`,
//!   `C:\Program Files (x86)\Google\Chrome\Application\chrome.exe`

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use tokio::process::Command;
use tracing::debug;

/// Default remote debugging port.
pub const DEFAULT_DEBUG_PORT: u16 = 9222;

/// Default timeout for waiting for the browser to start (10 seconds).
pub const DEFAULT_STARTUP_TIMEOUT_SECS: u64 = 10;

/// Find an installed Chrome or Chromium binary.
///
/// Searches platform-specific paths and returns the first match.
///
/// # Errors
///
/// Returns an error if no browser binary is found.
pub fn find_browser_binary() -> Result<PathBuf> {
    let candidates = browser_binary_candidates();

    for candidate in &candidates {
        if candidate.exists() {
            debug!(path = %candidate.display(), "found browser binary");
            return Ok(candidate.clone());
        }
    }

    // Also try `which` for PATH-resident binaries on Unix.
    #[cfg(unix)]
    {
        for name in &[
            "google-chrome",
            "google-chrome-stable",
            "chromium",
            "chromium-browser",
        ] {
            if let Ok(path) = which_binary(name) {
                debug!(path = %path.display(), "found browser binary via which: {name}");
                return Ok(path);
            }
        }
    }

    let searched = candidates
        .iter()
        .map(|c| c.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    bail!(
        "No Chrome or Chromium binary found. Searched: {searched}. \
         Install Chrome or Chromium, or start it manually with \
         --remote-debugging-port={DEFAULT_DEBUG_PORT}."
    );
}

/// Get platform-specific browser binary candidate paths.
fn browser_binary_candidates() -> Vec<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        vec![
            PathBuf::from("/usr/bin/google-chrome"),
            PathBuf::from("/usr/bin/google-chrome-stable"),
            PathBuf::from("/usr/bin/chromium"),
            PathBuf::from("/usr/bin/chromium-browser"),
            PathBuf::from("/usr/local/bin/chromium"),
            PathBuf::from("/snap/bin/chromium"),
        ]
    }

    #[cfg(target_os = "macos")]
    {
        vec![
            PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
            PathBuf::from("/Applications/Chromium.app/Contents/MacOS/Chromium"),
            PathBuf::from(
                "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
            ),
        ]
    }

    #[cfg(target_os = "windows")]
    {
        vec![
            PathBuf::from("C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe"),
            PathBuf::from("C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe"),
            PathBuf::from("C:\\Program Files\\Chromium\\Application\\chromium.exe"),
        ]
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        vec![]
    }
}

/// Search for a binary on PATH using `which` (Unix only).
#[cfg(unix)]
fn which_binary(name: &str) -> Result<PathBuf> {
    let output = std::process::Command::new("which")
        .arg(name)
        .output()
        .context("failed to run 'which'")?;
    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout);
        let path = path.trim();
        if !path.is_empty() {
            return Ok(PathBuf::from(path));
        }
    }
    bail!("'{name}' not found on PATH");
}

/// Launch a headless Chrome/Chromium instance with CDP enabled.
///
/// # Arguments
///
/// * `binary` — path to the Chrome/Chromium binary.
/// * `port` — remote debugging port (default 9222).
/// * `headless` — if `true`, run in headless mode.
/// * `user_data_dir` — optional user data directory (temp dir if not supplied).
///
/// # Errors
///
/// Returns an error if the browser cannot be started.
pub async fn launch_browser(
    binary: &std::path::Path,
    port: u16,
    headless: bool,
    user_data_dir: Option<&std::path::Path>,
) -> Result<tokio::process::Child> {
    let user_data_dir = if let Some(dir) = user_data_dir {
        dir.to_path_buf()
    } else {
        std::env::temp_dir().join(format!("ragent-browser-{port}"))
    };

    // Ensure the user data dir exists.
    std::fs::create_dir_all(&user_data_dir).with_context(|| {
        format!(
            "failed to create user data dir: {}",
            user_data_dir.display()
        )
    })?;

    let mut cmd = Command::new(binary);
    cmd.arg(format!("--remote-debugging-port={port}"))
        .arg(format!("--user-data-dir={}", user_data_dir.display()))
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--disable-extensions")
        .arg("--disable-popup-blocking")
        .arg("--disable-translate")
        .arg("--disable-background-networking")
        .arg("--disable-sync")
        .arg("--metrics-recording-only")
        .arg("--disable-default-apps");

    if headless {
        cmd.arg("--headless=new");
    }

    cmd.stdout(Stdio::null())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());

    debug!(binary = %binary.display(), port, headless, "launching browser");
    let child = cmd
        .spawn()
        .with_context(|| format!("failed to launch browser: {}", binary.display()))?;

    Ok(child)
}

/// Wait for the CDP endpoint to become available by polling `/json/version`.
///
/// # Arguments
///
/// * `http_endpoint` — the HTTP base URL (e.g. `"http://127.0.0.1:9222"`).
/// * `timeout_secs` — maximum time to wait.
///
/// # Errors
///
/// Returns an error if the endpoint doesn't become available within the
/// timeout.
pub async fn wait_for_endpoint(http_endpoint: &str, timeout_secs: u64) -> Result<()> {
    let deadline = std::time::Instant::now() + Duration::from_secs(timeout_secs);

    loop {
        if std::time::Instant::now() > deadline {
            bail!(
                "browser CDP endpoint did not become available within {timeout_secs}s at {http_endpoint}"
            );
        }

        match super::cdp::discover_version(http_endpoint).await {
            Ok(_) => {
                debug!(http_endpoint, "CDP endpoint is available");
                return Ok(());
            }
            Err(_) => {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
    }
}

/// Execute the `setup` action: find and launch a browser, wait for CDP.
///
/// Returns a JSON summary of the setup result.
///
/// # Errors
///
/// Returns an error if no browser is found, the launch fails, or the CDP
/// endpoint doesn't become available.
pub async fn action_setup(port: Option<u16>, headless: bool) -> Result<Value> {
    let port = port.unwrap_or(DEFAULT_DEBUG_PORT);
    let http_endpoint = format!("http://127.0.0.1:{port}");

    // Check if a browser is already running on this port.
    if super::cdp::discover_version(&http_endpoint).await.is_ok() {
        return Ok(json!({
            "action": "setup",
            "status": "already_running",
            "http_endpoint": http_endpoint,
            "port": port,
        }));
    }

    // Find a browser binary.
    let binary = find_browser_binary()?;

    // Launch the browser.
    let mut child = launch_browser(&binary, port, headless, None).await?;

    // Wait for the CDP endpoint.
    if let Err(e) = wait_for_endpoint(&http_endpoint, DEFAULT_STARTUP_TIMEOUT_SECS).await {
        // Kill the child process if the endpoint didn't come up.
        let _ = child.kill().await;
        bail!("browser launched but CDP endpoint not ready: {e}");
    }

    // Get version info.
    let version = super::cdp::discover_version(&http_endpoint).await?;

    // R-16: The browser process runs independently. Tokio's default
    // `kill_on_drop = false` means dropping the `Child` handle does NOT
    // kill the process — the browser keeps running. We read the PID for
    // the response payload, then let the `Child` handle drop naturally.
    // Tokio's internal process reaper prevents zombie accumulation.
    let pid = child.id().map(|p| json!(p)).unwrap_or(Value::Null);
    // Intentionally do not call `child.kill()` — the browser should persist.

    Ok(json!({
        "action": "setup",
        "status": "launched",
        "binary": binary.display().to_string(),
        "browser": version.browser,
        "http_endpoint": http_endpoint,
        "port": port,
        "headless": headless,
        "pid": pid,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_browser_binary_candidates_nonempty() {
        let candidates = browser_binary_candidates();
        // On any platform, we should have at least one candidate path.
        // On unknown platforms, the list may be empty — skip the assertion.
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        assert!(
            !candidates.is_empty(),
            "should have platform-specific browser candidates"
        );
    }

    #[test]
    fn test_default_debug_port() {
        assert_eq!(DEFAULT_DEBUG_PORT, 9222);
    }

    #[cfg(unix)]
    #[test]
    fn test_which_binary_nonexistent() {
        let result = which_binary("this-binary-definitely-does-not-exist-12345");
        assert!(result.is_err());
    }
}
