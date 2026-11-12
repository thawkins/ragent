//! Per-project services (spec MULTISESSION, milestone M8 phase A).
//!
//! A session in another project gets that project's code index; the launch
//! project's stays parked; the other project's is released once no open
//! session uses it. Fixture projects live under the git-ignored
//! `target/temp/` tree (each with its own `.git`, so it is its own project).

use std::path::{Path, PathBuf};

use ragent_tui::App;

#[path = "support/mod.rs"]
mod support;

/// A fresh project directory with `.git`, a `.ragent/ragent.json` that
/// enables code indexing, and one source file.
fn indexed_project(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/project_services")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir); // INTENTIONAL: may not exist yet
    std::fs::create_dir_all(dir.join(".git")).expect(".git");
    std::fs::create_dir_all(dir.join(".ragent")).expect(".ragent");
    std::fs::write(
        dir.join(".ragent/ragent.json"),
        r#"{ "code_index": { "enabled": true } }"#,
    )
    .expect("ragent.json");
    std::fs::create_dir_all(dir.join("src")).expect("src");
    std::fs::write(dir.join("src/lib.rs"), "pub fn answer() -> u32 { 42 }\n").expect("lib.rs");
    dir.canonicalize().expect("canonical project")
}

/// An app focused on session A in the launch directory, plus a stored
/// session in `project`; returns `(app, a, b)`.
async fn app_with_project_session(project: &Path) -> (App, String, String) {
    let mut app = support::make_app();
    app.execute_slash_command("/session list").await;
    let a = app.session.session_id.clone().expect("session A");
    let b = app
        .session_processor
        .session_manager
        .create_session(project.to_path_buf())
        .expect("session in project B")
        .id;
    (app, a, b)
}

/// Stops every watcher the test started.
fn shut_down(mut app: App) {
    app.stop_project_services();
    drop(app.code_index_watch_session.take());
}

#[tokio::test]
async fn test_project_services_focusing_another_project_opens_its_index() {
    let project = indexed_project("opens_index");
    let (mut app, _a, b) = app_with_project_session(&project).await;
    let launch_index = app.code_index.clone();

    app.switch_to_session(&b).expect("focus B");

    let index = app.code_index.clone().expect("project B's index is open");
    assert_eq!(index.project_root(), project.as_path());
    let routed = app
        .session_processor
        .code_index_for(&project.join("src"))
        .expect("tools in B get B's index");
    assert!(std::sync::Arc::ptr_eq(&routed, &index));
    assert!(
        app.session_processor
            .code_index_for(&app.cwd_path.join(".."))
            .is_none(),
        "a directory in no open project has no index"
    );
    let _ = launch_index; // INTENTIONAL: the launch index (None here) is parked, not dropped
    shut_down(app);
}

#[tokio::test]
async fn test_project_services_switching_back_restores_the_launch_project() {
    let project = indexed_project("restores_launch");
    let (mut app, a, b) = app_with_project_session(&project).await;
    let launch_cwd = app.cwd_path.clone();

    app.switch_to_session(&b).expect("focus B");
    app.switch_to_session(&a).expect("back to A");

    assert_eq!(app.cwd_path, launch_cwd);
    assert!(
        app.code_index.is_none(),
        "the launch project (no index here) is restored"
    );
    let parked = app
        .project_services
        .get(&project)
        .expect("B's services are parked while B is open in the background");
    assert!(parked.code_index.is_some());
    shut_down(app);
}

#[tokio::test]
async fn test_project_services_released_when_last_session_closes() {
    let project = indexed_project("released");
    let (mut app, _a, b) = app_with_project_session(&project).await;

    app.switch_to_session(&b).expect("focus B");
    app.execute_slash_command("/session close").await;

    assert!(
        !app.project_services.contains_key(&project),
        "no open session uses B, so its services are released"
    );
    assert!(app.session_processor.code_index_for(&project).is_none());
    shut_down(app);
}

