//! Integration tests for the OpenAPI 3.1 document (spec `openhands` FR-007).
//!
//! FR-007 requires the document to be *derived from the live router definition
//! so it cannot drift from the served routes*. These tests close that loop from
//! both directions against the real source tree:
//!
//! - **served -> documented:** every `.route(...)`/`.nest(...)` literal in
//!   `src/routes/` is scraped (comments stripped, paren-balanced) and compared
//!   to [`openapi::ROUTES`]; a route added to the router without a documented
//!   entry fails here.
//! - **documented -> served:** the same comparison catches a documented path
//!   that no route serves.
//! - **runtime:** `GET /openapi.json` serves a valid 3.1 document, and every
//!   documented non-SSE, parameterless `GET` path is reachable through the live
//!   router (a registered path never answers `404`).

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use ragent_agent::Config;
use ragent_agent::event::EventBus;
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::ProviderRegistry;
use ragent_agent::session::SessionManager;
use ragent_agent::session::processor::SessionProcessor;
use ragent_agent::storage::Storage;
use ragent_agent::tool::ToolRegistry;
use ragent_server::routes::{AppState, openapi, router};
use tower::ServiceExt;

/// Minimal [`AppState`] for exercising the router without a live model.
fn test_state(token: &str) -> AppState {
    let storage = Arc::new(Storage::open_in_memory().unwrap());
    let event_bus = Arc::new(EventBus::new(16));
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));
    let processor = Arc::new(SessionProcessor {
        session_manager,
        provider_registry: Arc::new(ProviderRegistry::new()),
        tool_registry: Arc::new(ToolRegistry::new()),
        permission_checker: Arc::new(parking_lot::RwLock::new(PermissionChecker::new(vec![]))),
        event_bus: event_bus.clone(),
        agent_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        connector_session: tokio::sync::RwLock::new(None),
        connector_statuses: tokio::sync::RwLock::new(None),
        code_index: std::sync::OnceLock::new(),
        stream_config: Default::default(),
        extraction_engine: std::sync::OnceLock::new(),
        auto_approve: false,
        active_spec: tokio::sync::RwLock::new(None),
        spec_manager: std::sync::OnceLock::new(),
        cached_tool_definitions: parking_lot::RwLock::new(None),
        cached_tool_names: parking_lot::RwLock::new(None),
        cached_tool_definition_bytes: parking_lot::RwLock::new(None),
        llm_client_cache: parking_lot::RwLock::new(std::collections::HashMap::new()),
        cached_config: parking_lot::Mutex::new(None),
        team_context_cache: std::sync::Arc::new(parking_lot::RwLock::new(
            std::collections::HashMap::new(),
        )),
        tool_repeat_guard: std::sync::Arc::new(parking_lot::Mutex::new(
            std::collections::HashMap::new(),
        )),
        system_prompt_cache: parking_lot::RwLock::new(None),
        skill_body_cache: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        telemetry: std::sync::Arc::new(ragent_agent::telemetry::TelemetrySubsystem::disabled()),
        activity_log: std::sync::OnceLock::new(),
        activity_log_tx: tokio::sync::Mutex::new(None),
        skill_registry_cache: parking_lot::Mutex::new(None),
        active_loops: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        active_loop_specs: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        loop_telemetry_recorded: std::sync::atomic::AtomicBool::new(false),
        active_loop_interrupts: parking_lot::RwLock::new(std::collections::HashMap::new()),
        active_loop_captures: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        last_message_end_reason: std::sync::RwLock::new(std::collections::HashMap::new()),
        bg_service: std::sync::OnceLock::new(),
    });
    AppState {
        event_bus,
        config: Arc::new(tokio::sync::RwLock::new(Config::default())),
        storage,
        session_processor: processor,
        auth_token: token.to_string(),
        rate_limiter: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        coordinator: None,
        research_runs: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
    }
}

/// Directory holding the route sources under test.
fn routes_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/routes")
}

