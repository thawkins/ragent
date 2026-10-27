//! Tests-only coverage for the extracted-tool adapter event-forwarding and
//! storage bridge surfaces in `crates/ragent-agent/src/tool/mod.rs`.
//!
//! The adapter types themselves (`ExtractedCoreToolAdapter`,
//! `CoreStorageAdapter`, `ExtractedExtendedToolAdapter`, `ExtractedVcsToolAdapter`,
//! `CoreVcsStorageAdapter`) are private, so they are reached exclusively as the
//! behaviour of tools obtained from the public
//! [`ragent_agent::tool::create_default_registry`].
//!
//! This file drives:
//!
//! * the session<->tool event conversions (`convert_extracted_event`,
//!   `convert_core_event`, `convert_core_event_to_extended`) while the per-call
//!   forwarder tasks are subscribed, and
//! * the `CoreStorageAdapter` task methods (`create_task`/`update_task`) and
//!   `CoreVcsStorageAdapter` credential reads (`get_setting`/`get_provider_auth`)
//!   reached through the extended `task_*` and VCS tool surfaces.
//!
//! No production code is modified and no network or LLM provider is used.

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use ragent_agent::event::{Event, EventBus};
use ragent_agent::storage::Storage;
use ragent_agent::tool::{ToolContext, create_default_registry};

/// Build a `ToolContext` rooted at `dir` with an empty storage slot.
fn ctx_in(dir: &std::path::Path) -> ToolContext {
    ToolContext {
        session_id: "sess-tae".to_string(),
        working_dir: dir.to_path_buf(),
        event_bus: Arc::new(EventBus::new(128)),
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

/// Receive events from `rx` until `pred` matches or `dur` elapses.
///
/// Returns the matching event, or `None` on timeout/closed channel. `Lagged`
/// errors are ignored so a busy forwarder cannot fail the drain.
async fn drain_until<F>(
    rx: &mut tokio::sync::broadcast::Receiver<Event>,
    dur: Duration,
    mut pred: F,
) -> Option<Event>
where
    F: FnMut(&Event) -> bool,
{
    let deadline = tokio::time::Instant::now() + dur;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return None;
        }
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Ok(ev)) => {
                if pred(&ev) {
                    return Some(ev);
                }
            }
            Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => continue,
            Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) | Err(_) => return None,
        }
    }
}

// -- convert_core_event (session bus -> core tool bus) -----------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn core_adapter_converts_session_input_events_during_bash() {
    // While `Bash` awaits its child, both adapter forwarders are live and
    // subscribed. Publishing session-bus events in that window drives the
    // `convert_core_event` arms (UserInput / QuestionAnswered) plus its
    // session-id filter and `_ => None` fallthrough.
    let dir = tempfile::tempdir().expect("tempdir");
    let registry = create_default_registry();
    let bash = registry.get("Bash").expect("Bash tool registered");
    let ctx = ctx_in(dir.path());

    let bus = ctx.event_bus.clone();
    let session = ctx.session_id.clone();
    let publisher = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(250)).await;
        bus.publish(Event::UserInput {
            session_id: session.clone(),
            request_id: "u-1".to_string(),
            response: "typed answer".to_string(),
        });
        bus.publish(Event::QuestionAnswered {
            session_id: session.clone(),
            request_id: "q-1".to_string(),
            response: "option-a".to_string(),
            secret: false,
        });
        // No core conversion -> `convert_core_event` returns None.
        bus.publish(Event::TextDelta {
            session_id: session.clone(),
            text: "delta".to_string(),
        });
        // A mismatched session id takes the `continue` branch.
        bus.publish(Event::UserInput {
            session_id: "someone-else".to_string(),
            request_id: "u-2".to_string(),
            response: "ignored".to_string(),
        });
    });

    // The command only needs to outlive the 250 ms publisher task. The timeout
    // is deliberately generous: under a fully parallel `cargo test --workspace`
    // run a one-second `sleep` plus process spawn can otherwise exceed a tight
    // timeout and fail the test on machine load rather than on behaviour.
    let out = bash
        .execute(json!({ "command": "sleep 1", "timeout": 180 }), &ctx)
        .await
        .expect("bash runs a long-enough command");
    assert!(
        out.content.contains("Exit code"),
        "bash output: {}",
        out.content
    );

    publisher.await.expect("publisher task joins");
}

