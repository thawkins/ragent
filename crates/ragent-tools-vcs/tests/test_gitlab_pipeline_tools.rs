//! Execute-path coverage for the GitLab pipeline and job tools
//! (`gitlab/gitlab_pipelines.rs`).
//!
//! Every `gitlab_*` pipeline/job tool had registry-name coverage only: the
//! tests asserted the tool exists, never that its `execute` builds the right
//! request. These drive each tool against a local `wiremock` server and pin the
//! request path, method and body together with the rendered output, so a
//! changed endpoint or a dropped query parameter fails here.
//!
//! The client is configured through the environment (`GITLAB_URL` +
//! `GITLAB_TOKEN`), which is the same layered loader production uses, so the
//! tools resolve their client exactly as they do in a session. Every test that
//! touches those variables holds `crate::global_env_lock()`; the guard is
//! intentionally held across the awaited tool call (serialisation, not
//! parallelism, is the point).

// Holds the crate-wide env lock across the body on purpose: it serialises the
// `GITLAB_*` environment against every other test in this binary.
#![allow(clippy::await_holding_lock)]

use std::sync::Arc;

use ragent_tools_vcs::ToolContext;
use ragent_tools_vcs::gitlab::{
    GitLabCancelJobTool, GitLabCancelPipelineTool, GitLabGetJobLogTool, GitLabGetJobTool,
    GitLabGetPipelineTool, GitLabListJobsTool, GitLabListPipelinesTool, GitLabRetryJobTool,
    GitLabRetryPipelineTool, Tool,
};
use ragent_tools_vcs::storage::StorageBackend;
use serde_json::{Value, json};
use tempfile::TempDir;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TOKEN: &str = "glpat-pipeline-test";

/// Minimal in-memory credential/settings store (always empty: the tests
/// configure GitLab through the environment instead).
#[derive(Default)]
struct EmptyStore;

impl StorageBackend for EmptyStore {
    fn get_provider_auth(&self, _provider_id: &str) -> anyhow::Result<Option<String>> {
        Ok(None)
    }
    fn set_provider_auth(&self, _provider_id: &str, _api_key: &str) -> anyhow::Result<()> {
        Ok(())
    }
    fn delete_provider_auth(&self, _provider_id: &str) -> anyhow::Result<()> {
        Ok(())
    }
    fn get_setting(&self, _key: &str) -> anyhow::Result<Option<String>> {
        Ok(None)
    }
    fn set_setting(&self, _key: &str, _value: &str) -> anyhow::Result<()> {
        Ok(())
    }
    fn delete_setting(&self, _key: &str) -> anyhow::Result<()> {
        Ok(())
    }
}

