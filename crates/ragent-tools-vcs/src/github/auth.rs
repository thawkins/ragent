//! GitHub OAuth device flow and token storage.

use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use ragent_config::user_dirs::global_github_token_path;

/// PERF-059: last token read from disk, keyed by the file's modification time
/// so a changed token file is re-read but an unchanged one is served from
/// memory. Keying on mtime (rather than a bare process-global cache) keeps the
/// cache correct when `HOME` changes between calls, as it does in tests.
static TOKEN_FILE_CACHE: Mutex<Option<(PathBuf, SystemTime, Option<String>)>> = Mutex::new(None);

/// Read the stored GitHub token file, caching the value against the file's
/// modification time.
fn read_cached_token(path: &Path) -> Option<String> {
    let mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();

    let mut cache = TOKEN_FILE_CACHE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let (Some(mtime), Some((cached_path, cached_mtime, value))) = (mtime, cache.as_ref())
        && cached_path == path
        && *cached_mtime == mtime
    {
        return value.clone();
    }

    let value = match std::fs::read_to_string(path) {
        Ok(raw) => {
            let trimmed = raw.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        }
        Err(_) => None,
    };

    *cache = mtime.map(|mtime| (path.to_path_buf(), mtime, value.clone()));
    value
}

/// Resolve GitHub token from the environment, the stored file, or the `gh`
/// CLI. Returns `None` if no token is configured.
///
/// Priority (see `ragent_config::github`): the `GITHUB_TOKEN` environment
/// variable wins outright; otherwise the cached stored-file token is used
/// unless it is a GitHub App token (`ghu_`/`ghs_`) — those cannot create
/// repositories, so the `gh auth token` credential is preferred when the CLI
/// is authenticated, falling back to the stored token when it is not.
#[must_use]
pub fn load_token() -> Option<String> {
    if let Some(token) = ragent_config::github::env_token() {
        return Some(token);
    }
    // Stored file (cached against mtime), downgraded to the `gh` CLI
    // credential when the stored token is a repository-admin-incapable app
    // token. The `gh` subprocess only runs when the stored token is unusable.
    let stored = token_file_path().and_then(|path| read_cached_token(&path));
    ragent_config::github::resolve_from_stored(stored)
}

/// Save a GitHub token to `~/.ragent/github_token`.
///
/// The file is created with mode `0o600` *at creation time* (unix) so it is
/// never momentarily world- or group-readable, closing the write-then-chmod
/// race window. The write is atomic: a temporary file in the same directory is
/// written, permissioned, and renamed over the destination, so a reader never
/// observes a partial token (FUNC-012).
pub fn save_token(token: &str) -> Result<()> {
    let path = token_file_path().context("Cannot determine home directory")?;
    let dir = path
        .parent()
        .context("token file path has no parent directory")?;
    std::fs::create_dir_all(dir)?;

    let tmp_path = dir.join(format!(".github_token.{}.tmp", std::process::id()));
    write_private_file(&tmp_path, token)?;
    std::fs::rename(&tmp_path, &path).inspect_err(|_| {
        // Best-effort cleanup so a failed rename does not leave the temp file.
        let _ = std::fs::remove_file(&tmp_path);
    })?;

    // Invalidate the mtime-keyed read cache so the new token is visible even
    // if the write lands inside the filesystem's mtime granularity.
    *TOKEN_FILE_CACHE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    Ok(())
}

/// Write `contents` to `path`, creating it owner-only (`0o600`) on unix.
///
/// On unix the mode is applied by the `open(2)` call itself, so the file is
/// never observable with looser permissions.
fn write_private_file(path: &Path, contents: &str) -> Result<()> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .with_context(|| format!("cannot create token file {}", path.display()))?;
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, contents)?;
    }
    Ok(())
}

/// Delete the stored GitHub token.
pub fn delete_token() -> Result<()> {
    if let Some(path) = token_file_path()
        && path.exists()
    {
        std::fs::remove_file(&path)?;
    }
    *TOKEN_FILE_CACHE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    Ok(())
}

fn token_file_path() -> Option<PathBuf> {
    global_github_token_path()
}

/// State returned from initiating a GitHub device flow.
#[derive(Debug, Clone)]
pub struct DeviceFlowState {
    /// Opaque code sent to the polling endpoint.
    pub device_code: String,
    /// Short code the user must enter at `verification_uri`.
    pub user_code: String,
    /// URL the user must visit to authorize the device.
    pub verification_uri: String,
    /// Seconds until the device code expires.
    pub expires_in: u64,
    /// Minimum polling interval in seconds.
    pub interval: u64,
}

/// Initiate GitHub OAuth device flow.
pub async fn start_device_flow(client_id: &str) -> Result<DeviceFlowState> {
    let client = crate::http_client::shared_client();
    let resp = client
        .post("https://github.com/login/device/code")
        .header("Accept", "application/json")
        .form(&[("client_id", client_id), ("scope", "repo,read:user")])
        .send()
        .await
        .context("Failed to initiate device flow")?;

    if !resp.status().is_success() {
        bail!("GitHub device flow failed: HTTP {}", resp.status());
    }

    let body: serde_json::Value = resp.json().await?;
    Ok(DeviceFlowState {
        device_code: body["device_code"]
            .as_str()
            .context("missing device_code")?
            .to_string(),
        user_code: body["user_code"]
            .as_str()
            .context("missing user_code")?
            .to_string(),
        verification_uri: body["verification_uri"]
            .as_str()
            .context("missing verification_uri")?
            .to_string(),
        expires_in: body["expires_in"].as_u64().unwrap_or(900),
        interval: body["interval"].as_u64().unwrap_or(5),
    })
}

/// Poll for OAuth token after user has authorized.
/// Returns `Ok(Some(token))` when authorized, `Ok(None)` to keep polling.
pub async fn poll_device_flow(client_id: &str, state: &DeviceFlowState) -> Result<Option<String>> {
    let client = crate::http_client::shared_client();
    let resp = client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&[
            ("client_id", client_id),
            ("device_code", &state.device_code),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])
        .send()
        .await
        .context("Failed to poll device flow")?;

    let body: serde_json::Value = resp.json().await?;

    if let Some(token) = body["access_token"].as_str() {
        return Ok(Some(token.to_string()));
    }

    let error = body["error"].as_str().unwrap_or("");
    match error {
        "authorization_pending" => Ok(None),
        "slow_down" => Ok(None),
        "expired_token" => bail!("Device flow token expired — please try /github login again"),
        "access_denied" => bail!("GitHub authorization was denied"),
        other => bail!("Device flow error: {other}"),
    }
}

/// Full device flow: initiate + poll until token received or timeout.
///
/// The `progress_cb` is called once with the `user_code` and `verification_uri`
/// so the caller can display them to the user.
pub async fn device_flow_login<F>(client_id: &str, progress_cb: F) -> Result<String>
where
    F: Fn(&str, &str),
{
    let state = start_device_flow(client_id).await?;
    progress_cb(&state.user_code, &state.verification_uri);

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(state.expires_in);
    let mut interval = std::time::Duration::from_secs(state.interval);

    loop {
        if std::time::Instant::now() > deadline {
            bail!("Device flow timed out — please try /github login again");
        }
        tokio::time::sleep(interval).await;

        match poll_device_flow(client_id, &state).await {
            Ok(Some(token)) => return Ok(token),
            Ok(None) => {}
            Err(e) if e.to_string().contains("slow_down") => {
                interval += std::time::Duration::from_secs(5);
            }
            Err(e) => return Err(e),
        }
    }
}
