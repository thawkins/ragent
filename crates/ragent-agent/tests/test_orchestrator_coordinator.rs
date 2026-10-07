//! Integration tests for the orchestrator `Coordinator` (audit T-705).
//!
//! The `Coordinator` and its `JobDescriptor`/`JobEvent`/`MetricsSnapshot`
//! types are re-exported from `ragent_agent::orchestrator`, so no `#[path]`
//! re-import is needed. A fake [`Router`] keyed by agent id gives fully
//! deterministic dispatch without touching the registry mailbox loop or its
//! 5-second request timeout.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use futures::future::FutureExt;
use ragent_agent::orchestrator::coordinator::OrchestrationMessage;
use ragent_agent::orchestrator::{
    AgentRegistry, Coordinator, JobDescriptor, JobEvent, Responder, Router,
};

/// A router that returns a canned `Result<String>` per agent id, or a default
/// when the id is absent. Avoids all spawned tasks and timeouts.
struct FakeRouter {
    responses: HashMap<String, anyhow::Result<String>>,
    default: anyhow::Result<String>,
}

#[async_trait::async_trait]
impl Router for FakeRouter {
    async fn send(&self, agent_id: &str, _msg: OrchestrationMessage) -> anyhow::Result<String> {
        match self.responses.get(agent_id) {
            Some(Ok(resp)) => Ok(resp.clone()),
            Some(Err(e)) => Err(anyhow::anyhow!("{e}")),
            None => match &self.default {
                Ok(resp) => Ok(resp.clone()),
                Err(e) => Err(anyhow::anyhow!("{e}")),
            },
        }
    }
}

fn router(
    responses: &[(&str, anyhow::Result<&str>)],
    default: anyhow::Result<&str>,
) -> Arc<dyn Router> {
    Arc::new(FakeRouter {
        responses: responses
            .iter()
            .map(|(id, r)| {
                let mapped = match r {
                    Ok(s) => Ok((*s).to_string()),
                    Err(e) => Err(anyhow::anyhow!("{e}")),
                };
                ((*id).to_string(), mapped)
            })
            .collect(),
        default: match default {
            Ok(s) => Ok(s.to_string()),
            Err(e) => Err(anyhow::anyhow!("{e}")),
        },
    })
}

async fn registry_with(agents: &[(&str, &[&str])]) -> AgentRegistry {
    let registry = AgentRegistry::new();
    for (id, caps) in agents {
        let caps: Vec<String> = caps.iter().map(|c| (*c).to_string()).collect();
        // A no-op responder is enough: the fake router never reaches the mailbox.
        let responder: Responder = Arc::new(|payload: String| async move { payload }.boxed());
        registry.register(*id, caps, Some(responder)).await;
    }
    registry
}

fn descriptor(id: &str, caps: &[&str]) -> JobDescriptor {
    JobDescriptor {
        id: id.to_string(),
        required_capabilities: caps.iter().map(|c| (*c).to_string()).collect(),
        payload: "payload".to_string(),
    }
}

// ---------------------------------------------------------------------------
// start_job_sync
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_start_job_sync_concatenates_agent_responses() {
    let registry = registry_with(&[("a", &["search"]), ("b", &["search"])]).await;
    let coord = Coordinator::with_router(
        registry,
        router(&[("a", Ok("alpha")), ("b", Ok("bravo"))], Ok("unused")),
    );

    let out = coord
        .start_job_sync(descriptor("job-1", &["search"]))
        .await
        .expect("sync job");

    assert!(out.contains("--- agent: a ---\nalpha"), "got: {out}");
    assert!(out.contains("--- agent: b ---\nbravo"), "got: {out}");

    let snap = coord.metrics_snapshot();
    assert_eq!(snap.completed_jobs, 1);
    assert_eq!(snap.errors, 0);
    assert_eq!(snap.active_jobs, 0, "guard must release active_jobs");
}

#[tokio::test]
async fn test_start_job_sync_no_matching_agents_is_error() {
    let registry = registry_with(&[("a", &["search"])]).await;
    let coord = Coordinator::with_router(registry, router(&[], Ok("unused")));

    let err = coord
        .start_job_sync(descriptor("job-none", &["absent"]))
        .await
        .expect_err("no match must fail")
        .to_string();
    assert!(err.contains("no agents match"), "got: {err}");

    let snap = coord.metrics_snapshot();
    assert_eq!(snap.errors, 1);
    assert_eq!(snap.completed_jobs, 0);
    assert_eq!(snap.active_jobs, 0);
}

