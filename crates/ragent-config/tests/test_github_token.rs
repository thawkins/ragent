//! Tests for the shared GitHub credential chain
//! (`ragent_config::github`, spec `newproj` FR-008).
//!
//! The pure precedence helper ([`choose_token`]) is exercised without any
//! process state; the environment/file readers are tested serially because
//! they mutate process-global state (`GITHUB_TOKEN`, `XDG_CONFIG_HOME`) and
//! `std::env::set_var` is `unsafe` in Rust 2024, so this target opts back in
//! explicitly (env mutation is contained to the test binary).

#![allow(unsafe_code)]

use ragent_config::github::{
    NO_GH_CLI_ENV, choose_token, env_token, is_app_token, resolve_from_stored,
    resolve_github_token, stored_token,
};

// ---------------------------------------------------------------- pure logic ---

#[test]
fn test_is_app_token_classifies_prefixes() {
    assert!(is_app_token("ghu_user_to_server"));
    assert!(is_app_token("ghs_server_to_server"));
    assert!(!is_app_token("ghp_classic_pat"));
    assert!(!is_app_token("gho_oauth"));
    assert!(!is_app_token("github_pat_fine_grained"));
    assert!(!is_app_token(""));
}

#[test]
fn test_choose_token_viable_stored_wins_over_gh() {
    // A repo-capable stored token is used as-is; the CLI credential is ignored.
    assert_eq!(
        choose_token(Some("ghp_pat".to_owned()), Some("gho_gh".to_owned())),
        Some("ghp_pat".to_owned())
    );
}

#[test]
fn test_choose_token_app_token_defers_to_gh() {
    // A GitHub App token cannot create repositories, so the `gh` credential
    // (repo scope) wins when it is present.
    assert_eq!(
        choose_token(Some("ghu_app".to_owned()), Some("gho_gh".to_owned())),
        Some("gho_gh".to_owned())
    );
    assert_eq!(
        choose_token(Some("ghs_app".to_owned()), Some("gho_gh".to_owned())),
        Some("gho_gh".to_owned())
    );
}

#[test]
fn test_choose_token_app_token_without_gh_is_kept() {
    // No `gh` credential: keep the app token so read-only GitHub tooling
    // (issues, PRs, /reverse) still authenticates.
    assert_eq!(
        choose_token(Some("ghu_app".to_owned()), None),
        Some("ghu_app".to_owned())
    );
}

#[test]
fn test_choose_token_uses_gh_when_no_stored_token() {
    assert_eq!(
        choose_token(None, Some("gho_gh".to_owned())),
        Some("gho_gh".to_owned())
    );
    assert_eq!(choose_token(None, None), None);
}

// ------------------------------------------------------------- env + file ---

#[test]
#[serial_test::serial]
fn test_env_token_blank_is_ignored() {
    let saved = std::env::var("GITHUB_TOKEN").ok();
    unsafe {
        std::env::set_var("GITHUB_TOKEN", "   ");
    }
    assert_eq!(env_token(), None, "whitespace-only token must be ignored");
    unsafe {
        std::env::set_var("GITHUB_TOKEN", "ghp_from_env");
    }
    assert_eq!(env_token().as_deref(), Some("ghp_from_env"));
    restore("GITHUB_TOKEN", saved);
}

#[test]
#[serial_test::serial]
fn test_stored_token_reads_config_file() {
    let temp = tempfile::tempdir().expect("tempdir");
    let dir = temp.path().join("ragent");
    std::fs::create_dir_all(&dir).expect("config dir");
    std::fs::write(dir.join("github_token"), "ghu_stored\n").expect("write token");

    let saved_xdg = std::env::var("XDG_CONFIG_HOME").ok();
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", temp.path());
    }
    assert_eq!(
        stored_token().as_deref(),
        Some("ghu_stored"),
        "stored token is trimmed from <xdg>/ragent/github_token"
    );
    restore("XDG_CONFIG_HOME", saved_xdg);
}

