//! Runtime tests for `ragent_config::bash_lists`.
//!
//! The bash allow/deny list mutators persist through
//! [`ragent_config::list_config::patch_config`], which writes relative to the
//! current working directory, so mutating tests run inside a private temp
//! working directory. The `bash_lists` statics and the process cwd are
//! process-global, so the tests are serialised.

#![allow(unsafe_code)]

use std::env;
use std::path::PathBuf;

use ragent_config::bash_lists::{self, BashLists};
use ragent_config::list_config::Scope;

/// Points the process at a private temp working directory and restores the
/// original cwd (plus `XDG_CONFIG_HOME`) on drop.
struct Sandbox {
    original_cwd: PathBuf,
    original_xdg: Option<String>,
    temp: tempfile::TempDir,
}

impl Sandbox {
    fn new() -> Self {
        let temp = tempfile::tempdir().expect("tempdir");
        let original_cwd = env::current_dir().expect("cwd");
        let original_xdg = env::var("XDG_CONFIG_HOME").ok();
        // SAFETY: Rust 2024 makes `set_var` unsafe; this test target opts in
        // to env mutation below and runs its tests serially.
        unsafe { env::set_var("XDG_CONFIG_HOME", temp.path().join("config")) };
        env::set_current_dir(temp.path()).expect("chdir to sandbox");
        Self {
            original_cwd,
            original_xdg,
            temp,
        }
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        env::set_current_dir(&self.original_cwd).ok();
        match self.original_xdg.take() {
            // SAFETY: see `Sandbox::new`.
            Some(v) => unsafe { env::set_var("XDG_CONFIG_HOME", v) },
            None => unsafe { env::remove_var("XDG_CONFIG_HOME") },
        }
        let _ = &self.temp;
    }
}

#[test]
#[serial_test::serial]
fn test_bash_lists_default_and_debug() {
    let lists = BashLists::default();
    assert_eq!(lists.allowlist, Vec::<String>::new());
    assert_eq!(lists.denylist, Vec::<String>::new());
    let cloned = lists.clone();
    assert_eq!(cloned.denylist, lists.denylist);
    let lists = BashLists {
        allowlist: vec!["curl".to_string()],
        denylist: vec!["rm -rf".to_string()],
    };
    let debug = format!("{lists:?}");
    assert!(debug.contains("curl") && debug.contains("rm -rf"));
}

#[test]
#[serial_test::serial]
fn test_load_from_config_default_is_empty() {
    let _sandbox = Sandbox::new();
    bash_lists::load_from_config();
    assert_eq!(bash_lists::get_allowlist(), Vec::<String>::new());
    assert_eq!(bash_lists::get_denylist(), Vec::<String>::new());
    assert!(!bash_lists::is_allowlisted("curl -s https://example.com"));
    assert!(bash_lists::matches_denylist("rm -rf /").is_none());
}

#[test]
#[serial_test::serial]
fn test_add_allowlist_matches_first_token_only() {
    let _sandbox = Sandbox::new();
    bash_lists::add_allowlist("curl", Scope::Project).expect("add allowlist");

    assert!(bash_lists::get_allowlist().iter().any(|e| e == "curl"));
    // Only the command's first token is compared.
    assert!(bash_lists::is_allowlisted("curl -s https://example.com"));
    assert!(bash_lists::is_allowlisted("  curl"));
    assert!(!bash_lists::is_allowlisted("curlie arg"));
    assert!(!bash_lists::is_allowlisted("echo curl"));
    // An empty command yields an empty first token.
    assert!(!bash_lists::is_allowlisted(""));

    // Re-adding is idempotent.
    bash_lists::add_allowlist("curl", Scope::Project).expect("re-add allowlist");
    assert_eq!(
        bash_lists::get_allowlist()
            .iter()
            .filter(|e| e.as_str() == "curl")
            .count(),
        1
    );

    assert!(bash_lists::remove_allowlist("curl", Scope::Project).expect("remove allowlist"));
    assert!(!bash_lists::is_allowlisted("curl -s"));
    assert!(!bash_lists::remove_allowlist("curl", Scope::Project).expect("remove again"));
}

#[test]
#[serial_test::serial]
fn test_add_denylist_matches_substring_and_persists() {
    let _sandbox = Sandbox::new();
    bash_lists::add_denylist("git push --force", Scope::Project).expect("add denylist");

    assert!(
        bash_lists::get_denylist()
            .iter()
            .any(|e| e == "git push --force")
    );
    assert_eq!(
        bash_lists::matches_denylist("cd repo && git push --force origin main"),
        Some("git push --force".to_string())
    );
    assert!(bash_lists::matches_denylist("git push origin main").is_none());

    // The mutation was persisted to the project config file.
    let raw = std::fs::read_to_string(".ragent/ragent.json").expect("read project config");
    assert!(raw.contains("git push --force"));

    // Re-adding is idempotent.
    bash_lists::add_denylist("git push --force", Scope::Project).expect("re-add denylist");
    assert_eq!(
        bash_lists::get_denylist()
            .iter()
            .filter(|e| e.as_str() == "git push --force")
            .count(),
        1
    );

    assert!(
        bash_lists::remove_denylist("git push --force", Scope::Project).expect("remove denylist")
    );
    assert!(bash_lists::matches_denylist("git push --force").is_none());
    assert!(
        !bash_lists::remove_denylist("git push --force", Scope::Project).expect("remove again")
    );
}

#[test]
#[serial_test::serial]
fn test_first_matching_denylist_entry_wins() {
    let _sandbox = Sandbox::new();
    bash_lists::add_denylist("alpha-block", Scope::Project).expect("add first");
    bash_lists::add_denylist("beta-block", Scope::Project).expect("add second");

    // Both patterns appear; the first list entry that matches is returned.
    let matched = bash_lists::matches_denylist("run alpha-block and beta-block").unwrap();
    assert_eq!(matched, "alpha-block");
    assert_eq!(
        bash_lists::matches_denylist("run beta-block only").as_deref(),
        Some("beta-block")
    );

    bash_lists::remove_denylist("alpha-block", Scope::Project).expect("cleanup a");
    bash_lists::remove_denylist("beta-block", Scope::Project).expect("cleanup b");
}
