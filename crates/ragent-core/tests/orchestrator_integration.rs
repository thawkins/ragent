//! Integration tests for the orchestration layer.
//!
//! These tests spin up multiple in-process agents and exercise the full
//! Coordinator flow: capability matching, subtask assignment, result
//! aggregation, timeout handling, and event subscription.

use ragent_core::orchestrator::*;
use std::sync::Arc;
use futures::future::FutureExt;
use tokio::time::{sleep, Duration};

// ── First-success strategy ───────────────────────────────────────────────────

#[tokio::test]
async fn test_first_success_with_retries() {
    let registry = AgentRegistry::new();

    // Agent 1: sleeps longer than timeout
    let slow: Responder = Arc::new(|payload: String| {
        async move {
            sleep(Duration::from_millis(200)).await;
            format!("slow handled: {}", payload)
        }
        .boxed()
    });

    // Agent 2: immediate error (simulate by returning an error-like string)
    let err: Responder = Arc::new(|payload: String| {
        async move { format!("error: failed to handle {}", payload) }.boxed()
    });

    // Agent 3: quick success
    let fast: Responder = Arc::new(|payload: String| {
        async move { format!("fast success: {}", payload) }.boxed()
    });

    registry.register("agent-slow", vec!["search".to_string()], Some(slow)).await;
    registry.register("agent-err", vec!["search".to_string()], Some(err)).await;
    registry.register("agent-fast", vec!["search".to_string()], Some(fast)).await;

    // Create coordinator with short per-agent timeout so the slow agent times out.
    let coord = Coordinator::with_request_timeout(registry.clone(), Duration::from_millis(50));

    let desc = JobDescriptor { id: "job-retry".to_string(), required_capabilities: vec!["search".to_string()], payload: "do work".to_string() };

    // Should eventually return the fast agent's response (first-success strategy)
    let res = coord.start_job_first_success(desc).await.unwrap();
    assert!(res.contains("agent-fast") || res.contains("fast success"));
}

#[tokio::test]
async fn test_first_success_all_fail() {
    let registry = AgentRegistry::new();

    // All agents sleep beyond timeout
    let slow1: Responder = Arc::new(|payload: String| {
        async move { sleep(Duration::from_millis(200)).await; format!("s1: {}", payload) }.boxed()
    });
    let slow2: Responder = Arc::new(|payload: String| {
        async move { sleep(Duration::from_millis(200)).await; format!("s2: {}", payload) }.boxed()
    });

    registry.register("agent-s1", vec!["analysis".to_string()], Some(slow1)).await;
    registry.register("agent-s2", vec!["analysis".to_string()], Some(slow2)).await;

    let coord = Coordinator::with_request_timeout(registry.clone(), Duration::from_millis(50));
    let desc = JobDescriptor { id: "job-fail".to_string(), required_capabilities: vec!["analysis".to_string()], payload: "work".to_string() };

    let res = coord.start_job_first_success(desc).await;
    assert!(res.is_err());
}

// ── Async job lifecycle ──────────────────────────────────────────────────────

