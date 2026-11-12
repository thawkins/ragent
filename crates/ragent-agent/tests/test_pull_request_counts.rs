//! Pull/merge request counts per session branch (spec `projects`, T-015),
//! against a local mock GitHub/GitLab API.

#[path = "support/project_fixture.rs"]
mod project_fixture;

use std::path::PathBuf;
use std::time::Duration;

use project_fixture::{Managers, managers};
use ragent_agent::event::Event;
use ragent_agent::project::requests::{
    BASE_INTERVAL, PullRequestCounts, RefreshOutcome, RequestSource, fetch_request_counts,
};
use ragent_agent::project::{Project, ProjectRemote, RemoteKind};
use ragent_storage::SessionCheckout;
use ragent_tools_vcs::github::client::GitHubClient;
use ragent_tools_vcs::gitlab::GitLabClient;
use serde_json::{Value, json};
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn remote(kind: RemoteKind, repo_path: &str) -> ProjectRemote {
    ProjectRemote {
        kind,
        host: "example.invalid".to_string(),
        path: repo_path.to_string(),
        url: String::new(),
        remote_name: "origin".to_string(),
        default_branch: Some("main".to_string()),
    }
}

/// A project with a remote, and one session per given branch (a branch may
/// repeat: several sessions can share one).
fn project_with_sessions(m: &Managers, name: &str, kind: RemoteKind, branches: &[&str]) -> Project {
    let dir = format!("/nonexistent/pr_counts/{name}");
    let mut project = m
        .projects
        .resolve_for_dir(std::path::Path::new(&dir))
        .expect("project");
    project.remote = Some(remote(kind, "acme/app"));
    for branch in branches {
        let checkout = SessionCheckout {
            branch: Some((*branch).to_string()),
            worktree: None,
            pr_count: None,
        };
        m.sessions
            .create_session_in_project(&project.id, PathBuf::from(&dir), &checkout)
            .expect("session");
    }
    project
}

fn github_pull(branch: &str, head_repo: &str) -> Value {
    json!({ "number": 1, "state": "closed",
            "head": { "ref": branch, "repo": { "full_name": head_repo } } })
}

fn github(server: &MockServer) -> RequestSource {
    RequestSource::GitHub(GitHubClient::with_base_url(
        server.uri(),
        "test-token".to_string(),
    ))
}

fn stored_counts(m: &Managers) -> Vec<(String, Option<i64>)> {
    let mut counts: Vec<(String, Option<i64>)> = m
        .storage
        .session_checkouts()
        .expect("checkouts")
        .into_values()
        .map(|c| (c.branch.unwrap_or_default(), c.pr_count))
        .collect();
    counts.sort();
    counts
}

#[tokio::test]
async fn test_pull_request_counts_github_groups_by_branch_across_pages() {
    let server = MockServer::start().await;
    // Page 1 is full (100 requests), so a second page is read.
    let mut page1: Vec<Value> = (0..97).map(|_| github_pull("other", "acme/app")).collect();
    page1.push(github_pull("feature/a", "acme/app"));
    page1.push(github_pull("feature/a", "ACME/App"));
    page1.push(github_pull("feature/a", "someone/fork"));
    let page2 = vec![
        github_pull("feature/a", "acme/app"),
        github_pull("feature/b", "acme/app"),
        json!({ "number": 9, "head": { "ref": "feature/b", "repo": null } }),
    ];
    for (page, body) in [("1", page1), ("2", page2)] {
        Mock::given(method("GET"))
            .and(path("/repos/acme/app/pulls"))
            .and(query_param("state", "all"))
            .and(query_param("per_page", "100"))
            .and(query_param("page", page))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .expect(1)
            .mount(&server)
            .await;
    }

    let counts = fetch_request_counts(&github(&server), &remote(RemoteKind::GitHub, "acme/app"))
        .await
        .expect("counts");

    assert_eq!(
        counts.get("feature/a"),
        Some(&3),
        "fork and deleted-repo heads are not counted"
    );
    assert_eq!(counts.get("feature/b"), Some(&1));
    assert_eq!(counts.get("other"), Some(&97));
    assert_eq!(counts.len(), 3);
}

