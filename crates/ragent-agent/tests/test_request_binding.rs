//! Request lookup, session binding and GitHub Enterprise routing (spec
//! SESSIONMENU M4, T4.1-T4.3), against a local mock GitHub/GitLab API.

#![allow(unsafe_code)] // Rust 2024: env::set_var/remove_var are unsafe.

use ragent_agent::project::requests::{
    BindOutcome, BindTarget, PullRequestCounts, RequestSource, RequestState, bind_session,
    get_request, open_requests_for_branch,
};
use ragent_agent::project::{ProjectRemote, RemoteKind, parse_remote_url_with_hosts};
use ragent_config::GitHubHost;
use ragent_storage::{SessionKind, Storage};
use ragent_tools_vcs::github::client::GitHubClient;
use ragent_tools_vcs::gitlab::GitLabClient;
use serde_json::{Value, json};
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn remote(kind: RemoteKind, host: &str, repo_path: &str) -> ProjectRemote {
    ProjectRemote {
        kind,
        host: host.to_string(),
        path: repo_path.to_string(),
        url: String::new(),
        remote_name: "origin".to_string(),
        default_branch: Some("main".to_string()),
    }
}

fn ghes_hosts() -> Vec<GitHubHost> {
    vec![GitHubHost {
        host: "github.example.com".to_string(),
        api_url: "https://github.example.com/api/v3".to_string(),
    }]
}

fn github_pull(number: u64, branch: &str, head_repo: &str) -> Value {
    json!({
        "number": number,
        "state": "open",
        "title": format!("PR {number}"),
        "html_url": format!("https://github.com/acme/app/pull/{number}"),
        "merged_at": null,
        "head": { "ref": branch, "repo": { "full_name": head_repo } }
    })
}

#[test]
fn test_request_binding_noun_per_forge() {
    let pr = RemoteKind::GitHub.noun();
    assert_eq!((pr.short, pr.long, pr.sigil), ("PR", "pull request", '#'));
    let mr = RemoteKind::GitLab.noun();
    assert_eq!((mr.short, mr.long, mr.sigil), ("MR", "merge request", '!'));
}

#[test]
fn test_request_binding_enterprise_host_is_github() {
    let hosts = ghes_hosts();
    for url in [
        "git@github.example.com:acme/app.git",
        "https://github.example.com/acme/app.git",
        "ssh://git@github.example.com:2222/acme/app.git",
    ] {
        let remote = parse_remote_url_with_hosts(url, "origin", None, &hosts)
            .unwrap_or_else(|| panic!("{url} is a configured GitHub host"));
        assert_eq!(remote.kind, RemoteKind::GitHub, "{url}");
        assert_eq!(remote.path, "acme/app", "{url}");
    }
    assert!(
        parse_remote_url_with_hosts("git@github.example.com:acme/app.git", "origin", None, &[])
            .is_none(),
        "an unconfigured host stays unknown"
    );
}

#[tokio::test]
async fn test_request_binding_github_open_requests_for_branch() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/app/pulls"))
        .and(query_param("state", "open"))
        .and(query_param("head", "acme:feature/x"))
        .and(header("authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            github_pull(4, "feature/x", "acme/app"),
            github_pull(5, "feature/x", "someone/fork"),
        ])))
        .expect(1)
        .mount(&server)
        .await;
    let source = RequestSource::GitHub(GitHubClient::with_base_url(
        server.uri(),
        "test-token".to_string(),
    ));

    let open = open_requests_for_branch(
        &source,
        &remote(RemoteKind::GitHub, "github.com", "acme/app"),
        "feature/x",
    )
    .await
    .expect("lookup");
    assert_eq!(
        open.len(),
        1,
        "a fork's branch of the same name is left out"
    );
    assert_eq!(open[0].number, 4);
    assert_eq!(open[0].state, RequestState::Open);
    assert_eq!(open[0].source_branch, "feature/x");
}

#[tokio::test]
async fn test_request_binding_github_enterprise_api_prefix() {
    let server = MockServer::start().await;
    let mut merged = github_pull(9, "fix", "acme/app");
    merged["state"] = json!("closed");
    merged["merged_at"] = json!("2026-01-01T00:00:00Z");
    Mock::given(method("GET"))
        .and(path("/api/v3/repos/acme/app/pulls/9"))
        .respond_with(ResponseTemplate::new(200).set_body_json(merged))
        .expect(1)
        .mount(&server)
        .await;
    let source = RequestSource::GitHub(GitHubClient::with_base_url(
        format!("{}/api/v3", server.uri()),
        "ghe-token".to_string(),
    ));

    let info = get_request(
        &source,
        &remote(RemoteKind::GitHub, "github.example.com", "acme/app"),
        9,
    )
    .await
    .expect("request");
    assert_eq!(info.number, 9);
    assert_eq!(info.state, RequestState::Merged);
}

