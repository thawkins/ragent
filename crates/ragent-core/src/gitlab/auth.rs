//! GitLab Personal Access Token and configuration persistence.
//!
//! Credentials are stored encrypted in the ragent SQLite database using the
//! same `provider_auth` / `settings` tables as LLM provider keys.
//!
//! Resolution priority (highest wins):
//! 1. Environment variables: `GITLAB_TOKEN`, `GITLAB_URL`, `GITLAB_USERNAME`
//! 2. `ragent.json` config file (`gitlab` section)
//! 3. Encrypted database via [`Storage`]

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::storage::Storage;

/// Database key for the GitLab Personal Access Token (stored encrypted in `provider_auth`).
const DB_PROVIDER_ID: &str = "gitlab";
/// Database settings key for the JSON-serialised GitLab config.
const DB_SETTING_KEY: &str = "gitlab_config";

/// Stored GitLab configuration (everything except the token).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitLabConfig {
    /// GitLab instance base URL, e.g. `https://gitlab.com`.
    pub instance_url: String,
    /// GitLab username / identity.
    pub username: String,
}

// ---------------------------------------------------------------------------
// Resolved credential loading (layered: env > ragent.json > database)
// ---------------------------------------------------------------------------

/// Resolve the GitLab PAT.
///
/// Priority: `GITLAB_TOKEN` env → `ragent.json` → encrypted database.
#[must_use]
pub fn load_token(storage: &Storage) -> Option<String> {
    // 1. Environment variable
    if let Ok(token) = std::env::var("GITLAB_TOKEN") {
        if !token.is_empty() {
            return Some(token);
        }
    }

    // 2. ragent.json
    if let Ok(cfg) = Config::load() {
        if let Some(ref t) = cfg.gitlab.token {
            if !t.is_empty() {
                return Some(t.clone());
            }
        }
    }

    // 3. Encrypted database
    storage.get_provider_auth(DB_PROVIDER_ID).ok().flatten()
}

/// Resolve the GitLab configuration (instance URL + username).
///
/// Priority: env vars → `ragent.json` → database settings.
#[must_use]
pub fn load_config(storage: &Storage) -> Option<GitLabConfig> {
    let env_url = std::env::var("GITLAB_URL").ok().filter(|s| !s.is_empty());
    let env_user = std::env::var("GITLAB_USERNAME").ok().filter(|s| !s.is_empty());

    // If both env vars are set, use them directly
    if let (Some(url), Some(user)) = (env_url.clone(), env_user.clone()) {
        return Some(GitLabConfig {
            instance_url: url,
            username: user,
        });
    }

    // Start with database values, then overlay ragent.json, then env vars
    let mut config = load_config_from_db(storage);

    // Overlay ragent.json values
    if let Ok(file_cfg) = Config::load() {
        if let Some(ref url) = file_cfg.gitlab.instance_url {
            if !url.is_empty() {
                let cfg = config.get_or_insert_with(|| GitLabConfig {
                    instance_url: String::new(),
                    username: String::new(),
                });
                cfg.instance_url = url.clone();
            }
        }
        if let Some(ref user) = file_cfg.gitlab.username {
            if !user.is_empty() {
                let cfg = config.get_or_insert_with(|| GitLabConfig {
                    instance_url: String::new(),
                    username: String::new(),
                });
                cfg.username = user.clone();
            }
        }
    }

    // Overlay env vars (highest priority)
    if let Some(url) = env_url {
        let cfg = config.get_or_insert_with(|| GitLabConfig {
            instance_url: String::new(),
            username: String::new(),
        });
        cfg.instance_url = url;
    }
    if let Some(user) = env_user {
        let cfg = config.get_or_insert_with(|| GitLabConfig {
            instance_url: String::new(),
            username: String::new(),
        });
        cfg.username = user;
    }

    // Only return if both fields are populated
    config.filter(|c| !c.instance_url.is_empty() && !c.username.is_empty())
}

// ---------------------------------------------------------------------------
// Database persistence (encrypted token, settings for config)
// ---------------------------------------------------------------------------

/// Save a GitLab PAT to the encrypted database.
pub fn save_token(storage: &Storage, token: &str) -> Result<()> {
    storage.set_provider_auth(DB_PROVIDER_ID, token)
}

/// Delete the stored GitLab token from the database.
pub fn delete_token(storage: &Storage) -> Result<()> {
    storage.delete_provider_auth(DB_PROVIDER_ID)
}

/// Save the GitLab configuration (instance URL + username) to the database.
pub fn save_config(storage: &Storage, config: &GitLabConfig) -> Result<()> {
    let json = serde_json::to_string(config)?;
    storage.set_setting(DB_SETTING_KEY, &json)
}

/// Delete the stored GitLab configuration from the database.
pub fn delete_config(storage: &Storage) -> Result<()> {
    storage.delete_setting(DB_SETTING_KEY)
}

/// Load GitLab configuration from the database only.
fn load_config_from_db(storage: &Storage) -> Option<GitLabConfig> {
    storage
        .get_setting(DB_SETTING_KEY)
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str(&json).ok())
}

// ---------------------------------------------------------------------------
// Token validation
// ---------------------------------------------------------------------------

/// Validate that a GitLab PAT can authenticate against the configured instance.
///
/// Calls `GET /api/v4/user` and returns the authenticated username on success.
pub async fn validate_token(instance_url: &str, token: &str) -> Result<String> {
    let url = format!("{}/api/v4/user", instance_url.trim_end_matches('/'));
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("PRIVATE-TOKEN", token)
        .header("User-Agent", "ragent/0.1")
        .send()
        .await
        .context("Failed to connect to GitLab instance")?;

    if resp.status().as_u16() == 401 {
        anyhow::bail!("Authentication failed — invalid Personal Access Token");
    }
    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("GitLab API error: {body}");
    }

    let body: serde_json::Value = resp.json().await?;
    let username = body["username"]
        .as_str()
        .context("Could not read username from GitLab response")?
        .to_string();
    Ok(username)
}

// ---------------------------------------------------------------------------
// Migration: file-based storage → database
// ---------------------------------------------------------------------------

/// Migrate legacy file-based GitLab credentials into the database.
///
/// If `~/.ragent/gitlab_token` or `~/.ragent/gitlab_config.json` exist and
/// the database has no corresponding entries, imports them and deletes the
/// old files.
pub fn migrate_legacy_files(storage: &Storage) {
    // Token migration
    if let Some(path) = legacy_token_file_path() {
        if path.exists() {
            if let Ok(token) = std::fs::read_to_string(&path) {
                let token = token.trim().to_string();
                if !token.is_empty()
                    && storage
                        .get_provider_auth(DB_PROVIDER_ID)
                        .ok()
                        .flatten()
                        .is_none()
                {
                    if storage
                        .set_provider_auth(DB_PROVIDER_ID, &token)
                        .is_ok()
                    {
                        let _ = std::fs::remove_file(&path);
                    }
                }
            }
        }
    }

    // Config migration
    if let Some(path) = legacy_config_file_path() {
        if path.exists() {
            if let Ok(data) = std::fs::read_to_string(&path) {
                if let Ok(config) = serde_json::from_str::<GitLabConfig>(&data) {
                    if load_config_from_db(storage).is_none()
                        && save_config(storage, &config).is_ok()
                    {
                        let _ = std::fs::remove_file(&path);
                    }
                }
            }
        }
    }
}

fn legacy_token_file_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| h.join(".ragent").join("gitlab_token"))
}

fn legacy_config_file_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| h.join(".ragent").join("gitlab_config.json"))
}
