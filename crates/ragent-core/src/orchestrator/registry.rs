use chrono::{DateTime, Utc};
use futures::future::BoxFuture;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc, oneshot};

/// Identifier for an agent.
pub type AgentId = String;

/// Responder callback type for in-process agents.
pub type Responder = Arc<dyn Fn(String) -> BoxFuture<'static, String> + Send + Sync>;

/// Internal request sent to an agent mailbox.
pub struct OrchestrationRequest {
    /// Job identifier.
    pub job_id: String,
    /// Request payload.
    pub payload: String,
    /// One-shot channel for reply.
    pub reply: oneshot::Sender<String>,
}

/// Agent metadata stored in the registry.
#[derive(Clone)]
pub struct AgentEntry {
    /// Unique agent identifier.
    pub id: AgentId,
    /// Tags/capabilities used for matching.
    pub capabilities: Vec<String>,
    /// Optional mailbox sender for actor-style message delivery.
    pub mailbox: Option<mpsc::Sender<OrchestrationRequest>>,
    /// Last seen heartbeat time (updated on register/heartbeat).
    pub last_heartbeat: Option<DateTime<Utc>>,
}

impl AgentEntry {
    /// Create a new agent entry.
    pub fn new(
        id: impl Into<String>,
        capabilities: Vec<String>,
        mailbox: Option<mpsc::Sender<OrchestrationRequest>>,
    ) -> Self {
        Self {
            id: id.into(),
            capabilities,
            mailbox,
            last_heartbeat: Some(Utc::now()),
        }
    }
}

/// Simple capability-based registry for agents.
///
/// The registry supports registering in-process agents with a mailbox, looking
/// up agents by id, and matching by required capabilities.
#[derive(Clone, Default)]
pub struct AgentRegistry {
    inner: Arc<RwLock<HashMap<AgentId, AgentEntry>>>,
}

impl AgentRegistry {
    /// Create a new empty agent registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register an agent with capabilities and an optional in-process responder.
    ///
    /// If a responder is provided the registry will create a mailbox (mpsc
    /// channel) and spawn a background task that pulls messages from the
    /// mailbox and invokes the responder, sending back responses via the
    /// one-shot reply channel.
    pub async fn register(
        &self,
        id: impl Into<String>,
        capabilities: Vec<String>,
        responder: Option<Responder>,
    ) {
        let id = id.into();

        let mut mailbox_opt = None;
        if let Some(responder) = responder {
            // create a channel for the agent mailbox
            let (tx, mut rx) = mpsc::channel::<OrchestrationRequest>(16);
            mailbox_opt = Some(tx);

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

        let entry = AgentEntry::new(id.clone(), capabilities, mailbox_opt);
        self.inner.write().await.insert(id, entry);
    }

    /// Unregister an agent by ID.
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

    /// Update heartbeat for an agent (mark it as alive now).
    pub async fn heartbeat(&self, id: &str) {
        let mut map = self.inner.write().await;
        if let Some(ent) = map.get_mut(id) {
            ent.last_heartbeat = Some(Utc::now());
        }
    }

    /// Remove agents whose last heartbeat is older than `stale_after`.
    pub async fn prune_stale(&self, stale_after: std::time::Duration) {
        let cutoff = Utc::now()
            - chrono::Duration::from_std(stale_after).unwrap_or(chrono::Duration::seconds(60));
        let mut map = self.inner.write().await;
        let keys: Vec<String> = map
            .iter()
            .filter_map(|(k, v)| {
                if let Some(last) = v.last_heartbeat {
                    if last < cutoff { Some(k.clone()) } else { None }
                } else {
                    Some(k.clone())
                }
            })
            .collect();
        for k in keys {
            map.remove(&k);
        }
    }

    /// Find agents whose capabilities include all of the required tags.
    /// Results are returned in registration/insertion order for determinism.
    pub async fn match_agents(&self, required: &[String]) -> Vec<AgentEntry> {
        let agents = self.inner.read().await;
        agents
            .values()
            .filter(|entry| {
                required
                    .iter()
                    .all(|req| entry.capabilities.iter().any(|c| c.contains(req)))
            })
            .cloned()
            .collect()
    }
}
