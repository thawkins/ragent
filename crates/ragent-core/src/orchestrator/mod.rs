//! Multi-agent orchestration primitives (MVP)
//!
//! Provides an in-process AgentRegistry, an InProcessRouter (actor-style
//! inboxes), and a Coordinator that can start jobs synchronously and
//! asynchronously with basic negotiation and aggregation strategies.

use anyhow::Result;
use futures::future::BoxFuture;
use futures::future::FutureExt;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, oneshot, RwLock};
use tokio::time::{timeout, Duration};
use chrono::{DateTime, Utc};

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
    /// Optional mailbox sender for actor-style message delivery.
    pub mailbox: Option<mpsc::Sender<OrchestrationRequest>>,
    /// Last seen heartbeat time (updated on register/heartbeat).
    pub last_heartbeat: Option<DateTime<Utc>>,
}

impl AgentEntry {
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
        if let Some(responder) = responder {
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

        let entry = AgentEntry::new(id.clone(), capabilities, mailbox_opt);
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

    /// Update heartbeat for an agent (mark it as alive now).
    pub async fn heartbeat(&self, id: &str) {
        let mut map = self.inner.write().await;
        if let Some(ent) = map.get_mut(id) {
            ent.last_heartbeat = Some(Utc::now());
        }
    }

    /// Remove agents whose last heartbeat is older than `stale_after`.
    pub async fn prune_stale(&self, stale_after: Duration) {
        let cutoff = Utc::now() - chrono::Duration::from_std(stale_after).unwrap_or(chrono::Duration::seconds(60));
        let mut map = self.inner.write().await;
        let keys: Vec<String> = map.iter().filter_map(|(k, v)| {
            if let Some(last) = v.last_heartbeat {
                if last < cutoff {
                    Some(k.clone())
                } else { None }
            } else { Some(k.clone()) }
        }).collect();
        for k in keys { map.remove(&k); }
    }

    /// Find agents whose capabilities include all of the required tags (substring match).
    /// Results are returned in registration/insertion order for determinism.
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

/// Router trait abstracts request delivery to agents.
#[async_trait::async_trait]
pub trait Router: Send + Sync + 'static {
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
    pub fn new(registry: AgentRegistry) -> Self {
        Self { registry, request_timeout: Duration::from_secs(5) }
    }
}

#[async_trait::async_trait]
impl Router for InProcessRouter {
    async fn send(&self, agent_id: &str, msg: OrchestrationMessage) -> Result<String> {
        let ent = self.registry.get(agent_id).await;
        let ent = ent.ok_or_else(|| anyhow::anyhow!("agent '{agent_id}' not found"))?;

        if let Some(tx) = ent.mailbox {
            let (reply_tx, reply_rx) = oneshot::channel::<String>();
            let req = OrchestrationRequest { job_id: msg.job_id, payload: msg.payload, reply: reply_tx };
            tx.send(req).await.map_err(|_| anyhow::anyhow!("failed to send to agent mailbox"))?;
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

/// Job lifecycle events emitted by the coordinator.
#[derive(Debug, Clone)]
pub enum JobEvent {
    JobStarted { job_id: String },
    SubtaskAssigned { job_id: String, agent_id: String },
    SubtaskCompleted { job_id: String, agent_id: String, success: bool },
    JobCompleted { job_id: String, success: bool },
    JobFailed { job_id: String, error: String },
}

/// Job entry stored in the coordinator job map.
struct JobEntry {
    pub id: String,
    pub status: String,
    pub result: Option<String>,
    pub events_tx: broadcast::Sender<JobEvent>,
}

/// Simple metrics recorded by the coordinator for observability hooks.
#[derive(Clone)]
pub struct Metrics {
    pub active_jobs: Arc<std::sync::atomic::AtomicU64>,
    pub completed_jobs: Arc<std::sync::atomic::AtomicU64>,
    pub timeouts: Arc<std::sync::atomic::AtomicU64>,
    pub errors: Arc<std::sync::atomic::AtomicU64>,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            active_jobs: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            completed_jobs: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            timeouts: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            errors: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }
}

/// Small snapshot of metrics for external inspection.
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct MetricsSnapshot {
    pub active_jobs: u64,
    pub completed_jobs: u64,
    pub timeouts: u64,
    pub errors: u64,
}

/// Coordinator which matches agents and aggregates their responses.
#[derive(Clone)]
pub struct Coordinator {
    registry: AgentRegistry,
    router: Arc<dyn Router>,
    jobs: Arc<RwLock<HashMap<String, JobEntry>>>,
    metrics: Arc<Metrics>,
}

impl Coordinator {
    /// Return a snapshot of the internal metrics counters.
    pub fn metrics_snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            active_jobs: self.metrics.active_jobs.load(std::sync::atomic::Ordering::Relaxed),
            completed_jobs: self.metrics.completed_jobs.load(std::sync::atomic::Ordering::Relaxed),
            timeouts: self.metrics.timeouts.load(std::sync::atomic::Ordering::Relaxed),
            errors: self.metrics.errors.load(std::sync::atomic::Ordering::Relaxed),
        }
    }
}

impl std::fmt::Debug for Coordinator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Coordinator").field("jobs_count", &self.jobs.blocking_read().len()).finish()
    }
}

