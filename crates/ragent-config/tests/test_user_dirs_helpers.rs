//! Tests for `ragent_config::user_dirs` - the canonical global-state path
//! helpers.
//!
//! Every helper derives from [`Config::global_state_dir`], which on Linux is
//! `dirs::config_dir()/ragent`. The tests point `XDG_CONFIG_HOME` (and `HOME`)
//! at a private temp directory so the assertions are deterministic and the real
//! user config is never consulted. Environment mutation is process-global, so
//! the tests are serialised.

#![allow(unsafe_code)]

use std::path::{Path, PathBuf};

use ragent_config::user_dirs;

/// Redirects the platform config/home/data directories at a temp root and
/// restores the previous values on drop.
struct EnvGuard {
    saved: Vec<(&'static str, Option<String>)>,
    _temp: tempfile::TempDir,
    root: PathBuf,
}

impl EnvGuard {
    fn new() -> Self {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().to_path_buf();
        let keys = ["XDG_CONFIG_HOME", "XDG_DATA_HOME", "HOME"];
        let saved = keys.iter().map(|k| (*k, std::env::var(k).ok())).collect();
        // SAFETY: Rust 2024 makes `set_var` unsafe; this target opts in and
        // runs its tests serially.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", root.join("config"));
            std::env::set_var("XDG_DATA_HOME", root.join("data"));
            std::env::set_var("HOME", &root);
        }
        Self {
            saved,
            _temp: temp,
            root,
        }
    }

    fn config_root(&self) -> PathBuf {
        self.root.join("config").join("ragent")
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (k, v) in &self.saved {
            match v {
                // SAFETY: see `EnvGuard::new`.
                Some(val) => unsafe { std::env::set_var(k, val) },
                None => unsafe { std::env::remove_var(k) },
            }
        }
    }
}

fn assert_child(parent: &Path, child: &Option<PathBuf>, leaf: &str) {
    let child = child.as_ref().expect("helper resolves a path");
    assert_eq!(
        child,
        &parent.join(leaf),
        "helper must join {leaf:?} under the global state dir"
    );
}

#[test]
#[serial_test::serial]
fn test_global_state_and_subdirectory_helpers() {
    let env = EnvGuard::new();
    let root = env.config_root();

    assert_eq!(
        user_dirs::global_state_dir().as_deref(),
        Some(root.as_path())
    );
    assert_eq!(
        user_dirs::global_instructions_dir().as_deref(),
        Some(root.as_path())
    );

    assert_child(&root, &user_dirs::global_memory_dir(), "memory");
    assert_child(&root, &user_dirs::global_agents_dir(), "agents");
    assert_child(&root, &user_dirs::global_skills_dir(), "skills");
    assert_child(&root, &user_dirs::global_teams_dir(), "teams");
    assert_child(&root, &user_dirs::global_blueprints_dir(), "blueprints");
    assert_child(&root, &user_dirs::global_templates_dir(), "templates");
    assert_child(&root, &user_dirs::global_agent_memory_dir(), "agent-memory");
    assert_child(&root, &user_dirs::global_mcp_servers_dir(), "servers");
    assert_child(&root, &user_dirs::global_models_dir(), "models");
    assert_child(&root, &user_dirs::global_loop_state_dir(), "loop-state");
    assert_child(&root, &user_dirs::global_inbox_dir(), "log/inbox");
    assert_child(
        &root,
        &user_dirs::global_history_path(),
        "input_history.txt",
    );
    assert_child(&root, &user_dirs::global_db_path(), "ragent.db");
    assert_child(
        &root,
        &user_dirs::global_github_token_path(),
        "github_token",
    );
    assert_child(
        &root,
        &user_dirs::global_gitlab_token_path(),
        "gitlab_token",
    );
    assert_child(
        &root,
        &user_dirs::global_gitlab_config_path(),
        "gitlab_config.json",
    );
    assert_child(
        &root.join("models"),
        &user_dirs::global_embedding_model_dir(),
        "all-MiniLM-L6-v2",
    );
}

#[test]
#[serial_test::serial]
fn test_legacy_dirs_point_at_home_and_data_roots() {
    let env = EnvGuard::new();
    // `legacy_home_dir` is the pre-XDG `~/.ragent` root.
    assert_eq!(user_dirs::legacy_home_dir(), Some(env.root.join(".ragent")));
    // `legacy_data_dir` folds into the XDG data root `<XDG_DATA_HOME>/ragent`.
    assert_eq!(
        user_dirs::legacy_data_dir(),
        Some(env.root.join("data").join("ragent"))
    );
}