#[tokio::test]
async fn test_request_binding_gitlab_self_managed_get_request() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(
            "/api/v4/projects/group%2Fsub%2Fproject/merge_requests/12",
        ))
        .and(header("PRIVATE-TOKEN", "glpat-test"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "iid": 12, "state": "closed", "title": "Tidy",
            "web_url": "https://gitlab.example.com/group/sub/project/-/merge_requests/12",
            "source_branch": "tidy"
        })))
        .expect(1)
        .mount(&server)
        .await;
    let source = RequestSource::GitLab(GitLabClient::with_credentials(
        server.uri(),
        "glpat-test".to_string(),
    ));

    let info = get_request(
        &source,
        &remote(
            RemoteKind::GitLab,
            "gitlab.example.com",
            "group/sub/project",
        ),
        12,
    )
    .await
    .expect("request");
    assert_eq!(info.state, RequestState::Closed);
    assert_eq!(info.title, "Tidy");
    assert_eq!(info.source_branch, "tidy");
}

#[tokio::test]
async fn test_request_binding_bind_branch_stores_on_top_level_session() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Fproject/merge_requests"))
        .and(query_param("state", "opened"))
        .and(query_param("source_branch", "feature/y"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "iid": 3, "state": "opened", "title": "Y",
            "web_url": "https://gitlab.example.com/group/project/-/merge_requests/3",
            "source_branch": "feature/y", "source_project_id": 1, "target_project_id": 1
        }])))
        .mount(&server)
        .await;
    let source = RequestSource::GitLab(GitLabClient::with_credentials(
        server.uri(),
        "glpat-test".to_string(),
    ));
    let storage = Storage::open_in_memory().expect("storage");
    storage
        .create_session("lead", "/nonexistent/bind")
        .expect("lead");
    storage
        .create_session_with(
            "child",
            "/nonexistent/bind",
            Some("lead"),
            SessionKind::Subagent,
        )
        .expect("child");
    let gitlab = remote(RemoteKind::GitLab, "gitlab.example.com", "group/project");

    let outcome = bind_session(
        &storage,
        "child",
        &source,
        &gitlab,
        BindTarget::Branch("feature/y".to_string()),
        false,
    )
    .await
    .expect("bind");
    assert!(matches!(outcome, BindOutcome::Bound(ref info) if info.number == 3));
    let bound = storage
        .session_request("lead")
        .expect("read")
        .expect("bound to the top-level session");
    assert_eq!(bound.forge, "gitlab");
    assert_eq!(bound.host, "gitlab.example.com");
    assert_eq!(bound.repo, "group/project");
    assert_eq!(bound.head_branch, "feature/y");
    assert_eq!(bound.state, "open");
    assert!(storage.session_request("child").expect("read").is_none());
}

#[tokio::test]
async fn test_request_binding_bind_branch_none_or_several_binds_nothing() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/app/pulls"))
        .and(query_param("head", "acme:empty"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/app/pulls"))
        .and(query_param("head", "acme:busy"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            github_pull(1, "busy", "acme/app"),
            github_pull(2, "busy", "acme/app"),
        ])))
        .mount(&server)
        .await;
    let source = RequestSource::GitHub(GitHubClient::with_base_url(
        server.uri(),
        "test-token".to_string(),
    ));
    let storage = Storage::open_in_memory().expect("storage");
    storage.create_session("s", "/nonexistent/bind").expect("s");
    let github = remote(RemoteKind::GitHub, "github.com", "acme/app");

    let none = bind_session(
        &storage,
        "s",
        &source,
        &github,
        BindTarget::Branch("empty".into()),
        false,
    )
    .await
    .expect("lookup");
    assert_eq!(none, BindOutcome::NoneOpen);
    let several = bind_session(
        &storage,
        "s",
        &source,
        &github,
        BindTarget::Branch("busy".into()),
        false,
    )
    .await
    .expect("lookup");
    assert!(matches!(several, BindOutcome::Several(ref open) if open.len() == 2));
    assert!(storage.session_request("s").expect("read").is_none());
}

#[test]
fn test_request_binding_enterprise_host_uses_only_enterprise_token() {
    let _env = crate::global_env_lock();
    let storage = std::sync::Arc::new(Storage::open_in_memory().expect("storage"));
    let events = std::sync::Arc::new(ragent_agent::event::EventBus::default());
    let service = PullRequestCounts::new(storage, events).with_github_hosts(ghes_hosts());
    let enterprise = remote(RemoteKind::GitHub, "github.example.com", "acme/app");
    let unknown = remote(RemoteKind::GitHub, "github.other.example", "acme/app");
    let saved: Vec<(&str, Option<String>)> = ["GH_ENTERPRISE_TOKEN", "GITHUB_ENTERPRISE_TOKEN"]
        .iter()
        .map(|k| (*k, std::env::var(k).ok()))
        .collect();

    unsafe {
        std::env::remove_var("GH_ENTERPRISE_TOKEN");
        std::env::remove_var("GITHUB_ENTERPRISE_TOKEN");
    }
    let without = service.source_for(&enterprise);
    unsafe { std::env::set_var("GITHUB_ENTERPRISE_TOKEN", "ghe-test") };
    let with = service.source_for(&enterprise);
    let other_host = service.source_for(&unknown);
    for (key, value) in saved {
        match value {
            Some(v) => unsafe { std::env::set_var(key, v) },
            None => unsafe { std::env::remove_var(key) },
        }
    }

    assert!(
        without.is_none(),
        "the github.com token is never sent to an enterprise host"
    );
    assert!(matches!(with, Some(RequestSource::GitHub(_))));
    assert!(other_host.is_none(), "an unconfigured host gets no client");
}
