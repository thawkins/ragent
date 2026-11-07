//! Layered configuration (FR-004).
//!
//! Configuration resolves from three layers in increasing precedence: the
//! vault config file, environment variables, then command-line flags. The
//! resolved [`Config`] exposes the vault root, provider selection, provider API
//! keys, request timeout, maximum fetch body size, and the active research
//! profile.

use std::path::PathBuf;
use std::time::Duration;

use crate::core::error::{Error, Result};

/// Environment variable naming the vault root.
pub const ENV_VAULT_ROOT: &str = "HYPERRESEARCH_VAULT";

/// Environment variable holding a comma-separated provider chain.
pub const ENV_PROVIDERS: &str = "HYPERRESEARCH_PROVIDERS";

/// Environment variable overriding the request timeout, in seconds.
pub const ENV_TIMEOUT_SECS: &str = "HYPERRESEARCH_TIMEOUT_SECS";

/// Environment variable overriding the maximum fetch body size, in bytes.
pub const ENV_MAX_BODY_BYTES: &str = "HYPERRESEARCH_MAX_BODY_BYTES";

/// Environment variable selecting the research profile.
pub const ENV_PROFILE: &str = "HYPERRESEARCH_PROFILE";

/// Default request timeout.
pub const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Default maximum fetch body size: 8 MiB.
pub const DEFAULT_MAX_BODY_BYTES: u64 = 8 * 1024 * 1024;

/// Fully resolved configuration.
///
/// Note there is no derived `Debug`: `provider_keys` holds plaintext provider
/// API keys, and a derived impl printed them verbatim on any `{:?}`. The
/// hand-written impl below routes the rendering through
/// [`ragent_config::config::render_redacted`], the same JSON-then-redact
/// renderer `ragent_config::Config` uses, so a newly-added secret field is
/// masked automatically.
#[derive(Clone, PartialEq, Eq, serde::Serialize)]
pub struct Config {
    /// Root directory of the research vault.
    pub vault_root: PathBuf,
    /// Enabled provider chain, in fallback order (FR-010, FR-011).
    pub providers: Vec<String>,
    /// API keys keyed by provider name.
    pub provider_keys: Vec<(String, String)>,
    /// Per-request timeout (FR-009).
    pub timeout: Duration,
    /// Maximum accepted response body size in bytes (FR-009).
    pub max_body_bytes: u64,
    /// Active research profile name (FR-031).
    pub profile: String,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&ragent_config::config::render_redacted(self))
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            vault_root: PathBuf::from("research"),
            providers: vec!["builtin".to_string()],
            provider_keys: Vec::new(),
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
            profile: "default".to_string(),
        }
    }
}

impl Config {
    /// Resolve configuration from all layers, with `overrides` applied last.
    ///
    /// `file_layers` are supplied by the caller (the vault config file is read
    /// by the caller so this function stays free of I/O and testable).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] when an environment variable or override is
    /// present but unparseable; a bad value is reported, never silently
    /// ignored.
    pub fn resolve(
        file_layers: impl IntoIterator<Item = (String, String)>,
        overrides: impl IntoIterator<Item = (String, String)>,
    ) -> Result<Self> {
        let mut config = Self::default();

        for (key, value) in file_layers {
            apply_layer(&mut config, &key, &value)?;
        }
        for (key, value) in env_layer() {
            apply_layer(&mut config, &key, &value)?;
        }
        for (key, value) in overrides {
            apply_layer(&mut config, &key, &value)?;
        }

        Ok(config)
    }

    /// Read the vault configuration file as `key = value` layers.
    ///
    /// A missing file is not an error: the file layer is simply empty, which is
    /// what makes an out-of-the-box vault work.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] when the file exists but cannot be read.
    pub fn read_file_layer(path: &std::path::Path) -> Result<Vec<(String, String)>> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => return Err(Error::io(path, err)),
        };

        Ok(text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .filter_map(|line| line.split_once('='))
            .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
            .collect())
    }
}

/// Collect the environment layer.
///
/// Only variables this crate defines are read, so an unrelated shell variable
/// can never change behaviour.
fn env_layer() -> Vec<(String, String)> {
    const NAMES: [&str; 5] = [
        ENV_VAULT_ROOT,
        ENV_PROVIDERS,
        ENV_TIMEOUT_SECS,
        ENV_MAX_BODY_BYTES,
        ENV_PROFILE,
    ];

    NAMES
        .iter()
        .filter_map(|name| {
            std::env::var(name)
                .ok()
                .map(|value| (name.to_string(), value))
        })
        .collect()
}

/// Apply one key/value pair to `config`.
///
/// Unknown keys are rejected rather than ignored: a typo'd key that silently
/// did nothing is the classic configuration bug.
fn apply_layer(config: &mut Config, key: &str, value: &str) -> Result<()> {
    match key {
        ENV_VAULT_ROOT => config.vault_root = PathBuf::from(value),
        ENV_PROVIDERS => {
            config.providers = value
                .split(',')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect();
        }
        ENV_TIMEOUT_SECS => {
            let secs = value.parse::<u64>().map_err(|_| {
                Error::Config(format!(
                    "{ENV_TIMEOUT_SECS} must be a whole number of seconds, got {value:?}"
                ))
            })?;
            config.timeout = Duration::from_secs(secs);
        }
        ENV_MAX_BODY_BYTES => {
            let bytes = value.parse::<u64>().map_err(|_| {
                Error::Config(format!(
                    "{ENV_MAX_BODY_BYTES} must be a whole number of bytes, got {value:?}"
                ))
            })?;
            config.max_body_bytes = bytes;
        }
        ENV_PROFILE => config.profile = value.to_string(),
        other => {
            return Err(Error::Config(format!(
                "unknown configuration key {other:?}"
            )));
        }
    }

    Ok(())
}
