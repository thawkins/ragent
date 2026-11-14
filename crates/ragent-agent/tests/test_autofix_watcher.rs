//! Auto-fix watcher (spec SESSIONMENU M5, T5.1): trigger computation, dedup,
//! attempt cap and lifecycle, against a local mock GitHub/GitLab API.

use std::sync::Arc;

use ragent_agent::autofix::{AutofixWatcher, SourceResolver};
use ragent_agent::event::{Event, EventBus};
use ragent_agent::project::requests::RequestSource;
use ragent_config::VcsAutofixConfig;
use ragent_storage::{SessionRequest, Storage};
use ragent_tools_vcs::github::client::GitHubClient;
use ragent_tools_vcs::gitlab::GitLabClient;
use serde_json::{Value, json};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SHA_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const SHA_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn storage_with_binding(forge: &str, repo: &str) -> Arc<Storage> {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    storage
        .create_session("s", "/nonexistent/autofix")
        .expect("s");
    storage
        .set_session_request(&SessionRequest {
            session_id: "s".to_string(),
            forge: forge.to_string(),
            host: "example.invalid".to_string(),
            repo: repo.to_string(),
            number: 7,
            title: "Fix".to_string(),
            web_url: "https://example.invalid/7".to_string(),
            head_branch: "fix".to_string(),
            state: "open".to_string(),
        })
        .expect("bind");
    assert!(storage.set_session_autofix("s", true).expect("on"));
    storage
}

fn github_watcher(storage: &Arc<Storage>, server: &MockServer) -> AutofixWatcher {
    let uri = server.uri();
    let sources: SourceResolver = Arc::new(move |_| {
        Some(RequestSource::GitHub(GitHubClient::with_base_url(
            uri.clone(),
            "t".to_string(),
        )))
    });
    AutofixWatcher::new(
        Arc::clone(storage),
        Arc::new(EventBus::default()),
        sources,
        &VcsAutofixConfig::default(),
    )
}

fn gitlab_watcher(storage: &Arc<Storage>, server: &MockServer) -> AutofixWatcher {
    let uri = server.uri();
    let sources: SourceResolver = Arc::new(move |_| {
        Some(RequestSource::GitLab(GitLabClient::with_credentials(
            uri.clone(),
            "t".to_string(),
        )))
    });
    AutofixWatcher::new(
        Arc::clone(storage),
        Arc::new(EventBus::default()),
        sources,
        &VcsAutofixConfig::default(),
    )
}

