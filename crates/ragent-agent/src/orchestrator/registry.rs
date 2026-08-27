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
pub struct AgentEntry {
    /// Unique agent identifier.
    pub id: AgentId,
    /// Tags/capabilities used for matching.
    pub capabilities: Vec<String>,
    /// Optional mailbox sender for actor-style message delivery.
    pub mailbox: Option<mpsc::Sender<OrchestrationRequest>>,
    /// Last seen heartbeat time (updated on register/heartbeat).
    pub last_heartbeat: Option<DateTime<Utc>>,
    /// R-14: Handle to the mailbox-loop task. Aborted on `unregister` so
    /// re-registration does not accumulate orphaned loops.
    pub mailbox_handle: Option<tokio::task::JoinHandle<()>>,
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
            mailbox_handle: None,
        }
    }

    /// Return a copy of this entry with the (non-`Clone`) mailbox
    /// `JoinHandle` cleared. Used when handing entries out to callers.
    fn view_without_handle(&self) -> Self {
        Self {
            id: self.id.clone(),
            capabilities: self.capabilities.clone(),
            mailbox: self.mailbox.clone(),
            last_heartbeat: self.last_heartbeat,
            mailbox_handle: None,
        }
    }
}

/// Maximum pending orchestration requests per agent mailbox.
const MAILBOX_BUFFER_SIZE: usize = 100;

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
        let mut mailbox_handle = None;
        if let Some(responder) = responder {
            // create a channel for the agent mailbox
            let (tx, mut rx) = mpsc::channel::<OrchestrationRequest>(MAILBOX_BUFFER_SIZE);
            mailbox_opt = Some(tx);

            // Spawn the agent loop which turns mailbox messages into responder calls.
            // R-14: store the JoinHandle so `unregister` can abort it.
            mailbox_handle = Some(tokio::spawn(async move {
                while let Some(req) = rx.recv().await {
                    let fut = (responder)(req.payload);
                    let resp = fut.await;
                    // best-effort: ignore send error
                    let _ = req.reply.send(resp);
                }
            }));
        }

        let mut entry = AgentEntry::new(id.clone(), capabilities, mailbox_opt);
        entry.mailbox_handle = mailbox_handle;
        self.inner.write().await.insert(id, entry);
    }

    /// Unregister an agent by ID.
    pub async fn unregister(&self, id: &str) {
        // R-14: abort the mailbox-loop task so re-registration does not
        // accumulate orphaned infinite loops.
        let value = self.inner.write().await.remove(id);
        if let Some(entry) = value {
            if let Some(handle) = entry.mailbox_handle {
                handle.abort();
            }
        }
    }

    /// List all agents.
    ///
    /// Returns entries without the mailbox `JoinHandle` (which is not
    /// `Clone`); callers that need the handle should use `get`.
    pub async fn list(&self) -> Vec<AgentEntry> {
        self.inner
            .read()
            .await
            .values()
            .map(AgentEntry::view_without_handle)
            .collect()
    }

    /// Get a specific agent by id.
    pub async fn get(&self, id: &str) -> Option<AgentEntry> {
        self.inner
            .read()
            .await
            .get(id)
            .map(AgentEntry::view_without_handle)
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
            // Abort the mailbox loop exactly as `unregister` does; a pruned
            // agent whose mailbox sender is still cloned elsewhere must not
            // leave an orphaned loop behind (see R-14).
            if let Some(entry) = map.remove(&k) {
                if let Some(handle) = entry.mailbox_handle {
                    handle.abort();
                }
            }
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
            .map(AgentEntry::view_without_handle)
            .collect()
    }
}