/// Start an async job, poll until completed, assert result.
#[tokio::test]
async fn test_async_job_lifecycle_poll_to_completion() {
    let registry = AgentRegistry::new();
    let r: Responder = Arc::new(|p: String| async move { format!("done:{}", p) }.boxed());
    registry.register("worker", vec!["task".to_string()], Some(r)).await;

    let coord = Coordinator::new(registry);
    let desc = JobDescriptor {
        id: "async-poll".to_string(),
        required_capabilities: vec!["task".to_string()],
        payload: "payload-x".to_string(),
    };

    let job_id = coord.start_job_async(desc).await.unwrap();

    // Poll until completed (max 2 s).
    let mut status = String::new();
    for _ in 0..40 {
        if let Some((s, _)) = coord.get_job_result(&job_id).await {
            status = s.clone();
            if s == "completed" || s == "failed" {
                break;
            }
        }
        sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(status, "completed");

    let (_, result) = coord.get_job_result(&job_id).await.unwrap();
    assert!(result.unwrap().contains("done:payload-x"));
}

/// get_job_result returns None for an unknown job id.
#[tokio::test]
async fn test_get_job_result_unknown_id_returns_none() {
    let registry = AgentRegistry::new();
    let coord = Coordinator::new(registry);
    assert!(coord.get_job_result("nonexistent-job").await.is_none());
}

// ── Event subscription ───────────────────────────────────────────────────────

/// Subscribing to a known job delivers JobStarted + SubtaskAssigned +
/// SubtaskCompleted + JobCompleted events in order.
#[tokio::test]
async fn test_event_subscription_receives_lifecycle_events() {
    let registry = AgentRegistry::new();
    let r: Responder =
        Arc::new(|p: String| async move { format!("resp:{}", p) }.boxed());
    registry.register("ev-agent", vec!["ev".to_string()], Some(r)).await;

    let coord = Coordinator::new(registry);
    let desc = JobDescriptor {
        id: "ev-job".to_string(),
        required_capabilities: vec!["ev".to_string()],
        payload: "evt".to_string(),
    };

    let job_id = coord.start_job_async(desc).await.unwrap();
    let mut sub = coord.subscribe_job_events(&job_id).await.unwrap();

    let mut started = false;
    let mut assigned = false;
    let mut completed = false;

    for _ in 0..10 {
        match tokio::time::timeout(Duration::from_secs(2), sub.recv()).await {
            Ok(Ok(ev)) => match ev {
                JobEvent::JobStarted { .. } => started = true,
                JobEvent::SubtaskAssigned { .. } => assigned = true,
                JobEvent::JobCompleted { .. } => {
                    completed = true;
                    break;
                }
                _ => {}
            },
            _ => break,
        }
    }

    assert!(started, "expected JobStarted event");
    assert!(assigned, "expected SubtaskAssigned event");
    assert!(completed, "expected JobCompleted event");
}

/// subscribe_job_events returns Err for an unknown job id.
#[tokio::test]
async fn test_subscribe_unknown_job_returns_error() {
    let registry = AgentRegistry::new();
    let coord = Coordinator::new(registry);
    let err = coord.subscribe_job_events("unknown-job-id").await;
    assert!(err.is_err());
}

// ── Concurrent jobs ──────────────────────────────────────────────────────────

/// Multiple independent jobs can run concurrently; each returns its own result.
#[tokio::test]
async fn test_concurrent_jobs_independent_results() {
    let registry = AgentRegistry::new();
    let r: Responder = Arc::new(|p: String| async move { format!("result-{}", p) }.boxed());
    registry.register("shared-agent", vec!["work".to_string()], Some(r)).await;

    let coord = Coordinator::new(registry);

    let jobs = futures::future::join_all((0..5).map(|i| {
        let coord = coord.clone();
        async move {
            let desc = JobDescriptor {
                id: format!("conc-job-{}", i),
                required_capabilities: vec!["work".to_string()],
                payload: format!("item-{}", i),
            };
            coord.start_job_sync(desc).await
        }
    }))
    .await;

    for (i, res) in jobs.into_iter().enumerate() {
        let s = res.unwrap();
        assert!(s.contains(&format!("item-{}", i)), "job {} result missing payload", i);
    }
}

// ── Timeout propagation ──────────────────────────────────────────────────────

/// When all agents time out, start_job_sync still returns Ok (partial result),
/// but timeout counter is incremented.
#[tokio::test]
async fn test_sync_job_timeout_increments_metrics() {
    let registry = AgentRegistry::new();
    let slow: Responder = Arc::new(|_: String| {
        async move {
            sleep(Duration::from_secs(5)).await;
            "never".to_string()
        }
        .boxed()
    });
    registry.register("slow-work", vec!["slowcap".to_string()], Some(slow)).await;

    let coord = Coordinator::with_request_timeout(registry, Duration::from_millis(30));
    let snap_before = coord.metrics_snapshot();

    let desc = JobDescriptor {
        id: "timeout-integ".to_string(),
        required_capabilities: vec!["slowcap".to_string()],
        payload: "big-task".to_string(),
    };
    let _ = coord.start_job_sync(desc).await;

    let snap_after = coord.metrics_snapshot();
    assert!(snap_after.timeouts > snap_before.timeouts);
}

// ── Async job: failed path ───────────────────────────────────────────────────

/// An async job with no matching agents finishes in "failed" state.
#[tokio::test]
async fn test_async_job_no_agents_status_failed() {
    let registry = AgentRegistry::new();
    let coord = Coordinator::new(registry);
    let desc = JobDescriptor {
        id: "will-fail".to_string(),
        required_capabilities: vec!["unknown-cap".to_string()],
        payload: "x".to_string(),
    };

    let job_id = coord.start_job_async(desc).await.unwrap();

    // Give background task a moment to run.
    sleep(Duration::from_millis(100)).await;

    let (status, _) = coord.get_job_result(&job_id).await.unwrap();
    assert_eq!(status, "failed");
}

// ── Multiple capabilities required ──────────────────────────────────────────

/// Only agents that satisfy ALL required capabilities are selected.
#[tokio::test]
async fn test_multi_capability_only_full_match_selected() {
    let registry = AgentRegistry::new();
    let full: Responder =
        Arc::new(|p: String| async move { format!("full:{}", p) }.boxed());
    let partial: Responder =
        Arc::new(|p: String| async move { format!("partial:{}", p) }.boxed());

    registry
        .register("full-agent", vec!["read".to_string(), "write".to_string()], Some(full))
        .await;
    registry
        .register("partial-agent", vec!["read".to_string()], Some(partial))
        .await;

    let coord = Coordinator::new(registry);
    let desc = JobDescriptor {
        id: "multi-cap".to_string(),
        required_capabilities: vec!["read".to_string(), "write".to_string()],
        payload: "content".to_string(),
    };

    let result = coord.start_job_sync(desc).await.unwrap();
    assert!(result.contains("full:content"), "expected full-agent result");
    assert!(
        !result.contains("partial:content"),
        "partial-agent should not have been selected"
    );
}

