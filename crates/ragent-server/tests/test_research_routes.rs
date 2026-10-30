//! HTTP endpoint tests for the research API (RESEARCHPLAN.md Phase 4).
//!
//! Tests the Axum routes for the research system using in-process `oneshot`
//! requests. The vault root comes from `AppState::research_root`, which here
//! is the git-ignored `target/temp/ragent-server-research/research` sandbox
//! (see [`crate::research_sandbox`]) - never `cwd/research` - so a test run
//! cannot rewrite the checked-in `crates/ragent-server/research/` tree.
//! Items still get unique `test-` prefixed names and are removed after each
//! test, and filesystem-touching tests stay serialized so concurrent
//! INDEX.md writes cannot race.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use ragent_research::ResearchManager;
use ragent_server::routes::{AppState, router};
use tower::ServiceExt;

/// Global mutex to serialize tests that touch the shared research sandbox.
static FS_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Build a test state whose research vault root is `research_root`.
///
/// Built on top of the shared [`crate::support::test_state`] fixture so this
/// file does not carry its own copy of the `AppState`/`SessionProcessor`
/// constructor.
fn research_state(token: &str, research_root: &std::path::Path) -> AppState {
    let mut state = crate::support::test_state(token);
    state.research_root = std::sync::Arc::new(research_root.to_path_buf());
    state
}

