//! GitLab Personal Access Token and configuration persistence.
//!
//! Credentials are stored encrypted in the ragent `SQLite` database using the
//! same `provider_auth` / `settings` tables as LLM provider keys.
//!
//! Resolution priority (highest wins):
//! 1. Environment variables: `GITLAB_TOKEN`, `GITLAB_URL`, `GITLAB_USERNAME`
//! 2. `ragent.json` config file (`gitlab` section)
//! 3. Encrypted database via [`Storage`]

use anyhow::{Context, Result};
use ragent_config::user_dirs::legacy_home_dir;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

use crate::storage::Storage;
use ragent_config::Config;

/// Database key for the GitLab Personal Access Token (stored encrypted in `provider_auth`).
const DB_PROVIDER_ID: &str = "gitlab";
/// Database settings key for the JSON-serialised GitLab config.
const DB_SETTING_KEY: &str = "gitlab_config";

/// PERF-059: last resolved GitLab token, keyed by the storage handle identity.
///
/// The token is encrypted at rest and decrypted on read, so resolving it on
/// every request costs a database round-trip plus a decryption. Caching it per
/// storage handle means the read happens once per process (until a save or
/// delete invalidates the entry). The key is the storage trait object's data
/// address, which is stable for the lifetime of the concrete store.
static TOKEN_CACHE: Mutex<Option<(usize, String)>> = Mutex::new(None);

/// Cache key standing for "the token is present but the store identity is not
/// tracked". Used when the caller passes a reference to a store we cannot
/// derive a stable identity for; caching under a constant key still collapses
/// the repeated decrypts within one flow.
const UNKNOWN_STORE_KEY: usize = usize::MAX;

/// Compute a stable identity key for a storage handle.
///
/// Uses only the data pointer of the trait object (never a dereference), so no
/// `unsafe` is required and the address is valid to compare while the caller
/// holds the same handle.
fn storage_cache_key(storage: &Storage) -> usize {
    let ptr: *const dyn crate::storage::StorageBackend = storage;
    let key = ptr.cast::<()>() as usize;
    if key == 0 { UNKNOWN_STORE_KEY } else { key }
}

/// Drop the cached token (called on save/delete so a changed credential is
/// re-read immediately).
fn invalidate_token_cache() {
    *TOKEN_CACHE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
}

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

/// Resolve the GitLab PAT, best-effort.
///
/// Priority: `GITLAB_TOKEN` env → `ragent.json` → encrypted database.
/// The database lookup is cached per storage handle (PERF-059) so repeat calls
/// do not re-decrypt the credential; [`save_token`] and [`delete_token`] clear
/// the cache.
///
/// This wrapper is for display/status paths: a credential-store read failure is
/// logged and reported as "no token". Callers that must distinguish a failed
/// read from genuinely-unconfigured credentials should use
/// [`load_token_checked`] (FUNC-011).
#[must_use]
pub fn load_token(storage: &Storage) -> Option<String> {
    match load_token_checked(storage) {
        Ok(token) => token,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "failed to read GitLab token from credential store; reporting as unconfigured"
            );
            None
        }
    }
}

/// Resolve the GitLab PAT, propagating credential-store read failures.
///
/// Priority: `GITLAB_TOKEN` env → `ragent.json` → encrypted database.
///
/// Returns `Ok(None)` only when no layer supplies a token. A database read
/// *error* is returned as `Err` so callers never mistake a broken store for
/// "not configured" (FUNC-011).
///
/// # Errors
///
/// Returns the underlying storage error when the credential store cannot be
/// read.
pub fn load_token_checked(storage: &Storage) -> Result<Option<String>> {
    // 1. Environment variable
    if let Ok(token) = std::env::var("GITLAB_TOKEN")
        && !token.is_empty()
    {
        return Ok(Some(token));
    }

    // 2. ragent.json (already parsed/cached by `Config::load`)
    if let Ok(cfg) = Config::load()
        && let Some(ref t) = cfg.gitlab.token
        && !t.is_empty()
    {
        return Ok(Some(t.clone()));
    }

    // 3. Encrypted database, cached by storage handle identity
    let key = storage_cache_key(storage);
    {
        let cache = TOKEN_CACHE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some((cached_key, token)) = cache.as_ref()
            && *cached_key == key
        {
            return Ok(Some(token.clone()));
        }
    }

    let token = storage.get_provider_auth(DB_PROVIDER_ID)?;
    if let Some(ref value) = token {
        *TOKEN_CACHE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some((key, value.clone()));
    }
    Ok(token)
}

/// Resolve the GitLab configuration (instance URL + username).
///
/// Priority: env vars → `ragent.json` → database settings.
#[must_use]
pub fn load_config(storage: &Storage) -> Option<GitLabConfig> {
    let env_url = std::env::var("GITLAB_URL").ok().filter(|s| !s.is_empty());
    let env_user = std::env::var("GITLAB_USERNAME")
        .ok()
        .filter(|s| !s.is_empty());

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
        if let Some(ref url) = file_cfg.gitlab.instance_url
            && !url.is_empty()
        {
            let cfg = config.get_or_insert_with(|| GitLabConfig {
                instance_url: String::new(),
                username: String::new(),
            });
            cfg.instance_url = url.clone();
        }
        if let Some(ref user) = file_cfg.gitlab.username
            && !user.is_empty()
        {
            let cfg = config.get_or_insert_with(|| GitLabConfig {
                instance_url: String::new(),
                username: String::new(),
            });
            cfg.username = user.clone();
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
    storage.set_provider_auth(DB_PROVIDER_ID, token)?;
    invalidate_token_cache();
    Ok(())
}

/// Delete the stored GitLab token from the database.
pub fn delete_token(storage: &Storage) -> Result<()> {
    storage.delete_provider_auth(DB_PROVIDER_ID)?;
    invalidate_token_cache();
    Ok(())
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
    let client = crate::http_client::shared_client();
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
    if let Some(path) = legacy_token_file_path()
        && path.exists()
        && let Ok(token) = std::fs::read_to_string(&path)
    {
        let token = token.trim().to_string();
        if !token.is_empty()
            && storage
                .get_provider_auth(DB_PROVIDER_ID)
                .ok()
                .flatten()
                .is_none()
            && storage.set_provider_auth(DB_PROVIDER_ID, &token).is_ok()
        {
            let _ = std::fs::remove_file(&path);
        }
    }

    // Config migration
    if let Some(path) = legacy_config_file_path()
        && path.exists()
        && let Ok(data) = std::fs::read_to_string(&path)
        && let Ok(config) = serde_json::from_str::<GitLabConfig>(&data)
        && load_config_from_db(storage).is_none()
        && save_config(storage, &config).is_ok()
    {
        let _ = std::fs::remove_file(&path);
    }
}

/// Legacy `~/.ragent/` GitLab token path — the only location the migration
/// scans. New code must not read or write here.
fn legacy_token_file_path() -> Option<std::path::PathBuf> {
    legacy_home_dir().map(|h| h.join("gitlab_token"))
}

fn legacy_config_file_path() -> Option<std::path::PathBuf> {
    // Legacy GitLab config path is intentionally untouched here; the
    // migration below reads and then deletes it.
    legacy_home_dir().map(|h| h.join("gitlab_config.json"))
}