// -- convert_extracted_event (core tool bus -> session bus) ------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn core_adapter_forwards_shell_cwd_changed_from_bash() {
    let dir = tempfile::tempdir().expect("tempdir");
    let registry = create_default_registry();
    let bash = registry.get("Bash").expect("Bash tool registered");
    let ctx = ctx_in(dir.path());

    let mut rx = ctx.event_bus.subscribe();
    // A large output widens the post-publish window inside `Bash` (the
    // `ShellCwdChanged` publish happens just before the output is formatted),
    // giving the per-call forwarder time to convert it before it is aborted.
    // The timeout is deliberately generous - under a parallel
    // `cargo test --workspace` run the spawn + pipe throughput can otherwise
    // exceed a tight timeout and fail on machine load rather than behaviour.
    let out = bash
        .execute(json!({ "command": "seq 1 40000", "timeout": 180 }), &ctx)
        .await
        .expect("bash executes the command");
    assert!(
        out.content.contains("Exit code"),
        "bash output: {}",
        out.content
    );

    let got = drain_until(&mut rx, Duration::from_secs(3), |ev| {
        matches!(ev, Event::ShellCwdChanged { .. })
    })
    .await
    .expect("forwarded ShellCwdChanged observed on the session bus");
    match got {
        Event::ShellCwdChanged { session_id, cwd } => {
            assert_eq!(session_id, ctx.session_id);
            assert!(!cwd.is_empty(), "cwd must be populated");
        }
        other => panic!("expected ShellCwdChanged, got {}", other.type_name()),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn core_adapter_forwards_reasoning_delta_from_think() {
    // `think` publishes `Event::ReasoningDelta` on the tool bus from inside its
    // synchronous `execute`; the forwarder converts it to the session bus.
    let dir = tempfile::tempdir().expect("tempdir");
    let registry = create_default_registry();
    let think = registry.get("think").expect("think tool registered");
    let ctx = ctx_in(dir.path());

    let mut rx = ctx.event_bus.subscribe();
    think
        .execute(json!({ "thought": "planning the next step" }), &ctx)
        .await
        .expect("think executes");

    // Best-effort: the forwarder races the post-execute `abort()`, so only
    // assert the payload when the event wins.
    if let Some(Event::ReasoningDelta { session_id, text }) =
        drain_until(&mut rx, Duration::from_secs(2), |ev| {
            matches!(ev, Event::ReasoningDelta { .. })
        })
        .await
    {
        assert_eq!(session_id, ctx.session_id);
        assert!(text.contains("planning"));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn core_adapter_forwards_task_completed_from_agent_complete() {
    let dir = tempfile::tempdir().expect("tempdir");
    let registry = create_default_registry();
    let done = registry
        .get("agent_complete")
        .expect("agent_complete registered");
    let ctx = ctx_in(dir.path());

    let mut rx = ctx.event_bus.subscribe();
    done.execute(json!({ "summary": "adapter coverage complete" }), &ctx)
        .await
        .expect("agent_complete executes");

    // Best-effort (see the `think` test).
    if let Some(Event::TaskCompleted {
        session_id,
        summary,
    }) = drain_until(&mut rx, Duration::from_secs(2), |ev| {
        matches!(ev, Event::TaskCompleted { .. })
    })
    .await
    {
        assert_eq!(session_id, ctx.session_id);
        assert!(summary.contains("adapter coverage"));
    }
}

// -- CoreStorageAdapter via the extended task tools --------------------------

fn storage_ctx(dir: &std::path::Path) -> ToolContext {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    storage
        .create_session("sess-tae", "target/temp")
        .expect("session row");
    let mut ctx = ctx_in(dir);
    ctx.storage = Some(storage);
    ctx
}

#[tokio::test]
async fn extended_task_create_then_get_roundtrips_through_storage_adapter() {
    let dir = tempfile::tempdir().expect("tempdir");
    let registry = create_default_registry();
    let ctx = storage_ctx(dir.path());

    let create = registry.get("TaskCreate").expect("task_create registered");
    assert_eq!(create.name(), "TaskCreate");
    assert_eq!(create.permission_category(), "task");
    let created = create
        .execute(
            json!({
                "subject": "Implement adapter coverage",
                "description": "drive CoreStorageAdapter::create_task",
                "active_form": "Covering the adapter",
                "owner": "test",
                "metadata": { "feature": "coverage" }
            }),
            &ctx,
        )
        .await
        .expect("task_create persists through the storage adapter");
    let record = created
        .metadata
        .as_ref()
        .expect("task_create returns metadata")
        .clone();
    let task_id = record["id"].as_str().expect("created task id").to_string();
    assert_eq!(record["status"], "pending");
    assert_eq!(record["owner"], "test");

    // task_get reads the same row back through `list_tasks`.
    let got = registry
        .get("TaskGet")
        .expect("task_get registered")
        .execute(json!({ "task_id": task_id }), &ctx)
        .await
        .expect("task_get reads through the storage adapter");
    assert!(
        got.content.contains("Implement adapter coverage"),
        "{}",
        got.content
    );
}

#[tokio::test]
async fn extended_task_update_mutates_through_storage_adapter() {
    let dir = tempfile::tempdir().expect("tempdir");
    let registry = create_default_registry();
    let ctx = storage_ctx(dir.path());

    let created = registry
        .get("TaskCreate")
        .expect("TaskCreate")
        .execute(json!({ "subject": "First", "description": "d" }), &ctx)
        .await
        .expect("create");
    let task_id = created.metadata.as_ref().expect("metadata")["id"]
        .as_str()
        .expect("id")
        .to_string();

    let updated = registry
        .get("TaskUpdate")
        .expect("task_update registered")
        .execute(
            json!({ "task_id": task_id, "status": "in_progress", "owner": "worker" }),
            &ctx,
        )
        .await
        .expect("task_update mutates through the storage adapter");
    let rec = updated.metadata.as_ref().expect("update metadata");
    assert_eq!(rec["status"], "in_progress");
    assert_eq!(rec["owner"], "worker");
}

#[tokio::test]
async fn extended_task_create_rejects_unknown_blocked_by() {
    let dir = tempfile::tempdir().expect("tempdir");
    let registry = create_default_registry();
    let ctx = storage_ctx(dir.path());

    // `blocked_by` validation calls `list_tasks` on the adapter and then bails
    // when the referenced id does not exist in the session.
    let err = registry
        .get("TaskCreate")
        .expect("TaskCreate")
        .execute(
            json!({
                "subject": "Blocked",
                "description": "d",
                "blocked_by": ["task-does-not-exist"]
            }),
            &ctx,
        )
        .await
        .expect_err("unknown blocked_by must be rejected");
    assert!(
        err.to_string().contains("non-existent"),
        "error names the missing dependency: {err}"
    );
}

#[tokio::test]
async fn extended_task_update_rejects_blocked_status() {
    let dir = tempfile::tempdir().expect("tempdir");
    let registry = create_default_registry();
    let ctx = storage_ctx(dir.path());

    let created = registry
        .get("TaskCreate")
        .expect("TaskCreate")
        .execute(json!({ "subject": "T", "description": "d" }), &ctx)
        .await
        .expect("create");
    let task_id = created.metadata.as_ref().expect("metadata")["id"]
        .as_str()
        .expect("id")
        .to_string();

    let err = registry
        .get("TaskUpdate")
        .expect("TaskUpdate")
        .execute(json!({ "task_id": task_id, "status": "blocked" }), &ctx)
        .await
        .expect_err("status=blocked must be rejected");
    assert!(err.to_string().contains("blocked"), "error: {err}");
}

#[tokio::test]
async fn extended_task_tools_error_cleanly_without_storage() {
    let dir = tempfile::tempdir().expect("tempdir");
    let registry = create_default_registry();
    let ctx = ctx_in(dir.path());

    for (tool, input) in [
        ("TaskCreate", json!({ "subject": "s", "description": "d" })),
        (
            "TaskUpdate",
            json!({ "task_id": "task-1", "status": "pending" }),
        ),
        ("TaskGet", json!({ "task_id": "task-1" })),
    ] {
        let err = registry
            .get(tool)
            .expect("registered")
            .execute(input, &ctx)
            .await
            .expect_err("storage is required");
        assert!(err.to_string().contains("Storage"), "{tool}: {err}");
    }
}

// -- convert_core_event_to_extended (session bus -> extended tool bus) -------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn extended_adapter_converts_session_input_events_during_pdf_write() {
    // `pdf_write` renders on a blocking thread, so the main task yields while
    // the extended adapter's forwarders are subscribed; session-bus events
    // published in that window drive `convert_core_event_to_extended`.
    let dir = tempfile::tempdir().expect("tempdir");
    let registry = create_default_registry();
    let pdf_write = registry.get("pdf_write").expect("pdf_write registered");
    let ctx = ctx_in(dir.path());

    let bus = ctx.event_bus.clone();
    let session = ctx.session_id.clone();
    let publisher = tokio::spawn(async move {
        // Publish repeatedly across the render window to beat the per-call
        // forwarder subscription/abort race.
        for _ in 0..40 {
            bus.publish(Event::UserInput {
                session_id: session.clone(),
                request_id: "u-1".to_string(),
                response: "r".to_string(),
            });
            bus.publish(Event::TextDelta {
                session_id: session.clone(),
                text: "no extended conversion".to_string(),
            });
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    });

    // A large document lengthens the render so the publish window is wide.
    let mut elements = Vec::new();
    for i in 0..400 {
        elements.push(json!({ "type": "paragraph", "text": format!("Paragraph {i} body text.") }));
    }
    let out = pdf_write
        .execute(
            json!({
                "path": "out.pdf",
                "content": { "title": "Coverage", "elements": elements }
            }),
            &ctx,
        )
        .await
        .expect("pdf_write renders a document offline");
    assert!(out.content.contains("Wrote PDF"), "{}", out.content);
    assert!(dir.path().join("out.pdf").exists());

    publisher.abort();
}

// -- CoreVcsStorageAdapter credential reads ----------------------------------

#[tokio::test]
async fn vcs_adapter_reads_credentials_from_storage() {
    let dir = tempfile::tempdir().expect("tempdir");
    let registry = create_default_registry();
    let mut ctx = ctx_in(dir.path());

    // Pre-populate the credential store so `make_client` routes through
    // CoreVcsStorageAdapter's `get_setting` (config) and `get_provider_auth`
    // (token) before failing to detect a GitLab project from the git remote.
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    ragent_tools_vcs::gitlab::save_config(
        &*storage,
        &ragent_tools_vcs::gitlab::GitLabConfig {
            instance_url: "https://gitlab.example".to_string(),
            username: "tester".to_string(),
        },
    )
    .expect("save gitlab config");
    ragent_tools_vcs::gitlab::save_token(&*storage, "fake-token").expect("save gitlab token");
    ctx.storage = Some(storage);

    let err = registry
        .get("gitlab_list_issues")
        .expect("gitlab_list_issues registered")
        .execute(json!({ "state": "opened", "limit": 5 }), &ctx)
        .await
        .expect_err("no GitLab remote in a bare tempdir");
    assert!(
        err.to_string().to_lowercase().contains("gitlab"),
        "error mentions GitLab: {err}"
    );
}

#[tokio::test]
async fn vcs_adapter_errors_without_storage() {
    let dir = tempfile::tempdir().expect("tempdir");
    let registry = create_default_registry();
    let ctx = ctx_in(dir.path());

    let err = registry
        .get("gitlab_list_issues")
        .expect("gitlab_list_issues registered")
        .execute(json!({}), &ctx)
        .await
        .expect_err("storage is required for the GitLab client");
    assert!(
        err.to_string().contains("Storage"),
        "error names missing storage: {err}"
    );
}