/// Helper to build an authenticated GET request.
fn auth_get(token: &str, uri: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

/// Helper to build an authenticated DELETE request.
fn auth_delete(token: &str, uri: &str) -> Request<Body> {
    Request::builder()
        .method("DELETE")
        .uri(uri)
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

/// Helper to build an authenticated POST request with a JSON body.
fn auth_post(token: &str, uri: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

/// Read the response body as a UTF-8 string.
async fn body_string(resp: axum::http::Response<Body>) -> String {
    use http_body_util::BodyExt;
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// Unique test item name to avoid conflicts with real research items.
fn test_name() -> String {
    format!(
        "test-research-routes-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
}

/// Create a research item in `root` and wait until it is visible there.
///
/// Each test owns its vault root: `ResearchManager::refresh_index` republishes
/// `INDEX.md` from the whole vault, so two tests sharing a root overwrite each
/// other's index.
async fn create_test_item(root: &std::path::Path, name: &str, topic: &str) {
    let mgr = ResearchManager::new(root);
    // A write can fail when another test's `POST /research` background run
    // deletes the whole sandbox while this one is publishing (see
    // `crate::research_sandbox::isolated_research_root`, which is what makes the
    // sandbox safe to wipe at all). Retrying a failed create keeps the
    // helper's promise - an empty vault at this test's root, and this item in
    // it - instead of failing on another test's cleanup.
    let mut last_err = None;
    for _ in 0..20 {
        match mgr.create(name, &format!("Test: {topic}"), topic).await {
            Ok(_) => {
                last_err = None;
                break;
            }
            Err(e) => {
                last_err = Some(e);
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            }
        }
    }
    if let Some(e) = last_err {
        panic!("could not create research item '{name}': {e}");
    }
    // Publishing an item is a directory create followed by two file renames
    // (`RESEARCH.md`, then `CORPA.md`), so a reader arriving in between sees a
    // directory with only part of the pair. Wait for the files this helper
    // promises rather than handing a half-published item to the request.
    let Ok(research_name) = ragent_research::ResearchName::try_new(name) else {
        return;
    };
    let item_dir = ragent_research::ResearchIo::item_dir(root, &research_name);
    for _ in 0..100 {
        // A missing directory means the item was deleted while this waited;
        // that is a finished state, not a half-created one.
        if !item_dir.is_dir() {
            return;
        }
        if ragent_research::ResearchIo::research_md_path(root, &research_name).is_file()
            && ragent_research::ResearchIo::corpa_md_path(root, &research_name).is_file()
        {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    let entries: Vec<String> = std::fs::read_dir(&item_dir)
        .map(|dir| {
            dir.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    panic!("research item '{name}' was created but its files never appeared: {entries:?}");
}

/// Delete a research item from `root`.
async fn delete_test_item(root: &std::path::Path, name: &str) {
    let mgr = ResearchManager::new(root);
    let _ = mgr.delete(name).await;
}

/// Helper to build an authenticated PUT request.
fn auth_put(token: &str, uri: &str) -> Request<Body> {
    Request::builder()
        .method("PUT")
        .uri(uri)
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

/// Create a research item on disk and stamp a recorded invocation line into
/// its RESEARCH.md frontmatter, as a real invocation-aware front-end would.
async fn create_test_item_with_invocation(root: &std::path::Path, name: &str, invocation: &str) {
    create_test_item(root, name, "replay topic").await;
    let path = root.join(name).join("RESEARCH.md");
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {} after create: {e}", path.display()));
    let stamped = content.replacen(
        "\n---\n",
        &format!("\ninvocation: \"{invocation}\"\n---\n"),
        1,
    );
    ragent_research::ResearchIo::atomic_write(&path, &stamped)
        .await
        .unwrap();
}

// -- Auth tests -----------------------------------------------------------

#[tokio::test]
async fn test_research_list_requires_auth() {
    let root = crate::research_sandbox::isolated_research_root();
    let app = router(research_state("secret", &root));
    let req = Request::builder()
        .uri("/research")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_research_list_returns_empty_or_items() {
    let root = crate::research_sandbox::isolated_research_root();
    let app = router(research_state("tok", &root));
    let resp = app.oneshot(auth_get("tok", "/research")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    // The response should be valid JSON with an "items" array and "count".
    assert!(body.contains("\"items\""));
    assert!(body.contains("\"count\""));
}

// -- Show (GET /research/{name}) tests -----------------------------------

#[tokio::test]
async fn test_research_show_not_found() {
    let root = crate::research_sandbox::isolated_research_root();
    let _guard = FS_LOCK.lock().await;
    let app = router(research_state("tok", &root));
    let resp = app
        .oneshot(auth_get("tok", "/research/nonexistent-item-xyz"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body = body_string(resp).await;
    assert!(body.contains("not found"));
}

#[tokio::test]
async fn test_research_show_returns_item_fields() {
    let _guard = FS_LOCK.lock().await;
    let root = crate::research_sandbox::isolated_research_root();
    let name = test_name();
    create_test_item(&root, &name, "test topic for show").await;

    let app = router(research_state("tok", &root));
    let resp = app
        .oneshot(auth_get("tok", &format!("/research/{name}")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(parsed["item"]["name"], name);
    assert!(
        parsed["item"]["title"]
            .as_str()
            .unwrap()
            .contains("test topic")
    );
    assert_eq!(parsed["item"]["status"], "draft");
    assert_eq!(parsed["item"]["sources"], 0);
    // Without ?full=true, extended fields should be absent (skip_serialized)
    assert!(parsed["item"].get("topic").is_none() || parsed["item"]["topic"].is_null());
    assert!(parsed["item"].get("queries").is_none() || parsed["item"]["queries"].is_null());

    delete_test_item(&root, &name).await;
}

#[tokio::test]
async fn test_research_show_full_includes_extended_fields() {
    let _guard = FS_LOCK.lock().await;
    let root = crate::research_sandbox::isolated_research_root();
    let name = test_name();
    create_test_item(&root, &name, "test topic for full show").await;

    let app = router(research_state("tok", &root));
    let resp = app
        .oneshot(auth_get("tok", &format!("/research/{name}?full=true")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    // With ?full=true, the topic field should be present
    assert_eq!(parsed["item"]["topic"], "test topic for full show");
    // queries should be an empty array (skip_serializing_if empty, but present)
    // Actually skip_serializing_if = "Vec::is_empty" means it's omitted when empty.
    // So we just verify topic is present.

    delete_test_item(&root, &name).await;
}

// -- Delete tests ---------------------------------------------------------

#[tokio::test]
async fn test_research_delete_requires_confirmation() {
    let _guard = FS_LOCK.lock().await;
    let root = crate::research_sandbox::isolated_research_root();
    let name = test_name();
    create_test_item(&root, &name, "topic").await;

    let app = router(research_state("tok", &root));
    // Without ?confirm=delete-{name}
    let resp = app
        .oneshot(auth_delete("tok", &format!("/research/{name}")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::PRECONDITION_FAILED);
    let body = body_string(resp).await;
    assert!(body.contains("confirm"));

    delete_test_item(&root, &name).await;
}

#[tokio::test]
async fn test_research_delete_with_confirmation() {
    let _guard = FS_LOCK.lock().await;
    let root = crate::research_sandbox::isolated_research_root();
    let name = test_name();
    create_test_item(&root, &name, "topic").await;

    let app = router(research_state("tok", &root));
    let confirm = format!("delete-{name}");
    let resp = app
        .oneshot(auth_delete(
            "tok",
            &format!("/research/{name}?confirm={confirm}"),
        ))
        .await
        .unwrap();
    // F-L3: the body-carrying success is 200 OK, not 204 (RFC 9110 forbids a
    // body on 204).
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains("\"deleted\""));

    // Verify it's actually gone
    let mgr = ResearchManager::new(&root);
    assert!(mgr.show(&name).await.is_err());
}

#[tokio::test]
async fn test_research_delete_not_found() {
    let root = crate::research_sandbox::isolated_research_root();
    let _guard = FS_LOCK.lock().await;
    let app = router(research_state("tok", &root));
    let resp = app
        .oneshot(auth_delete(
            "tok",
            "/research/nonexistent-xyz?confirm=delete-nonexistent-xyz",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// -- POST /research tests ------------------------------------------------

#[tokio::test]
async fn test_research_post_invalid_name() {
    let root = crate::research_sandbox::isolated_research_root();
    let _guard = FS_LOCK.lock().await;
    let app = router(research_state("tok", &root));
    let body = r#"{"name":"INVALID NAME!","topic":"test"}"#;
    let resp = app
        .oneshot(auth_post("tok", "/research", body))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_research_post_returns_202_with_location() {
    let _guard = FS_LOCK.lock().await;
    let root = crate::research_sandbox::isolated_research_root();
    let name = test_name();
    let body = format!(r#"{{"name":"{name}","topic":"test topic for post"}}"#);
    let app = router(research_state("tok", &root));
    let resp = app
        .oneshot(auth_post("tok", "/research", &body))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    // Location header should point to the SSE stream
    let location = resp
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(location, format!("/research/{name}/events"));

    // Body should contain the name and "accepted" status
    let body = body_string(resp).await;
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(parsed["name"], name);
    assert_eq!(parsed["status"], "accepted");

    // Clean up - the background task may fail (no LLM), but the item
    // was created on disk. Wait a moment for the spawn to start, then
    // delete the item.
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    delete_test_item(&root, &name).await;
}

#[tokio::test]
async fn test_research_post_accepts_output_limit_fields() {
    // researchmax FR-012: the new `max_concepts` / `max_findings` JSON fields
    let root = crate::research_sandbox::isolated_research_root();
    // must be accepted (not rejected as unknown) so a POST carrying them starts
    // a run.
    let _guard = FS_LOCK.lock().await;
    let name = test_name();
    let body =
        format!(r#"{{"name":"{name}","topic":"test topic","max_concepts":2,"max_findings":3}}"#);
    let app = router(research_state("tok", &root));
    let resp = app
        .oneshot(auth_post("tok", "/research", &body))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    let body = body_string(resp).await;
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(parsed["name"], name);
    assert_eq!(parsed["status"], "accepted");

    // Clean up - the background run may fail (no LLM), but the item exists.
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    delete_test_item(&root, &name).await;
}

#[tokio::test]
async fn test_research_post_duplicate_returns_conflict() {
    let _guard = FS_LOCK.lock().await;
    let root = crate::research_sandbox::isolated_research_root();
    let name = test_name();
    create_test_item(&root, &name, "topic").await;

    let app = router(research_state("tok", &root));
    let body = format!(r#"{{"name":"{name}","topic":"duplicate"}}"#);
    let resp = app
        .oneshot(auth_post("tok", "/research", &body))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    let body = body_string(resp).await;
    assert!(body.contains("already exists"));

    delete_test_item(&root, &name).await;
}

// -- SSE events endpoint -------------------------------------------------

// -- Update (PUT /research/{name}) tests ---------------------------------

#[tokio::test]
async fn test_research_update_not_found() {
    let root = crate::research_sandbox::isolated_research_root();
    let _guard = FS_LOCK.lock().await;
    let app = router(research_state("tok", &root));
    let resp = app
        .oneshot(auth_put("tok", "/research/nonexistent-item-xyz"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body = body_string(resp).await;
    assert!(body.contains("not found"));
}

#[tokio::test]
async fn test_research_update_without_invocation_returns_bad_request() {
    let _guard = FS_LOCK.lock().await;
    let root = crate::research_sandbox::isolated_research_root();
    let name = test_name();
    create_test_item(&root, &name, "topic").await;

    let app = router(research_state("tok", &root));
    let resp = app
        .oneshot(auth_put("tok", &format!("/research/{name}")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = body_string(resp).await;
    assert!(body.contains("no invocation recorded"));

    delete_test_item(&root, &name).await;
}

#[tokio::test]
async fn test_research_update_with_unparseable_invocation_returns_bad_request() {
    let _guard = FS_LOCK.lock().await;
    let root = crate::research_sandbox::isolated_research_root();
    let name = test_name();
    create_test_item_with_invocation(&root, &name, "ragent research list --all").await;

    let app = router(research_state("tok", &root));
    let resp = app
        .oneshot(auth_put("tok", &format!("/research/{name}")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = body_string(resp).await;
    assert!(body.contains("cannot replay"));

    delete_test_item(&root, &name).await;
}

#[tokio::test]
async fn test_research_update_accepts_recorded_invocation() {
    let _guard = FS_LOCK.lock().await;
    let root = crate::research_sandbox::isolated_research_root();
    let name = test_name();
    // The recorded invocation names the same item; the frontmatter name is
    // authoritative for the replay either way.
    create_test_item_with_invocation(
        &root,
        &name,
        &format!("ragent research create {name} \"replay topic\" --tier full"),
    )
    .await;

    let app = router(research_state("tok", &root));
    let resp = app
        .oneshot(auth_put("tok", &format!("/research/{name}")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    // Location header should point to the SSE stream.
    let location = resp
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(location, format!("/research/{name}/events"));

    let body = body_string(resp).await;
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(parsed["name"], name);
    assert_eq!(parsed["status"], "accepted");
    assert!(
        parsed["replayed_invocation"]
            .as_str()
            .unwrap()
            .starts_with("ragent research create")
    );

    // Clean up - the background replay may fail (no LLM), but the item
    // exists on disk and the run registry must drain.
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    delete_test_item(&root, &name).await;
}

#[tokio::test]
async fn test_research_update_replays_recorded_output_limits() {
    // researchmax FR-013: a recorded HTTP summary carrying the output-limit
    let root = crate::research_sandbox::isolated_research_root();
    // flags must replay through `from_invocation` (the run is accepted rather
    // than rejected as unparseable).
    let _guard = FS_LOCK.lock().await;
    let name = test_name();
    create_test_item_with_invocation(
        &root,
        &name,
        &format!("POST /research {name} \"replay topic\" --max-concepts 2 --max-findings 3"),
    )
    .await;

    let app = router(research_state("tok", &root));
    let resp = app
        .oneshot(auth_put("tok", &format!("/research/{name}")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    let body = body_string(resp).await;
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(parsed["status"], "accepted");

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    delete_test_item(&root, &name).await;
}

#[tokio::test]
async fn test_research_update_conflicts_with_in_flight_run() {
    let _guard = FS_LOCK.lock().await;
    let root = crate::research_sandbox::isolated_research_root();
    let name = test_name();
    create_test_item_with_invocation(
        &root,
        &name,
        &format!("ragent research create {name} \"replay topic\""),
    )
    .await;

    let state = research_state("tok", &root);
    // Simulate an in-flight run by pre-registering the name.
    {
        let mut runs = state.research_runs.lock().await;
        let (tx, _rx) = tokio::sync::broadcast::channel(16);
        runs.insert(name.clone(), tx);
    }
    let app = router(state);
    let resp = app
        .oneshot(auth_put("tok", &format!("/research/{name}")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    let body = body_string(resp).await;
    assert!(body.contains("already in progress"));

    delete_test_item(&root, &name).await;
}

// -- SSE events endpoint (continued) -------------------------------------

#[tokio::test]
async fn test_research_events_no_active_run_returns_status() {
    let _guard = FS_LOCK.lock().await;
    let root = crate::research_sandbox::isolated_research_root();
    let name = test_name();
    create_test_item(&root, &name, "topic").await;

    let app = router(research_state("tok", &root));
    let resp = app
        .oneshot(auth_get("tok", &format!("/research/{name}/events")))
        .await
        .unwrap();
    // When there's no active run but the item exists, it returns 200 with
    // a JSON status blob.
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains("No active research run"));

    delete_test_item(&root, &name).await;
}

#[tokio::test]
async fn test_research_events_not_found() {
    let root = crate::research_sandbox::isolated_research_root();
    let _guard = FS_LOCK.lock().await;
    let app = router(research_state("tok", &root));
    let resp = app
        .oneshot(auth_get("tok", "/research/nonexistent-xyz/events"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
