//! Auth-shape handling over the encrypted credential store (spec `connectors`
//! T-007; FR-005, FR-014, FR-022, FR-023, FR-032).
//!
//! A connector declares a *shape* describing how it is authenticated
//! ([`ConnectorAuthShape`]): nothing (`none`), an environment variable
//! (`env`), a stored secret (`token` / `password`), or an OAuth
//! authorization-code exchange (`oauth`). This module turns that shape plus the
//! descriptor's non-secret credential *name* into:
//!
//! - an [`AuthRequirement`] - what the connector needs, the value `/connectors
//!   add` and `/connectors list` surface without contacting the catalogue again
//!   (FR-023);
//! - an [`AuthState`] - `none`, `needs auth`, `authenticated`, or `auth failed`,
//!   the value `/connectors list` prints (FR-014, FR-022, FR-032);
//! - an [`AuthAction`] - what `/connectors auth <id>` should do for the shape
//!   (prompt for a secret, or begin the OAuth flow) (FR-014).
//!
//! # Secrets and configuration are separate (FR-005)
//!
//! A secret value is written **only** to the encrypted credential store, under
//! the descriptor's `credential` name; the name is the only thing recorded in a
//! connector manifest or `ragent.json`. [`store_secret`] performs that write and
//! returns an [`AuthState`] carrying **no** secret material, so a report line can
//! be printed without echoing the value (FR-014).
//!
//! # The credential store is a seam
//!
//! This crate must not depend on `ragent-storage` (the SQLite store) or on the
//! session layer, so the encrypted store enters through the [`CredentialStore`]
//! trait: the session implements it over `Storage::get_provider_auth` /
//! `set_provider_auth` / `delete_provider_auth`, and a test uses
//! [`InMemoryCredentialStore`]. The environment enters through [`EnvSource`] for
//! the same reason, so the `env` shape is testable without mutating the process
//! environment ([`ProcessEnv`] is the production reader).
//!
//! Every failure is contained: a missing credential *name* or a store fault is
//! returned as an [`AuthError`] and never panics.

use std::collections::BTreeMap;
use std::sync::Mutex;

use crate::descriptor::{ConnectorAuthShape, ConnectorDescriptor};
use crate::error::ConnectorError;

/// What a connector needs before it may connect (FR-023).
///
/// Built from a descriptor by [`AuthRequirement::for_descriptor`]; the fields
/// carry only non-secret names, so the requirement is safe to record in a
/// manifest and print in a report. `env_var` names the environment variable the
/// `env` shape reads; `credential` names the encrypted credential the other
/// shapes store their secret under; `scopes` lists the OAuth scopes (empty for
/// every other shape).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AuthRequirement {
    /// The connector the requirement belongs to (for guidance text).
    pub connector: String,
    /// The declared authentication shape.
    pub shape: ConnectorAuthShape,
    /// The environment variable an `env` shape reads, when declared.
    pub env_var: Option<String>,
    /// The encrypted-credential name a `token`/`password`/`oauth` shape uses,
    /// and the name an `env` shape stores its value under when prompted
    /// (FR-014).
    pub credential: Option<String>,
    /// OAuth scopes the connector requests; empty for every other shape.
    pub scopes: Vec<String>,
}

impl AuthRequirement {
    /// Derive the requirement from a connector descriptor (FR-023).
    ///
    /// The descriptor carries the shape, the OAuth scopes, and the credential
    /// *name*, so the requirement is recovered without contacting the catalogue
    /// again. For the `env` shape the declared credential name doubles as the
    /// environment-variable name.
    #[must_use]
    pub fn for_descriptor(descriptor: &ConnectorDescriptor) -> Self {
        let shape = descriptor.auth;
        let (env_var, credential) = match shape {
            ConnectorAuthShape::None => (None, None),
            ConnectorAuthShape::Env => {
                let name = descriptor.credential.clone();
                (name.clone(), name)
            }
            ConnectorAuthShape::Token
            | ConnectorAuthShape::Password
            | ConnectorAuthShape::Oauth => (None, descriptor.credential.clone()),
        };
        Self {
            connector: descriptor.id.to_string(),
            shape,
            env_var,
            credential,
            scopes: descriptor.auth_scope.clone(),
        }
    }

