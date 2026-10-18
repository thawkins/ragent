//! Coverage tests for the shared GitLab helpers (`gitlab/helpers.rs`):
//! `make_client`, `detect_project`, and the combined `detect`.
//!
//! These are the thin wrappers every `gitlab_*` tool calls first; exercising
//! them directly pins the two canonical error strings (`NOT_CONFIGURED` and
//! the "no remote" message) and the happy path (a configured instance plus a
//! GitLab remote) without any network use.

// Holds the crate-wide env lock across the body on purpose: it
// serialises against the PATH-swapping git-shim test, so serialisation
// (not parallelism) is the goal. See AGENTS.md ("Test Layout").
#![allow(clippy::await_holding_lock)]

use std::path::PathBuf;
use std::sync::Arc;

use ragent_tools_vcs::ToolContext;
use ragent_tools_vcs::gitlab::{detect, detect_project, make_client};
use ragent_tools_vcs::storage::StorageBackend;

/// Serialises tests that mutate `GITLAB_*` environment variables.

/// Minimal in-memory credential/settings store (always empty here).
#[derive(Default)]
struct EmptyStore;

impl StorageBackend for EmptyStore {
    fn get_provider_auth(&self, _provider_id: &str) -> anyhow::Result<Option<String>> {
        Ok(None)
    }
    fn set_provider_auth(&self, _provider_id: &str, _api_key: &str) -> anyhow::Result<()> {
        Ok(())
    }
    fn delete_provider_auth(&self, _provider_id: &str) -> anyhow::Result<()> {
        Ok(())
    }
    fn get_setting(&self, _key: &str) -> anyhow::Result<Option<String>> {
        Ok(None)
    }
    fn set_setting(&self, _key: &str, _value: &str) -> anyhow::Result<()> {
        Ok(())
    }
    fn delete_setting(&self, _key: &str) -> anyhow::Result<()> {
        Ok(())
    }
}

