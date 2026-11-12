//! `Config::load_for` (spec MULTISESSION, milestone M6).
//!
//! A session working in another project must read that project's
//! `.ragent/ragent.json`, not the one in the process cwd. Fixtures live under
//! the git-ignored `target/temp/` tree. `XDG_CONFIG_HOME` is pointed at an
//! empty directory so the user's real global config cannot leak in; the env
//! mutation is serialised and restored.

#![allow(unsafe_code)]

use std::path::{Path, PathBuf};

/// A fresh fixture directory unique to `name`.
fn fixture(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/config_load_for")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir); // INTENTIONAL: may not exist yet
    std::fs::create_dir_all(&dir).expect("fixture dir");
    dir.canonicalize().expect("canonical fixture")
}

/// Writes `<project>/.ragent/ragent.json`.
fn write_project_config(project: &Path, json: &str) {
    let dir = project.join(".ragent");
    std::fs::create_dir_all(&dir).expect(".ragent");
    std::fs::write(dir.join("ragent.json"), json).expect("ragent.json");
}

/// Runs `body` with `XDG_CONFIG_HOME` pointed at an empty directory and the
/// config env overrides cleared, restoring all three afterwards.
fn with_isolated_env(root: &Path, body: impl FnOnce()) {
    let keys = ["XDG_CONFIG_HOME", "RAGENT_CONFIG", "RAGENT_CONFIG_CONTENT"];
    let saved: Vec<Option<String>> = keys.iter().map(|k| std::env::var(k).ok()).collect();
    let xdg = root.join("xdg");
    std::fs::create_dir_all(&xdg).expect("xdg dir");
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", &xdg);
        std::env::remove_var("RAGENT_CONFIG");
        std::env::remove_var("RAGENT_CONFIG_CONTENT");
    }
    body();
    unsafe {
        for (key, value) in keys.iter().zip(saved) {
            match value {
                Some(v) => std::env::set_var(key, v),
                None => std::env::remove_var(key),
            }
        }
    }
}

#[test]
#[serial_test::serial]
fn test_config_load_for_reads_the_given_projects_config() {
    let root = fixture("reads_project");
    let project = root.join("other_project");
    write_project_config(&project, r#"{ "default_agent": "plan" }"#);

    with_isolated_env(&root, || {
        let config = ragent_config::Config::load_for(&project).expect("load_for");
        assert_eq!(config.default_agent, "plan");
        assert!(
            config
                .config_paths
                .contains(&project.join(".ragent").join("ragent.json")),
            "the project file is recorded with its full path: {:?}",
            config.config_paths
        );
    });
}

#[test]
#[serial_test::serial]
fn test_config_load_for_sees_an_edited_project_config() {
    let root = fixture("sees_edit");
    let project = root.join("other_project");
    write_project_config(&project, r#"{ "default_agent": "plan" }"#);

    with_isolated_env(&root, || {
        let first = ragent_config::Config::load_for(&project).expect("first load");
        assert_eq!(first.default_agent, "plan");

        // A different size invalidates the cached entry even within one
        // mtime tick.
        write_project_config(&project, r#"{ "default_agent": "general-purpose" }"#);
        let second = ragent_config::Config::load_for(&project).expect("second load");
        assert_eq!(second.default_agent, "general-purpose");
    });
}

#[test]
#[serial_test::serial]
fn test_config_load_for_keeps_projects_apart() {
    let root = fixture("apart");
    let one = root.join("one");
    let two = root.join("two");
    write_project_config(&one, r#"{ "default_agent": "plan" }"#);
    write_project_config(&two, r#"{ "default_agent": "explore" }"#);

    with_isolated_env(&root, || {
        // Interleaved loads must not serve one project's cached config for
        // the other.
        for _ in 0..2 {
            assert_eq!(
                ragent_config::Config::load_for(&one)
                    .expect("one")
                    .default_agent,
                "plan"
            );
            assert_eq!(
                ragent_config::Config::load_for(&two)
                    .expect("two")
                    .default_agent,
                "explore"
            );
        }
    });
}