    /// Whether the connector needs no authentication at all (`none` shape).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        matches!(self.shape, ConnectorAuthShape::None)
    }

    /// Whether the shape requires a credential before the connector may connect
    /// (FR-022).
    #[must_use]
    pub fn requires_credential(&self) -> bool {
        self.shape.requires_credential()
    }

    /// The name the secret is stored under, or the environment variable the
    /// `env` shape reads.
    #[must_use]
    pub fn credential_name(&self) -> Option<&str> {
        self.credential.as_deref().or(self.env_var.as_deref())
    }

    /// One-line description of what the connector needs (FR-023).
    ///
    /// `none` for an unauthenticated connector; otherwise it names the
    /// environment variable, the credential, and (for OAuth) the scopes. No
    /// secret value is ever part of this string.
    #[must_use]
    pub fn describe(&self) -> String {
        match self.shape {
            ConnectorAuthShape::None => "none".to_string(),
            ConnectorAuthShape::Env => match &self.env_var {
                Some(var) => format!("env var {var}"),
                None => "env (variable name not declared)".to_string(),
            },
            ConnectorAuthShape::Token => match &self.credential {
                Some(name) => format!("token ({name})"),
                None => "token (credential name not declared)".to_string(),
            },
            ConnectorAuthShape::Password => match &self.credential {
                Some(name) => format!("password ({name})"),
                None => "password (credential name not declared)".to_string(),
            },
            ConnectorAuthShape::Oauth => {
                let name = self
                    .credential
                    .as_deref()
                    .unwrap_or("(credential name not declared)");
                if self.scopes.is_empty() {
                    format!("oauth ({name})")
                } else {
                    format!("oauth ({name}; scopes: {})", self.scopes.join(", "))
                }
            }
        }
    }

    /// How to satisfy the requirement (FR-014, FR-022, FR-032).
    ///
    /// The guidance names the shape-specific action and the credential or
    /// environment variable involved; it never contains a secret value.
    #[must_use]
    pub fn guidance(&self) -> String {
        let id = &self.connector;
        match self.shape {
            ConnectorAuthShape::None => String::new(),
            ConnectorAuthShape::Env => match &self.env_var {
                Some(var) => format!(
                    "set the {var} environment variable, or run `/connectors auth {id}` to store it"
                ),
                None => format!("run `/connectors auth {id}` to provide the environment value"),
            },
            ConnectorAuthShape::Token => format!(
                "run `/connectors auth {id}` and paste a token (stored as {})",
                self.credential.as_deref().unwrap_or("<credential>")
            ),
            ConnectorAuthShape::Password => format!(
                "run `/connectors auth {id}` and enter the username and password (stored as {})",
                self.credential.as_deref().unwrap_or("<credential>")
            ),
            ConnectorAuthShape::Oauth => {
                let scopes = if self.scopes.is_empty() {
                    String::new()
                } else {
                    format!(" (scopes: {})", self.scopes.join(", "))
                };
                format!(
                    "run `/connectors auth {id}` to begin the OAuth authorization-code flow{scopes}"
                )
            }
        }
    }

    /// The action `/connectors auth <id>` should take for this shape (FR-014).
    #[must_use]
    pub fn action(&self) -> AuthAction {
        match self.shape {
            ConnectorAuthShape::None => AuthAction::None,
            ConnectorAuthShape::Env | ConnectorAuthShape::Token | ConnectorAuthShape::Password => {
                match self.credential_name() {
                    Some(name) => AuthAction::Prompt {
                        credential: name.to_string(),
                        shape: self.shape,
                    },
                    None => AuthAction::None,
                }
            }
            ConnectorAuthShape::Oauth => match &self.credential {
                Some(name) => AuthAction::OAuth {
                    credential: name.clone(),
                    scopes: self.scopes.clone(),
                },
                None => AuthAction::None,
            },
        }
    }
}

/// What `/connectors auth <id>` does for a connector's shape (FR-014).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthAction {
    /// No action: the connector needs no authentication.
    None,
    /// Prompt for a value and store it encrypted under the credential name
    /// (`env`, `token`, `password`).
    Prompt {
        /// The credential name the value is stored under.
        credential: String,
        /// The shape that prompted the request.
        shape: ConnectorAuthShape,
    },
    /// Begin the OAuth authorization-code flow and store the resulting token
    /// under the credential name.
    OAuth {
        /// The credential name the resulting token is stored under.
        credential: String,
        /// The scopes the flow requests.
        scopes: Vec<String>,
    },
}

