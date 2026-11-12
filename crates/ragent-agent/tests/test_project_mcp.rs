//! Per-project MCP tool registries (spec MULTISESSION, milestone M8 phase B).
//!
//! A project that opens its own MCP servers gets its own tool registry: the
//! base tools without the launch project's MCP tools, plus its own servers'
//! tools. A local HTTP mock serves both the "launch" and the "project"
//! server, so nothing is spawned and nothing leaves the machine. The project
//! root is a fixture under the git-ignored `target/temp/` tree.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::Router;
use axum::response::IntoResponse;
use axum::routing::post;
use ragent_agent::mcp::McpClient;
use ragent_agent::tool::mcp_tool::McpToolWrapper;
use ragent_config::{McpServerConfig, McpTransport};
use serde_json::json;

#[allow(dead_code)] // INTENTIONAL: this file uses only `test_processor`
#[path = "task_support/mod.rs"]
mod task_support;

/// A minimal streamable-HTTP MCP server advertising one `ping` tool.
async fn handler(body: String) -> axum::response::Response {
    let request: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    let id = request.get("id").cloned().unwrap_or(json!(1));
    let sse = |frame: serde_json::Value| {
        (
            [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
            format!("event: message\ndata: {frame}\n\n"),
        )
            .into_response()
    };
    match request.get("method").and_then(|m| m.as_str()).unwrap_or("") {
        "initialize" => {
            let mut response = sse(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "protocolVersion": "2024-11-05" }
            }));
            response.headers_mut().insert(
                "mcp-session-id",
                axum::http::HeaderValue::from_static("project-mcp"),
            );
            response
        }
        "tools/list" => sse(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": { "tools": [{
                "name": "ping",
                "description": "ping",
                "inputSchema": { "type": "object" }
            }] }
        })),
        // Notifications and anything else: accepted, nothing to return.
        _ => axum::http::StatusCode::ACCEPTED.into_response(),
    }
}

async fn spawn_server() -> SocketAddr {
    let router = Router::new().route("/mcp", post(handler));
    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .expect("bind mock MCP server");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await; // INTENTIONAL: test server lifetime
    });
    addr
}

fn http_config(addr: SocketAddr) -> McpServerConfig {
    McpServerConfig {
        type_: McpTransport::Http,
        url: Some(format!("http://{addr}/mcp")),
        ..McpServerConfig::default()
    }
}

/// A fresh project root (it has its own `.git`) with a `src` directory.
fn project(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/project_mcp")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir); // INTENTIONAL: may not exist yet
    std::fs::create_dir_all(dir.join(".git")).expect(".git");
    std::fs::create_dir_all(dir.join("src")).expect("src");
    dir.canonicalize().expect("canonical project")
}

/// Puts the built-in `read` tool into the (empty) test base registry, to
/// stand in for the built-ins a real processor has.
fn register_builtin(processor: &ragent_agent::session::processor::SessionProcessor) {
    let read = ragent_agent::tool::create_default_registry()
        .get("read")
        .expect("built-in read tool");
    processor.tool_registry.register(read);
}

#[tokio::test]
async fn test_project_mcp_registry_holds_only_its_own_mcp_tools() {
    let addr = spawn_server().await;
    let processor = task_support::test_processor();
    register_builtin(&processor);
    // The launch project's server, registered into the base registry as at
    // startup.
    let launch_client = Arc::new(tokio::sync::RwLock::new(McpClient::new()));
    launch_client
        .write()
        .await
        .connect("launchsrv", http_config(addr))
        .await
        .expect("launch server");
    processor.set_mcp_client(Arc::clone(&launch_client)).await;
    let launch_tool = McpToolWrapper::ragent_name_for("launchsrv", "ping");
    let project_tool = McpToolWrapper::ragent_name_for("projsrv", "ping");
    let root = project("own_tools");

    let connected = processor
        .open_project_mcp(
            root.clone(),
            vec![("projsrv".to_string(), http_config(addr))],
        )
        .await;
    assert_eq!(connected, 1);

    let in_project = processor.tool_registry_for(&root.join("src"));
    assert!(
        in_project.get(&project_tool).is_some(),
        "the project's own MCP tool"
    );
    assert!(
        in_project.get(&launch_tool).is_none(),
        "not the launch project's"
    );
    assert!(in_project.get("read").is_some(), "built-in tools are kept");

    let launch = processor.tool_registry_for(&std::env::current_dir().expect("cwd"));
    assert!(launch.get(&launch_tool).is_some());
    assert!(launch.get(&project_tool).is_none());
    processor.shutdown_mcp().await;
}

#[tokio::test]
async fn test_project_mcp_registry_follows_base_registry_changes() {
    let addr = spawn_server().await;
    let processor = task_support::test_processor();
    register_builtin(&processor);
    let root = project("follows_base");
    processor
        .open_project_mcp(
            root.clone(),
            vec![("projsrv".to_string(), http_config(addr))],
        )
        .await;
    assert!(processor.tool_registry_for(&root).get("read").is_some());

    processor.tool_registry.remove_all(&["read".to_string()]);

    assert!(
        processor.tool_registry_for(&root).get("read").is_none(),
        "the project registry is rebuilt when the base registry changes"
    );
    processor.shutdown_mcp().await;
}

#[tokio::test]
async fn test_project_mcp_release_falls_back_to_base_registry() {
    let addr = spawn_server().await;
    let processor = task_support::test_processor();
    register_builtin(&processor);
    let root = project("release");
    processor
        .open_project_mcp(
            root.clone(),
            vec![("projsrv".to_string(), http_config(addr))],
        )
        .await;
    assert!(processor.project_mcp_client(&root).is_some());

    processor.release_project_mcp(&root).await;

    assert!(processor.project_mcp_client(&root).is_none());
    assert!(
        Arc::ptr_eq(
            &processor.tool_registry_for(&root),
            &processor.tool_registry
        ),
        "a released project's sessions use the base registry"
    );
}

#[test]
fn test_project_mcp_registries_never_share_a_version() {
    let a = ragent_agent::tool::ToolRegistry::new();
    let b = ragent_agent::tool::ToolRegistry::new();
    assert_ne!(
        a.version(),
        b.version(),
        "version-keyed caches must tell two registries apart"
    );
}

#[tokio::test]
async fn test_project_mcp_stdio_servers_start_in_the_project_root() {
    use std::os::unix::fs::PermissionsExt;
    let root = project("stdio_cwd");
    // Not a real MCP server: a script that records its working directory in
    // a file named by a *relative* path and exits. The connect fails; the
    // file landing in the project root is what is under test.
    let script = root.join("probe.sh");
    std::fs::write(&script, "#!/bin/sh\npwd > started_in.txt\n").expect("probe script");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let config = McpServerConfig {
        type_: McpTransport::Stdio,
        command: Some(script.display().to_string()),
        ..McpServerConfig::default()
    };
    let mut client = McpClient::with_working_dir(root.clone());

    let _ = client.connect("cwd-probe", config).await; // INTENTIONAL: expected to fail

    let marker = root.join("started_in.txt");
    let mut started_in = String::new();
    for _ in 0..40 {
        if let Ok(text) = std::fs::read_to_string(&marker) {
            started_in = text;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert_eq!(std::path::PathBuf::from(started_in.trim()), root);
    client.shutdown().await;
}
