//! Multi-agent orchestration primitives (MVP)
//!
//! Provides an in-process AgentRegistry, a router using actor-style inboxes
//! (Tokio mpsc channels) for agent mailboxes, and a Coordinator that can start
//! a job and collect responses from multiple agents.
//!
//! Messaging pattern chosen for MVP:
//! - Each in-process agent is given a mailbox (mpsc channel) that receives
//!   OrchestrationRequest items. The router sends a request to the mailbox and
//!   awaits a one-shot reply. This gives actor-like inbox semantics and allows
//!   later swapping to a remote transport without changing Coordinator logic.

use anyhow::Result;
use futures::future::BoxFuture;
use futures::future::FutureExt;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio::time::{timeout, Duration};

/// Identifier for an agent.
pub type AgentId = String;

/// Responder callback type for in-process agents.
pub type Responder = Arc<dyn Fn(String) -> BoxFuture<'static, String> + Send + Sync>;

/// Internal request sent to an agent mailbox.
pub struct OrchestrationRequest {
    pub job_id: String,
    pub payload: String,
    pub reply: oneshot::Sender<String>,
}

/// Agent metadata stored in the registry.
#[derive(Clone)]
pub struct AgentEntry {
    pub id: AgentId,
    pub capabilities: Vec<String>,
    /// Optional in-process responder (kept for compatibility). If present,
    /// registry.register will create a mailbox and spawn a task that invokes
    /// this responder for incoming requests.
    pub responder: Option<Responder>,
    /// Mailbox sender for actor-style message delivery.
    pub mailbox: Option<mpsc::Sender<OrchestrationRequest>>,
}

impl AgentEntry {
    pub fn new(
        id: impl Into<String>,
        capabilities: Vec<String>,
        responder: Option<Responder>,
        mailbox: Option<mpsc::Sender<OrchestrationRequest>>,
    ) -> Self {
        Self {
            id: id.into(),
            capabilities,
            responder,
            mailbox,
        }
    }
}

/// Simple capability-based registry for agents.
#[derive(Clone, Default)]
pub struct AgentRegistry {
    inner: Arc<RwLock<HashMap<AgentId, AgentEntry>>>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self { inner: Arc::new(RwLock::new(HashMap::new())) }
    }

    /// Register an agent with capabilities and an optional in-process responder.
    ///
    /// If a responder is provided the registry will create a mailbox (mpsc
    /// channel) and spawn a background task that pulls messages from the
    /// mailbox and invokes the responder, sending back responses via the
    /// one-shot reply channel.
    pub async fn register(&self, id: impl Into<String>, capabilities: Vec<String>, responder: Option<Responder>) {
        let id = id.into();

        let mut mailbox_opt = None;
        if let Some(responder) = responder.clone() {
            // create a channel for the agent mailbox
            let (tx, mut rx) = mpsc::channel::<OrchestrationRequest>(16);
            mailbox_opt = Some(tx.clone());

            // Spawn the agent loop which turns mailbox messages into responder calls.
            tokio::spawn(async move {
                while let Some(req) = rx.recv().await {
                    let fut = (responder)(req.payload);
                    let resp = fut.await;
                    // best-effort: ignore send error
                    let _ = req.reply.send(resp);
                }
            });
        }

        let entry = AgentEntry::new(id.clone(), capabilities, None, mailbox_opt);
        self.inner.write().await.insert(id, entry);
    }

    /// Unregister an agent.
    pub async fn unregister(&self, id: &str) {
        self.inner.write().await.remove(id);
    }

    /// List all agents.
    pub async fn list(&self) -> Vec<AgentEntry> {
        self.inner.read().await.values().cloned().collect()
    }

    /// Get a specific agent by id.
    pub async fn get(&self, id: &str) -> Option<AgentEntry> {
        self.inner.read().await.get(id).cloned()
    }

    /// Find agents whose capabilities include all of the required tags (substring match).
    pub async fn match_agents(&self, required: &[String]) -> Vec<AgentEntry> {
        let agents = self.inner.read().await;
        agents
            .values()
            .filter(|entry| {
                required.iter().all(|req| entry.capabilities.iter().any(|c| c.contains(req)))
            })
            .cloned()
            .collect()
    }
}

/// Router abstraction for sending requests to agents.
#[derive(Clone)]
pub struct InProcessRouter {
    registry: AgentRegistry,
    /// Default per-request timeout applied by the router when awaiting a reply.
    pub request_timeout: Duration,
}

impl InProcessRouter {
    pub fn new(registry: AgentRegistry) -> Self {
        Self { registry, request_timeout: Duration::from_secs(5) }
    }

