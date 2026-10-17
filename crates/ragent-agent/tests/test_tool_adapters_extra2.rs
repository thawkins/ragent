//! Extra tests-only coverage for the extracted-tool adapter surface in
//! `crates/ragent-agent/src/tool/mod.rs`.
//!
//! The adapter types (`ExtractedCoreToolAdapter`, `LegacyMultiEditAlias`,
//! `ExtractedExtendedToolAdapter`, `ExtractedVcsToolAdapter`, and the storage
//! bridges) are private, so they are reached exclusively through the tools the
//! public [`ragent_agent::tool::create_default_registry`] returns.
//!
//! This file drives the residual branches the existing suites leave open:
//! the `LegacyMultiEditAlias` deprecation-metadata emission and its
//! non-array / non-object `edits` passthrough, and the extended/VCS adapter
//! error arms that name a missing parameter, a missing storage backend, or an
//! empty VCS credential store. No production code is modified and no network or
//! LLM provider is used.

use std::sync::Arc;

use serde_json::json;

use ragent_agent::event::EventBus;
use ragent_agent::storage::Storage;
use ragent_agent::tool::{ToolContext, create_default_registry};

/// Build a `ToolContext` rooted at `dir` with an empty storage slot.
fn ctx_in(dir: &std::path::Path) -> ToolContext {
    ToolContext {
        session_id: "sess-tae2".to_string(),
        working_dir: dir.to_path_buf(),
        event_bus: Arc::new(EventBus::new(64)),
        storage: None,
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
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        tool_registry: ToolContext::default_tool_registry(),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
    }
}

/// A `ToolContext` whose session row exists in an in-memory storage backend.
fn storage_ctx(dir: &std::path::Path) -> ToolContext {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    storage
        .create_session("sess-tae2", "target/temp")
        .expect("session row");
    let mut ctx = ctx_in(dir);
    ctx.storage = Some(storage);
    ctx
}

// -- LegacyMultiEditAlias residual branches ----------------------------------

#[tokio::test]
async fn multiedit_alias_emits_deprecation_warning_metadata() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("a.txt");
    std::fs::write(&path, "hello world\n").expect("write");

    let registry = create_default_registry();
    let alias = registry.get("multiedit").expect("multiedit alias");
    let ctx = ctx_in(dir.path());

    let out = alias
        .execute(
            json!({
                "edits": [
                    { "file_path": "a.txt", "old_string": "hello", "new_string": "goodbye" }
                ]
            }),
            &ctx,
        )
        .await
        .expect("canonical multiedit executes");

    // editrenewal FR-012: the alias injects a deprecation marker into the
    // returned metadata regardless of whether the wrapped tool produced any.
    let meta = out.metadata.expect("deprecation metadata is attached");
    let warning = meta["deprecation_warning"]
        .as_str()
        .expect("deprecation_warning is a string");
    assert!(
        warning.contains("multi_edit"),
        "warning must steer to the canonical tool: {warning}"
    );
}

#[tokio::test]
async fn multiedit_alias_passes_through_non_array_edits() {
    let dir = tempfile::tempdir().expect("tempdir");
    let registry = create_default_registry();
    let alias = registry.get("multiedit").expect("multiedit alias");
    let ctx = ctx_in(dir.path());

    // `edits` is not an array: `normalise_legacy_params` returns the input
    // unchanged and the wrapped core tool rejects the malformed shape.
    let err = alias
        .execute(json!({ "edits": "not-an-array" }), &ctx)
        .await
        .expect_err("a non-array edits value must be rejected");
    assert!(!err.to_string().is_empty(), "error is descriptive");
}

#[tokio::test]
async fn multiedit_alias_passes_through_non_object_edit_members() {
    let dir = tempfile::tempdir().expect("tempdir");
    let registry = create_default_registry();
    let alias = registry.get("multiedit").expect("multiedit alias");
    let ctx = ctx_in(dir.path());

    // Each array member is expected to be an object; a scalar member is left
    // untouched by the normaliser and rejected downstream.
    let err = alias
        .execute(json!({ "edits": [42, "nope"] }), &ctx)
        .await
        .expect_err("scalar edit members must be rejected");
    assert!(!err.to_string().is_empty(), "error is descriptive");
}

// -- ExtractedExtendedToolAdapter error arms ---------------------------------

#[tokio::test]
async fn task_update_missing_task_id_names_the_parameter() {
    let dir = tempfile::tempdir().expect("tempdir");
    let registry = create_default_registry();
    let ctx = storage_ctx(dir.path());

    let err = registry
        .get("task_update")
        .expect("task_update registered")
        .execute(json!({ "status": "pending" }), &ctx)
        .await
        .expect_err("a missing task_id must be rejected");
    assert!(
        err.to_string().contains("task_id"),
        "error names the missing parameter: {err}"
    );
}

#[tokio::test]
async fn task_get_unknown_id_reports_not_found() {
    let dir = tempfile::tempdir().expect("tempdir");
    let registry = create_default_registry();
    let ctx = storage_ctx(dir.path());

    let err = registry
        .get("task_get")
        .expect("task_get registered")
        .execute(json!({ "task_id": "task-does-not-exist" }), &ctx)
        .await
        .expect_err("an unknown task id must be rejected");
    assert!(
        err.to_string().to_lowercase().contains("not found"),
        "error reports the missing task: {err}"
    );
}