#[tokio::test]
async fn test_start_job_sync_all_sends_fail_is_error() {
    let registry = registry_with(&[("a", &["search"]), ("b", &["search"])]).await;
    let coord = Coordinator::with_router(
        registry,
        router(
            &[
                ("a", Err(anyhow::anyhow!("boom a"))),
                ("b", Err(anyhow::anyhow!("boom b"))),
            ],
            Ok("unused"),
        ),
    );

    let err = coord
        .start_job_sync(descriptor("job-fail", &["search"]))
        .await
        .expect_err("all failures must error")
        .to_string();
    assert!(err.contains("no successful responses"), "got: {err}");

    let snap = coord.metrics_snapshot();
    // Two generic send errors plus the terminal no-response error.
    assert_eq!(snap.errors, 3);
    assert_eq!(snap.completed_jobs, 1);
}

#[tokio::test]
async fn test_start_job_sync_with_policy_resolver() {
    use ragent_agent::orchestrator::policy::{ConflictPolicy, ConflictResolver};

    let registry = registry_with(&[("a", &["search"]), ("b", &["search"])]).await;
    let coord = Coordinator::with_router(
        registry,
        router(&[("a", Ok("answer")), ("b", Ok("answer"))], Ok("unused")),
    )
    .with_policy(ConflictResolver::new(ConflictPolicy::Consensus {
        threshold: 2,
    }));

    let out = coord
        .start_job_sync(descriptor("job-consensus", &["search"]))
        .await
        .expect("consensus job");
    assert!(out.contains("consensus"), "got: {out}");
}

#[tokio::test]
async fn test_timeout_send_error_is_classified_as_timeout() {
    let registry = registry_with(&[("slow", &["search"])]).await;
    let coord = Coordinator::with_router(
        registry,
        router(
            &[("slow", Err(anyhow::anyhow!("request to agent timed out")))],
            Ok("unused"),
        ),
    );

    let _ = coord
        .start_job_sync(descriptor("job-timeout", &["search"]))
        .await
        .expect_err("timed-out send leaves no responses");

    let snap = coord.metrics_snapshot();
    assert_eq!(
        snap.timeouts, 1,
        "timeout wording must bump the timeouts counter"
    );
    assert_eq!(
        snap.errors, 1,
        "only the terminal no-response error is a generic error"
    );
}

// ---------------------------------------------------------------------------
// start_job_first_success
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_first_success_returns_first_non_error_response() {
    let registry = registry_with(&[("only", &["search"])]).await;
    let coord = Coordinator::with_router(registry, router(&[("only", Ok("good result"))], Ok("x")));

    let out = coord
        .start_job_first_success(descriptor("job-first", &["search"]))
        .await
        .expect("first success");
    assert_eq!(out, "--- agent: only ---\ngood result");

    let snap = coord.metrics_snapshot();
    assert_eq!(snap.completed_jobs, 1);
    assert_eq!(snap.active_jobs, 0);
}

#[tokio::test]
async fn test_first_success_all_error_prefixed_responses_fail() {
    let registry = registry_with(&[("only", &["search"])]).await;
    let coord = Coordinator::with_router(registry, router(&[("only", Ok("error: nope"))], Ok("x")));

    let err = coord
        .start_job_first_success(descriptor("job-first-err", &["search"]))
        .await
        .expect_err("an error:-prefixed response is not a success")
        .to_string();
    assert!(err.contains("no agent succeeded"), "got: {err}");

    // Current semantics: an `error:`-prefixed response is skipped without
    // bumping the errors counter.
    assert_eq!(coord.metrics_snapshot().errors, 0);
}

#[tokio::test]
async fn test_first_success_send_error_counts_and_bails() {
    let registry = registry_with(&[("only", &["search"])]).await;
    let coord = Coordinator::with_router(
        registry,
        router(&[("only", Err(anyhow::anyhow!("down")))], Ok("x")),
    );

    let err = coord
        .start_job_first_success(descriptor("job-first-down", &["search"]))
        .await
        .expect_err("a failed send leaves no success")
        .to_string();
    assert!(err.contains("no agent succeeded"), "got: {err}");
    assert_eq!(coord.metrics_snapshot().errors, 1);
}

#[tokio::test]
async fn test_first_success_no_matching_agents_is_error() {
    let registry = registry_with(&[("a", &["search"])]).await;
    let coord = Coordinator::with_router(registry, router(&[], Ok("x")));

    let err = coord
        .start_job_first_success(descriptor("job-first-none", &["absent"]))
        .await
        .expect_err("no match must fail")
        .to_string();
    assert!(err.contains("no agents match"), "got: {err}");
    assert_eq!(coord.metrics_snapshot().errors, 1);
}

