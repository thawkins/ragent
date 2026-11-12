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
    assert!(counts.complete, "the last page was reached");
    let counts = counts.counts;

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
    assert!(counts.complete, "the last page was reached");
    let counts = counts.counts;

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

#[tokio::test]
async fn test_pull_request_counts_truncated_fetch_keeps_last_known_counts() {
    let server = MockServer::start().await;
    // Every page is full, so the page limit stops the read before the end.
    let mut page: Vec<Value> = (0..98).map(|_| github_pull("other", "acme/app")).collect();
    page.push(github_pull("feature/recent", "acme/app"));
    page.push(github_pull("feature/busy", "acme/app"));
    Mock::given(method("GET"))
        .and(path("/repos/acme/app/pulls"))
        .respond_with(ResponseTemplate::new(200).set_body_json(page))
        .expect(10)
        .mount(&server)
        .await;
    let m = managers();
    let project = project_with_sessions(
        &m,
        "truncated",
        RemoteKind::GitHub,
        &[
            "feature/recent",
            "feature/busy",
            "feature/old",
            "feature/never",
        ],
    );
    for (branch, known) in [
        ("feature/recent", 4),
        ("feature/busy", 50),
        ("feature/old", 7),
    ] {
        m.storage
            .set_branch_pr_count(&project.id, branch, Some(known))
            .expect("last known count");
    }

    let fetched = fetch_request_counts(&github(&server), &remote(RemoteKind::GitHub, "acme/app"))
        .await
        .expect("counts");
    assert!(
        !fetched.complete,
        "1,000 requests were read and more remain"
    );
    server.reset().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/app/pulls"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(
                (0..100)
                    .map(|i| {
                        github_pull(
                            match i {
                                0 => "feature/recent",
                                1 => "feature/busy",
                                _ => "other",
                            },
                            "acme/app",
                        )
                    })
                    .collect::<Vec<_>>(),
            ),
        )
        .mount(&server)
        .await;
    let service = PullRequestCounts::new(m.storage.clone(), m.events.clone());

    let outcome = service.refresh_with(&project, Some(&github(&server))).await;

    assert_eq!(outcome, RefreshOutcome::Updated(1));
    assert_eq!(
        stored_counts(&m),
        vec![
            // Seen 10 times in the pages that were read; 50 were already known.
            ("feature/busy".to_string(), Some(50)),
            // Never fetched and not in the partial result: still unknown, not zero.
            ("feature/never".to_string(), None),
            // Older than the pages that were read: the last known count stays.
            ("feature/old".to_string(), Some(7)),
            // Seen 10 times, more than the 4 known: raised.
            ("feature/recent".to_string(), Some(10)),
        ]
    );
}

#[tokio::test]
async fn test_pull_request_counts_overlapping_refreshes_make_one_request() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/app/pulls"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(300))
                .set_body_json(json!([github_pull("feature/a", "acme/app")])),
        )
        .expect(1)
        .mount(&server)
        .await;
    let m = managers();
    let project = project_with_sessions(&m, "overlap", RemoteKind::GitHub, &["feature/a"]);
    let service = PullRequestCounts::new(m.storage.clone(), m.events.clone());
    let source = github(&server);

    // A sweep that arrives while the first fetch is still waiting.
    let slow = service.refresh_with(&project, Some(&source));
    let second = async {
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(!service.is_due(&project.id), "a running refresh is not due");
        // An explicit refresh does not start a second fetch either.
        service.force_due(&project.id);
        service.refresh_with(&project, Some(&source)).await
    };
    let (first, second) = tokio::join!(slow, second);

    assert_eq!(first, RefreshOutcome::Updated(1));
    assert_eq!(second, RefreshOutcome::NotDue);
    assert_eq!(stored_counts(&m), vec![("feature/a".to_string(), Some(1))]);
}

#[tokio::test]
async fn test_pull_request_counts_cancelled_refresh_releases_the_project() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/app/pulls"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_secs(30))
                .set_body_json(json!([])),
        )
        .mount(&server)
        .await;
    let m = managers();
    let project = project_with_sessions(&m, "cancelled", RemoteKind::GitHub, &["feature/a"]);
    let service = PullRequestCounts::new(m.storage.clone(), m.events.clone());
    let source = github(&server);

    let cancelled = tokio::time::timeout(
        Duration::from_millis(100),
        service.refresh_with(&project, Some(&source)),
    )
    .await;

    assert!(cancelled.is_err(), "the refresh was dropped mid-fetch");
    assert!(
        service.is_due(&project.id),
        "a dropped refresh must not leave the project reserved forever"
    );
}

#[test]
fn test_pull_request_counts_gitlab_token_only_goes_to_the_configured_host() {
    use ragent_tools_vcs::gitlab::auth::{GitLabConfig, load_config, save_config, save_token};
    let m = managers();
    // An instance served under a path prefix.
    let instance = "https://gitlab.example.com/gitlab";
    save_config(
        m.storage.as_ref(),
        &GitLabConfig {
            instance_url: instance.to_string(),
            username: "tester".to_string(),
        },
    )
    .expect("config");
    save_token(m.storage.as_ref(), "glpat-test").expect("token");
    // Environment or file configuration outranks the database; the test
    // only speaks for the configuration it wrote.
    if load_config(m.storage.as_ref())
        .map(|c| c.instance_url)
        .as_deref()
        != Some(instance)
    {
        return;
    }
    let service = PullRequestCounts::new(m.storage.clone(), m.events.clone());
    let mut on_instance = remote(RemoteKind::GitLab, "group/project");
    on_instance.host = "gitlab.example.com".to_string();
    let mut elsewhere = remote(RemoteKind::GitLab, "group/project");
    elsewhere.host = "gitlab.other.example".to_string();

    assert!(
        matches!(
            service.source_for(&on_instance),
            Some(RequestSource::GitLab(_))
        ),
        "the path prefix of the instance URL must not stop the host matching"
    );
    assert!(
        service.source_for(&elsewhere).is_none(),
        "the token is never offered to another host"
    );
}
