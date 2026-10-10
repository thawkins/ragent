//! Sandbox secret injection (spec `openhands` T-006; FR-010, FR-035).
//!
//! A container or remote backend is provisioned with the credentials it needs,
//! resolved from the encrypted credential store **at spawn time** and injected
//! as environment variables (assumption A8). A value is never baked into an
//! image, written into the workspace, or emitted in a `/config` dump (FR-010,
//! FR-035).
//!
//! # The credential store is a seam
//!
//! The production resolver ([`StorageSecretResolver`]) reads the encrypted
//! SQLite store through `Storage::get_provider_auth`; tests use
//! [`MapSecretResolver`] so the injection path is exercised offline without
//! touching the database. The descriptor names credentials by *name* only -
//! [`ragent_config::BackendConfig::credentials`] - so a config file never holds
//! a secret value.
//!
//! # Fail closed
//!
//! When a descriptor names a credential that the store does not hold, the spawn
//! fails with an error naming it rather than provisioning a sandbox with the
//! credential missing. Every resolved value is registered with the shared
//! redaction registry, so any later rendering of tool output or a diagnostic
//! masks it (FR-035).

use std::collections::BTreeMap;
use std::sync::Arc;

use ragent_config::BackendConfig;

use crate::storage::Storage;

/// The seam through which the encrypted credential store is reached (FR-010).
///
/// Implementers must be `Send + Sync` because a spawn may run off the event
/// loop. A store fault is surfaced as an [`anyhow::Error`] so a read failure is
/// distinguishable from an absent credential.
pub trait SecretResolver: Send + Sync {
    /// Look up the secret stored under `name`, or `None` when it is absent.
    ///
    /// # Errors
    ///
    /// Returns an error when the store cannot be read (for example a corrupt
    /// ciphertext).
    fn resolve(&self, name: &str) -> anyhow::Result<Option<String>>;
}

/// The production [`SecretResolver`] over the SQLite encrypted credential store.
///
/// Reads through `Storage::get_provider_auth`, the same table the `/auth`
/// surface and the provider key resolution use, so a credential stored once is
/// available to every consumer.
pub struct StorageSecretResolver {
    storage: Arc<Storage>,
}

impl StorageSecretResolver {
    /// Wrap a storage handle.
    #[must_use]
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }
}

impl std::fmt::Debug for StorageSecretResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StorageSecretResolver")
    }
}

impl SecretResolver for StorageSecretResolver {
    fn resolve(&self, name: &str) -> anyhow::Result<Option<String>> {
        self.storage.get_provider_auth(name)
    }
}

/// An offline [`SecretResolver`] backed by an in-memory map (T-006 test seam).
///
/// Mirrors the connector crate's `InMemoryCredentialStore`: it makes the
/// injection path hermetic, so a test never reads or writes the real database.
#[derive(Debug, Clone, Default)]
pub struct MapSecretResolver {
    entries: BTreeMap<String, String>,
}

impl MapSecretResolver {
    /// Build a resolver from `(name, value)` pairs.
    #[must_use]
    pub fn new(entries: impl IntoIterator<Item = (String, String)>) -> Self {
        Self {
            entries: entries.into_iter().collect(),
        }
    }

    /// Return a resolver with one more entry added.
    #[must_use]
    pub fn with(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.entries.insert(name.into(), value.into());
        self
    }
}

impl SecretResolver for MapSecretResolver {
    fn resolve(&self, name: &str) -> anyhow::Result<Option<String>> {
        Ok(self.entries.get(name).cloned())
    }
}

/// One resolved credential: the environment-variable name and its value.
///
/// The value is a secret, so `Debug` is hand-written to never print it - the
/// struct may travel through a log line or a panic message.
#[derive(Clone)]
pub struct ResolvedSecret {
    /// The name the value is injected under (also the stored credential name).
    pub name: String,
    /// The secret value; never rendered.
    pub value: String,
}

impl std::fmt::Debug for ResolvedSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolvedSecret")
            .field("name", &self.name)
            .field("value", &"[REDACTED]")
            .finish()
    }
}

/// Resolve every credential the descriptor names, or fail naming the first miss
/// (FR-010).
///
/// Each resolved value is registered with [`ragent_types::sanitize`] so it is
/// masked in any later rendering (FR-035). A blank configured name is skipped;
/// a name the store does not hold is a hard error so the sandbox is never
/// provisioned with a credential silently missing.
///
/// # Errors
///
/// Returns an error when a named credential is absent from the store or the
/// store cannot be read.
pub fn resolve_descriptor_secrets(
    descriptor: &BackendConfig,
    resolver: &dyn SecretResolver,
) -> anyhow::Result<Vec<ResolvedSecret>> {
    resolve_secret_names(&descriptor.credentials, descriptor.display_name(), resolver)
}

/// Resolve every credential in `names`, or fail naming the first miss (FR-010).
///
/// `owner` names the backend in the error so a miss is actionable. Each resolved
/// value is registered with [`ragent_types::sanitize`] so it is masked in any
/// later rendering (FR-035).
///
/// # Errors
///
/// Returns an error when a named credential is absent from the store or the
/// store cannot be read.
pub fn resolve_secret_names(
    names: &[String],
    owner: &str,
    resolver: &dyn SecretResolver,
) -> anyhow::Result<Vec<ResolvedSecret>> {
    let mut resolved = Vec::new();
    for raw in names {
        let name = raw.trim();
        if name.is_empty() {
            continue;
        }
        match resolver.resolve(name)? {
            Some(value) if !value.is_empty() => {
                ragent_types::sanitize::register_secret(&value);
                resolved.push(ResolvedSecret {
                    name: name.to_string(),
                    value,
                });
            }
            _ => anyhow::bail!(
                "the backend '{owner}' requires the credential '{name}', which is not present in \
                 the encrypted credential store; refusing to provision without it (FR-010)"
            ),
        }
    }
    Ok(resolved)
}

/// The `-e NAME=VALUE` arguments injected into `<runtime> run` (FR-010).
///
/// Interleaving the literal `-e` with its assignment keeps the container's
/// environment as the injection surface: the value lives in the container's
/// process environment, never in a workspace file or the image.
#[must_use]
pub fn container_env_args(secrets: &[ResolvedSecret]) -> Vec<String> {
    let mut args = Vec::with_capacity(secrets.len() * 2);
    for secret in secrets {
        args.push("-e".to_string());
        args.push(format!("{}={}", secret.name, secret.value));
    }
    args
}

/// Whether the container environment text `env` already carries every resolved
/// credential as a `NAME=VALUE` line (FR-010, FR-023).
///
/// Compares without allocating per line: a `format!` of the expected assignment
/// would allocate once per scanned line for every secret.
#[must_use]
pub fn secret_names_match(env: &str, secrets: &[ResolvedSecret]) -> bool {
    secrets.iter().all(|secret| {
        env.lines().any(|line| {
            line.trim()
                .strip_prefix(secret.name.as_str())
                .and_then(|rest| rest.strip_prefix('='))
                == Some(secret.value.as_str())
        })
    })
}

/// The credential names a descriptor declares, trimmed, deduplicated, sorted.
///
/// Non-secret identifiers, used as part of a sandbox's deterministic name so a
/// changed credential set replaces the container instead of reusing one that
/// was created without (or with different) credentials (FR-010, FR-023).
#[must_use]
pub fn credential_names(descriptor: &BackendConfig) -> Vec<String> {
    let mut names: Vec<String> = descriptor
        .credentials
        .iter()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .collect();
    names.sort();
    names.dedup();
    names
}
