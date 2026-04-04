use anyhow::Result;
use tokio::sync::oneshot;
use tokio::time::{Duration, timeout};

use super::coordinator::OrchestrationMessage;
use super::registry::{AgentRegistry, OrchestrationRequest};

/// Router trait abstracts request delivery to agents.
#[async_trait::async_trait]
pub trait Router: Send + Sync + 'static {
    /// Send a message to an agent and await a response.
    async fn send(&self, agent_id: &str, msg: OrchestrationMessage) -> Result<String>;
}

/// Router implementation for in-process agents using mailboxes.
#[derive(Clone)]
pub struct InProcessRouter {
    registry: AgentRegistry,
    /// Default per-request timeout applied by the router when awaiting a reply.
    pub request_timeout: Duration,
}

impl InProcessRouter {
    /// Create a new in-process router with the given registry.
    #[must_use]
    pub fn new(registry: AgentRegistry) -> Self {
        Self {
            registry,
            request_timeout: Duration::from_secs(5),
        }
    }
}

#[async_trait::async_trait]
impl Router for InProcessRouter {
    async fn send(&self, agent_id: &str, msg: OrchestrationMessage) -> Result<String> {
        let ent = self.registry.get(agent_id).await;
        let ent = ent.ok_or_else(|| anyhow::anyhow!("agent '{agent_id}' not found"))?;

        if let Some(tx) = ent.mailbox {
            let (reply_tx, reply_rx) = oneshot::channel::<String>();
            let req = OrchestrationRequest {
                job_id: msg.job_id,
                payload: msg.payload,
                reply: reply_tx,
            };
            tx.send(req)
                .await
                .map_err(|_| anyhow::anyhow!("failed to send to agent mailbox"))?;
            let res = timeout(self.request_timeout, reply_rx).await;
            match res {
                Ok(Ok(resp)) => Ok(resp),
                Ok(Err(_)) => Err(anyhow::anyhow!("agent dropped reply channel")),
                Err(_) => Err(anyhow::anyhow!("request to agent timed out")),
            }
        } else {
            Err(anyhow::anyhow!("agent '{agent_id}' has no mailbox"))
        }
    }
}