async fn get(server: &MockServer, at: &str, body: Value) {
    Mock::given(method("GET"))
        .and(path(at))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

/// Mounts a GitHub pull request at `sha` with the given failed check-run
/// ids, conflict flag and review comments.
async fn github_pull(
    server: &MockServer,
    sha: &str,
    failed_checks: &[u64],
    dirty: bool,
    comments: Value,
) {
    server.reset().await;
    get(
        server,
        "/repos/acme/app/pulls/7",
        json!({
            "number": 7, "state": "open", "title": "Fix", "merged_at": null,
            "html_url": "https://github.com/acme/app/pull/7",
            "head": { "ref": "fix", "sha": sha, "repo": { "full_name": "acme/app" } },
            "base": { "ref": "main" },
            "mergeable": !dirty,
            "mergeable_state": if dirty { "dirty" } else { "clean" }
        }),
    )
    .await;
    let runs: Vec<Value> = failed_checks
        .iter()
        .map(|id| {
            json!({
                "id": id, "name": "clippy", "status": "completed",
                "conclusion": "failure", "app": { "slug": "other-ci" }
            })
        })
        .chain(std::iter::once(json!({
            "id": 1, "name": "fmt", "status": "completed",
            "conclusion": "success", "app": { "slug": "other-ci" }
        })))
        .collect();
    get(
        server,
        &format!("/repos/acme/app/commits/{sha}/check-runs"),
        json!({ "check_runs": runs }),
    )
    .await;
    get(
        server,
        &format!("/repos/acme/app/commits/{sha}/statuses"),
        json!([]),
    )
    .await;
    get(server, "/user", json!({ "login": "bot" })).await;
    get(server, "/repos/acme/app/pulls/7/comments", comments).await;
    get(server, "/repos/acme/app/pulls/7/reviews", json!([])).await;
    get(server, "/graphql", review_threads(&[])).await;
}

/// A GraphQL `reviewThreads` response whose `resolved` threads hold comments
/// with the given node ids.
fn review_threads(resolved: &[(&str, &[&str])]) -> Value {
    let nodes: Vec<Value> = resolved
        .iter()
        .map(|(thread_id, comment_ids)| {
            json!({
                "id": thread_id,
                "isResolved": true,
                "comments": {
                    "nodes": comment_ids
                        .iter()
                        .map(|id| json!({ "id": id }))
                        .collect::<Vec<Value>>()
                }
            })
        })
        .collect();
    json!({ "data": { "repository": { "pullRequest": { "reviewThreads": { "nodes": nodes } } } } })
}

/// `(kind, summary, prompt)` of each trigger event.
fn triggers(events: &[Event]) -> Vec<(String, String, String)> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::AutofixTrigger {
                kind,
                summary,
                prompt,
                ..
            } => Some((kind.clone(), summary.clone(), prompt.clone())),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn test_autofix_watcher_ci_failure_fires_once_per_commit() {
    let server = MockServer::start().await;
    let storage = storage_with_binding("github", "acme/app");
    let watcher = github_watcher(&storage, &server);

    github_pull(&server, SHA_A, &[11], false, json!([])).await;
    let first = triggers(&watcher.tick().await);
    assert_eq!(first.len(), 1, "{first:?}");
    let (kind, summary, prompt) = &first[0];
    assert_eq!(kind, "ci");
    assert_eq!(summary, "PR #7 auto-fix: CI failed (attempt 1/3)");
    assert!(prompt.contains("job `clippy`"), "{prompt}");
    assert!(!prompt.contains("`fmt`"), "a passing check is left out");
    assert!(prompt.contains("never force-push"), "{prompt}");

    assert!(
        watcher.tick().await.is_empty(),
        "the same failure on the same commit is not reported again"
    );

    github_pull(&server, SHA_B, &[11], false, json!([])).await;
    let pushed = triggers(&watcher.tick().await);
    assert_eq!(pushed.len(), 1, "a new commit starts counting again");
    assert!(pushed[0].1.contains("attempt 1/3"), "{:?}", pushed[0]);
    let state = storage.session_autofix("s").expect("read").expect("bound");
    assert_eq!(state.head_sha.as_deref(), Some(SHA_B));
    assert!(state.last_polled_at.is_some());
}

#[tokio::test]
async fn test_autofix_watcher_pauses_after_max_attempts() {
    let server = MockServer::start().await;
    let storage = storage_with_binding("github", "acme/app");
    let watcher = github_watcher(&storage, &server);

    // Each re-run of the failing job is a new check run on the same commit.
    for (attempt, id) in [21_u64, 22, 23].into_iter().enumerate() {
        github_pull(&server, SHA_A, &[id], false, json!([])).await;
        let fired = triggers(&watcher.tick().await);
        assert_eq!(fired.len(), 1, "attempt {attempt}: {fired:?}");
        assert!(fired[0].1.contains(&format!("attempt {}/3", attempt + 1)));
    }
    github_pull(&server, SHA_A, &[24], false, json!([])).await;
    let paused = triggers(&watcher.tick().await);
    assert_eq!(paused.len(), 1);
    assert_eq!(paused[0].0, "paused");
    assert!(paused[0].1.contains("needs you"), "{:?}", paused[0]);
    assert!(paused[0].2.is_empty(), "a pause sends no prompt");
    assert!(
        storage
            .session_autofix("s")
            .expect("read")
            .expect("bound")
            .paused
    );
    assert_eq!(storage.autofix_watch_list().expect("list").len(), 0);
    assert!(
        watcher.tick().await.is_empty(),
        "a paused binding is not polled"
    );

    // `/autofix on` resumes.
    storage.set_session_autofix("s", true).expect("on");
    assert_eq!(storage.autofix_watch_list().expect("list").len(), 1);
}

#[tokio::test]
async fn test_autofix_watcher_github_conflict_and_review() {
    let server = MockServer::start().await;
    let storage = storage_with_binding("github", "acme/app");
    let watcher = github_watcher(&storage, &server);

    let comments = json!([
        { "id": 501, "user": { "login": "alice" }, "path": "src/lib.rs", "line": 12,
          "body": "Please rename this." },
        { "id": 502, "user": { "login": "bot" }, "path": "src/lib.rs", "line": 13,
          "body": "My own note." }
    ]);
    github_pull(&server, SHA_A, &[], true, comments).await;
    let fired = triggers(&watcher.tick().await);
    let kinds: Vec<&str> = fired.iter().map(|(k, _, _)| k.as_str()).collect();
    assert_eq!(kinds, ["conflict", "review"], "{fired:?}");
    assert!(
        fired[0].2.contains("`fix` conflicts with `main`"),
        "{}",
        fired[0].2
    );
    let review = &fired[1].2;
    assert!(review.contains("alice on `src/lib.rs:12`"), "{review}");
    assert!(review.contains("Please rename this."), "{review}");
    assert!(
        !review.contains("My own note."),
        "the token's own comments never wake the session"
    );
    assert_eq!(watcher.tick().await.len(), 0, "each item fires once");
}

#[tokio::test]
async fn test_autofix_watcher_github_resolved_thread_does_not_wake() {
    let server = MockServer::start().await;
    let storage = storage_with_binding("github", "acme/app");
    let watcher = github_watcher(&storage, &server);

    let comments = json!([
        { "id": 501, "node_id": "PRRC_open", "user": { "login": "alice" },
          "path": "src/lib.rs", "line": 12, "body": "Please rename this." },
        { "id": 502, "node_id": "PRRC_done", "user": { "login": "bob" },
          "path": "src/lib.rs", "line": 20, "body": "Rename `a` to `b`." }
    ]);
    github_pull(&server, SHA_A, &[], false, comments).await;
    // The fixture mounts an empty thread list; a second response replaces it.
    server.reset().await;
    github_pull(&server, SHA_A, &[], false, json!([])).await;
    server.reset().await;
    get(
        &server,
        "/repos/acme/app/pulls/7",
        json!({
            "number": 7, "state": "open", "title": "Fix", "merged_at": null,
            "html_url": "https://github.com/acme/app/pull/7",
            "head": { "ref": "fix", "sha": SHA_A, "repo": { "full_name": "acme/app" } },
            "base": { "ref": "main" },
            "mergeable": true, "mergeable_state": "clean"
        }),
    )
    .await;
    get(
        &server,
        &format!("/repos/acme/app/commits/{SHA_A}/check-runs"),
        json!({ "check_runs": [] }),
    )
    .await;
    get(
        &server,
        &format!("/repos/acme/app/commits/{SHA_A}/statuses"),
        json!([]),
    )
    .await;
    get(&server, "/user", json!({ "login": "bot" })).await;
    get(
        &server,
        "/repos/acme/app/pulls/7/comments",
        json!([
            { "id": 501, "node_id": "PRRC_open", "user": { "login": "alice" },
              "path": "src/lib.rs", "line": 12, "body": "Please rename this." },
            { "id": 502, "node_id": "PRRC_done", "user": { "login": "bob" },
              "path": "src/lib.rs", "line": 20, "body": "Rename `a` to `b`." }
        ]),
    )
    .await;
    get(&server, "/repos/acme/app/pulls/7/reviews", json!([])).await;
    get(
        &server,
        "/graphql",
        review_threads(&[("PRRT_1", &["PRRC_done"])]),
    )
    .await;

    let fired = triggers(&watcher.tick().await);
    let kinds: Vec<&str> = fired.iter().map(|(k, _, _)| k.as_str()).collect();
    assert_eq!(kinds, ["review"], "{fired:?}");
    let review = &fired[0].2;
    assert!(review.contains("Please rename this."), "{review}");
    assert!(
        !review.contains("Rename `a` to `b`."),
        "a comment in a resolved thread must not wake the session: {review}"
    );
}

#[tokio::test]
async fn test_autofix_watcher_gitlab_pipeline_conflict_and_notes() {
    let server = MockServer::start().await;
    let storage = storage_with_binding("gitlab", "group/project");
    let watcher = gitlab_watcher(&storage, &server);
    let project = "/api/v4/projects/group%2Fproject";

    get(
        &server,
        &format!("{project}/merge_requests/7"),
        json!({
            "iid": 7, "state": "opened", "title": "Fix", "sha": SHA_A,
            "web_url": "https://gitlab.example.com/group/project/-/merge_requests/7",
            "source_branch": "fix", "target_branch": "main",
            "detailed_merge_status": "conflict",
            "head_pipeline": { "id": 90, "status": "failed", "sha": SHA_A }
        }),
    )
    .await;
    get(
        &server,
        &format!("{project}/pipelines/90/jobs"),
        json!([
            { "id": 301, "name": "test", "allow_failure": false },
            { "id": 302, "name": "flaky", "allow_failure": true }
        ]),
    )
    .await;
    Mock::given(method("GET"))
        .and(path(format!("{project}/jobs/301/trace")))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "compiling\ncurl -H 'Authorization: Bearer abcdefghijklmnopqrstuvwxyz'\nerror: boom\n",
        ))
        .expect(1)
        .mount(&server)
        .await;
    get(&server, "/api/v4/user", json!({ "username": "bot" })).await;
    // Page 1 points at page 2 through `x-next-page`.
    Mock::given(method("GET"))
        .and(path(format!("{project}/merge_requests/7/discussions")))
        .and(query_param("page", "1"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-next-page", "2")
                .set_body_json(json!([{ "notes": [] }])),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("{project}/merge_requests/7/discussions")))
        .and(query_param("page", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            { "notes": [
                { "id": 1, "resolvable": true, "resolved": false, "system": false,
                  "author": { "username": "carol" }, "body": "Add a test.",
                  "position": { "new_path": "src/main.rs", "new_line": 4 } },
                { "id": 2, "resolvable": true, "resolved": true, "system": false,
                  "author": { "username": "carol" }, "body": "Already done." }
            ]},
            { "notes": [
                { "id": 3, "resolvable": false, "resolved": false, "system": false,
                  "author": { "username": "dave" }, "body": "Nice." }
            ]}
        ])))
        .mount(&server)
        .await;

    let fired = triggers(&watcher.tick().await);
    let kinds: Vec<&str> = fired.iter().map(|(k, _, _)| k.as_str()).collect();
    assert_eq!(kinds, ["ci", "conflict", "review"], "{fired:?}");
    let ci = &fired[0];
    assert_eq!(ci.1, "MR !7 auto-fix: CI failed (attempt 1/3)");
    assert!(
        ci.2.contains("job `test`") && ci.2.contains("error: boom"),
        "{}",
        ci.2
    );
    assert!(!ci.2.contains("flaky"), "an allowed failure is left out");
    assert!(
        !ci.2.contains("abcdefghijklmnopqrstuvwxyz") && ci.2.contains("[REDACTED]"),
        "credentials in the log are redacted: {}",
        ci.2
    );
    let review = &fired[2].2;
    assert!(
        review.contains("carol on `src/main.rs:4`"),
        "a note on page 2 is read: {review}"
    );
    assert!(!review.contains("Already done.") && !review.contains("Nice."));
    assert_eq!(
        watcher.tick().await.len(),
        0,
        "nothing new; the log of a job already acted on is not read again"
    );
}

