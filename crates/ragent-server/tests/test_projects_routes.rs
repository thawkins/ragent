//! Project routes and the `project_id` form of `POST /sessions` (spec
//! `projects`, T-012), in process via `tower::ServiceExt::oneshot`.
//!
//! Git fixtures live under the git-ignored `target/temp/` tree, one directory
//! per test, and are made a permitted root through the test state's config,
//! so nothing is written into the repository.

use std::path::{Path, PathBuf};
use std::process::Command;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use ragent_server::routes::{AppState, router};
use serde_json::{Value, json};
use tower::ServiceExt;

const TOKEN: &str = "projects-token";

fn fixture_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/projects_routes")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir); // INTENTIONAL: may not exist yet
    std::fs::create_dir_all(&dir).expect("fixture dir");
    dir.canonicalize().expect("canonical")
}

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.invalid")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.invalid")
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// A test state whose permitted roots include a fresh fixture directory
/// holding a git repository `app`, and whose worktree root is inside it.
async fn state_with_repo(name: &str) -> (AppState, PathBuf) {
    let base = fixture_dir(name);
    let repo = base.join("app");
    std::fs::create_dir_all(&repo).expect("repo dir");
    git(&repo, &["init", "--quiet", "--initial-branch=main"]);
    std::fs::write(repo.join("README.md"), "fixture\n").expect("write");
    git(&repo, &["add", "README.md"]);
    git(&repo, &["commit", "--quiet", "-m", "initial"]);
    let mut state = crate::support::test_state(TOKEN);
    state
        .config
        .write()
        .await
        .dirs
        .allowed_roots
        .push(base.display().to_string());
    state.worktree_root = std::sync::Arc::new(base.join("trees"));
    (state, repo)
}