    /// Sends a request to the named agent and awaits a response. Returns an error
    /// if the agent is not registered, has no mailbox, or the reply times out.
    pub async fn request_response(&self, agent_id: &str, msg: OrchestrationMessage) -> Result<String> {
        let ent = self.registry.get(agent_id).await;
        let ent = ent.ok_or_else(|| anyhow::anyhow!("agent '{agent_id}' not found"))?;

        // If the agent has a mailbox use the actor-style delivery.
        if let Some(tx) = ent.mailbox {
            let (reply_tx, reply_rx) = oneshot::channel::<String>();
            let req = OrchestrationRequest { job_id: msg.job_id, payload: msg.payload, reply: reply_tx };
            // send to mailbox
            tx.send(req).await.map_err(|_| anyhow::anyhow!("failed to send to agent mailbox"))?;

            // await reply with timeout
            let res = timeout(self.request_timeout, reply_rx).await;
            match res {
                Ok(Ok(resp)) => Ok(resp),
                Ok(Err(_)) => Err(anyhow::anyhow!("agent dropped reply channel")),
                Err(_) => Err(anyhow::anyhow!("request to agent timed out")),
            }
        } else if let Some(responder) = ent.responder {
            // fallback: direct call
            let fut = (responder)(msg.payload);
            let res = fut.await;
            Ok(res)
        } else {
            Err(anyhow::anyhow!("agent '{agent_id}' has no mailbox or responder"))
        }
    }
}

/// Messages sent to agents by the router (public-facing payload wrapper).
#[derive(Debug, Clone)]
pub struct OrchestrationMessage {
    pub job_id: String,
    pub payload: String,
}

/// Descriptor for a coordination job.
#[derive(Debug, Clone)]
pub struct JobDescriptor {
    pub id: String,
    /// Required capabilities/tags for selecting agents.
    pub required_capabilities: Vec<String>,
    /// Arbitrary payload for agents.
    pub payload: String,
}

/// Coordinator which matches agents and aggregates their responses.
#[derive(Clone)]
pub struct Coordinator {
    registry: AgentRegistry,
    router: InProcessRouter,
}

impl Coordinator {
    pub fn new(registry: AgentRegistry) -> Self {
        let router = InProcessRouter::new(registry.clone());
        Self { registry, router }
    }

    /// Start a job synchronously: match agents, send the payload to each matched
    /// agent, and aggregate responses. Returns concatenated results.
    pub async fn start_job_sync(&self, desc: JobDescriptor) -> Result<String> {
        let matches = self.registry.match_agents(&desc.required_capabilities).await;
        if matches.is_empty() {
            anyhow::bail!("no agents match the required capabilities")
        }

        let mut handles = Vec::new();
        for agent in matches.iter() {
            let router = self.router.clone();
            let agent_id = agent.id.clone();
            let msg = OrchestrationMessage { job_id: desc.id.clone(), payload: desc.payload.clone() };
            let h = tokio::spawn(async move {
                match router.request_response(&agent_id, msg).await {
                    Ok(resp) => Ok((agent_id, resp)),
                    Err(e) => Err(e),
                }
            });
            handles.push(h);
        }

        // Collect responses
        let mut parts = Vec::new();
        for h in handles {
            match h.await? {
                Ok((agent_id, resp)) => {
                    parts.push(format!("--- agent: {} ---\n{}", agent_id, resp));
                }
                Err(e) => {
                    parts.push(format!("--- agent error: {} ---\n{}", e.to_string(), ""));
                }
            }
        }

        Ok(parts.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_orchestration_happy_path() {
        let registry = AgentRegistry::new();

        // responder that echoes payload
        let responder_a: Responder = Arc::new(|payload: String| {
            async move { format!("agent-a received: {}", payload) }.boxed()
        });
        let responder_b: Responder = Arc::new(|payload: String| {
            async move {
                // simulate some delay
                sleep(Duration::from_millis(10)).await;
                format!("agent-b processed: {}", payload)
            }
            .boxed()
        });

        registry.register("agent-a", vec!["search".to_string(), "analysis".to_string()], Some(responder_a)).await;
        registry.register("agent-b", vec!["search".to_string(), "compile".to_string()], Some(responder_b)).await;

        let coord = Coordinator::new(registry.clone());
        let desc = JobDescriptor { id: "job-1".to_string(), required_capabilities: vec!["search".to_string()], payload: "find TODOs".to_string() };

        let res = coord.start_job_sync(desc).await.unwrap();
        assert!(res.contains("agent-a received"));
        assert!(res.contains("agent-b processed"));
    }
}