#[tokio::test]
async fn task_list_rejects_an_invalid_status_filter() {
    let dir = tempfile::tempdir().expect("tempdir");
    let registry = create_default_registry();
    let ctx = storage_ctx(dir.path());

    let err = registry
        .get("task_list")
        .expect("task_list registered")
        .execute(json!({ "status": "bogus" }), &ctx)
        .await
        .expect_err("an invalid status filter must be rejected");
    assert!(
        err.to_string().contains("Invalid status"),
        "error names the invalid filter: {err}"
    );
}

#[tokio::test]
async fn task_create_rejects_a_non_object_metadata_value() {
    let dir = tempfile::tempdir().expect("tempdir");
    let registry = create_default_registry();
    let ctx = storage_ctx(dir.path());

    let err = registry
        .get("task_create")
        .expect("task_create registered")
        .execute(
            json!({ "subject": "s", "description": "d", "metadata": "not-an-object" }),
            &ctx,
        )
        .await
        .expect_err("a non-object metadata value must be rejected");
    assert!(
        err.to_string().contains("metadata"),
        "error names the metadata parameter: {err}"
    );
}

#[tokio::test]
async fn task_create_missing_subject_names_the_parameter() {
    let dir = tempfile::tempdir().expect("tempdir");
    let registry = create_default_registry();
    let ctx = storage_ctx(dir.path());

    let err = registry
        .get("task_create")
        .expect("task_create registered")
        .execute(json!({ "description": "d" }), &ctx)
        .await
        .expect_err("a missing subject must be rejected");
    assert!(
        err.to_string().contains("subject"),
        "error names the missing parameter: {err}"
    );
}

// -- Extended document / web adapter error arms ------------------------------

#[tokio::test]
async fn extended_tools_reject_missing_required_parameters() {
    let dir = tempfile::tempdir().expect("tempdir");
    let registry = create_default_registry();
    let ctx = ctx_in(dir.path());

    for (tool, input, needle) in [
        ("pdf_read", json!({}), "path"),
        ("webfetch", json!({}), "url"),
        ("http_request", json!({}), "url"),
    ] {
        let err = registry
            .get(tool)
            .expect("registered")
            .execute(input, &ctx)
            .await
            .expect_err("a missing required parameter must be rejected");
        assert!(
            err.to_string().contains(needle),
            "{tool} error should name '{needle}': {err}"
        );
    }
}

#[tokio::test]
async fn websearch_rejects_an_empty_query() {
    let dir = tempfile::tempdir().expect("tempdir");
    let registry = create_default_registry();
    let ctx = ctx_in(dir.path());

    let err = registry
        .get("websearch")
        .expect("websearch registered")
        .execute(json!({ "query": "   " }), &ctx)
        .await
        .expect_err("an empty query must be rejected");
    assert!(
        err.to_string().contains("empty"),
        "error explains the empty query: {err}"
    );
}

#[tokio::test]
async fn webfetch_rejects_a_non_http_scheme() {
    let dir = tempfile::tempdir().expect("tempdir");
    let registry = create_default_registry();
    let ctx = ctx_in(dir.path());

    // A `file://` URL is rejected by the SSRF/URL-safety guard before any
    // outbound request; a non-http scheme (e.g. `ftp://`) that clears the guard
    // is rejected by the scheme check. Either way the `webfetch` adapter
    // surfaces a descriptive error and never issues a request.
    let err = registry
        .get("webfetch")
        .expect("webfetch registered")
        .execute(json!({ "url": "file:///etc/hosts" }), &ctx)
        .await
        .expect_err("a non-http(s) scheme must be rejected");
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("validation") || msg.contains("http"),
        "error explains the rejection: {err}"
    );

    let err = registry
        .get("webfetch")
        .expect("webfetch registered")
        .execute(json!({ "url": "ftp://example.com/x" }), &ctx)
        .await
        .expect_err("a non-http(s) scheme must be rejected");
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("validation") || msg.contains("http") || msg.contains("scheme"),
        "error explains the rejection: {err}"
    );
}

// -- ExtractedVcsToolAdapter storage-bridge error arms -----------------------

#[tokio::test]
async fn vcs_adapter_without_storage_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let registry = create_default_registry();
    let ctx = ctx_in(dir.path());

    let err = registry
        .get("gitlab_list_issues")
        .expect("gitlab_list_issues registered")
        .execute(json!({}), &ctx)
        .await
        .expect_err("the GitLab client needs a storage backend for credentials");
    assert!(!err.to_string().is_empty(), "error is descriptive: {err}");
}

#[tokio::test]
async fn vcs_adapter_with_empty_credential_store_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let registry = create_default_registry();
    // Storage present but containing neither a GitLab config nor a token, so
    // the bridge's `get_setting` and `get_provider_auth` both return `None`.
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let mut ctx = ctx_in(dir.path());
    ctx.storage = Some(storage);

    let err = registry
        .get("gitlab_list_issues")
        .expect("gitlab_list_issues registered")
        .execute(json!({}), &ctx)
        .await
        .expect_err("an unauthenticated GitLab client must be refused");
    assert!(!err.to_string().is_empty(), "error is descriptive: {err}");
}
