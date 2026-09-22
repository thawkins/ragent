//! GitHub credential discovery shared by the VCS tool family and the `/new`
//! scaffolder.
//!
//! ragent can authenticate to GitHub with three credentials, tried in this
//! order:
//!
//! 1. `GITHUB_TOKEN` — an explicit environment override; always wins.
//! 2. The stored token file written by `/github login`
//!    (`~/.config/ragent/github_token`, legacy `~/.ragent/github_token`).
//! 3. The authenticated `gh` CLI (`gh auth token`), used as a fallback.
//!
//! Step 3 exists because the OAuth application ragent shares with the Copilot
//! provider mints GitHub App tokens (`ghu_`) whose permissions do not include
//! repository administration, so `POST /user/repos` is rejected with
//! `403 Resource not accessible by integration`. A stored token that is a
//! GitHub App token therefore defers to the `gh` CLI credential when one is
//! available; an ordinary PAT/OAuth token (`ghp_`/`gho_`/`github_pat_`)
//! continues to be used unchanged, and a read-only app token is still used
//! when no `gh` credential exists.
//!
//! The `gh` CLI is only spawned when the stored token is absent or is an app
//! token, and a successful lookup is cached for the process so the subprocess
//! runs at most once per session.

use std::process::Command;
use std::sync::Mutex;

use crate::user_dirs::global_github_token_path;

/// Environment variable that disables the `gh` CLI fallback when set to a
/// non-empty value.
///
/// Used by tests (and by users who do not want ragent to run `gh`) to keep
/// credential resolution deterministic. The Copilot provider's token
/// resolution is unaffected — it never consults `gh`.
pub const NO_GH_CLI_ENV: &str = "RAGENT_GITHUB_NO_GH_CLI";

/// Cache of the successful `gh auth token` lookup. `None` until the first
/// successful lookup; a failed lookup is not cached so a later in-session
/// `gh auth login` is still picked up.
static GH_CLI_TOKEN_CACHE: Mutex<Option<String>> = Mutex::new(None);

/// Read the `GITHUB_TOKEN` environment variable, ignoring a blank value.
#[must_use]
pub fn env_token() -> Option<String> {
    match std::env::var("GITHUB_TOKEN") {
        Ok(token) if !token.trim().is_empty() => Some(token.trim().to_owned()),
        _ => None,
    }
}

/// True when `token` is a GitHub App token: `ghu_` (user-to-server) or `ghs_`
/// (server-to-server).
///
/// These tokens are scoped by the app's own permissions and cannot create
/// repositories for the user via `POST /user/repos`. Classic PATs (`ghp_`),
/// OAuth tokens (`gho_`), and fine-grained PATs (`github_pat_`) are not app
/// tokens and are treated as viable.
#[must_use]
pub fn is_app_token(token: &str) -> bool {
    token.starts_with("ghu_") || token.starts_with("ghs_")
}

/// Read the stored token file (`~/.config/ragent/github_token`, legacy
/// `~/.ragent/github_token`).
///
/// Returns `None` when the file is absent, unreadable, or blank.
#[must_use]
pub fn stored_token() -> Option<String> {
    let path = global_github_token_path()?;
    let raw = std::fs::read_to_string(path).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

/// True when the [`NO_GH_CLI_ENV`] escape hatch is set to a non-empty value.
fn gh_cli_disabled() -> bool {
    std::env::var_os(NO_GH_CLI_ENV).is_some_and(|value| !value.is_empty())
}

/// Resolve the `gh` CLI credential via `gh auth token`.
///
/// Returns `None` when the `gh` CLI is absent or unauthenticated, or when the
/// [`NO_GH_CLI_ENV`] escape hatch is set. A successful lookup is cached for
/// the process (the token value is never logged).
#[must_use]
pub fn gh_cli_token() -> Option<String> {
    if gh_cli_disabled() {
        return None;
    }
    // Scope the lock so the guard is dropped before any `gh` subprocess runs.
    let cached = GH_CLI_TOKEN_CACHE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    if cached.is_some() {
        return cached;
    }

    let output = Command::new("gh").args(["auth", "token"]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let token = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if token.is_empty() {
        return None;
    }
    tracing::debug!("using `gh auth token` as the GitHub credential");
    *GH_CLI_TOKEN_CACHE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(token.clone());
    Some(token)
}

/// Combine an already-read stored token with the credential chain, spawning
/// the `gh` CLI only when the stored token is unusable.
///
/// A viable (non-app, non-empty) stored token wins outright. An app token —
/// which cannot create repositories — defers to the `gh` token when present,
/// and is otherwise returned unchanged so read-only GitHub tooling keeps
/// working.
#[must_use]
pub fn resolve_from_stored(stored: Option<String>) -> Option<String> {
    match stored {
        Some(token) if !is_app_token(&token) => Some(token),
        other => gh_cli_token().or(other),
    }
}

/// Apply the credential precedence to an explicit stored token and `gh` token.
///
/// Pure helper (no process access) mirroring [`resolve_from_stored`]; exposed
/// so callers that already hold a `gh` token — and tests — can combine the
/// two without spawning a subprocess.
#[must_use]
pub fn choose_token(stored: Option<String>, gh: Option<String>) -> Option<String> {
    match stored {
        Some(token) if !is_app_token(&token) => Some(token),
        other => gh.or(other),
    }
}

/// Resolve the effective GitHub token: `GITHUB_TOKEN` env → stored file →
/// `gh` CLI (with the app-token downgrade of [`resolve_from_stored`]).
///
/// Uses the plain (uncached) stored-token read; callers with their own cache
/// (the VCS tool family) should combine [`env_token`] and their cached read
/// through [`resolve_from_stored`] directly.
#[must_use]
pub fn resolve_github_token() -> Option<String> {
    env_token().or_else(|| resolve_from_stored(stored_token()))
}
