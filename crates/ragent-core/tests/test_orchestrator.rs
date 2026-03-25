//! Tests for test_orchestrator.rs

//! Unit tests for orchestrator primitives: AgentRegistry, InProcessRouter,
//! Coordinator routing, aggregation strategies, and Metrics.

use std::sync::Arc;

use futures::future::FutureExt;
use tokio::time::{Duration, sleep};

use ragent_core::orchestrator::{
    AgentRegistry, Coordinator, InProcessRouter, JobDescriptor, OrchestrationMessage, Responder,
    Router,
};

// ── AgentRegistry ────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_registry_register_and_list() {
    let registry = AgentRegistry::new();
    assert_eq!(registry.list().await.len(), 0);

    registry
        .register("alpha", vec!["search".to_string()], None)
        .await;
    registry
        .register("beta", vec!["compile".to_string()], None)
        .await;

    let agents = registry.list().await;
    assert_eq!(agents.len(), 2);
}

#[tokio::test]
async fn test_registry_get_known_and_unknown() {
    let registry = AgentRegistry::new();
    registry.register("x", vec![], None).await;

    assert!(registry.get("x").await.is_some());
    assert!(registry.get("does-not-exist").await.is_none());
}

#[tokio::test]
async fn test_registry_unregister() {
    let registry = AgentRegistry::new();
    registry.register("removable", vec![], None).await;
    assert!(registry.get("removable").await.is_some());

    registry.unregister("removable").await;
    assert!(registry.get("removable").await.is_none());
    assert_eq!(registry.list().await.len(), 0);
}

#[tokio::test]
async fn test_registry_unregister_unknown_is_noop() {
    let registry = AgentRegistry::new();
    // Should not panic or error.
    registry.unregister("ghost").await;
}

#[tokio::test]
async fn test_registry_heartbeat_marks_alive() {
    let registry = AgentRegistry::new();
    registry.register("ping", vec![], None).await;

    // Record heartbeat timestamp before calling heartbeat().
    let before = registry.get("ping").await.unwrap();
    let ts_before = before.last_heartbeat;

    registry.heartbeat("ping").await;

    let after = registry.get("ping").await.unwrap();
    // Heartbeat should be set and at least as recent as before.
    assert!(after.last_heartbeat.is_some());
    if let (Some(t_after), Some(t_before)) = (after.last_heartbeat, ts_before) {
        assert!(t_after >= t_before);
    }
}

#[tokio::test]
async fn test_registry_heartbeat_unknown_is_noop() {
    let registry = AgentRegistry::new();
    // Should not panic.
    registry.heartbeat("nobody").await;
}

#[tokio::test]
async fn test_registry_prune_stale_removes_old_entries() {
    let registry = AgentRegistry::new();
    registry.register("stale-1", vec![], None).await;
    registry.register("stale-2", vec![], None).await;

    // Both have no heartbeat, so they are stale immediately.
    registry.prune_stale(Duration::from_millis(0)).await;
    assert_eq!(registry.list().await.len(), 0);
}

#[tokio::test]
async fn test_registry_prune_stale_keeps_fresh_entries() {
    let registry = AgentRegistry::new();
    registry.register("fresh", vec![], None).await;

    // Send a heartbeat right now.
    registry.heartbeat("fresh").await;

    // Prune with a very long stale window — should not be removed.
    registry.prune_stale(Duration::from_secs(3600)).await;
    assert!(registry.get("fresh").await.is_some());
}

#[tokio::test]
async fn test_registry_match_agents_exact_tag() {
    let registry = AgentRegistry::new();
    registry
        .register(
            "a",
            vec!["search".to_string(), "analysis".to_string()],
            None,
        )
        .await;
    registry
        .register("b", vec!["compile".to_string()], None)
        .await;

    let matches = registry.match_agents(&["search".to_string()]).await;
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].id, "a");
}

#[tokio::test]
async fn test_registry_match_agents_multiple_required() {
    let registry = AgentRegistry::new();
    registry
        .register(
            "full",
            vec!["search".to_string(), "compile".to_string()],
            None,
        )
        .await;
    registry
        .register("partial", vec!["search".to_string()], None)
        .await;

    // Only "full" satisfies both required capabilities.
    let matches = registry
        .match_agents(&["search".to_string(), "compile".to_string()])
        .await;
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].id, "full");
}

#[tokio::test]
async fn test_registry_match_agents_none() {
    let registry = AgentRegistry::new();
    registry.register("a", vec!["lint".to_string()], None).await;

    let matches = registry.match_agents(&["test".to_string()]).await;
    assert!(matches.is_empty());
}

#[tokio::test]
async fn test_registry_match_agents_substring() {
    // Capability matching uses substring semantics.
    let registry = AgentRegistry::new();
    registry
        .register("a", vec!["file:read".to_string()], None)
        .await;

    // "file" is a substring of "file:read" → should match.
    let matches = registry.match_agents(&["file".to_string()]).await;
    assert_eq!(matches.len(), 1);
}