/// RAII guard restoring the named environment variables on drop.
struct EnvRestore(Vec<(&'static str, Option<String>)>);

impl EnvRestore {
    fn apply(changes: &[(&'static str, Option<String>)]) -> Self {
        let mut saved = Vec::new();
        for (name, value) in changes {
            let previous = std::env::var(name).ok();
            // SAFETY: test-only; `global_env_lock` serialises all callers.
            #[allow(unsafe_code)]
            unsafe {
                match value.clone() {
                    Some(v) => std::env::set_var(name, v),
                    None => std::env::remove_var(name),
                }
            }
            saved.push((*name, previous));
        }
        Self(saved)
    }
}

impl Drop for EnvRestore {
    fn drop(&mut self) {
        for (name, previous) in self.0.drain(..) {
            // SAFETY: test-only; see `apply`.
            #[allow(unsafe_code)]
            unsafe {
                match previous {
                    Some(v) => std::env::set_var(name, v),
                    None => std::env::remove_var(name),
                }
            }
        }
    }
}

/// Run `git` in `dir`, asserting success.
fn git(dir: &std::path::Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?} failed");
}

/// Build a working directory whose `origin` remote is a GitLab URL, and point
/// the client at `server` with [`TOKEN`].
///
/// Returns the context and the guard that restores the environment; the caller
/// must keep the guard alive for the whole test and hold
/// `crate::global_env_lock()`.
fn context_for(server: &MockServer, project_path: &str) -> (ToolContext, EnvRestore, TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    git(dir.path(), &["init", "-q"]);
    git(
        dir.path(),
        &[
            "remote",
            "add",
            "origin",
            &format!("https://gitlab.example.com/{project_path}.git"),
        ],
    );
    let working_dir = dir.path().to_path_buf();
    let env = EnvRestore::apply(&[
        ("GITLAB_URL", Some(server.uri())),
        ("GITLAB_TOKEN", Some(TOKEN.to_string())),
        ("GITLAB_USERNAME", Some("pipeline-tester".to_string())),
    ]);
    let ctx = ToolContext {
        session_id: "test-session".to_string(),
        working_dir,
        storage: Some(Arc::new(EmptyStore)),
        config: None,
    };
    // The handle is returned so the directory outlives the context.
    (ctx, env, dir)
}

/// URL-encoded API project path for `{project}` interpolation.
const ENC: &str = "group%2Fproject";

/// One pipeline object as the GitLab API returns it.
fn pipeline_json() -> Value {
    json!({
        "id": 501,
        "status": "failed",
        "ref": "main",
        "sha": "0123456789abcdef0123",
        "source": "push",
        "created_at": "2026-01-02T03:04:05Z",
        "started_at": "2026-01-02T03:05:00Z",
        "finished_at": "2026-01-02T03:09:30Z",
        "duration": 270.5,
        "web_url": "https://gitlab.example.com/group/project/-/pipelines/501",
        "user": {"username": "tester"}
    })
}

#[tokio::test]
async fn test_gitlab_list_pipelines_builds_query_and_renders_rows() {
    let _lock = crate::global_env_lock();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v4/projects/{ENC}/pipelines")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([pipeline_json()])))
        .mount(&server)
        .await;

    let (ctx, _env, _dir) = context_for(&server, "group/project");
    let tool = GitLabListPipelinesTool;
    let out = tool
        .execute(json!({"status": "failed", "ref": "main", "limit": 5}), &ctx)
        .await
        .expect("list pipelines");

    // `limit` becomes `per_page`, and the optional filters are appended.
    let request = &server.received_requests().await.expect("requests recorded")[0];
    let query = request.url.query().unwrap_or_default();
    assert!(query.contains("per_page=5"), "query: {query}");
    assert!(query.contains("status=failed"), "query: {query}");
    assert!(query.contains("ref=main"), "query: {query}");
    assert_eq!(
        request
            .headers
            .get("PRIVATE-TOKEN")
            .and_then(|v| v.to_str().ok()),
        Some(TOKEN),
        "the PAT must authenticate the request"
    );

    assert!(out.content.contains("Pipelines:"), "{}", out.content);
    assert!(
        out.content
            .contains("#501 [failed] ref:main (01234567) source:push"),
        "row must carry id, status, ref, short sha and source: {}",
        out.content
    );
    assert!(out.content.contains("[x]"), "failed renders the [x] icon");
    let meta = out.metadata.expect("metadata");
    assert_eq!(meta["count"], 1);
    assert_eq!(meta["project"], ENC);
}

#[tokio::test]
async fn test_gitlab_list_pipelines_empty_result_is_reported() {
    let _lock = crate::global_env_lock();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v4/projects/{ENC}/pipelines")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(&server)
        .await;

    let (ctx, _env, _dir) = context_for(&server, "group/project");
    let out = GitLabListPipelinesTool
        .execute(json!({}), &ctx)
        .await
        .expect("list pipelines");
    assert_eq!(out.content, "No pipelines found.");
    assert!(out.metadata.is_none());
}

#[tokio::test]
async fn test_gitlab_get_pipeline_renders_detail_and_requires_id() {
    let _lock = crate::global_env_lock();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v4/projects/{ENC}/pipelines/501")))
        .respond_with(ResponseTemplate::new(200).set_body_json(pipeline_json()))
        .mount(&server)
        .await;

    let (ctx, _env, _dir) = context_for(&server, "group/project");
    let tool = GitLabGetPipelineTool;
    let out = tool
        .execute(json!({"pipeline_id": 501}), &ctx)
        .await
        .expect("get pipeline");

    for expected in [
        "Pipeline #501",
        "Status:   failed",
        "Ref:      main (0123456789abcdef0123)",
        "User:     tester",
        "Duration: 270.5s",
        "URL:      https://gitlab.example.com/group/project/-/pipelines/501",
    ] {
        assert!(
            out.content.contains(expected),
            "missing {expected:?} in:\n{}",
            out.content
        );
    }
    assert_eq!(out.metadata.expect("metadata")["status"], "failed");

    // A missing id is refused before any request is made.
    let err = tool
        .execute(json!({}), &ctx)
        .await
        .expect_err("pipeline_id is required");
    assert!(err.to_string().contains("pipeline_id is required"));
}

#[tokio::test]
async fn test_gitlab_list_jobs_percent_encodes_scope_and_follows_pagination() {
    let _lock = crate::global_env_lock();
    let server = MockServer::start().await;
    // Page 1 advertises page 2; page 2 has no cursor.
    Mock::given(method("GET"))
        .and(path(format!("/api/v4/projects/{ENC}/pipelines/501/jobs")))
        .and(wiremock::matchers::query_param("page", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "id": 12, "name": "deploy", "stage": "deploy", "status": "success",
            "duration": 3.0, "runner": {"description": "runner-b"}
        }])))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v4/projects/{ENC}/pipelines/501/jobs")))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-next-page", "2")
                .set_body_json(json!([{
                    "id": 11, "name": "build", "stage": "build", "status": "failed",
                    "duration": 12.25, "runner": {"description": "runner-a"}
                }])),
        )
        .mount(&server)
        .await;

    let (ctx, _env, _dir) = context_for(&server, "group/project");
    let out = GitLabListJobsTool
        .execute(json!({"pipeline_id": 501, "scope": "success"}), &ctx)
        .await
        .expect("list jobs");

    let requests = server.received_requests().await.expect("requests recorded");
    assert_eq!(requests.len(), 2, "both pages must be fetched");
    let first_query = requests[0].url.query().unwrap_or_default();
    assert!(
        first_query.contains("scope[]=success"),
        "scope must be percent-encoded into the query: {first_query}"
    );

    assert!(out.content.contains("Jobs for pipeline #501:"));
    assert!(
        out.content
            .contains("[build] build (id:11) - failed, 12.2s, runner:runner-a"),
        "{}",
        out.content
    );
    assert!(
        out.content
            .contains("[ok] [deploy] deploy (id:12) - success, 3.0s, runner:runner-b"),
        "{}",
        out.content
    );
    assert_eq!(out.metadata.expect("metadata")["count"], 2);
}

