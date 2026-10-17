//! Coverage tests for `github::auth` - the stored-token file, the environment
//! precedence, and the OAuth client-id resolver.
//!
//! The token file location is derived from `dirs::config_dir()`, which on
//! Linux honours `$XDG_CONFIG_HOME`; each test points that at a private temp
//! directory under a process-wide lock so the process-global token cache and
//! the environment mutations cannot interleave. No token file is ever read or
//! written outside the temp directory, and no network is touched.

use std::path::PathBuf;
use std::sync::Mutex;

use ragent_tools_vcs::github::GitHubClient;
use ragent_tools_vcs::github::auth::{delete_token, load_token, save_token};

/// Serialises every test in this binary that mutates the environment or the
/// process-global token cache.
static LOCK: Mutex<()> = Mutex::new(());

/// RAII guard that restores a set of environment variables on drop.
struct EnvRestore(Vec<(&'static str, Option<String>)>);

impl EnvRestore {
    /// Replace the named variables with `set` (key, value) pairs, remembering
    /// the previous values. A `None` value removes the variable.
    fn apply(changes: &[(&'static str, Option<&str>)]) -> Self {
        let mut saved = Vec::new();
        for (name, value) in changes {
            let previous = std::env::var(name).ok();
            // SAFETY: test-only; a process-wide lock serialises all callers.
            #[allow(unsafe_code)]
            unsafe {
                match value {
                    Some(v) => std::env::set_var(name, v),
                    None => std::env::remove_var(name),
                }
            }
            saved.push((*name, previous));
        }
        Self(saved)
    }
}

impl Drop for EnvRestore {
    fn drop(&mut self) {
        for (name, previous) in self.0.drain(..) {
            // SAFETY: test-only; see `apply`.
            #[allow(unsafe_code)]
            unsafe {
                match previous {
                    Some(v) => std::env::set_var(name, v),
                    None => std::env::remove_var(name),
                }
            }
        }
    }
}

/// A temp config root plus the derived token-file path.
struct TempHome {
    dir: tempfile::TempDir,
}

impl TempHome {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().expect("tempdir"),
        }
    }

    fn root(&self) -> PathBuf {
        self.dir.path().to_path_buf()
    }

    /// The `~/.config/ragent/github_token` path under this temp root.
    fn token_path(&self) -> PathBuf {
        self.root().join("ragent").join("github_token")
    }
}

/// Run `f` with a private `XDG_CONFIG_HOME`, `GITHUB_TOKEN` cleared, and the
/// `gh` CLI fallback disabled, all under the process lock.
fn with_isolated_env<T>(f: impl FnOnce(&TempHome) -> T) -> T {
    let _lock = LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let home = TempHome::new();
    let root = home.root().to_string_lossy().to_string();
    let _restore = EnvRestore::apply(&[
        ("XDG_CONFIG_HOME", Some(root.as_str())),
        ("GITHUB_TOKEN", None),
        ("RAGENT_GITHUB_NO_GH_CLI", Some("1")),
    ]);
    f(&home)
}

#[test]
fn test_save_then_load_round_trips_and_caches() {
    with_isolated_env(|home| {
        save_token("ghp_roundtrip_token").expect("save");
        // First load reads the file (cache empty), second is a cache hit.
        assert_eq!(load_token().as_deref(), Some("ghp_roundtrip_token"));
        assert_eq!(load_token().as_deref(), Some("ghp_roundtrip_token"));
        assert!(home.token_path().exists(), "token file must be written");
    });
}

#[test]
fn test_delete_token_removes_file_and_reports_absent() {
    with_isolated_env(|home| {
        save_token("ghp_to_delete").expect("save");
        assert_eq!(load_token().as_deref(), Some("ghp_to_delete"));
        delete_token().expect("delete");
        assert!(!home.token_path().exists(), "file must be removed");
        assert_eq!(load_token(), None, "no token after delete");
        // Deleting again is a no-op (path does not exist).
        delete_token().expect("delete again");
    });
}

#[test]
fn test_blank_token_file_reads_as_none() {
    with_isolated_env(|home| {
        let path = home.token_path();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "   \n  \n").unwrap();
        assert_eq!(load_token(), None, "whitespace-only file is not a token");
    });
}

#[test]
fn test_unreadable_token_path_reads_as_none() {
    with_isolated_env(|home| {
        // Create a *directory* where the token file is expected: reading it as
        // a string fails, which must resolve to "no token", not a panic.
        let path = home.token_path();
        std::fs::create_dir_all(&path).unwrap();
        assert_eq!(load_token(), None);
    });
}

#[test]
fn test_env_token_takes_precedence_over_stored_file() {
    with_isolated_env(|_home| {
        save_token("ghp_stored_loses").expect("save");
        let _env = EnvRestore::apply(&[("GITHUB_TOKEN", Some("ghp_env_wins"))]);
        assert_eq!(load_token().as_deref(), Some("ghp_env_wins"));
    });
}

#[test]
fn test_blank_env_token_is_ignored_and_file_used() {
    with_isolated_env(|_home| {
        save_token("ghp_stored_used").expect("save");
        // A present-but-blank GITHUB_TOKEN must not shadow the stored token.
        let _env = EnvRestore::apply(&[("GITHUB_TOKEN", Some("   "))]);
        assert_eq!(load_token().as_deref(), Some("ghp_stored_used"));
    });
}

#[test]
fn test_app_token_stored_without_gh_cli_is_returned_unchanged() {
    with_isolated_env(|_home| {
        // `ghu_`/`ghs_` are GitHub App tokens; with the `gh` CLI fallback
        // disabled the stored app token is still returned as-is.
        save_token("ghu_app_token").expect("save");
        assert_eq!(load_token().as_deref(), Some("ghu_app_token"));
    });
}

#[test]
fn test_stored_file_wins_when_no_env_and_no_gh_cli() {
    with_isolated_env(|_home| {
        save_token("ghp_plain").expect("save");
        // No GITHUB_TOKEN, `gh` disabled: the stored (non-app) token is used.
        assert_eq!(load_token().as_deref(), Some("ghp_plain"));
    });
}

#[test]
fn test_client_id_env_override_and_default() {
    let _lock = LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    // Default: the compiled OAuth client id, which is non-empty.
    {
        let _env = EnvRestore::apply(&[("RAGENT_GITHUB_CLIENT_ID", None)]);
        let default = GitHubClient::client_id();
        assert!(!default.is_empty(), "compiled client id must be non-empty");
    }
    // Override: the env value is returned verbatim.
    {
        let _env = EnvRestore::apply(&[("RAGENT_GITHUB_CLIENT_ID", Some("env-client-id"))]);
        assert_eq!(GitHubClient::client_id(), "env-client-id");
    }
    // Restored default after the guard drops.
    let default_again = GitHubClient::client_id();
    assert_ne!(default_again, "");
}