#[tokio::test]
async fn test_autofix_watcher_merged_request_stops() {
    let server = MockServer::start().await;
    let storage = storage_with_binding("github", "acme/app");
    let watcher = github_watcher(&storage, &server);
    get(
        &server,
        "/repos/acme/app/pulls/7",
        json!({
            "number": 7, "state": "closed", "title": "Fix",
            "merged_at": "2026-11-01T00:00:00Z",
            "html_url": "https://github.com/acme/app/pull/7",
            "head": { "ref": "fix", "sha": SHA_A, "repo": { "full_name": "acme/app" } }
        }),
    )
    .await;

    let fired = triggers(&watcher.tick().await);
    assert_eq!(fired.len(), 1);
    assert_eq!(fired[0].0, "closed");
    assert_eq!(fired[0].1, "PR #7 merged; auto-fix stopped");
    let bound = storage.session_request("s").expect("read").expect("kept");
    assert_eq!(bound.state, "merged", "the binding stays, read-only");
    assert!(
        watcher.tick().await.is_empty(),
        "a merged request is not polled"
    );
}

#[tokio::test]
async fn test_autofix_watcher_skips_bindings_with_autofix_off() {
    let server = MockServer::start().await;
    let storage = storage_with_binding("github", "acme/app");
    storage.set_session_autofix("s", false).expect("off");
    let watcher = github_watcher(&storage, &server);
    assert_eq!(
        watcher.tick().await.len(),
        0,
        "a binding with Auto-fix off is not polled"
    );
    assert!(
        server
            .received_requests()
            .await
            .unwrap_or_default()
            .is_empty(),
        "nothing is polled"
    );
}

