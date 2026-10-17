//! Runtime tests for `ragent_config::dir_lists`.
//!
//! These exercise the process-global directory allow/deny list state and the
//! compiled glob caches. The list mutators persist through
//! [`ragent_config::list_config::patch_config`], which writes to a path
//! relative to the current working directory, so every mutating test runs
//! inside a private temp working directory. The `dir_lists` statics and the
//! process cwd are process-global, so the tests are serialised.

#![allow(unsafe_code)]

use std::env;
use std::path::PathBuf;

use globset::{Glob, GlobSet, GlobSetBuilder};
use ragent_config::Config;
use ragent_config::dir_lists::{
    self, BUILTIN_ALLOWLIST, BUILTIN_DENYLIST, CompiledDirLists, DirLists,
};
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

fn globset(patterns: &[&str]) -> GlobSet {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(Glob::new(pattern).expect("valid test glob"));
    }
    builder.build().expect("build globset")
}

#[test]
fn test_get_builtin_lists_allowlist_empty_denylist_populated() {
    let (allow, deny) = dir_lists::get_builtin_lists();
    assert!(
        allow.is_empty(),
        "built-in allowlist is intentionally empty"
    );
    assert_eq!(
        allow.len(),
        BUILTIN_ALLOWLIST.len(),
        "built-in allowlist count must match the constant"
    );
    assert_eq!(deny.len(), BUILTIN_DENYLIST.len());
    assert!(deny.contains(&"/etc/**".to_string()));
    assert!(deny.contains(&"C:/Windows/**".to_string()));
    assert!(
        !deny.iter().any(|d| d.is_empty()),
        "no built-in denylist entry may be empty"
    );
}

#[test]
fn test_compiled_dir_lists_is_allowed_and_is_denied() {
    let compiled = CompiledDirLists {
        allowlist: globset(&["src/**", "*.rs"]),
        denylist: globset(&["/etc/**", "secrets/**"]),
    };
    assert!(compiled.is_allowed("src/main.rs"));
    assert!(compiled.is_allowed("lib.rs"));
    assert!(!compiled.is_allowed("docs/readme.md"));
    assert!(compiled.is_denied("/etc/passwd"));
    assert!(compiled.is_denied("secrets/key.pem"));
    assert!(!compiled.is_denied("src/main.rs"));
}

#[test]
fn test_dir_lists_derives_and_default() {
    let lists = DirLists::default();
    assert_eq!(lists.allowlist, Vec::<String>::new());
    assert_eq!(lists.denylist, Vec::<String>::new());
    assert_eq!(lists.allowed_roots, Vec::<String>::new());

    let lists = DirLists {
        allowlist: vec!["a".to_string()],
        denylist: vec!["b".to_string()],
        allowed_roots: vec!["/root".to_string()],
    };
    let cloned = lists.clone();
    assert_eq!(cloned.allowlist, lists.allowlist);
    assert!(format!("{lists:?}").contains("allowed_roots"));
}

#[test]
fn test_merged_dir_lists_appends_builtins_dedup_and_keeps_roots() {
    let mut cfg = Config::default();
    // One user entry duplicates a built-in; the other is unique.
    cfg.dirs.denylist = vec!["/etc/**".to_string(), "mysecret/**".to_string()];
    cfg.dirs.allowlist = vec!["src/**".to_string()];
    cfg.dirs.allowed_roots = vec!["/extra/root".to_string()];

    let lists = dir_lists::merged_dir_lists_for(&cfg);

    assert_eq!(lists.allowlist, vec!["src/**".to_string()]);
    assert_eq!(lists.allowed_roots, vec!["/extra/root".to_string()]);
    // The pre-existing duplicate is not repeated.
    assert_eq!(
        lists
            .denylist
            .iter()
            .filter(|d| d.as_str() == "/etc/**")
            .count(),
        1,
        "a built-in already present in the user list must not be duplicated"
    );
    // Every built-in is present, plus the user entry.
    for builtin in BUILTIN_DENYLIST {
        assert!(
            lists.denylist.iter().any(|d| d == builtin),
            "built-in {builtin} must be enforced"
        );
    }
    assert_eq!(lists.denylist.len(), BUILTIN_DENYLIST.len() + 1);
}

#[test]
fn test_builtin_only_dir_lists_has_no_user_entries() {
    let lists = dir_lists::builtin_only_dir_lists();
    assert_eq!(lists.allowlist, Vec::<String>::new());
    assert_eq!(lists.allowed_roots, Vec::<String>::new());
    assert_eq!(lists.denylist.len(), BUILTIN_DENYLIST.len());
    assert!(lists.denylist.contains(&"/boot/**".to_string()));
}

#[test]
#[serial_test::serial]
fn test_load_from_config_populates_lists_and_compiled_caches() {
    let _sandbox = Sandbox::new();

    dir_lists::load_from_config();

    let allow = dir_lists::get_allowlist();
    let deny = dir_lists::get_denylist();
    let roots = dir_lists::get_allowed_roots();
    assert!(allow.is_empty(), "default config has no dir allowlist");
    assert!(roots.is_empty(), "default config has no allowed_roots");
    for builtin in BUILTIN_DENYLIST {
        assert!(
            deny.iter().any(|d| d == builtin),
            "load_from_config must enforce built-in {builtin}"
        );
    }

    // The compiled denylist is populated from the merged lists.
    let compiled = dir_lists::get_compiled_denylist();
    assert!(compiled.is_match("/etc/passwd"));
    assert!(!compiled.is_match("/home/user/notes.txt"));
    // The default allowlist is empty, so nothing matches.
    assert!(!dir_lists::get_compiled_allowlist().is_match("anything.txt"));
}