/// The authentication state of a connector, the value `/connectors list` prints
/// (FR-014, FR-022, FR-032).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthState {
    /// The connector needs no authentication (shape `none`).
    NotRequired,
    /// The connector declares an auth shape and no valid credential is stored
    /// (FR-022). The connector refuses to connect.
    NeedsAuth,
    /// A valid credential (or environment variable) is present; the connector
    /// may connect.
    Satisfied,
    /// The last authentication attempt failed (an absent, expired, or rejected
    /// secret, FR-032). The connector refuses to connect and is not retried in a
    /// loop.
    Failed,
}

impl AuthState {
    /// The short label reports print (FR-014, FR-022).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::NotRequired => "none",
            Self::NeedsAuth => "needs auth",
            Self::Satisfied => "authenticated",
            Self::Failed => "auth failed",
        }
    }

    /// Whether the connector may connect in this state (FR-022, FR-032).
    ///
    /// Only [`AuthState::NotRequired`] and [`AuthState::Satisfied`] permit a
    /// connection; `needs auth` and `auth failed` both refuse, and a `failed`
    /// state must not be retried in a loop.
    #[must_use]
    pub const fn permits_connect(self) -> bool {
        matches!(self, Self::NotRequired | Self::Satisfied)
    }
}

impl std::fmt::Display for AuthState {
    /// Render the state label, never any secret value (FR-014).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// The seam through which the encrypted credential store is reached (FR-005).
///
/// The session implements this over the SQLite credential store
/// (`Storage::get_provider_auth` / `set_provider_auth` / `delete_provider_auth`);
/// [`InMemoryCredentialStore`] is the test implementation. Keeping it a trait
/// lets the connector crate hold no dependency on `ragent-storage`.
///
/// Implementers must be `Send + Sync` because a connect may run off the event
/// loop. Every failure is a contained [`AuthError`]; no implementation panics.
pub trait CredentialStore: Send + Sync {
    /// Look up the secret stored under `name`, or `None` when absent.
    ///
    /// # Errors
    ///
    /// Returns [`AuthError::StoreFailed`] when the store cannot be read (for
    /// example a corrupt ciphertext), so a read fault is distinguishable from an
    /// absent credential.
    fn get(&self, name: &str) -> Result<Option<String>, AuthError>;

    /// Store `value` under `name`, replacing any previous value (encrypted by
    /// the implementation).
    ///
    /// # Errors
    ///
    /// Returns [`AuthError::StoreFailed`] when the value cannot be persisted.
    fn set(&self, name: &str, value: &str) -> Result<(), AuthError>;

    /// Remove the secret stored under `name`. Removing an absent name is not an
    /// error.
    ///
    /// # Errors
    ///
    /// Returns [`AuthError::StoreFailed`] when the value cannot be removed.
    fn remove(&self, name: &str) -> Result<(), AuthError>;
}

/// A [`CredentialStore`] that holds no secret.
///
/// Reading is always a miss and writing is refused with a clear cause, so the
/// lifecycle reports `needs auth` rather than pretending a credential exists.
/// It is the shared default for the command surfaces that have no encrypted
/// store wired yet (the TUI `/connectors` glue, the `ragent connectors` CLI, and
/// the crate's async dispatcher); wiring the session's encrypted store is the
/// session-start integration's job (T-008).
#[derive(Debug, Default)]
pub struct NullCredentialStore;

impl CredentialStore for NullCredentialStore {
    fn get(&self, _name: &str) -> Result<Option<String>, AuthError> {
        Ok(None)
    }

    fn set(&self, _name: &str, _value: &str) -> Result<(), AuthError> {
        Err(AuthError::StoreFailed {
            detail: "no credential store wired into this surface".to_string(),
        })
    }

