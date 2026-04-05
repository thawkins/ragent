//! GitHub OAuth device flow and token storage.

use anyhow::{Context, Result, bail};
use std::path::PathBuf;

/// Resolve GitHub token from environment or stored file.
/// Returns `None` if no token is configured.
#[must_use]
pub fn load_token() -> Option<String> {
    // 1. Environment variable
    if let Ok(token) = std::env::var("GITHUB_TOKEN")
        && !token.is_empty()
    {
        return Some(token);
    }
    // 2. Stored file
    if let Some(path) = token_file_path()
        && let Ok(token) = std::fs::read_to_string(&path)
    {
        let token = token.trim().to_string();
        if !token.is_empty() {
            return Some(token);
        }
    }
    None
}

/// Save a GitHub token to `~/.ragent/github_token`.
pub fn save_token(token: &str) -> Result<()> {
    let path = token_file_path().context("Cannot determine home directory")?;
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(&path, token)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
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
    Ok(())
}

fn token_file_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".ragent").join("github_token"))
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
    let client = reqwest::Client::new();
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
    let client = reqwest::Client::new();
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