async fn call(
    state: &AppState,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header("Authorization", format!("Bearer {TOKEN}"));
    let body = match body {
        Some(json) => {
            request = request.header("Content-Type", "application/json");
            Body::from(json.to_string())
        }
        None => Body::empty(),
    };
    let response = router(state.clone())
        .oneshot(request.body(body).expect("request"))
        .await
        .expect("response");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

async fn register(state: &AppState, repo: &Path) -> Value {
    let (status, project) = call(
        state,
        "POST",
        "/projects",
        Some(json!({ "directory": repo.display().to_string() })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{project}");
    project
}

#[tokio::test]
async fn test_projects_routes_register_get_and_list() {
    let (state, repo) = state_with_repo("register").await;

    let project = register(&state, &repo).await;
    let id = project["id"].as_str().expect("id").to_string();
    assert_eq!(project["name"], "app");
    assert_eq!(project["worktree"], repo.display().to_string());
    assert_eq!(project["remote"], Value::Null);
    assert_eq!(project["base_model"], Value::Null);

    // Registering a subdirectory of the same checkout is the same project.
    std::fs::create_dir_all(repo.join("src")).expect("subdir");
    let again = register(&state, &repo.join("src")).await;
    assert_eq!(again["id"], project["id"]);

    let (status, fetched) = call(&state, "GET", &format!("/projects/{id}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(fetched["id"], project["id"]);

    let (status, list) = call(&state, "GET", "/projects", None).await;
    assert_eq!(status, StatusCode::OK);
    let ids: Vec<&str> = list
        .as_array()
        .expect("array")
        .iter()
        .filter_map(|p| p["id"].as_str())
        .collect();
    assert_eq!(ids, vec![id.as_str()]);

    let (status, _) = call(&state, "GET", "/projects/no-such-project", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(&state, "GET", "/projects/no-such-project/sessions", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_projects_routes_register_rejects_directories_outside_permitted_roots() {
    let (state, _repo) = state_with_repo("register_outside").await;
    for directory in ["/", "/etc", "/no/such/directory"] {
        let (status, body) = call(
            &state,
            "POST",
            "/projects",
            Some(json!({ "directory": directory })),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{directory}");
        assert_eq!(body["error"], "invalid directory", "no host detail leaks");
    }
    let (_, list) = call(&state, "GET", "/projects", None).await;
    assert_eq!(list, json!([]), "nothing was registered");
}

#[tokio::test]
async fn test_projects_routes_session_in_project_with_and_without_branch() {
    let (state, repo) = state_with_repo("sessions").await;
    let project = register(&state, &repo).await;
    let id = project["id"].as_str().expect("id");

    let (status, plain) = call(
        &state,
        "POST",
        "/sessions",
        Some(json!({ "project_id": id })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{plain}");
    assert_eq!(plain["project_id"], project["id"]);
    assert_eq!(plain["directory"], repo.display().to_string());
    assert_eq!(plain["branch"], Value::Null);
    assert_eq!(plain["worktree"], Value::Null);

    let (status, branched) = call(
        &state,
        "POST",
        "/sessions",
        Some(json!({ "project_id": id, "branch": "feature/api" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{branched}");
    assert_eq!(branched["branch"], "feature/api");
    let worktree = PathBuf::from(branched["worktree"].as_str().expect("worktree"));
    assert!(
        worktree.starts_with(state.worktree_root.as_path()),
        "{}",
        worktree.display()
    );
    assert_eq!(
        git(&worktree, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "feature/api"
    );
    assert_eq!(branched["directory"], branched["worktree"]);

    let session_id = branched["id"].as_str().expect("id");
    let (status, fetched) = call(&state, "GET", &format!("/sessions/{session_id}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(fetched["project_id"], project["id"]);
    assert_eq!(fetched["branch"], "feature/api");
    assert_eq!(fetched["worktree"], branched["worktree"]);

    let (status, sessions) = call(&state, "GET", &format!("/projects/{id}/sessions"), None).await;
    assert_eq!(status, StatusCode::OK);
    let mut branches: Vec<Value> = sessions
        .as_array()
        .expect("array")
        .iter()
        .map(|s| s["branch"].clone())
        .collect();
    branches.sort_by_key(Value::to_string);
    assert_eq!(branches, vec![json!("feature/api"), Value::Null]);

    let (status, all) = call(&state, "GET", "/sessions", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        all.as_array()
            .expect("array")
            .iter()
            .all(|s| s["project_id"] == project["id"]),
        "every listed session reports its project: {all}"
    );
}

#[tokio::test]
async fn test_projects_routes_directory_sessions_are_attached_to_a_project() {
    let (state, repo) = state_with_repo("directory_form").await;

    let (status, created) = call(
        &state,
        "POST",
        "/sessions",
        Some(json!({ "directory": repo.display().to_string() })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let session_id = created["id"].as_str().expect("id");

    let (_, session) = call(&state, "GET", &format!("/sessions/{session_id}"), None).await;
    let project_id = session["project_id"].as_str().expect("project id");
    let (status, project) = call(&state, "GET", &format!("/projects/{project_id}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(project["worktree"], repo.display().to_string());
}

#[tokio::test]
async fn test_projects_routes_session_requests_are_validated() {
    let (state, repo) = state_with_repo("validation").await;
    let project = register(&state, &repo).await;
    let id = project["id"].as_str().expect("id");
    let dir = repo.display().to_string();

    for (body, expected) in [
        (json!({}), StatusCode::BAD_REQUEST),
        (
            json!({ "directory": dir, "project_id": id }),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({ "directory": dir, "branch": "x" }),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({ "project_id": "no-such-project" }),
            StatusCode::NOT_FOUND,
        ),
        (
            json!({ "project_id": id, "branch": "bad..name" }),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({ "project_id": id, "branch": "--force" }),
            StatusCode::BAD_REQUEST,
        ),
        (json!({ "directory": "/" }), StatusCode::BAD_REQUEST),
    ] {
        let (status, response) = call(&state, "POST", "/sessions", Some(body.clone())).await;
        assert_eq!(status, expected, "{body} -> {response}");
    }
    let (_, sessions) = call(&state, "GET", "/sessions", None).await;
    assert_eq!(sessions, json!([]), "no rejected request created a session");
    assert!(!state.worktree_root.exists(), "and no worktree was created");
}

#[tokio::test]
async fn test_projects_routes_project_outside_permitted_roots_cannot_host_a_session() {
    let (state, _repo) = state_with_repo("containment").await;
    // A project record can exist for a directory the server may not use
    // (created by the TUI in another launch directory, for example).
    let outside = state.storage.ensure_project("/").expect("project");

    let (status, body) = call(
        &state,
        "POST",
        "/sessions",
        Some(json!({ "project_id": outside.id })),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"], "invalid directory");
    assert!(state.storage.list_sessions().expect("list").is_empty());
}

#[tokio::test]
async fn test_projects_routes_remove_needs_no_live_sessions() {
    let (state, repo) = state_with_repo("remove").await;
    let project = register(&state, &repo).await;
    let id = project["id"].as_str().expect("id");
    let (_, session) = call(
        &state,
        "POST",
        "/sessions",
        Some(json!({ "project_id": id })),
    )
    .await;
    let session_id = session["id"].as_str().expect("session id");

    let (status, body) = call(&state, "DELETE", &format!("/projects/{id}"), None).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    let (status, _) = call(&state, "DELETE", &format!("/sessions/{session_id}"), None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = call(&state, "DELETE", &format!("/projects/{id}"), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (_, list) = call(&state, "GET", "/projects", None).await;
    assert_eq!(list, json!([]));
    assert!(
        repo.join("README.md").is_file(),
        "the project's files are kept"
    );

    let (status, _) = call(&state, "DELETE", "/projects/no-such-project", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_projects_routes_require_authentication() {
    let (state, _repo) = state_with_repo("auth").await;
    for (method, uri) in [
        ("GET", "/projects"),
        ("POST", "/projects"),
        ("GET", "/projects/x"),
        ("DELETE", "/projects/x"),
        ("GET", "/projects/x/sessions"),
    ] {
        let response = router(state.clone())
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "{method} {uri}"
        );
    }
}