#[tokio::test]
async fn test_project_services_project_without_ragent_dir_gets_no_index() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/project_services/plain_project");
    let _ = std::fs::remove_dir_all(&dir); // INTENTIONAL: may not exist yet
    std::fs::create_dir_all(dir.join(".git")).expect(".git");
    let dir = dir.canonicalize().expect("canonical");
    let (mut app, _a, b) = app_with_project_session(&dir).await;

    app.switch_to_session(&b).expect("focus B");

    assert!(app.code_index.is_none());
    assert!(
        !dir.join(".ragent").exists(),
        "focusing a session must not create files in the project"
    );
    shut_down(app);
}

/// A local streamable-HTTP MCP server advertising one `ping` tool.
async fn spawn_mcp_server() -> std::net::SocketAddr {
    use axum::response::IntoResponse;
    async fn handler(body: String) -> axum::response::Response {
        let request: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
        let id = request.get("id").cloned().unwrap_or(serde_json::json!(1));
        let sse = |frame: serde_json::Value| {
            (
                [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
                format!("event: message\ndata: {frame}\n\n"),
            )
                .into_response()
        };
        match request.get("method").and_then(|m| m.as_str()).unwrap_or("") {
            "initialize" => {
                let mut response = sse(serde_json::json!({
                    "jsonrpc": "2.0", "id": id,
                    "result": { "protocolVersion": "2024-11-05" }
                }));
                response.headers_mut().insert(
                    "mcp-session-id",
                    axum::http::HeaderValue::from_static("tui-project-mcp"),
                );
                response
            }
            "tools/list" => sse(serde_json::json!({
                "jsonrpc": "2.0", "id": id,
                "result": { "tools": [{
                    "name": "ping", "description": "ping",
                    "inputSchema": { "type": "object" }
                }] }
            })),
            _ => axum::http::StatusCode::ACCEPTED.into_response(),
        }
    }
    let router = axum::Router::new().route("/mcp", axum::routing::post(handler));
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind mock MCP server");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await; // INTENTIONAL: test server lifetime
    });
    addr
}

/// Polls `check` for up to five seconds.
async fn eventually(mut check: impl FnMut() -> bool) -> bool {
    for _ in 0..100 {
        if check() {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    false
}

#[tokio::test]
async fn test_project_services_project_mcp_opens_lists_and_releases() {
    let addr = spawn_mcp_server().await;
    let project = indexed_project("project_mcp");
    // A project's own ragent.json may not add MCP servers (SEC-001), so the
    // server comes from the user-level config, injected here as the
    // processor's config. Without its own ragent.json, B runs under it.
    std::fs::remove_file(project.join(".ragent/ragent.json")).expect("drop project config");
    let (mut app, _a, b) = app_with_project_session(&project).await;
    let mut config = ragent_config::Config::default();
    config.code_index.enabled = false;
    config.mcp.insert(
        "projsrv".to_string(),
        serde_json::from_value(serde_json::json!({
            "type": "http",
            "url": format!("http://{addr}/mcp")
        }))
        .expect("server config"),
    );
    *app.session_processor.cached_config.lock() =
        Some(ragent_agent::session::processor::CachedConfig {
            config: std::sync::Arc::new(config),
            file_mtimes: Vec::new(),
            env_overrides_present: false,
        });

    app.switch_to_session(&b).expect("focus B");
    let processor = std::sync::Arc::clone(&app.session_processor);
    assert!(
        eventually(|| {
            processor
                .tool_registry_for(&project)
                .get(
                    &ragent_agent::tool::mcp_tool::McpToolWrapper::ragent_name_for(
                        "projsrv", "ping",
                    ),
                )
                .is_some()
        })
        .await,
        "B's sessions get B's MCP tool"
    );

    app.execute_slash_command("/mcp").await;
    let listing = app
        .session
        .messages
        .last()
        .map(|m| m.text_content())
        .unwrap_or_default();
    assert!(
        listing.contains("projsrv"),
        "/mcp lists B's server: {listing}"
    );

    app.execute_slash_command("/session close").await;
    assert!(
        eventually(|| processor.project_mcp_client(&project).is_none()).await,
        "B's servers are released with its last session"
    );
    shut_down(app);
}
