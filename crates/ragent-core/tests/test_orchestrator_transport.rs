//! Tests for test_orchestrator_transport.rs

//! Tests for the pluggable transport layer (Task 5.1):
//! RemoteAgentDescriptor, HttpRouter (without live network), RouterComposite.

use std::sync::Arc;
use std::time::Duration;

use futures::future::FutureExt;

use ragent_core::orchestrator::transport::{HttpRouter, RemoteAgentDescriptor, RouterComposite};
use ragent_core::orchestrator::{
    AgentRegistry, Coordinator, InProcessRouter, JobDescriptor, OrchestrationMessage, Responder,
    Router,
};

// ── HttpRouter registration API ──────────────────────────────────────────────

#[tokio::test]
async fn test_http_router_register_and_list() {
    let router = HttpRouter::new(Duration::from_secs(5));
    assert_eq!(router.list().await.len(), 0);

    router
        .register(RemoteAgentDescriptor {
            id: "remote-a".to_string(),
            capabilities: vec!["search".to_string()],
            endpoint_url: "http://localhost:9090/agent".to_string(),
        })
        .await;
    assert_eq!(router.list().await.len(), 1);
}

#[tokio::test]
async fn test_http_router_unregister() {
    let router = HttpRouter::new(Duration::from_secs(5));
    router
        .register(RemoteAgentDescriptor {
            id: "tmp".to_string(),
            capabilities: vec![],
            endpoint_url: "http://localhost:9090".to_string(),
        })
        .await;
    router.unregister("tmp").await;
    assert!(router.list().await.is_empty());
}

#[tokio::test]
async fn test_http_router_match_agents() {
    let router = HttpRouter::new(Duration::from_secs(5));
    router
        .register(RemoteAgentDescriptor {
            id: "a".to_string(),
            capabilities: vec!["search".to_string(), "compile".to_string()],
            endpoint_url: "http://a.local/".to_string(),
        })
        .await;
    router
        .register(RemoteAgentDescriptor {
            id: "b".to_string(),
            capabilities: vec!["lint".to_string()],
            endpoint_url: "http://b.local/".to_string(),
        })
        .await;

    let matches = router.match_agents(&["search".to_string()]).await;
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].id, "a");
}

#[tokio::test]
async fn test_http_router_match_agents_none() {
    let router = HttpRouter::new(Duration::from_secs(5));
    router
        .register(RemoteAgentDescriptor {
            id: "z".to_string(),
            capabilities: vec!["write".to_string()],
            endpoint_url: "http://z.local/".to_string(),
        })
        .await;
    let matches = router.match_agents(&["read".to_string()]).await;
    assert!(matches.is_empty());
}

/// Sending to an unknown agent id returns Err.
#[tokio::test]
async fn test_http_router_send_unknown_agent_returns_err() {
    let router = HttpRouter::new(Duration::from_millis(200));
    let msg = OrchestrationMessage {
        job_id: "j".to_string(),
        payload: "x".to_string(),
    };
    let err = router.send("ghost", msg).await;
    assert!(err.is_err());
    assert!(err.unwrap_err().to_string().contains("not registered"));
}

/// Sending to a registered but unreachable endpoint times out / fails.
#[tokio::test]
async fn test_http_router_send_unreachable_endpoint_returns_err() {
    let router = HttpRouter::new(Duration::from_millis(100));
    router
        .register(RemoteAgentDescriptor {
            id: "unreachable".to_string(),
            capabilities: vec![],
            // No real server listening here.
            endpoint_url: "http://127.0.0.1:19999/agent".to_string(),
        })
        .await;

    let msg = OrchestrationMessage {
        job_id: "j".to_string(),
        payload: "test".to_string(),
    };
    let err = router.send("unreachable", msg).await;
    assert!(err.is_err(), "expected error when endpoint is unreachable");
}

// ── RouterComposite ──────────────────────────────────────────────────────────

/// Composite with an in-process router as primary: local agent is resolved.
#[tokio::test]
async fn test_composite_uses_first_router_on_success() {
    let registry = AgentRegistry::new();
    let echo: Responder = Arc::new(|p: String| async move { format!("local:{}", p) }.boxed());
    registry.register("local-agent", vec![], Some(echo)).await;

    let local = Arc::new(InProcessRouter::new(registry.clone()));
    let http = Arc::new(HttpRouter::new(Duration::from_millis(100)));
    let composite = RouterComposite::new(vec![local, http]);

    let msg = OrchestrationMessage {
        job_id: "j".to_string(),
        payload: "ping".to_string(),
    };
    let result = composite.send("local-agent", msg).await.unwrap();
    assert_eq!(result, "local:ping");
}

/// Composite falls back to second router when first fails.
#[tokio::test]
async fn test_composite_falls_back_to_second_router() {
    // First router: in-process registry has no agent registered → returns Err.
    let empty_registry = AgentRegistry::new();
    let local = Arc::new(InProcessRouter::new(empty_registry));

    // Second router: in-process registry has the agent.
    let registry2 = AgentRegistry::new();
    let r: Responder = Arc::new(|p: String| async move { format!("fallback:{}", p) }.boxed());
    registry2.register("fb-agent", vec![], Some(r)).await;
    let fallback = Arc::new(InProcessRouter::new(registry2));

    let composite = RouterComposite::new(vec![local, fallback]);

    let msg = OrchestrationMessage {
        job_id: "j".to_string(),
        payload: "hi".to_string(),
    };
    let result = composite.send("fb-agent", msg).await.unwrap();
    assert_eq!(result, "fallback:hi");
}

/// Composite returns Err when all routers fail.
#[tokio::test]
async fn test_composite_all_fail_returns_err() {
    let r1 = Arc::new(InProcessRouter::new(AgentRegistry::new()));
    let r2 = Arc::new(InProcessRouter::new(AgentRegistry::new()));
    let composite = RouterComposite::new(vec![r1, r2]);

    let msg = OrchestrationMessage {
        job_id: "j".to_string(),
        payload: "x".to_string(),
    };
    assert!(composite.send("nobody", msg).await.is_err());
}

/// Empty composite returns Err with a helpful message.
#[tokio::test]
async fn test_composite_empty_returns_err() {
    let composite = RouterComposite::new(vec![]);
    let msg = OrchestrationMessage {
        job_id: "j".to_string(),
        payload: "y".to_string(),
    };
    let err = composite.send("anyone", msg).await;
    assert!(err.is_err());
    assert!(err.unwrap_err().to_string().contains("no routers"));
}

// ── Coordinator + custom router ───────────────────────────────────────────────

/// Coordinator accepts a composite router and routes correctly.
#[tokio::test]
async fn test_coordinator_with_composite_router() {
    let registry = AgentRegistry::new();
    let r: Responder = Arc::new(|p: String| async move { format!("comp:{}", p) }.boxed());
    registry
        .register("comp-agent", vec!["cap".to_string()], Some(r))
        .await;

    let local = Arc::new(InProcessRouter::new(registry.clone()));
    let composite = RouterComposite::new(vec![local]);

    let coord = Coordinator::with_router(registry.clone(), Arc::new(composite));
    let desc = JobDescriptor {
        id: "c-job".to_string(),
        required_capabilities: vec!["cap".to_string()],
        payload: "work".to_string(),
    };
    let result = coord.start_job_sync(desc).await.unwrap();
    assert!(result.contains("comp:work"));
}