#[tokio::test]
async fn test_gitlab_list_jobs_empty_result_is_reported() {
    let _lock = crate::global_env_lock();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v4/projects/{ENC}/pipelines/77/jobs")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(&server)
        .await;

    let (ctx, _env, _dir) = context_for(&server, "group/project");
    let out = GitLabListJobsTool
        .execute(json!({"pipeline_id": 77}), &ctx)
        .await
        .expect("list jobs");
    assert_eq!(out.content, "No jobs found for pipeline #77.");
}

#[tokio::test]
async fn test_gitlab_get_job_renders_failure_artifacts_and_allow_failure() {
    let _lock = crate::global_env_lock();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v4/projects/{ENC}/jobs/42")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 42,
            "name": "test",
            "stage": "test",
            "status": "failed",
            "ref": "main",
            "created_at": "2026-01-02T03:04:05Z",
            "started_at": "2026-01-02T03:04:10Z",
            "finished_at": null,
            "duration": null,
            "web_url": "https://gitlab.example.com/group/project/-/jobs/42",
            "runner": {"description": "runner-1"},
            "pipeline": {"id": 501},
            "failure_reason": "script_failure",
            "allow_failure": true,
            "artifacts": [{"filename": "report.xml"}, {"filename": "log.txt"}, {}]
        })))
        .mount(&server)
        .await;

    let (ctx, _env, _dir) = context_for(&server, "group/project");
    let out = GitLabGetJobTool
        .execute(json!({"job_id": 42}), &ctx)
        .await
        .expect("get job");

    for expected in [
        "Job: test (id:42)",
        "Status:        failed",
        "Pipeline:      #501",
        "Allow failure: true",
        "Finished:      not finished",
        "Duration:      N/A",
        "Runner:        runner-1",
        "Failure:       script_failure",
        "Artifacts:     report.xml, log.txt",
    ] {
        assert!(
            out.content.contains(expected),
            "missing {expected:?} in:\n{}",
            out.content
        );
    }
    assert_eq!(out.metadata.expect("metadata")["job_id"], 42);
}

#[tokio::test]
async fn test_gitlab_get_job_log_tails_plain_text_trace() {
    let _lock = crate::global_env_lock();
    let server = MockServer::start().await;
    // The trace endpoint returns plain text and names the project by id.
    Mock::given(method("GET"))
        .and(path(format!("/api/v4/projects/{ENC}/jobs/42/trace")))
        .respond_with(ResponseTemplate::new(200).set_body_string("one\ntwo\nthree\nfour"))
        .mount(&server)
        .await;

    let (ctx, _env, _dir) = context_for(&server, "group/project");
    let out = GitLabGetJobLogTool
        .execute(json!({"job_id": 42, "tail": 2}), &ctx)
        .await
        .expect("job log");

    assert!(
        out.content
            .contains("--- showing last 2 of 4 lines ---\nthree\nfour"),
        "{}",
        out.content
    );
    let meta = out.metadata.expect("metadata");
    assert_eq!(meta["total_lines"], 4);
    assert_eq!(meta["returned_lines"], 2);
    assert_eq!(meta["truncated"], true);
}