    fn remove(&self, _name: &str) -> Result<(), AuthError> {
        Ok(())
    }
}

/// The seam through which environment variables are read for the `env` shape
/// (FR-022).
///
/// Production uses [`ProcessEnv`]; tests use [`MapEnv`] so the `env` shape is
/// exercised without mutating the process environment.
pub trait EnvSource: Send + Sync {
    /// The value of the environment variable `name`, or `None` when it is unset.
    fn var(&self, name: &str) -> Option<String>;
}

/// The production [`EnvSource`]: reads the process environment.
#[derive(Debug, Clone, Copy, Default)]
pub struct ProcessEnv;

impl EnvSource for ProcessEnv {
    fn var(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }
}

/// An offline [`EnvSource`] backed by an in-memory map (spec `connectors` T-007
/// test seam).
///
/// Mirrors [`crate::fetch::FixtureCatalogueFetcher`]: it makes the `env` shape
/// hermetic, so a test never mutates the process environment.
#[derive(Debug, Clone, Default)]
pub struct MapEnv {
    vars: BTreeMap<String, String>,
}

impl MapEnv {
    /// An empty environment.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `name` with `value`.
    #[must_use]
    pub fn with_var(mut self, name: &str, value: &str) -> Self {
        self.vars.insert(name.to_string(), value.to_string());
        self
    }
}

impl EnvSource for MapEnv {
    fn var(&self, name: &str) -> Option<String> {
        self.vars.get(name).cloned()
    }
}

/// An in-memory [`CredentialStore`] (spec `connectors` T-007 test seam).
///
/// Holds secrets in a map for the lifetime of the value; it performs no
/// encryption and is intended for tests and hermetic runs. Its [`Debug`]
/// implementation prints key **names** only, never the stored values, so a
/// `tracing::debug!("{store:?}")` cannot leak a secret.
#[derive(Default)]
pub struct InMemoryCredentialStore {
    secrets: Mutex<BTreeMap<String, String>>,
}

impl InMemoryCredentialStore {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Acquire the lock, tolerating poisoning (the map is never left in an
    /// inconsistent state).
    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, String>> {
        self.secrets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl std::fmt::Debug for InMemoryCredentialStore {
    /// Print the stored names only (never the secret values).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let secrets = self.lock();
        let keys: Vec<&String> = secrets.keys().collect();
        f.debug_struct("InMemoryCredentialStore")
            .field("names", &keys)
            .finish()
    }
}

impl CredentialStore for InMemoryCredentialStore {
    fn get(&self, name: &str) -> Result<Option<String>, AuthError> {
        Ok(self.lock().get(name).cloned())
    }

    fn set(&self, name: &str, value: &str) -> Result<(), AuthError> {
        self.lock().insert(name.to_string(), value.to_string());
        Ok(())
    }

    fn remove(&self, name: &str) -> Result<(), AuthError> {
        self.lock().remove(name);
        Ok(())
    }
}

/// A contained auth failure (FR-032).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuthError {
    /// A connector declares an auth shape that needs a secret, but names no
    /// credential (so there is nowhere to store or read the secret).
    #[error("connector '{connector}' declares auth shape '{shape}' with no credential name")]
    MissingCredentialName {
        /// The connector id the shape belongs to.
        connector: String,
        /// The shape that required the name.
        shape: ConnectorAuthShape,
    },

    /// The credential store could not be read or written.
    #[error("connector credential store failed: {detail}")]
    StoreFailed {
        /// The underlying store failure detail.
        detail: String,
    },
}

impl From<AuthError> for ConnectorError {
    /// Convert an auth failure into the reportable connector error (FR-032).
    fn from(error: AuthError) -> Self {
        Self::Auth {
            detail: error.to_string(),
        }
    }
}

/// Resolve the current authentication state of `descriptor` (FR-022).
///
/// A `none` shape is [`AuthState::NotRequired`]. An `env` shape is
/// [`AuthState::Satisfied`] when its environment variable is set (non-blank) or
/// a value has been stored under the credential name; otherwise
/// [`AuthState::NeedsAuth`]. Every other shape is `Satisfied` when a credential
/// is stored under the credential name, else `NeedsAuth`.
///
/// # Errors
///
/// Returns [`AuthError::StoreFailed`] when the credential store cannot be read.
pub fn resolve(
    descriptor: &ConnectorDescriptor,
    store: &dyn CredentialStore,
    env: &dyn EnvSource,
) -> Result<AuthState, AuthError> {
    let req = AuthRequirement::for_descriptor(descriptor);
    match req.shape {
        ConnectorAuthShape::None => Ok(AuthState::NotRequired),
        ConnectorAuthShape::Env => {
            // The env var counts only when non-blank; the stored credential is a
            // fallback. `env_shape_value` owns that resolution so `resolve` and
            // `secret_value` cannot disagree.
            if env_shape_value(&req, store, env)?.is_some_and(|value| !value.trim().is_empty()) {
                Ok(AuthState::Satisfied)
            } else {
                Ok(AuthState::NeedsAuth)
            }
        }
        ConnectorAuthShape::Token | ConnectorAuthShape::Password | ConnectorAuthShape::Oauth => {
            if credential_present(&req, store)? {
                Ok(AuthState::Satisfied)
            } else {
                Ok(AuthState::NeedsAuth)
            }
        }
    }
}

/// Whether a stored credential satisfies `req` (FR-022).
fn credential_present(
    req: &AuthRequirement,
    store: &dyn CredentialStore,
) -> Result<bool, AuthError> {
    match req.credential_name() {
        Some(name) => Ok(store
            .get(name)?
            .is_some_and(|value| !value.trim().is_empty())),
        None => Ok(false),
    }
}

/// The `env` auth shape's live value: the non-blank environment variable, else
/// the value stored under the credential name (FR-022, FR-005).
///
/// Returns `Ok(None)` when neither the environment variable is set nor a
/// credential name is declared.
fn env_shape_value(
    req: &AuthRequirement,
    store: &dyn CredentialStore,
    env: &dyn EnvSource,
) -> Result<Option<String>, AuthError> {
    if let Some(value) = req
        .env_var
        .as_deref()
        .and_then(|var| env.var(var))
        .filter(|value| !value.trim().is_empty())
    {
        return Ok(Some(value));
    }
    match req.credential_name() {
        Some(name) => store.get(name),
        None => Ok(None),
    }
}

/// Read the secret that satisfies `descriptor`, for injection into a server's
/// environment or headers at connect time (FR-005).
///
/// The `env` shape reads the live environment variable first and falls back to
/// the value stored under the credential name; every other shape reads the
/// stored credential. A `none` shape yields `None`.
///
/// # Errors
///
/// Returns [`AuthError::StoreFailed`] when the credential store cannot be read.
pub fn secret_value(
    descriptor: &ConnectorDescriptor,
    store: &dyn CredentialStore,
    env: &dyn EnvSource,
) -> Result<Option<String>, AuthError> {
    let req = AuthRequirement::for_descriptor(descriptor);
    match req.shape {
        ConnectorAuthShape::None => Ok(None),
        ConnectorAuthShape::Env => env_shape_value(&req, store, env),
        ConnectorAuthShape::Token | ConnectorAuthShape::Password | ConnectorAuthShape::Oauth => {
            match req.credential {
                Some(name) => store.get(&name),
                None => Ok(None),
            }
        }
    }
}

/// Store `value` as the connector's secret and return the resulting state
/// (FR-005, FR-014).
///
/// The value is written **only** to the encrypted credential store, under the
/// descriptor's credential name (or the `env` shape's variable name); it is
/// never returned, logged, or written to a manifest. The returned [`AuthState`]
/// carries no secret material, so the caller can print a state line without
/// echoing the value.
///
/// # Errors
///
/// Returns [`AuthError::MissingCredentialName`] when the shape needs a secret
/// but names no credential, and [`AuthError::StoreFailed`] when the value cannot
/// be persisted.
pub fn store_secret(
    descriptor: &ConnectorDescriptor,
    value: &str,
    store: &dyn CredentialStore,
) -> Result<AuthState, AuthError> {
    let req = AuthRequirement::for_descriptor(descriptor);
    if req.shape == ConnectorAuthShape::None {
        // Nothing to store; an unauthenticated connector is already satisfied.
        return Ok(AuthState::NotRequired);
    }
    let name = req
        .credential_name()
        .ok_or_else(|| AuthError::MissingCredentialName {
            connector: descriptor.id.to_string(),
            shape: req.shape,
        })?;
    store.set(name, value)?;
    Ok(AuthState::Satisfied)
}

/// Remove the connector's stored secret (FR-014).
///
/// After removal the connector reverts to [`AuthState::NeedsAuth`] unless its
/// `env` shape is satisfied by the live environment.
///
/// # Errors
///
/// Returns [`AuthError::StoreFailed`] when the credential store cannot be
/// written.
pub fn clear_secret(
    descriptor: &ConnectorDescriptor,
    store: &dyn CredentialStore,
) -> Result<(), AuthError> {
    let req = AuthRequirement::for_descriptor(descriptor);
    if let Some(name) = req.credential_name() {
        store.remove(name)?;
    }
    Ok(())
}

/// Record an authentication failure as [`AuthState::Failed`] (FR-032).
///
/// The connect lifecycle calls this when a secret is absent, expired, or
/// rejected by the remote service. The state refuses to connect
/// (`permits_connect` is `false`), which is what stops the caller retrying the
/// connection in a loop; the caller surfaces
/// [`AuthRequirement::guidance`] for the connector's shape and stops.
#[must_use]
pub fn record_auth_failure() -> AuthState {
    AuthState::Failed
}
