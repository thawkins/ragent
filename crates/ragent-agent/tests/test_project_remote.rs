//! Remote descriptions for projects (spec `projects`, T-004).

#[path = "support/project_fixture.rs"]
mod project_fixture;

use project_fixture::{fixture_dir, git, init_repo};
use ragent_agent::project::{RemoteKind, detect_remote, parse_remote_url};

#[test]
fn test_project_remote_github_https_and_ssh() {
    for url in [
        "https://github.com/acme/app.git",
        "https://github.com/acme/app",
        "https://www.github.com/acme/app/",
        "git@github.com:acme/app.git",
        "ssh://git@github.com/acme/app.git",
    ] {
        let remote = parse_remote_url(url, "origin", None)
            .unwrap_or_else(|| panic!("{url} should be recognised"));
        assert_eq!(remote.kind, RemoteKind::GitHub, "{url}");
        assert_eq!(remote.path, "acme/app", "{url}");
        assert_eq!(remote.remote_name, "origin");
        assert_eq!(remote.default_branch, None);
    }
    assert_eq!(
        parse_remote_url("https://github.com/acme/app.git", "origin", None)
            .expect("remote")
            .host,
        "github.com"
    );
}

#[test]
fn test_project_remote_gitlab_com_and_nested_groups() {
    let remote = parse_remote_url("git@gitlab.com:group/sub/project.git", "upstream", None)
        .expect("gitlab.com ssh");
    assert_eq!(remote.kind, RemoteKind::GitLab);
    assert_eq!(remote.host, "gitlab.com");
    assert_eq!(remote.path, "group/sub/project");
    assert_eq!(remote.remote_name, "upstream");

    let https =
        parse_remote_url("https://gitlab.com/group/project.git", "origin", None).expect("https");
    assert_eq!(https.kind, RemoteKind::GitLab);
    assert_eq!(https.path, "group/project");
}

#[test]
fn test_project_remote_self_hosted_gitlab() {
    // A host named gitlab.* is GitLab without any configuration.
    let named = parse_remote_url(
        "https://gitlab.example.com:8443/team/platform/api.git",
        "origin",
        None,
    )
    .expect("gitlab.example.com");
    assert_eq!(named.kind, RemoteKind::GitLab);
    assert_eq!(named.host, "gitlab.example.com:8443");
    assert_eq!(named.path, "team/platform/api");

    // Any other host is GitLab only when it is the configured instance.
    let url = "git@code.example.org:team/api.git";
    assert!(parse_remote_url(url, "origin", None).is_none());
    for configured in [
        "https://code.example.org",
        "code.example.org",
        "https://code.example.org/",
    ] {
        let remote = parse_remote_url(url, "origin", Some(configured))
            .unwrap_or_else(|| panic!("configured as {configured}"));
        assert_eq!(remote.kind, RemoteKind::GitLab);
        assert_eq!(remote.host, "code.example.org");
    }
}

#[test]
fn test_project_remote_unknown_hosts_and_bad_urls_yield_none() {
    for url in [
        "https://example.com/acme/app.git",
        "https://bitbucket.org/acme/app.git",
        "https://github.com.evil.example/acme/app.git",
        "https://notgithub.com/acme/app.git",
        "/home/user/repos/app.git",
        "../app",
        "https://github.com/acme",
        "https://github.com/acme/app/extra",
        "https://gitlab.com/onlyone",
        "",
    ] {
        assert!(
            parse_remote_url(url, "origin", None).is_none(),
            "{url:?} must not be described as a remote"
        );
    }
}

#[test]
fn test_project_remote_strips_embedded_credentials() {
    let token = parse_remote_url(
        "https://x-access-token:ghp_secret123@github.com/acme/app.git",
        "origin",
        None,
    )
    .expect("remote");
    assert_eq!(token.url, "https://github.com/acme/app.git");
    assert_eq!(token.host, "github.com");

    let user_only = parse_remote_url(
        "https://someone@gitlab.com/group/project.git",
        "origin",
        None,
    )
    .expect("remote");
    assert_eq!(user_only.url, "https://gitlab.com/group/project.git");

    // The ssh login is not a secret and is needed to use the URL.
    let ssh = parse_remote_url("git@github.com:acme/app.git", "origin", None).expect("remote");
    assert_eq!(ssh.url, "git@github.com:acme/app.git");
    let ssh_url =
        parse_remote_url("ssh://git:pw@github.com/acme/app", "origin", None).expect("remote");
    assert_eq!(ssh_url.url, "ssh://git@github.com/acme/app.git");

    for remote in [token, user_only, ssh_url] {
        assert!(!remote.url.contains("ghp_secret123"));
        assert!(!remote.url.contains(":pw@"));
    }
}

#[test]
fn test_project_remote_detects_origin_and_default_branch() {
    let repo = fixture_dir("remote_detect").join("repo");
    init_repo(&repo);
    assert!(
        detect_remote(&repo, None).is_none(),
        "a repository with no origin has no remote"
    );

    git(
        &repo,
        &[
            "remote",
            "add",
            "origin",
            "https://tok:secret@github.com/acme/app.git",
        ],
    );
    let remote = detect_remote(&repo, None).expect("origin detected");
    assert_eq!(remote.kind, RemoteKind::GitHub);
    assert_eq!(remote.path, "acme/app");
    assert_eq!(remote.url, "https://github.com/acme/app.git");
    assert_eq!(remote.default_branch, None, "origin/HEAD is not known yet");

    git(&repo, &["update-ref", "refs/remotes/origin/trunk", "HEAD"]);
    git(
        &repo,
        &[
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/trunk",
        ],
    );
    let remote = detect_remote(&repo, None).expect("origin detected");
    assert_eq!(remote.default_branch.as_deref(), Some("trunk"));
}

#[test]
fn test_project_remote_none_outside_a_checkout() {
    assert!(detect_remote(std::path::Path::new("/"), None).is_none());
}