#[tokio::test]
async fn test_gitlab_get_job_log_surfaces_api_error() {
    let _lock = crate::global_env_lock();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v4/projects/{ENC}/jobs/42/trace")))
        .respond_with(ResponseTemplate::new(404).set_body_string("404 Not Found"))
        .mount(&server)
        .await;

    let (ctx, _env, _dir) = context_for(&server, "group/project");
    let err = GitLabGetJobLogTool
        .execute(json!({"job_id": 42}), &ctx)
        .await
        .expect_err("a 404 must fail");
    let message = err.to_string();
    assert!(message.contains("GitLab API error"), "got: {message}");
    assert!(message.contains("fetching job trace"), "got: {message}");
}

#[tokio::test]
async fn test_gitlab_retry_job_posts_and_reports_new_id() {
    let _lock = crate::global_env_lock();
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!("/api/v4/projects/{ENC}/jobs/42/retry")))
        .and(body_json(json!({})))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": 43, "name": "test", "status": "pending"
        })))
        .mount(&server)
        .await;

    let (ctx, _env, _dir) = context_for(&server, "group/project");
    let out = GitLabRetryJobTool
        .execute(json!({"job_id": 42}), &ctx)
        .await
        .expect("retry job");
    assert_eq!(
        out.content,
        "Retried job 'test': new job id=43, status=pending"
    );
    assert_eq!(out.metadata.expect("metadata")["new_job_id"], 43);
}

#[tokio::test]
async fn test_gitlab_cancel_job_posts_and_reports_status() {
    let _lock = crate::global_env_lock();
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!("/api/v4/projects/{ENC}/jobs/42/cancel")))
        .and(body_json(json!({})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"id": 42, "name": "test", "status": "canceled"})),
        )
        .mount(&server)
        .await;

    let (ctx, _env, _dir) = context_for(&server, "group/project");
    let out = GitLabCancelJobTool
        .execute(json!({"job_id": 42}), &ctx)
        .await
        .expect("cancel job");
    assert_eq!(out.content, "Cancelled job 'test' (id:42), status=canceled");
}

#[tokio::test]
async fn test_gitlab_retry_pipeline_posts_and_reports_status() {
    let _lock = crate::global_env_lock();
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!("/api/v4/projects/{ENC}/pipelines/501/retry")))
        .and(body_json(json!({})))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"id": 501, "status": "pending"})),
        )
        .mount(&server)
        .await;

    let (ctx, _env, _dir) = context_for(&server, "group/project");
    let out = GitLabRetryPipelineTool
        .execute(json!({"pipeline_id": 501}), &ctx)
        .await
        .expect("retry pipeline");
    assert_eq!(
        out.content,
        "Retried failed jobs in pipeline #501, status=pending"
    );
    assert_eq!(out.metadata.expect("metadata")["pipeline_id"], 501);
}

#[tokio::test]
async fn test_gitlab_cancel_pipeline_posts_and_reports_status() {
    let _lock = crate::global_env_lock();
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!("/api/v4/projects/{ENC}/pipelines/501/cancel")))
        .and(body_json(json!({})))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"id": 501, "status": "canceled"})),
        )
        .mount(&server)
        .await;

    let (ctx, _env, _dir) = context_for(&server, "group/project");
    let out = GitLabCancelPipelineTool
        .execute(json!({"pipeline_id": 501}), &ctx)
        .await
        .expect("cancel pipeline");
    assert_eq!(out.content, "Cancelled pipeline #501, status=canceled");
}

#[tokio::test]
async fn test_write_tools_require_a_mutating_permission_category() {
    // The registry classifies a tool by this string; a pipeline mutation that
    // declared `gitlab:read` would run under the read-only permission set.
    assert_eq!(GitLabRetryJobTool.permission_category(), "gitlab:write");
    assert_eq!(GitLabCancelJobTool.permission_category(), "gitlab:write");
    assert_eq!(
        GitLabRetryPipelineTool.permission_category(),
        "gitlab:write"
    );
    assert_eq!(
        GitLabCancelPipelineTool.permission_category(),
        "gitlab:write"
    );
    assert_eq!(GitLabListPipelinesTool.permission_category(), "gitlab:read");
}