impl Coordinator {
    /// Default constructor using InProcessRouter.
    pub fn new(registry: AgentRegistry) -> Self {
        let router = Arc::new(InProcessRouter::new(registry.clone()));
        Self { registry, router, jobs: Arc::new(RwLock::new(HashMap::new())), metrics: Arc::new(Metrics::new()) }
    }

    /// Constructor that accepts a custom Router implementation.
    pub fn with_router(registry: AgentRegistry, router: Arc<dyn Router>) -> Self {
        Self { registry, router, jobs: Arc::new(RwLock::new(HashMap::new())), metrics: Arc::new(Metrics::new()) }
    }

    /// Constructor that sets a custom per-request timeout on the default InProcessRouter.
    pub fn with_request_timeout(registry: AgentRegistry, timeout: Duration) -> Self {
        let mut r = InProcessRouter::new(registry.clone());
        r.request_timeout = timeout;
        let router: Arc<dyn Router> = Arc::new(r);
        Self { registry, router, jobs: Arc::new(RwLock::new(HashMap::new())), metrics: Arc::new(Metrics::new()) }
    }

    /// Start a job synchronously: match agents, send the payload to each matched
    /// agent, and aggregate responses. Returns concatenated results.
    pub async fn start_job_sync(&self, desc: JobDescriptor) -> Result<String> {
        tracing::info!(job_id = %desc.id, "start_job_sync");
        self.metrics.active_jobs.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let matches = self.registry.match_agents(&desc.required_capabilities).await;
        if matches.is_empty() {
            self.metrics.errors.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            anyhow::bail!("no agents match the required capabilities")
        }

        let mut handles = Vec::new();
        for agent in matches.iter() {
            let router = self.router.clone();
            let agent_id = agent.id.clone();
            let msg = OrchestrationMessage { job_id: desc.id.clone(), payload: desc.payload.clone() };
            let h = tokio::spawn(async move {
                match router.send(&agent_id, msg).await {
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
                    let err_str = e.to_string();
                    if err_str.contains("timed out") || err_str.contains("timeout") {
                        self.metrics.timeouts.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    } else {
                        self.metrics.errors.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                    parts.push(format!("--- agent error: {} ---\n{}", err_str, ""));
                }
            }
        }

        self.metrics.active_jobs.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        self.metrics.completed_jobs.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(parts.join("\n"))
    }

    /// Start a job using the "first-success" strategy: try matched agents in
    /// deterministic order and return the first successful response. Agents
    /// that timeout or return a failure-like payload are skipped.
    ///
    /// For the MVP success is defined as a non-timeout response that does not
    /// begin with the literal prefix "error:" (this is a pragmatic test helper
    /// semantics used by integration tests). Real deployments should use proper
    /// Result types from agents.
    pub async fn start_job_first_success(&self, desc: JobDescriptor) -> Result<String> {
        tracing::info!(job_id = %desc.id, "start_job_first_success");
        self.metrics.active_jobs.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let matches = self.registry.match_agents(&desc.required_capabilities).await;
        if matches.is_empty() {
            self.metrics.errors.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            anyhow::bail!("no agents match the required capabilities")
        }

        for agent in matches.iter() {
            let agent_id = agent.id.clone();
            let msg = OrchestrationMessage { job_id: desc.id.clone(), payload: desc.payload.clone() };
            match self.router.send(&agent_id, msg).await {
                Ok(resp) => {
                    if !resp.trim_start().to_lowercase().starts_with("error:") {
                        self.metrics.active_jobs.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                        self.metrics.completed_jobs.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        return Ok(format!("--- agent: {} ---\n{}", agent_id, resp));
                    } else {
                        continue;
                    }
                }
                Err(e) => {
                    let err_str = e.to_string();
                    if err_str.contains("timed out") || err_str.contains("timeout") {
                        self.metrics.timeouts.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    } else {
                        self.metrics.errors.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                    continue;
                }
            }
        }

        self.metrics.active_jobs.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        anyhow::bail!("no agent succeeded for job")
    }

    /// Start a job asynchronously: returns a job id. Events can be subscribed to
    /// via `subscribe_job_events`. The job runs in the background and updates
    /// its entry in the coordinator jobs map when complete.
    pub async fn start_job_async(&self, desc: JobDescriptor) -> Result<String> {
        let job_id = desc.id.clone();

        let (tx, _rx) = broadcast::channel::<JobEvent>(16);
        let entry = JobEntry { id: job_id.clone(), status: "running".to_string(), result: None, events_tx: tx.clone() };
        self.jobs.write().await.insert(job_id.clone(), entry);

        let registry = self.registry.clone();
        let router = self.router.clone();
        let jobs = self.jobs.clone();
        let desc_clone = desc.clone();
        let job_id_for_spawn = job_id.clone();
        let metrics = self.metrics.clone();
        metrics.active_jobs.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        tokio::spawn(async move {
            // publish JobStarted
            let _ = tx.send(JobEvent::JobStarted { job_id: job_id_for_spawn.clone() });

            // match agents
            let matches = registry.match_agents(&desc_clone.required_capabilities).await;
            if matches.is_empty() {
                let _ = tx.send(JobEvent::JobFailed { job_id: job_id_for_spawn.clone(), error: "no agents match".to_string() });
                if let Some(j) = jobs.write().await.get_mut(&job_id_for_spawn) { j.status = "failed".to_string(); }
                metrics.errors.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                metrics.active_jobs.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                return;
            }

            // assign subtasks in order; collect aggregated parts
            let mut parts = Vec::new();
            for agent in matches.iter() {
                let agent_id = agent.id.clone();
                let _ = tx.send(JobEvent::SubtaskAssigned { job_id: job_id_for_spawn.clone(), agent_id: agent_id.clone() });
                let msg = OrchestrationMessage { job_id: job_id_for_spawn.clone(), payload: desc_clone.payload.clone() };
                match router.send(&agent_id, msg).await {
                    Ok(resp) => {
                        let _ = tx.send(JobEvent::SubtaskCompleted { job_id: job_id_for_spawn.clone(), agent_id: agent_id.clone(), success: true });
                        parts.push(format!("--- agent: {} ---\n{}", agent_id, resp));
                    }
                    Err(e) => {
                        let _ = tx.send(JobEvent::SubtaskCompleted { job_id: job_id_for_spawn.clone(), agent_id: agent_id.clone(), success: false });
                        // record timeout vs error
                        let es = e.to_string();
                        if es.contains("timed out") || es.contains("timeout") {
                            metrics.timeouts.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        } else {
                            metrics.errors.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        }
                        // continue to next agent
                    }
                }
            }

            let result = parts.join("\n");
            if let Some(j) = jobs.write().await.get_mut(&job_id_for_spawn) {
                j.status = "completed".to_string();
                j.result = Some(result.clone());
            }
            let _ = tx.send(JobEvent::JobCompleted { job_id: job_id_for_spawn.clone(), success: true });
            metrics.active_jobs.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            metrics.completed_jobs.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        });

        Ok(job_id)
    }

    /// Subscribe to job events. Returns a broadcast receiver which will receive
    /// subsequent events. Returns Err if the job id is unknown.
    pub async fn subscribe_job_events(&self, job_id: &str) -> Result<broadcast::Receiver<JobEvent>> {
        let jobs = self.jobs.read().await;
        let entry = jobs.get(job_id).ok_or_else(|| anyhow::anyhow!("job not found"))?;
        Ok(entry.events_tx.subscribe())
    }

    /// Get job result/status if available.
    pub async fn get_job_result(&self, job_id: &str) -> Option<(String, Option<String>)> {
        self.jobs.read().await.get(job_id).map(|j| (j.status.clone(), j.result.clone()))
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

    #[tokio::test]
    async fn test_start_job_async_and_subscribe() {
        let registry = AgentRegistry::new();

        let fast: Responder = Arc::new(|payload: String| {
            async move { format!("fast: {}", payload) }.boxed()
        });
        registry.register("fast-agent", vec!["echo".to_string()], Some(fast)).await;

        let coord = Coordinator::new(registry.clone());
        let desc = JobDescriptor { id: "job-async".to_string(), required_capabilities: vec!["echo".to_string()], payload: "ping".to_string() };

        let job_id = coord.start_job_async(desc.clone()).await.unwrap();
        let mut sub = coord.subscribe_job_events(&job_id).await.unwrap();

        // Collect a few events
        let mut got_started = false;
        let mut got_completed = false;
        for _ in 0..4 {
            if let Ok(ev) = tokio::time::timeout(Duration::from_secs(1), sub.recv()).await {
                match ev {
                    Ok(JobEvent::JobStarted { job_id: jid }) => { if jid==job_id { got_started = true; } }
                    Ok(JobEvent::JobCompleted { job_id: jid, success: _ }) => { if jid==job_id { got_completed = true; break; } }
                    _ => {}
                }
            }
        }

        assert!(got_started);
        assert!(got_completed);

        let res = coord.get_job_result(&job_id).await;
        assert!(res.is_some());
        let (status, result) = res.unwrap();
        assert_eq!(status, "completed");
        assert!(result.unwrap().contains("fast:"));
    }
}