#[test]
fn test_autofix_watcher_rebind_keeps_setting_and_default_on() {
    let storage = storage_with_binding("github", "acme/app");
    storage.set_autofix_paused("s", true).expect("pause");
    let mut other = storage.session_request("s").expect("read").expect("bound");
    other.number = 8;
    storage.set_session_request(&other).expect("rebind");
    let state = storage.session_autofix("s").expect("read").expect("bound");
    assert!(state.enabled, "on/off survives a rebind");
    assert!(!state.paused, "a different request starts unpaused");

    storage
        .create_session("t", "/nonexistent/autofix")
        .expect("t");
    other.session_id = "t".to_string();
    storage.bind_session_request(&other, true).expect("bind");
    assert!(
        storage
            .session_autofix("t")
            .expect("read")
            .expect("bound")
            .enabled
    );
    storage.clear_session_request("t").expect("unbind");
    storage.bind_session_request(&other, false).expect("bind");
    assert!(
        !storage
            .session_autofix("t")
            .expect("read")
            .expect("bound")
            .enabled
    );
}

#[tokio::test]
async fn test_autofix_watcher_actions_log_by_job_id_and_no_user_identity() {
    let server = MockServer::start().await;
    let storage = storage_with_binding("github", "acme/app");
    let watcher = github_watcher(&storage, &server);
    github_pull(&server, SHA_A, &[], false, json!([])).await;
    // An Actions check run: its workflow job id comes from `details_url`,
    // not from the check run id.
    Mock::given(method("GET"))
        .and(path(format!("/repos/acme/app/commits/{SHA_A}/check-runs")))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "check_runs": [{
                "id": 9001, "name": "test", "status": "completed", "conclusion": "failure",
                "app": { "slug": "github-actions" },
                "details_url": "https://github.com/acme/app/actions/runs/55/job/777"
            }]})),
        )
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/app/actions/jobs/777/logs"))
        .respond_with(ResponseTemplate::new(200).set_body_string("step 1\nassertion failed\n"))
        .expect(1)
        .mount(&server)
        .await;
    // An installation token cannot read `/user`.
    Mock::given(method("GET"))
        .and(path("/user"))
        .respond_with(ResponseTemplate::new(403).set_body_string("Resource not accessible"))
        .with_priority(1)
        .mount(&server)
        .await;

    let fired = triggers(&watcher.tick().await);
    assert_eq!(
        fired.len(),
        1,
        "CI still fires without a token user: {fired:?}"
    );
    assert_eq!(fired[0].0, "ci");
    assert!(fired[0].2.contains("assertion failed"), "{}", fired[0].2);
}