/// RAII guard restoring the named environment variables on drop.
struct EnvRestore(Vec<(&'static str, Option<String>)>);

impl EnvRestore {
    fn apply(changes: &[(&'static str, Option<String>)]) -> Self {
        let mut saved = Vec::new();
        for (name, value) in changes {
            let previous = std::env::var(name).ok();
            // SAFETY: test-only; a process-wide lock serialises all callers.
            #[allow(unsafe_code)]
            unsafe {
                match value.clone() {
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

/// A context with no storage and a throwaway working dir.
fn ctx_with(dir: PathBuf, storage: Option<Arc<dyn StorageBackend>>) -> ToolContext {
    ToolContext {
        session_id: "test-session".to_string(),
        working_dir: dir,
        storage,
        config: None,
    }
}

/// Extract the error message from a result, panicking (with a clear message)
/// when it unexpectedly succeeded. Avoids the `Debug` bound that
/// `unwrap_err`/`expect_err` impose on `Ok` types such as `GitLabClient`.
fn err_string<T>(result: anyhow::Result<T>) -> String {
    match result {
        Ok(_) => panic!("expected an error, got Ok"),
        Err(e) => e.to_string(),
    }
}

fn git(dir: &std::path::Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(status.status.success(), "git {args:?} failed");
}

#[test]
fn test_make_client_requires_storage() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ctx = ctx_with(dir.path().to_path_buf(), None);
    let err = err_string(make_client(&ctx));
    assert!(err.contains("Storage not available"), "got: {err}");
}

#[test]
fn test_make_client_unconfigured_reports_not_configured() {
    let _lock = crate::global_env_lock();
    let dir = tempfile::tempdir().expect("tempdir");
    let _env = EnvRestore::apply(&[
        ("GITLAB_TOKEN", None),
        ("GITLAB_URL", None),
        ("GITLAB_USERNAME", None),
        ("HOME", Some(dir.path().to_string_lossy().to_string())),
        ("XDG_CONFIG_HOME", None),
    ]);
    let ctx = ctx_with(dir.path().to_path_buf(), Some(Arc::new(EmptyStore)));
    let err = err_string(make_client(&ctx));
    assert!(
        err.contains("GitLab not configured"),
        "unconfigured client must surface the canonical message: {err}"
    );
}

#[test]
fn test_make_client_succeeds_with_env_configuration() {
    let _lock = crate::global_env_lock();
    let dir = tempfile::tempdir().expect("tempdir");
    let _env = EnvRestore::apply(&[
        ("GITLAB_TOKEN", Some("glpat-helper".to_string())),
        ("GITLAB_URL", Some("https://gitlab.example.com".to_string())),
        ("GITLAB_USERNAME", Some("helper-user".to_string())),
    ]);
    let ctx = ctx_with(dir.path().to_path_buf(), Some(Arc::new(EmptyStore)));
    let client = make_client(&ctx).expect("configured client");
    assert_eq!(client.instance_url(), "https://gitlab.example.com");
    assert_eq!(client.token(), "glpat-helper");
}

#[test]
fn test_detect_project_parses_gitlab_remote() {
    // Holds the crate-wide env lock: `test_git_single_spawn` swaps the
    // process-global `PATH` to a recording shim, which would poison this
    // test's own `git` invocations in a shared test binary.
    let _env_lock = crate::global_env_lock();
    let dir = tempfile::tempdir().expect("tempdir");
    git(dir.path(), &["init", "-q"]);
    git(
        dir.path(),
        &[
            "remote",
            "add",
            "origin",
            "git@gitlab.example.com:group/sub/project.git",
        ],
    );
    let ctx = ctx_with(dir.path().to_path_buf(), None);
    let project = detect_project(&ctx).expect("project detected");
    assert_eq!(project, "group%2Fsub%2Fproject");
}

#[test]
fn test_detect_project_without_remote_errors() {
    // Holds the crate-wide env lock: `test_git_single_spawn` swaps the
    // process-global `PATH` to a recording shim, which would poison this
    // test's own `git` invocations in a shared test binary.
    let _env_lock = crate::global_env_lock();
    let dir = tempfile::tempdir().expect("tempdir");
    git(dir.path(), &["init", "-q"]);
    let ctx = ctx_with(dir.path().to_path_buf(), None);
    let err = err_string(detect_project(&ctx));
    assert!(
        err.contains("Could not detect GitLab project"),
        "got: {err}"
    );
}

#[test]
fn test_detect_combines_client_and_project() {
    let _lock = crate::global_env_lock();
    let dir = tempfile::tempdir().expect("tempdir");
    git(dir.path(), &["init", "-q"]);
    git(
        dir.path(),
        &[
            "remote",
            "add",
            "origin",
            "https://gitlab.example.com/team/app.git",
        ],
    );
    let _env = EnvRestore::apply(&[
        ("GITLAB_TOKEN", Some("glpat-combined".to_string())),
        ("GITLAB_URL", Some("https://gitlab.example.com".to_string())),
        ("GITLAB_USERNAME", Some("combiner".to_string())),
    ]);
    let ctx = ctx_with(dir.path().to_path_buf(), Some(Arc::new(EmptyStore)));
    let (client, project) = detect(&ctx).expect("detect ok");
    assert_eq!(client.instance_url(), "https://gitlab.example.com");
    assert_eq!(project, "team%2Fapp");
}

#[test]
fn test_detect_propagates_client_error_first() {
    // No storage, no env config: `detect` fails on the client step.
    let _lock = crate::global_env_lock();
    let dir = tempfile::tempdir().expect("tempdir");
    let _env = EnvRestore::apply(&[("GITLAB_TOKEN", None), ("GITLAB_URL", None)]);
    let ctx = ctx_with(dir.path().to_path_buf(), None);
    let err = err_string(detect(&ctx));
    assert!(err.contains("Storage not available"), "got: {err}");
}