#[test]
#[serial_test::serial]
fn test_add_and_remove_allowlist_round_trip() {
    let _sandbox = Sandbox::new();
    let pattern = "zz_allow_probe/**";

    assert!(
        !dir_lists::get_allowlist().iter().any(|e| e == pattern),
        "precondition: probe pattern must be absent"
    );

    dir_lists::add_allowlist(pattern, Scope::Project).expect("add allowlist");
    assert!(dir_lists::get_allowlist().iter().any(|e| e == pattern));
    // The compiled cache is recompiled by `add_allowlist`.
    assert!(dir_lists::get_compiled_allowlist().is_match("zz_allow_probe/x/y.rs"));

    // Re-adding is idempotent.
    dir_lists::add_allowlist(pattern, Scope::Project).expect("re-add allowlist");
    assert_eq!(
        dir_lists::get_allowlist()
            .iter()
            .filter(|e| e.as_str() == pattern)
            .count(),
        1
    );

    // The mutation was persisted to the project config file.
    let raw = std::fs::read_to_string(".ragent/ragent.json").expect("read project config");
    assert!(
        raw.contains(pattern),
        "persisted config must contain {pattern}"
    );

    assert!(
        dir_lists::remove_allowlist(pattern, Scope::Project).expect("remove allowlist"),
        "removing a present entry reports removed"
    );
    assert!(!dir_lists::get_allowlist().iter().any(|e| e == pattern));
    assert!(
        !dir_lists::remove_allowlist(pattern, Scope::Project).expect("remove again"),
        "removing an absent entry reports not-removed"
    );
}

#[test]
#[serial_test::serial]
fn test_add_and_remove_denylist_round_trip_and_invalid_glob() {
    let _sandbox = Sandbox::new();
    let pattern = "zz_deny_probe/**";

    dir_lists::add_denylist(pattern, Scope::Project).expect("add denylist");
    assert!(dir_lists::get_denylist().iter().any(|e| e == pattern));
    assert!(dir_lists::get_compiled_denylist().is_match("zz_deny_probe/secret"));

    // Re-adding is idempotent.
    dir_lists::add_denylist(pattern, Scope::Project).expect("re-add denylist");
    assert_eq!(
        dir_lists::get_denylist()
            .iter()
            .filter(|e| e.as_str() == pattern)
            .count(),
        1
    );

    // An unparseable glob is skipped by `compile_patterns` rather than aborting.
    dir_lists::add_denylist("[unterminated", Scope::Project).expect("add invalid glob");
    assert!(
        dir_lists::get_denylist()
            .iter()
            .any(|e| e == "[unterminated")
    );

    assert!(dir_lists::remove_denylist(pattern, Scope::Project).expect("remove denylist"));
    assert!(!dir_lists::get_denylist().iter().any(|e| e == pattern));
    assert!(!dir_lists::remove_denylist(pattern, Scope::Project).expect("remove again"));
}

#[test]
#[serial_test::serial]
fn test_add_and_remove_allowed_roots_round_trip() {
    let _sandbox = Sandbox::new();
    let root = "/tmp/zz_allowed_root_probe";

    dir_lists::add_allowed_roots(root, Scope::Project).expect("add root");
    assert!(dir_lists::get_allowed_roots().iter().any(|e| e == root));

    // Re-adding is idempotent.
    dir_lists::add_allowed_roots(root, Scope::Project).expect("re-add root");
    assert_eq!(
        dir_lists::get_allowed_roots()
            .iter()
            .filter(|e| e.as_str() == root)
            .count(),
        1
    );

    assert!(dir_lists::remove_allowed_roots(root, Scope::Project).expect("remove root"));
    assert!(!dir_lists::get_allowed_roots().iter().any(|e| e == root));
    assert!(!dir_lists::remove_allowed_roots(root, Scope::Project).expect("remove again"));
}

#[test]
#[serial_test::serial]
fn test_invalidate_compiled_caches_rebuilds_from_memory() {
    let _sandbox = Sandbox::new();
    let pattern = "zz_invalidate_probe/**";

    dir_lists::add_allowlist(pattern, Scope::Project).expect("add allowlist");
    // Wipe then rebuild: the recompile reads the in-memory pattern list.
    dir_lists::invalidate_compiled_caches();
    assert!(dir_lists::get_compiled_allowlist().is_match("zz_invalidate_probe/a.rs"));
    assert!(dir_lists::get_allowlist().iter().any(|e| e == pattern));

    // `get_compiled_*` returns independent `Arc`s (cloning the handle is cheap).
    let first = dir_lists::get_compiled_allowlist();
    let second = dir_lists::get_compiled_allowlist();
    assert!(std::sync::Arc::ptr_eq(&first, &second));

    dir_lists::remove_allowlist(pattern, Scope::Project).expect("cleanup");
}