// ---------------------------------------------------------------------------
// start_job_async
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_start_job_async_emits_events_and_stores_result() {
    let registry = registry_with(&[("fast", &["echo"])]).await;
    let coord = Coordinator::with_router(registry, router(&[("fast", Ok("pong"))], Ok("x")));

    let desc = descriptor("job-async", &["echo"]);
    let job_id = coord.start_job_async(desc).await.expect("spawn async job");
    assert_eq!(job_id, "job-async");

    // Subscribe immediately so no event is missed.
    let mut sub = coord
        .subscribe_job_events(&job_id)
        .await
        .expect("subscribe to job events");

    let mut saw_started = false;
    let mut saw_completed = false;
    while !saw_completed {
        match tokio::time::timeout(Duration::from_secs(2), sub.recv()).await {
            Ok(Ok(JobEvent::JobStarted { .. })) => saw_started = true,
            Ok(Ok(JobEvent::SubtaskAssigned { agent_id, .. })) => assert_eq!(agent_id, "fast"),
            Ok(Ok(JobEvent::SubtaskCompleted { success, .. })) => assert!(success),
            Ok(Ok(JobEvent::JobCompleted { success, .. })) => {
                assert!(success);
                saw_completed = true;
            }
            Ok(Ok(JobEvent::JobFailed { error, .. })) => panic!("unexpected failure: {error}"),
            Ok(Err(e)) => panic!("event stream error: {e}"),
            Err(e) => panic!("timed out waiting for completion event: {e}"),
        }
    }
    assert!(saw_started, "JobStarted must be observed");

    // The spawned task may not have written the result the instant the
    // completion event fires; poll briefly.
    let mut stored = None;
    for _ in 0..50 {
        if let Some((status, result)) = coord.get_job_result(&job_id).await {
            if status == "completed" {
                stored = result;
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(stored.as_deref(), Some("--- agent: fast ---\npong"));

    assert_eq!(coord.metrics_snapshot().completed_jobs, 1);
    assert_eq!(coord.metrics_snapshot().active_jobs, 0);
}

#[tokio::test]
async fn test_start_job_async_no_matches_reports_failure() {
    let registry = registry_with(&[("a", &["search"])]).await;
    let coord = Coordinator::with_router(registry, router(&[], Ok("x")));

    let job_id = coord
        .start_job_async(descriptor("job-async-fail", &["absent"]))
        .await
        .expect("spawn async job");
    let mut sub = coord
        .subscribe_job_events(&job_id)
        .await
        .expect("subscribe");

    let mut failed = false;
    while !failed {
        match tokio::time::timeout(Duration::from_secs(2), sub.recv()).await {
            Ok(Ok(JobEvent::JobFailed { error, .. })) => {
                assert!(error.contains("no agents match"), "got: {error}");
                failed = true;
            }
            Ok(Ok(JobEvent::JobCompleted { .. })) => panic!("must not complete without matches"),
            Ok(Ok(_)) => {}
            Ok(Err(e)) => panic!("event stream error: {e}"),
            Err(e) => panic!("timed out waiting for JobFailed: {e}"),
        }
    }

    let mut status = None;
    for _ in 0..50 {
        status = Some(coord.get_job_result(&job_id).await.expect("entry exists").0);
        if status.as_deref() == Some("failed") {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(status.as_deref(), Some("failed"));
    assert_eq!(coord.metrics_snapshot().errors, 1);
}

#[tokio::test]
async fn test_subscribe_unknown_job_is_error() {
    let registry = AgentRegistry::new();
    let coord = Coordinator::with_router(registry, router(&[], Ok("x")));
    assert!(coord.subscribe_job_events("missing").await.is_err());
    assert!(coord.get_job_result("missing").await.is_none());
}

// ---------------------------------------------------------------------------
// metrics snapshot surface
// ---------------------------------------------------------------------------

#[test]
fn test_metrics_snapshot_serialises_all_counters() {
    let snap = ragent_agent::orchestrator::MetricsSnapshot {
        active_jobs: 1,
        completed_jobs: 2,
        timeouts: 3,
        errors: 4,
    };
    let json = serde_json::to_value(&snap).expect("snapshot serialises");
    assert_eq!(json["active_jobs"], 1);
    assert_eq!(json["completed_jobs"], 2);
    assert_eq!(json["timeouts"], 3);
    assert_eq!(json["errors"], 4);
}