#[test]
fn test_autofix_watcher_stale_results_are_dropped() {
    let storage = storage_with_binding("github", "acme/app");
    let old = storage.session_request("s").expect("read").expect("bound");
    let mut rebound = old.clone();
    rebound.number = 8;
    rebound.title = "Other".to_string();
    storage.set_session_request(&rebound).expect("rebind");

    assert!(
        !storage
            .record_autofix_poll(&old, "closed", "Fix", SHA_A)
            .expect("poll"),
        "a poll of the old request is dropped"
    );
    let now = storage.session_request("s").expect("read").expect("bound");
    assert_eq!((now.number, now.state.as_str()), (8, "open"));
    assert_eq!(
        storage
            .record_autofix_wakeup(&old, SHA_A, "ci", &["check:1".to_string()], "x")
            .expect("record"),
        0,
        "a trigger for the old request is dropped"
    );

    storage.set_session_autofix("s", false).expect("off");
    assert!(
        !storage
            .record_autofix_poll(&rebound, "open", "Other", SHA_A)
            .expect("poll")
    );
    assert_eq!(
        storage
            .record_autofix_wakeup(&rebound, SHA_A, "ci", &["check:1".to_string()], "x")
            .expect("record"),
        0,
        "turning Auto-fix off during a poll prevents the wake-up"
    );
}

#[tokio::test]
async fn test_autofix_watcher_on_resumes_a_paused_binding() {
    let server = MockServer::start().await;
    let storage = storage_with_binding("github", "acme/app");
    let watcher = github_watcher(&storage, &server);
    for id in [31_u64, 32, 33] {
        github_pull(&server, SHA_A, &[id], false, json!([])).await;
        assert_eq!(triggers(&watcher.tick().await).len(), 1);
    }
    github_pull(&server, SHA_A, &[34], false, json!([])).await;
    assert_eq!(triggers(&watcher.tick().await)[0].0, "paused");

    // Small gap so the resume time sorts after the recorded wake-ups.
    std::thread::sleep(std::time::Duration::from_millis(2));
    storage.set_session_autofix("s", true).expect("on");
    let resumed = triggers(&watcher.tick().await);
    assert_eq!(resumed.len(), 1, "{resumed:?}");
    assert_eq!(resumed[0].0, "ci");
    assert!(resumed[0].1.contains("attempt 1/3"), "{:?}", resumed[0]);
    assert!(
        resumed[0].2.contains("clippy"),
        "the item that paused it is sent"
    );
    assert_eq!(watcher.tick().await.len(), 0, "and only once");
}