/// Remove `//` line comments (the route sources contain no string literals that
/// embed `//`, so a line scan is sufficient).
fn strip_line_comments(source: &str) -> String {
    source
        .lines()
        .map(|line| match line.find("//") {
            Some(idx) => &line[..idx],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Find `needle` occurrences and return the byte offset of the opening
/// parenthesis of the balanced `(...)` group that follows, and the group text.
fn balanced_group(source: &str, open_index: usize) -> Option<String> {
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    let mut start = None;
    for (offset, byte) in bytes.iter().enumerate().skip(open_index) {
        match byte {
            b'(' => {
                if start.is_none() {
                    start = Some(offset);
                }
                depth += 1;
            }
            b')' => {
                depth -= 1;
                if depth == 0 {
                    let s = start?;
                    return Some(source[s + 1..offset].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

/// Extract the first string literal in `text`, e.g. `"/sessions"`.
fn first_string_literal(text: &str) -> Option<String> {
    let start = text.find('"')?;
    let rest = &text[start + 1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Every method token used by a `.route(...)` block, uppercase and sorted.
fn methods_in(block: &str) -> Vec<String> {
    const METHODS: [&str; 5] = ["get", "post", "delete", "put", "patch"];
    let bytes = block.as_bytes();
    let mut found = BTreeSet::new();
    for method in METHODS {
        let needle = format!("{method}(");
        let mut from = 0;
        while let Some(rel) = block[from..].find(&needle) {
            let abs = from + rel;
            // Accept both spellings: `get(x)` and the chained `.delete(x)`.
            // The preceding byte must not be alphanumeric or `_`, so a longer
            // identifier that merely ends in a method name is not matched.
            let preceded_ok =
                abs == 0 || !bytes[abs - 1].is_ascii_alphanumeric() && bytes[abs - 1] != b'_';
            if preceded_ok {
                found.insert(method.to_uppercase());
            }
            from = abs + needle.len();
        }
    }
    found.into_iter().collect()
}

/// Parse every `.route("<path>", <methods>)` call in `source`.
fn parse_route_calls(source: &str) -> Vec<(String, Vec<String>)> {
    let mut routes = Vec::new();
    let mut search_from = 0;
    while let Some(rel) = source[search_from..].find(".route(") {
        // Walk into the balanced `(...)` group that follows `.route`.
        let group_open = search_from + rel + ".route".len();
        let Some(group) = balanced_group(source, group_open) else {
            break;
        };
        // `.route("/path", ...)` - the path is the leading string literal.
        if let Some((path, rest)) = group.split_once(',') {
            if let Some(path) = first_string_literal(path) {
                routes.push((path, methods_in(rest)));
            }
        }
        search_from = group_open + 1;
    }
    routes
}

/// Parse every `.nest("<prefix>", <module>::...)` call in `source`, mapping the
/// nest prefix to the module name the nested router comes from.
fn parse_nest_prefixes(source: &str) -> Vec<(String, String)> {
    let mut nests = Vec::new();
    let mut search_from = 0;
    while let Some(rel) = source[search_from..].find(".nest(") {
        let group_open = search_from + rel + ".nest".len();
        let Some(group) = balanced_group(source, group_open) else {
            break;
        };
        let Some(prefix) = first_string_literal(&group) else {
            break;
        };
        // The module that owns the nested router, e.g. `memory` in
        // `memory::memory_routes()`.
        let module = group
            .split_once(',')
            .and_then(|(_, rest)| rest.split_once("::"))
            .map(|(module, _)| module.trim().to_string())
            .unwrap_or_default();
        nests.push((prefix, module));
        search_from = group_open + 1;
    }
    nests
}

/// Build the served `(METHOD, /path)` set from the route sources on disk.
fn served_routes() -> BTreeSet<(String, String)> {
    let dir = routes_dir();
    let mod_source = strip_line_comments(
        &std::fs::read_to_string(dir.join("mod.rs")).expect("read routes/mod.rs"),
    );
    let nests = parse_nest_prefixes(&mod_source);
    let prefix_for = |module: &str| -> String {
        nests
            .iter()
            .find(|(_, m)| m == module)
            .map(|(prefix, _)| prefix.clone())
            .unwrap_or_default()
    };

    let mut served = BTreeSet::new();
    for entry in std::fs::read_dir(&dir).expect("read routes dir") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        // `mod.rs` holds the top-level routes (no prefix); the module files
        // contribute relative routes that the matching nest prefixes.
        let prefix = if stem == "mod" {
            String::new()
        } else {
            prefix_for(&stem)
        };
        let source = strip_line_comments(&std::fs::read_to_string(&path).expect("read route file"));
        for (route_path, methods) in parse_route_calls(&source) {
            // Skip axum's internal test-only `/` root spelling that a nested
            // router may carry; combine the nest prefix with the route path.
            let full = if prefix.is_empty() {
                route_path.clone()
            } else if route_path == "/" {
                format!("{prefix}/")
            } else {
                format!("{prefix}{route_path}")
            };
            for method in methods {
                served.insert((method, full.clone()));
            }
        }
    }
    served
}

/// Build the documented `(METHOD, /path)` set from [`openapi::ROUTES`].
fn documented_routes() -> BTreeSet<(String, String)> {
    openapi::ROUTES
        .iter()
        .map(|op| (op.method.to_uppercase(), op.path.to_string()))
        .collect()
}

#[tokio::test]
async fn test_openapi_json_served_without_auth() {
    let app = router(test_state("secret"));
    let req = Request::builder()
        .uri("/openapi.json")
        .body(Body::empty())
        .expect("request");
    let resp = app.oneshot(req).await.expect("oneshot");
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "the OpenAPI document must be reachable without a token"
    );
    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    let doc: serde_json::Value = serde_json::from_slice(&bytes).expect("valid JSON");
    assert_eq!(doc["openapi"], "3.1.0");
    assert!(
        doc["paths"].is_object(),
        "the document must carry a `paths` object"
    );
    assert_eq!(doc["info"]["title"], "ragent REST API");
}

#[tokio::test]
async fn test_document_paths_match_every_served_route() {
    let served = served_routes();
    let documented = documented_routes();

    let missing_from_doc: Vec<_> = served.difference(&documented).collect();
    let missing_from_router: Vec<_> = documented.difference(&served).collect();

    assert!(
        missing_from_doc.is_empty(),
        "routes served but missing from the OpenAPI document (FR-007): {missing_from_doc:?}"
    );
    assert!(
        missing_from_router.is_empty(),
        "paths documented but not served by the router (drift the other way): {missing_from_router:?}"
    );
}

#[tokio::test]
async fn test_every_documented_parameterless_get_is_reachable() {
    let app = router(test_state("tok"));
    for op in openapi::ROUTES {
        if op.method != "GET" || op.sse || !openapi::path_params(op.path).is_empty() {
            continue;
        }
        // A nested router's `/` route (`/research/`) is served at the nest
        // prefix with the trailing slash normalised away, so probe both.
        let mut candidates = vec![op.path.to_string()];
        if let Some(stripped) = op.path.strip_suffix('/') {
            if !stripped.is_empty() {
                candidates.push(stripped.to_string());
            }
        }

        let mut status = StatusCode::NOT_FOUND;
        for candidate in &candidates {
            let req = Request::builder()
                .uri(candidate)
                .header("Authorization", "Bearer tok")
                .body(Body::empty())
                .expect("request");
            let resp = app.clone().oneshot(req).await.expect("oneshot");
            status = resp.status();
            if status != StatusCode::NOT_FOUND {
                break;
            }
        }
        assert_ne!(
            status,
            StatusCode::NOT_FOUND,
            "documented path {} is not registered on the router (FR-007)",
            op.path
        );
    }
}

#[test]
fn test_openapi_operation_and_client_generation() {
    let doc = openapi::document();
    let paths = doc["paths"].as_object().expect("paths object");

    // Every operation carries a stable operationId and a tag.
    let mut ids = BTreeSet::new();
    for (path, methods) in paths {
        let methods = methods.as_object().expect("method map");
        for op in methods.values() {
            let id = op["operationId"].as_str().expect("operationId");
            assert!(
                ids.insert(id.to_string()),
                "operationId '{id}' is duplicated"
            );
            assert!(op["tags"].as_array().is_some_and(|t| !t.is_empty()));
            assert!(op["responses"]["200"].is_object());
        }
        // Path parameters are declared.
        let params = openapi::path_params(path);
        if !params.is_empty() {
            let declared: Vec<String> = methods
                .values()
                .flat_map(|op| {
                    op["parameters"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                        .filter_map(|p| p["name"].as_str().map(str::to_string))
                        .collect::<Vec<_>>()
                })
                .collect();
            for name in params {
                assert!(
                    declared.contains(&name),
                    "path parameter '{name}' in '{path}' is undeclared"
                );
            }
        }
    }

    // Protected operations declare the bearer scheme; public ones do not.
    assert_eq!(
        doc["paths"]["/config"]["get"]["security"][0]["bearerAuth"],
        serde_json::json!([])
    );
    assert!(doc["paths"]["/health"]["get"].get("security").is_none());

    let client = openapi::typescript_client();
    // The client must expose an operation for every documented operation.
    for op in openapi::ROUTES {
        let id = openapi::operation_id(op);
        assert!(
            client.contains(&format!("async {id}(")),
            "generated client is missing {id}"
        );
    }
    // FR-035: the document and client carry no secret material.
    let text = format!("{doc}\n{client}");
    for needle in ["password", "api_key", "secret-token", "Bearer sk-"] {
        assert!(
            !text.contains(needle),
            "generated artefact must not embed secrets (found '{needle}')"
        );
    }
}

#[test]
fn test_checked_in_client_matches_the_generator() {
    let generated = openapi::typescript_client();
    let on_disk = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/openapi/ragent-client.d.ts"),
    )
    .unwrap_or_default();
    assert!(
        !on_disk.is_empty(),
        "docs/openapi/ragent-client.d.ts is missing"
    );
    assert_eq!(
        generated, on_disk,
        "the checked-in typed client has drifted from the generator; \
         regenerate with `ragent openapi --client`"
    );
}
