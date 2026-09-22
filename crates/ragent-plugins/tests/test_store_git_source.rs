//! Tests for the `git+` source handling added to `/plugins add`: a shallow,
//! sparse `git clone` of a `git+<https-url>#<ref>[:<subpath>]` source, the form
//! the store providers emit for vendor marketplace entries (spec `plugins`
//! T-006; FR-007, FR-010, FR-024).
//!
//! Every test is offline: the "remote" is a local git repository produced with
//! `git init` in a scratch directory and addressed through a `file://` URL. The
//! tests drive the parser and the end-to-end install against that local remote,
//! so no network is contacted and no real marketplace repository is cloned.

use std::path::{Path, PathBuf};

use ragent_plugins::{AddError, StoreDirs, add, parse_git_source};

static TEMP_SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// A unique scratch directory under `target/temp/` (no `/tmp`, per AGENTS.md).
fn temp_dir(name: &str) -> PathBuf {
    let unique = TEMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/temp/plugin-store-git/{name}-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir_all(&path).expect("temp dir creatable");
    path
}

/// Run git in `dir`, panicking on failure (test-only helper).
fn git(dir: &Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Build a local git repository containing `subpath` as a Codex plugin, and
/// return its `file://` remote URL plus a working tree root.
fn remote_with_plugin(root: &Path, subpath: &str, id: &str) -> (String, PathBuf) {
    let remote = root.join("remote");
    let plugin_dir = if subpath == "." {
        remote.clone()
    } else {
        remote.join(subpath)
    };
    std::fs::create_dir_all(&plugin_dir).expect("remote subdir");
    std::fs::write(
        plugin_dir.join("codex-plugin.json"),
        format!(r#"{{ "name": "{id}", "version": "1.0.0", "entry": "index.js" }}"#),
    )
    .expect("manifest");
    std::fs::write(plugin_dir.join("index.js"), "// entry").expect("entry");
    git(&remote, &["init", "-q"]);
    git(&remote, &["config", "user.email", "t@example.org"]);
    git(&remote, &["config", "user.name", "Test"]);
    git(&remote, &["add", "-A"]);
    git(&remote, &["commit", "-q", "-m", "plugin"]);
    let url = format!("file://{}", remote.display());
    (url, remote)
}

/// Store directories rooted at a scratch store, so installs never touch the real
/// user store.
fn store_dirs(root: &Path) -> StoreDirs {
    StoreDirs {
        project: Some(root.join("store")),
        global: None,
    }
}

// ── Parser ──────────────────────────────────────────────────────────────────

#[test]
fn parse_git_source_reads_url_ref_and_subpath() {
    let spec = parse_git_source("git+https://github.com/awslabs/agent-plugins#main:plugins/aws")
        .expect("a well-formed git source parses");
    assert_eq!(spec.url, "https://github.com/awslabs/agent-plugins");
    assert_eq!(spec.ref_name.as_deref(), Some("main"));
    assert_eq!(spec.subpath.as_deref(), Some("plugins/aws"));
}

#[test]
fn parse_git_source_treats_head_and_empty_ref_as_the_default_branch() {
    for raw in [
        "git+https://x/y#HEAD",
        "git+https://x/y#",
        "git+https://x/y",
    ] {
        let spec = parse_git_source(raw).expect("parses");
        assert!(spec.ref_name.is_none(), "{raw} uses the default branch");
        assert!(spec.subpath.is_none(), "{raw} has no subpath");
    }
}

#[test]
fn parse_git_source_refuses_non_https_remotes() {
    for raw in [
        "git+http://x/y#main",
        "git+git@github.com:x/y.git#main",
        "git+ssh://git@x/y#main",
    ] {
        assert!(
            parse_git_source(raw).is_none(),
            "{raw} must be refused (FR-024)"
        );
    }
}

#[test]
fn parse_git_source_normalises_a_dot_slash_subpath() {
    let spec = parse_git_source("git+https://x/y#main:./plugins/z").expect("parses");
    assert_eq!(spec.subpath.as_deref(), Some("plugins/z"));
}

// ── End-to-end install through `add` (offline, file:// remote) ──────────────

#[test]
fn add_installs_a_plugin_from_a_git_subdirectory_source() {
    let temp = temp_dir("git-subdir");
    let (url, _remote) = remote_with_plugin(&temp, "plugins/weather", "git-weather");
    let dirs = store_dirs(&temp);

    let source = format!("git+{url}#HEAD:plugins/weather");
    let outcome = add(&dirs, &temp, &source, false).expect("the plugin installs");

    assert_eq!(outcome.parsed.descriptor.id, "git-weather");
    assert!(
        dirs.project
            .as_ref()
            .expect("project store")
            .join("git-weather")
            .is_dir(),
        "the plugin was committed into the store"
    );
}

#[test]
fn add_installs_a_plugin_from_a_whole_repo_git_source() {
    let temp = temp_dir("git-whole");
    // The plugin manifest sits at the repository root.
    let (url, _remote) = remote_with_plugin(&temp, ".", "git-root-plugin");
    let dirs = store_dirs(&temp);

    let source = format!("git+{url}#HEAD");
    let outcome = add(&dirs, &temp, &source, false).expect("the root plugin installs");
    assert_eq!(outcome.parsed.descriptor.id, "git-root-plugin");
}

#[test]
fn add_refuses_a_non_https_git_source_without_cloning() {
    let temp = temp_dir("git-refuse");
    let dirs = store_dirs(&temp);
    let err = add(&dirs, &temp, "git+http://example.org/x.git#main", false)
        .expect_err("a non-https git source is refused");
    assert!(
        matches!(err, AddError::UnknownSource(_)),
        "expected UnknownSource, got {err:?}"
    );
    // Nothing was written into the store.
    assert!(
        !dirs
            .project
            .as_ref()
            .expect("store")
            .join(".add-staging")
            .exists()
    );
}

#[test]
fn add_failed_git_subdir_install_leaves_no_staging_directory() {
    let temp = temp_dir("git-subdir-fail");
    // A repository whose selected subpath exists but is not a plugin.
    let remote = temp.join("remote");
    std::fs::create_dir_all(remote.join("not-a-plugin")).expect("subdir");
    std::fs::write(remote.join("not-a-plugin/readme.md"), "no manifest here").expect("file");
    git(&remote, &["init", "-q"]);
    git(&remote, &["config", "user.email", "t@example.org"]);
    git(&remote, &["config", "user.name", "Test"]);
    git(&remote, &["add", "-A"]);
    git(&remote, &["commit", "-q", "-m", "docs"]);
    let url = format!("file://{}", remote.display());
    let dirs = store_dirs(&temp);

    let source = format!("git+{url}#HEAD:not-a-plugin");
    let err = add(&dirs, &temp, &source, false).expect_err("that subpath is not a plugin");
    assert!(matches!(err, AddError::NotAPlugin(_)), "got {err:?}");

    // The whole per-call staging directory is gone, including the transient git
    // checkout and its `.git` which lives above the selected subpath.
    let staging_root = dirs
        .project
        .as_ref()
        .expect("project store")
        .join(".add-staging");
    assert!(
        !staging_root.exists(),
        "staging must be pruned after a failed git install: {staging_root:?}"
    );
}
