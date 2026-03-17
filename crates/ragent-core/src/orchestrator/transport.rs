//! Pluggable transport adapters for remote agent communication (Task 5.1).
//!
//! Provides:
//! - [`RemoteAgentDescriptor`] — metadata for a remote agent (URL + capabilities).
//! - [`HttpRouter`] — [`Router`] impl that dispatches to remote agents via HTTP POST.
//! - [`RouterComposite`] — tries an ordered list of routers in sequence, returning
//!   the first success.  Typical use: in-process first, HTTP fallback.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use super::{AgentId, OrchestrationMessage, Router};

// ── Remote agent descriptor ──────────────────────────────────────────────────

/// Metadata for an agent hosted on a remote endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteAgentDescriptor {
    /// Unique agent identifier (must match the server-side agent id).
    pub id: AgentId,
    /// Capability tags advertised by the remote agent.
    pub capabilities: Vec<String>,
    /// HTTP(S) endpoint that accepts `POST` requests with a JSON
    /// [`RemoteAgentRequest`] body.
    pub endpoint_url: String,
}

/// JSON body POSTed to a remote agent.
#[derive(Serialize)]
struct RemoteAgentRequest {
    job_id: String,
    payload: String,
}

/// JSON response expected from a remote agent.
#[derive(Deserialize)]
struct RemoteAgentResponse {
    result: String,
}

// ── HttpRouter ───────────────────────────────────────────────────────────────

/// Routes orchestration messages to remote agents via HTTP POST.
///
/// Register remote agents with [`HttpRouter::register`], then use as a
/// [`Router`] in a [`crate::orchestrator::Coordinator`].
///
/// # Remote agent contract
///
/// The remote endpoint must accept:
/// ```json
/// POST <endpoint_url>
/// Content-Type: application/json
/// { "job_id": "…", "payload": "…" }
/// ```
/// and respond with:
/// ```json
/// { "result": "…" }
/// ```
#[derive(Clone)]
pub struct HttpRouter {
    agents: Arc<RwLock<HashMap<AgentId, RemoteAgentDescriptor>>>,
    client: reqwest::Client,
    /// Per-request HTTP timeout.
    pub request_timeout: Duration,
}

impl HttpRouter {
    /// Create a new router with the given per-request timeout.
    pub fn new(request_timeout: Duration) -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            client: reqwest::Client::new(),
            request_timeout,
        }
    }

    /// Register a remote agent.
    pub async fn register(&self, desc: RemoteAgentDescriptor) {
        self.agents.write().await.insert(desc.id.clone(), desc);
    }

    /// Unregister a remote agent.
    pub async fn unregister(&self, id: &str) {
        self.agents.write().await.remove(id);
    }

    /// List all registered remote agents.
    pub async fn list(&self) -> Vec<RemoteAgentDescriptor> {
        self.agents.read().await.values().cloned().collect()
    }

    /// Find remote agents that satisfy all `required` capability tags.
    pub async fn match_agents(&self, required: &[String]) -> Vec<RemoteAgentDescriptor> {
        self.agents
            .read()
            .await
            .values()
            .filter(|d| {
                required.iter().all(|r| d.capabilities.iter().any(|c| c.contains(r.as_str())))
            })
            .cloned()
            .collect()
    }
}

#[async_trait::async_trait]
impl Router for HttpRouter {
    async fn send(&self, agent_id: &str, msg: OrchestrationMessage) -> Result<String> {
        let desc = self
            .agents
            .read()
            .await
            .get(agent_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("remote agent '{}' not registered", agent_id))?;

        let body = RemoteAgentRequest { job_id: msg.job_id, payload: msg.payload };

        let resp = tokio::time::timeout(
            self.request_timeout,
            self.client.post(&desc.endpoint_url).json(&body).send(),
        )
        .await
        .map_err(|_| anyhow::anyhow!("HTTP request to agent '{}' timed out", agent_id))?
        .map_err(|e| anyhow::anyhow!("HTTP request failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            return Err(anyhow::anyhow!(
                "remote agent '{}' returned HTTP {}",
                agent_id,
                status
            ));
        }

        let parsed: RemoteAgentResponse = resp
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("failed to parse agent response: {}", e))?;

        Ok(parsed.result)
    }
}

// ── RouterComposite ──────────────────────────────────────────────────────────

/// Tries a sequence of [`Router`]s in order, returning the first success.
///
/// Typical usage: pair an [`super::InProcessRouter`] (fast local delivery) with
/// an [`HttpRouter`] (remote fallback) so that local agents are preferred but
/// remote agents are reachable when the local registry has no match.
///
/// ```rust,ignore
/// let composite = RouterComposite::new(vec![
///     Arc::new(InProcessRouter::new(registry.clone())),
///     Arc::new(http_router),
/// ]);
/// let coord = Coordinator::with_router(registry, Arc::new(composite));
/// ```
#[derive(Clone)]
pub struct RouterComposite {
    routers: Vec<Arc<dyn Router>>,
}

impl RouterComposite {
    /// Create a composite from an ordered list of routers.  The first router
    /// that returns `Ok` wins; all others are skipped.
    pub fn new(routers: Vec<Arc<dyn Router>>) -> Self {
        Self { routers }
    }
}

#[async_trait::async_trait]
impl Router for RouterComposite {
    async fn send(&self, agent_id: &str, msg: OrchestrationMessage) -> Result<String> {
        let mut last_err = anyhow::anyhow!("no routers configured");
        for router in &self.routers {
            match router.send(agent_id, msg.clone()).await {
                Ok(resp) => return Ok(resp),
                Err(e) => last_err = e,
            }
        }
        Err(last_err)
    }
}