#[test]
#[serial_test::serial]
fn test_resolve_github_token_prefers_env_then_stored() {
    let temp = tempfile::tempdir().expect("tempdir");
    let dir = temp.path().join("ragent");
    std::fs::create_dir_all(&dir).expect("config dir");
    std::fs::write(dir.join("github_token"), "ghp_from_file").expect("write token");

    let saved_xdg = std::env::var("XDG_CONFIG_HOME").ok();
    let saved_token = std::env::var("GITHUB_TOKEN").ok();
    let saved_no_gh = std::env::var(NO_GH_CLI_ENV).ok();
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", temp.path());
        std::env::set_var(NO_GH_CLI_ENV, "1");
        std::env::remove_var("GITHUB_TOKEN");
    }

    // No env token → the stored file is used.
    assert_eq!(resolve_github_token().as_deref(), Some("ghp_from_file"));

    // The environment variable wins outright.
    unsafe {
        std::env::set_var("GITHUB_TOKEN", "ghp_from_env");
    }
    assert_eq!(resolve_github_token().as_deref(), Some("ghp_from_env"));

    restore("GITHUB_TOKEN", saved_token);
    restore(NO_GH_CLI_ENV, saved_no_gh);
    restore("XDG_CONFIG_HOME", saved_xdg);
}

#[test]
#[serial_test::serial]
fn test_resolve_from_stored_skips_gh_for_viable_token() {
    // A viable stored token is returned without spawning `gh`, even with the
    // CLI fallback enabled.
    let saved_no_gh = std::env::var(NO_GH_CLI_ENV).ok();
    unsafe {
        std::env::remove_var(NO_GH_CLI_ENV);
    }
    assert_eq!(
        resolve_from_stored(Some("ghp_viable".to_owned())).as_deref(),
        Some("ghp_viable")
    );
    restore(NO_GH_CLI_ENV, saved_no_gh);
}

#[test]
#[serial_test::serial]
fn test_resolve_from_stored_keeps_app_token_when_gh_disabled() {
    // The `gh` fallback is disabled, so an app token is returned unchanged
    // (read-only GitHub tooling keeps working) rather than resolving to None.
    let saved_no_gh = std::env::var(NO_GH_CLI_ENV).ok();
    unsafe {
        std::env::set_var(NO_GH_CLI_ENV, "1");
    }
    assert_eq!(
        resolve_from_stored(Some("ghu_app".to_owned())).as_deref(),
        Some("ghu_app")
    );
    assert_eq!(resolve_from_stored(None), None);
    restore(NO_GH_CLI_ENV, saved_no_gh);
}

/// Live check of the app-token downgrade with the real `gh` CLI (spec
/// `newproj` FR-008): a stored GitHub App token must defer to `gh auth token`
/// when the CLI is authenticated. Skipped when `gh` is not installed or not
/// logged in, so it is a no-op on a machine without the CLI.
#[test]
#[serial_test::serial]
fn test_resolve_github_token_defers_app_token_to_gh_cli() {
    let temp = tempfile::tempdir().expect("tempdir");
    let dir = temp.path().join("ragent");
    std::fs::create_dir_all(&dir).expect("config dir");
    // Synthetic GitHub App token: the shape of the real stored credential.
    std::fs::write(dir.join("github_token"), "ghu_synthetic_app_token").expect("write token");

    let saved_xdg = std::env::var("XDG_CONFIG_HOME").ok();
    let saved_token = std::env::var("GITHUB_TOKEN").ok();
    let saved_no_gh = std::env::var(NO_GH_CLI_ENV).ok();
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", temp.path());
        std::env::remove_var("GITHUB_TOKEN");
        std::env::remove_var(NO_GH_CLI_ENV);
    }

    let gh = ragent_config::github::gh_cli_token();
    let resolved = resolve_github_token();
    if gh.is_none() {
        eprintln!("skipping: `gh` CLI not authenticated in this environment");
        assert_eq!(
            resolved.as_deref(),
            Some("ghu_synthetic_app_token"),
            "with no gh credential the app token is kept for read-only use"
        );
    } else {
        assert_eq!(
            resolved, gh,
            "an app token must defer to the gh CLI credential so repo creation works"
        );
    }

    restore("GITHUB_TOKEN", saved_token);
    restore(NO_GH_CLI_ENV, saved_no_gh);
    restore("XDG_CONFIG_HOME", saved_xdg);
}

/// Restore an environment variable to a previously-captured value.
fn restore(key: &str, value: Option<String>) {
    unsafe {
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }
}