// ── InProcessRouter ──────────────────────────────────────────────────────────

#[tokio::test]
async fn test_router_delivers_message_to_agent() {
    let registry = AgentRegistry::new();
    let echo: Responder = Arc::new(|p: String| async move { format!("echo:{}", p) }.boxed());
    registry.register("echo-agent", vec![], Some(echo)).await;

    let router = InProcessRouter::new(registry.clone());
    let msg = OrchestrationMessage {
        job_id: "j1".to_string(),
        payload: "hello".to_string(),
    };
    let _result = router.send("echo-agent", msg).await.unwrap();
    assert_eq!(result, "echo:hello");
}

#[tokio::test]
async fn test_router_error_on_missing_agent() {
    let registry = AgentRegistry::new();
    let router = InProcessRouter::new(registry.clone());
    let msg = OrchestrationMessage {
        job_id: "j".to_string(),
        payload: "x".to_string(),
    };
    let err = router.send("ghost", msg).await;
    assert!(err.is_err());
    assert!(err.unwrap_err().to_string().contains("not found"));
}

#[tokio::test]
async fn test_router_error_on_agent_without_mailbox() {
    let registry = AgentRegistry::new();
    registry.register("no-mailbox", vec![], None).await;

    let router = InProcessRouter::new(registry.clone());
    let msg = OrchestrationMessage {
        job_id: "j".to_string(),
        payload: "y".to_string(),
    };
    let err = router.send("no-mailbox", msg).await;
    assert!(err.is_err());
}

#[tokio::test]
async fn test_router_timeout_on_slow_agent() {
    let registry = AgentRegistry::new();
    let slow: Responder = Arc::new(|_: String| {
        async move {
            sleep(Duration::from_secs(5)).await;
            "late".to_string()
        }
        .boxed()
    });
    registry.register("slow-agent", vec![], Some(slow)).await;

    let mut router = InProcessRouter::new(registry.clone());
    router.request_timeout = Duration::from_millis(50);

    let msg = OrchestrationMessage {
        job_id: "j".to_string(),
        payload: "ping".to_string(),
    };
    let err = router.send("slow-agent", msg).await;
    assert!(err.is_err());
    let msg = err.unwrap_err().to_string();
    assert!(msg.contains("timed out") || msg.contains("timeout"));
}

// ── Coordinator: routing and aggregation ────────────────────────────────────

#[tokio::test]
async fn test_coordinator_sync_aggregates_all_agents() {
    let registry = AgentRegistry::new();
    let r_a: Responder = Arc::new(|p: String| async move { format!("A:{}", p) }.boxed());
    let r_b: Responder = Arc::new(|p: String| async move { format!("B:{}", p) }.boxed());
    registry
        .register("a", vec!["cap".to_string()], Some(r_a))
        .await;
    registry
        .register("b", vec!["cap".to_string()], Some(r_b))
        .await;

    let coord = Coordinator::new(registry);
    let desc = JobDescriptor {
        id: "agg-job".to_string(),
        required_capabilities: vec!["cap".to_string()],
        payload: "work".to_string(),
    };

    let _result = coord.start_job_sync(desc).await.unwrap();
    assert!(result.contains("A:work") || result.contains("agent: a"));
    assert!(result.contains("B:work") || result.contains("agent: b"));
}

#[tokio::test]
async fn test_coordinator_sync_error_no_matching_agents() {
    let registry = AgentRegistry::new();
    registry.register("a", vec!["lint".to_string()], None).await;

    let coord = Coordinator::new(registry);
    let desc = JobDescriptor {
        id: "no-match".to_string(),
        required_capabilities: vec!["missing-cap".to_string()],
        payload: "test".to_string(),
    };

    let err = coord.start_job_sync(desc).await;
    assert!(err.is_err());
    let msg = err.unwrap_err().to_string();
    assert!(msg.contains("no agents"));
}

#[tokio::test]
async fn test_coordinator_sync_error_when_all_agents_fail() {
    let registry = AgentRegistry::new();
    // Register an agent with no mailbox so the router will fail to send.
    registry
        .register("broken-agent", vec!["cap".to_string()], None)
        .await;

    let coord = Coordinator::new(registry);
    let desc = JobDescriptor {
        id: "all-fail".to_string(),
        required_capabilities: vec!["cap".to_string()],
        payload: "payload".to_string(),
    };

    let before = coord.metrics_snapshot();
    let err = coord.start_job_sync(desc).await;
    assert!(err.is_err());
    let msg = err.unwrap_err().to_string();
    assert!(msg.contains("no successful responses"));

    let after = coord.metrics_snapshot();
    assert_eq!(after.errors, before.errors + 1);
}

