//! `autofix_add` tool (spec SESSIONMENU): base-branch guess, push, opening
//! or reusing the branch's pull request, binding and Auto-fix on.
//!
//! No network: the checkout's `origin` fetch URL names github.com so remote
//! detection sees GitHub, but its push URL is a local bare repository, and
//! the API calls go to a local mock server.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use ragent_agent::event::EventBus;
use ragent_agent::project::detect_remote_with_hosts;
use ragent_agent::project::requests::RequestSource;
use ragent_agent::tool::autofix_add::{AddOutcome, add_request, pick_base};
use ragent_agent::tool::{ToolContext, create_default_registry};
use ragent_storage::Storage;
use ragent_tools_vcs::github::client::GitHubClient;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Runs git in `dir` with no user or system config.
fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
        ])
        .args([
            "-c",
            "init.defaultBranch=main",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn commit(dir: &Path, file: &str, message: &str) {
    std::fs::write(dir.join(file), message).expect("write");
    git(dir, &["add", file]);
    git(dir, &["commit", "-q", "-m", message]);
}

/// A checkout whose `origin` is github.com/acme/app for fetching and a
/// local bare repository for pushing, with `main` pushed. Returns
/// `(checkout, bare)`.
fn fixture(name: &str) -> (PathBuf, PathBuf) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/autofix-add")
        .join(name);
    let _ = std::fs::remove_dir_all(&root); // INTENTIONAL: may not exist yet
    let bare = root.join("remote.git");
    let work = root.join("work");
    std::fs::create_dir_all(&bare).expect("bare dir");
    std::fs::create_dir_all(&work).expect("work dir");
    git(&bare, &["init", "-q", "--bare"]);
    git(&work, &["init", "-q"]);
    git(
        &work,
        &["remote", "add", "origin", "https://github.com/acme/app.git"],
    );
    let bare_url = bare.canonicalize().expect("bare path");
    git(
        &work,
        &[
            "remote",
            "set-url",
            "--push",
            "origin",
            bare_url.to_str().expect("utf-8 path"),
        ],
    );
    commit(&work, "a.txt", "base");
    git(&work, &["push", "-q", "origin", "main"]);
    git(
        &work,
        &[
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/main",
        ],
    );
    (work.canonicalize().expect("work path"), bare_url)
}

fn ctx(storage: Arc<Storage>, dir: PathBuf) -> ToolContext {
    ToolContext {
        session_id: "s".to_string(),
        working_dir: dir,
        event_bus: Arc::new(EventBus::new(16)),
        storage: Some(storage),
        agent_manager: None,
        active_model: None,
        provider_registry: None,
        team_context: None,
        team_manager: None,
        code_index: None,
        bg_service: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        allowed_roots: Vec::new(),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        tool_registry: ToolContext::default_tool_registry(),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
    }
}

fn storage() -> Arc<Storage> {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    storage
        .create_session("s", "/nonexistent/autofix-add")
        .expect("session");
    storage
}

fn pull(number: u64, branch: &str) -> serde_json::Value {
    json!({
        "number": number, "state": "open", "title": format!("PR {number}"),
        "html_url": format!("https://github.com/acme/app/pull/{number}"), "merged_at": null,
        "head": { "ref": branch, "repo": { "full_name": "acme/app" } }
    })
}

async fn no_open_pulls(server: &MockServer, branch: &str) {
    Mock::given(method("GET"))
        .and(path("/repos/acme/app/pulls"))
        .and(query_param("head", format!("acme:{branch}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(server)
        .await;
}

fn source(server: &MockServer) -> RequestSource {
    RequestSource::GitHub(GitHubClient::with_base_url(server.uri(), "t".to_string()))
}

#[test]
fn test_autofix_add_tool_registered_as_git_write() {
    let registry = create_default_registry();
    let tool = registry.get("autofix_add").expect("registered");
    assert_eq!(tool.permission_category(), "git:write");
}

#[test]
fn test_autofix_add_pick_base() {
    let c = |pairs: &[(&str, u64)]| -> Vec<(String, u64)> {
        pairs.iter().map(|(n, a)| ((*n).to_string(), *a)).collect()
    };
    assert_eq!(
        pick_base(&c(&[("main", 5), ("develop", 2)]), Some("main")),
        Some("develop".to_string()),
        "the most recently diverged branch wins"
    );
    assert_eq!(
        pick_base(&c(&[("release", 2), ("main", 2)]), Some("main")),
        Some("main".to_string()),
        "a tie prefers the default branch"
    );
    assert_eq!(
        pick_base(&c(&[("main", 0), ("old", 0)]), Some("main")),
        Some("main".to_string()),
        "branches HEAD already belongs to are skipped; default is the fallback"
    );
    assert_eq!(pick_base(&[], None), None);
}

#[tokio::test]
async fn test_autofix_add_pushes_opens_and_binds() {
    let (work, bare) = fixture("opens");
    git(&work, &["checkout", "-q", "-b", "fix/thing"]);
    commit(&work, "b.txt", "Fix the thing");

    let server = MockServer::start().await;
    no_open_pulls(&server, "fix/thing").await;
    Mock::given(method("POST"))
        .and(path("/repos/acme/app/pulls"))
        .and(body_partial_json(json!({
            "head": "fix/thing", "base": "main", "title": "Fix the thing"
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(pull(42, "fix/thing")))
        .expect(1)
        .mount(&server)
        .await;

    let storage = storage();
    let remote = detect_remote_with_hosts(&work, None, &[]).expect("github remote");
    let outcome = add_request(
        &ctx(Arc::clone(&storage), work.clone()),
        &json!({}),
        &remote,
        &source(&server),
    )
    .await
    .expect("add");

    match &outcome {
        AddOutcome::Opened { info, base, pushed } => {
            assert_eq!(info.number, 42);
            assert_eq!(base, "main");
            assert!(pushed);
        }
        other => panic!("expected a new PR, got {other:?}"),
    }
    assert_eq!(
        git(&bare, &["rev-parse", "fix/thing"]),
        git(&work, &["rev-parse", "HEAD"]),
        "the branch was pushed"
    );
    let bound = storage.session_request("s").expect("read").expect("bound");
    assert_eq!((bound.number, bound.repo.as_str()), (42, "acme/app"));
    assert!(
        storage
            .session_autofix("s")
            .expect("read")
            .expect("state")
            .enabled
    );
}

#[tokio::test]
async fn test_autofix_add_guesses_the_branch_it_came_from() {
    let (work, bare) = fixture("guess");
    git(&work, &["checkout", "-q", "-b", "develop"]);
    commit(&work, "d.txt", "develop work");
    git(&work, &["push", "-q", "origin", "develop"]);
    git(&work, &["checkout", "-q", "-b", "feature"]);
    commit(&work, "f.txt", "Feature");
    // Pushed, then deleted on the remote: the local `origin/feature` still
    // matches HEAD, but the live remote lacks the branch.
    git(&work, &["push", "-q", "origin", "feature"]);
    git(&bare, &["branch", "-D", "feature"]);

    let server = MockServer::start().await;
    no_open_pulls(&server, "feature").await;
    Mock::given(method("POST"))
        .and(path("/repos/acme/app/pulls"))
        .and(body_partial_json(
            json!({ "head": "feature", "base": "develop" }),
        ))
        .respond_with(ResponseTemplate::new(201).set_body_json(pull(7, "feature")))
        .expect(1)
        .mount(&server)
        .await;

    let storage = storage();
    let remote = detect_remote_with_hosts(&work, None, &[]).expect("remote");
    let outcome = add_request(&ctx(storage, work), &json!({}), &remote, &source(&server))
        .await
        .expect("add");
    assert!(
        matches!(outcome, AddOutcome::Opened { ref base, pushed: true, .. } if base == "develop"),
        "{outcome:?}"
    );
    assert!(
        !git(&bare, &["branch", "--list", "feature"]).is_empty(),
        "the branch deleted on the remote is pushed again"
    );
}

#[tokio::test]
async fn test_autofix_add_binds_existing_open_request() {
    let (work, bare) = fixture("existing");
    git(&work, &["checkout", "-q", "-b", "wip"]);
    commit(&work, "w.txt", "wip");
    git(&work, &["push", "-q", "origin", "wip"]);
    // A follow-up commit not yet on the remote.
    commit(&work, "w2.txt", "follow-up");
    let work_head = work.clone();

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/app/pulls"))
        .and(query_param("head", "acme:wip"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([pull(9, "wip")])))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;

    let storage = storage();
    let remote = detect_remote_with_hosts(&work, None, &[]).expect("remote");
    let outcome = add_request(
        &ctx(Arc::clone(&storage), work),
        &json!({}),
        &remote,
        &source(&server),
    )
    .await
    .expect("add");
    assert!(
        matches!(outcome, AddOutcome::Existing { ref info, pushed: true } if info.number == 9),
        "the follow-up commit is pushed before reusing the PR: {outcome:?}"
    );
    assert_eq!(
        git(&bare, &["rev-parse", "wip"]),
        git(&work_head, &["rev-parse", "HEAD"]),
        "the existing PR's branch has the new commit"
    );
    assert_eq!(
        storage
            .session_request("s")
            .expect("read")
            .expect("bound")
            .number,
        9
    );
    assert!(
        storage
            .session_autofix("s")
            .expect("read")
            .expect("state")
            .enabled
    );
}

#[tokio::test]
async fn test_autofix_add_refuses_nothing_to_merge_and_detached_head() {
    let (work, _bare) = fixture("refuse");
    let server = MockServer::start().await;
    no_open_pulls(&server, "same").await;
    git(&work, &["checkout", "-q", "-b", "same"]);
    let storage = storage();
    let remote = detect_remote_with_hosts(&work, None, &[]).expect("remote");

    let err = add_request(
        &ctx(Arc::clone(&storage), work.clone()),
        &json!({ "base": "main" }),
        &remote,
        &source(&server),
    )
    .await
    .expect_err("nothing to merge");
    assert!(err.to_string().contains("nothing to merge"), "{err}");

    git(&work, &["checkout", "-q", "--detach"]);
    let err = add_request(
        &ctx(Arc::clone(&storage), work),
        &json!({}),
        &remote,
        &source(&server),
    )
    .await
    .expect_err("detached");
    assert!(err.to_string().contains("detached"), "{err}");
    assert!(storage.session_request("s").expect("read").is_none());
}