#[tokio::test]
async fn test_pull_request_counts_gitlab_follows_next_page() {
    let server = MockServer::start().await;
    let mr = |branch: &str, source: u64| {
        json!({ "iid": 1, "source_branch": branch,
                "source_project_id": source, "target_project_id": 7 })
    };
    Mock::given(method("GET"))
        .and(path(
            "/api/v4/projects/group%2Fsub%2Fproject/merge_requests",
        ))
        .and(query_param("state", "all"))
        .and(query_param("page", "1"))
        .and(header("PRIVATE-TOKEN", "glpat-test"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-next-page", "2")
                .set_body_json(json!([mr("feature/a", 7), mr("feature/a", 99)])),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(
            "/api/v4/projects/group%2Fsub%2Fproject/merge_requests",
        ))
        .and(query_param("page", "2"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!([mr("feature/a", 7), mr("fix", 7)])),
        )
        .expect(1)
        .mount(&server)
        .await;
    let source = RequestSource::GitLab(GitLabClient::with_credentials(
        server.uri(),
        "glpat-test".to_string(),
    ));

    let counts = fetch_request_counts(&source, &remote(RemoteKind::GitLab, "group/sub/project"))
        .await
        .expect("counts");

    assert_eq!(
        counts.get("feature/a"),
        Some(&2),
        "the fork's request is not counted"
    );
    assert_eq!(counts.get("fix"), Some(&1));
}

#[tokio::test]
async fn test_pull_request_counts_one_request_series_per_project() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/app/pulls"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            github_pull("feature/a", "acme/app"),
            github_pull("feature/a", "acme/app"),
        ])))
        .expect(1) // three sessions, two branches, one call
        .mount(&server)
        .await;
    let m = managers();
    let project = project_with_sessions(
        &m,
        "series",
        RemoteKind::GitHub,
        &["feature/a", "feature/a", "feature/none"],
    );
    let service = PullRequestCounts::new(m.storage.clone(), m.events.clone());
    let mut rx = m.events.subscribe();

    let outcome = service.refresh_with(&project, Some(&github(&server))).await;

    assert_eq!(outcome, RefreshOutcome::Updated(3));
    assert_eq!(
        stored_counts(&m),
        vec![
            ("feature/a".to_string(), Some(2)),
            ("feature/a".to_string(), Some(2)),
            ("feature/none".to_string(), Some(0)),
        ],
        "a branch with no request is stored as zero, not unknown"
    );
    let mut updated = false;
    while let Ok(event) = rx.try_recv() {
        updated |=
            matches!(&event, Event::ProjectUpdated { project_id } if *project_id == project.id);
    }
    assert!(updated, "a change is announced so the sidebar reloads");

    // Within the interval nothing is fetched again (`expect(1)` above).
    assert!(!service.is_due(&project.id));
    assert_eq!(
        service.refresh_with(&project, Some(&github(&server))).await,
        RefreshOutcome::NotDue
    );
}

#[tokio::test]
async fn test_pull_request_counts_failures_keep_stored_counts() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/app/pulls"))
        .respond_with(ResponseTemplate::new(401).set_body_string("bad credentials"))
        .mount(&server)
        .await;
    let m = managers();
    let project = project_with_sessions(&m, "failures", RemoteKind::GitHub, &["feature/a"]);
    m.storage
        .set_branch_pr_count(&project.id, "feature/a", Some(4))
        .expect("last known count");
    let service = PullRequestCounts::new(m.storage.clone(), m.events.clone());

    // No credentials: no request is made at all.
    assert_eq!(
        service.refresh_with(&project, None).await,
        RefreshOutcome::NoCredentials
    );
    assert_eq!(server.received_requests().await.expect("requests").len(), 0);
    service.force_due(&project.id);

    // An API error.
    assert_eq!(
        service.refresh_with(&project, Some(&github(&server))).await,
        RefreshOutcome::Failed {
            rate_limited: false
        }
    );
    assert_eq!(service.interval(&project.id), BASE_INTERVAL);
    service.force_due(&project.id);

    // A network failure: nothing listens on this port.
    let dead = RequestSource::GitHub(GitHubClient::with_base_url(
        "http://127.0.0.1:1".to_string(),
        "test-token".to_string(),
    ));
    assert_eq!(
        service.refresh_with(&project, Some(&dead)).await,
        RefreshOutcome::Failed {
            rate_limited: false
        }
    );

    assert_eq!(
        stored_counts(&m),
        vec![("feature/a".to_string(), Some(4))],
        "the count shown before the fetch is still the stored one"
    );
}

#[tokio::test]
async fn test_pull_request_counts_rate_limit_widens_the_interval() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/app/pulls"))
        .respond_with(
            ResponseTemplate::new(403)
                .insert_header("x-ratelimit-remaining", "0")
                .insert_header("x-ratelimit-reset", "1700000000"),
        )
        .mount(&server)
        .await;
    let m = managers();
    let project = project_with_sessions(&m, "rate_limit", RemoteKind::GitHub, &["feature/a"]);
    let service = PullRequestCounts::new(m.storage.clone(), m.events.clone());
    let source = github(&server);

    // 5 -> 10 -> 20 -> 40 -> 60 -> 60 minutes: doubles, capped at one hour.
    let mut expected = BASE_INTERVAL;
    for _ in 0..6 {
        assert_eq!(
            service.refresh_with(&project, Some(&source)).await,
            RefreshOutcome::Failed { rate_limited: true }
        );
        expected = (expected * 2).min(Duration::from_secs(3600));
        assert_eq!(service.interval(&project.id), expected);
        assert!(!service.is_due(&project.id), "no retry inside the interval");
        // An explicit refresh may try again at once; the back-off is kept.
        service.force_due(&project.id);
        assert!(service.is_due(&project.id));
    }
    assert_eq!(expected, Duration::from_secs(3600));
    assert_eq!(stored_counts(&m), vec![("feature/a".to_string(), None)]);
}

#[tokio::test]
async fn test_pull_request_counts_skips_projects_with_nothing_to_show() {
    let m = managers();
    let service = PullRequestCounts::new(m.storage.clone(), m.events.clone());

    let mut no_remote = project_with_sessions(&m, "no_remote", RemoteKind::GitHub, &["x"]);
    no_remote.remote = None;
    assert_eq!(
        service.refresh_with(&no_remote, None).await,
        RefreshOutcome::NoRemote
    );

    let no_branches = project_with_sessions(&m, "no_branches", RemoteKind::GitHub, &[]);
    m.sessions
        .create_session(PathBuf::from("/nonexistent/pr_counts/no_branches"))
        .expect("a session without a branch");
    assert_eq!(
        service.refresh_with(&no_branches, None).await,
        RefreshOutcome::NoBranches
    );
}