#[tokio::test]
async fn test_coordinator_first_success_picks_fast_agent() {
    let registry = AgentRegistry::new();
    let fast: Responder = Arc::new(|p: String| async move { format!("fast:{}", p) }.boxed());
    let also_fast: Responder =
        Arc::new(|p: String| async move { format!("also-fast:{}", p) }.boxed());
    registry
        .register("fast", vec!["task".to_string()], Some(fast))
        .await;
    registry
        .register("also-fast", vec!["task".to_string()], Some(also_fast))
        .await;

    let coord = Coordinator::new(registry);
    let desc = JobDescriptor {
        id: "fs-job".to_string(),
        required_capabilities: vec!["task".to_string()],
        payload: "do-it".to_string(),
    };

    let _result = coord.start_job_first_success(desc).await.unwrap();
    // Should return one agent's result (not both).
    assert!(result.contains("fast:") || result.contains("also-fast:"));
}

#[tokio::test]
async fn test_coordinator_first_success_skips_error_response() {
    let registry = AgentRegistry::new();
    let bad: Responder =
        Arc::new(|_: String| async move { "error: bad agent".to_string() }.boxed());
    let good: Responder = Arc::new(|p: String| async move { format!("good:{}", p) }.boxed());
    // Register bad first so it is tried first.
    registry
        .register("bad-agent", vec!["cap".to_string()], Some(bad))
        .await;
    registry
        .register("good-agent", vec!["cap".to_string()], Some(good))
        .await;

    let coord = Coordinator::new(registry);
    let desc = JobDescriptor {
        id: "skip-err".to_string(),
        required_capabilities: vec!["cap".to_string()],
        payload: "query".to_string(),
    };

    let _result = coord.start_job_first_success(desc).await.unwrap();
    assert!(result.contains("good:query") || result.contains("agent: good-agent"));
}

// ── Coordinator: metrics ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_coordinator_metrics_increment_on_completed_job() {
    let registry = AgentRegistry::new();
    let r: Responder = Arc::new(|p: String| async move { format!("ok:{}", p) }.boxed());
    registry
        .register("m-agent", vec!["m".to_string()], Some(r))
        .await;

    let coord = Coordinator::new(registry);
    let snap_before = coord.metrics_snapshot();

    let desc = JobDescriptor {
        id: "m-job".to_string(),
        required_capabilities: vec!["m".to_string()],
        payload: "go".to_string(),
    };
    coord.start_job_sync(desc).await.unwrap();

    let snap_after = coord.metrics_snapshot();
    assert_eq!(snap_after.completed_jobs, snap_before.completed_jobs + 1);
    assert_eq!(snap_after.active_jobs, 0);
    assert_eq!(snap_after.errors, 0);
}

#[tokio::test]
async fn test_coordinator_metrics_error_increments_on_no_match() {
    let registry = AgentRegistry::new();
    let coord = Coordinator::new(registry);
    let snap_before = coord.metrics_snapshot();

    let desc = JobDescriptor {
        id: "bad-job".to_string(),
        required_capabilities: vec!["nope".to_string()],
        payload: "x".to_string(),
    };
    let _ = coord.start_job_sync(desc).await;

    let snap_after = coord.metrics_snapshot();
    assert_eq!(snap_after.errors, snap_before.errors + 1);
}

#[tokio::test]
async fn test_coordinator_metrics_timeout_increments() {
    let registry = AgentRegistry::new();
    let slow: Responder = Arc::new(|_: String| {
        async move {
            sleep(Duration::from_secs(5)).await;
            "late".to_string()
        }
        .boxed()
    });
    registry
        .register("slow", vec!["work".to_string()], Some(slow))
        .await;

    let coord = Coordinator::with_request_timeout(registry, Duration::from_millis(30));
    let snap_before = coord.metrics_snapshot();

    let desc = JobDescriptor {
        id: "timeout-job".to_string(),
        required_capabilities: vec!["work".to_string()],
        payload: "slow-task".to_string(),
    };
    // start_job_sync: the agent times out, error counted as timeout.
    let _ = coord.start_job_sync(desc).await;

    let snap_after = coord.metrics_snapshot();
    assert_eq!(snap_after.timeouts, snap_before.timeouts + 1);
}

#[tokio::test]
async fn test_inprocess_router_send_agent_not_found() {
    let registry = AgentRegistry::new();
    let router = InProcessRouter::new(registry);

    let res = router
        .send(
            "missing",
            OrchestrationMessage {
                job_id: "job".to_string(),
                payload: "payload".to_string(),
            },
        )
        .await;

    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("not found"));
}

#[tokio::test]
async fn test_inprocess_router_send_agent_no_mailbox() {
    let registry = AgentRegistry::new();
    registry
        .register("a", vec!["cap".to_string()], None)
        .await;
    let router = InProcessRouter::new(registry);

    let res = router
        .send(
            "a",
            OrchestrationMessage {
                job_id: "job".to_string(),
                payload: "payload".to_string(),
            },
        )
        .await;

    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("has no mailbox"));
}
