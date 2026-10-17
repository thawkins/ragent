//! Tests-only coverage for the orchestration transport adapters
//! (`ragent_agent::orchestrator::transport`).
//!
//! `HttpRouter`'s registry (register/list/match/unregister) and its
//! unregistered-agent error path need no network; `RouterComposite` is driven
//! through fake routers.

use std::sync::Arc;
use std::time::Duration;

use ragent_agent::orchestrator::Router;
use ragent_agent::orchestrator::coordinator::OrchestrationMessage;
use ragent_agent::orchestrator::transport::{HttpRouter, RemoteAgentDescriptor, RouterComposite};

fn desc(id: &str, caps: &[&str]) -> RemoteAgentDescriptor {
    RemoteAgentDescriptor {
        id: id.to_string(),
        capabilities: caps.iter().map(|c| (*c).to_string()).collect(),
        endpoint_url: "http://127.0.0.1:1/agent".to_string(),
    }
}

/// A router returning a fixed result, or always failing.
struct FakeRouter {
    ok: Option<&'static str>,
}

#[async_trait::async_trait]
impl Router for FakeRouter {
    async fn send(&self, _agent_id: &str, _msg: OrchestrationMessage) -> anyhow::Result<String> {
        match self.ok {
            Some(r) => Ok(r.to_string()),
            None => Err(anyhow::anyhow!("fake router failed")),
        }
    }
}

fn msg() -> OrchestrationMessage {
    OrchestrationMessage {
        job_id: "job-1".to_string(),
        payload: "p".to_string(),
    }
}

#[tokio::test]
async fn http_router_registry_round_trip() {
    let router = HttpRouter::new(Duration::from_secs(2));
    assert!(router.list().await.is_empty());

    router.register(desc("agent-a", &["search"])).await;
    router.register(desc("agent-b", &["write"])).await;
    assert_eq!(router.list().await.len(), 2);

    router.unregister("agent-a").await;
    let remaining = router.list().await;
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, "agent-b");
}

#[tokio::test]
async fn http_router_capability_matching_requires_all_tags() {
    let router = HttpRouter::new(Duration::from_secs(2));
    router.register(desc("both", &["search", "write"])).await;
    router.register(desc("search-only", &["search"])).await;

    let both = router
        .match_agents(&["search".to_string(), "write".to_string()])
        .await;
    assert_eq!(both.len(), 1, "only the agent with both caps matches");
    assert_eq!(both[0].id, "both");

    let search = router.match_agents(&["search".to_string()]).await;
    assert_eq!(search.len(), 2, "both agents advertise search");
}

#[tokio::test]
async fn http_router_unregistered_agent_errors_without_a_request() {
    let router = HttpRouter::new(Duration::from_secs(1));
    let err = router
        .send("ghost", msg())
        .await
        .expect_err("an unregistered agent must error before any HTTP call");
    assert!(err.to_string().contains("not registered"), "err: {err}");
}

#[tokio::test]
async fn router_composite_returns_first_success() {
    let composite = RouterComposite::new(vec![
        Arc::new(FakeRouter { ok: None }),
        Arc::new(FakeRouter { ok: Some("second") }),
    ]);
    let out = composite
        .send("a", msg())
        .await
        .expect("second router wins");
    assert_eq!(out, "second");
}

#[tokio::test]
async fn router_composite_all_failing_returns_the_last_error() {
    let composite = RouterComposite::new(vec![
        Arc::new(FakeRouter { ok: None }),
        Arc::new(FakeRouter { ok: None }),
    ]);
    let err = composite
        .send("a", msg())
        .await
        .expect_err("all routers failing must error");
    assert!(err.to_string().contains("fake router failed"), "err: {err}");
}

#[tokio::test]
async fn router_composite_with_no_routers_errors() {
    let composite = RouterComposite::new(vec![]);
    let err = composite
        .send("a", msg())
        .await
        .expect_err("empty composite must error");
    assert!(
        err.to_string().contains("no routers configured"),
        "err: {err}"
    );
}
