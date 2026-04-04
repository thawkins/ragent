use anyhow::Result;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use tokio::time::Duration;

use super::policy;
use super::{AgentRegistry, Router};

/// Messages sent to agents by the router (public-facing payload wrapper).
#[derive(Debug, Clone)]
pub struct OrchestrationMessage {
    /// Job identifier.
    pub job_id: String,
    /// Payload for the agent.
    pub payload: String,
}

/// Descriptor for a coordination job.
#[derive(Debug, Clone)]
pub struct JobDescriptor {
    /// Unique job identifier.
    pub id: String,
    /// Required capabilities/tags for selecting agents.
    pub required_capabilities: Vec<String>,
    /// Arbitrary payload for agents.
    pub payload: String,
}

/// Job lifecycle events emitted by the coordinator.
#[derive(Debug, Clone)]
pub enum JobEvent {
    /// Job started.
    JobStarted {
        /// Job identifier.
        job_id: String,
    },
    /// Subtask assigned to an agent.
    SubtaskAssigned {
        /// Job identifier.
        job_id: String,
        /// Agent identifier.
        agent_id: String,
    },
    /// Subtask completed by an agent.
    SubtaskCompleted {
        /// Job identifier.
        job_id: String,
        /// Agent identifier.
        agent_id: String,
        /// Whether the subtask succeeded.
        success: bool,
    },
    /// Job completed.
    JobCompleted {
        /// Job identifier.
        job_id: String,
        /// Whether the job succeeded.
        success: bool,
    },
    /// Job failed.
    JobFailed {
        /// Job identifier.
        job_id: String,
        /// Error message.
        error: String,
    },
}

/// Job entry stored in the coordinator job map.
struct JobEntry {
    _id: String,
    pub status: String,
    pub result: Option<String>,
    pub events_tx: broadcast::Sender<JobEvent>,
}

/// Simple metrics recorded by the coordinator for observability hooks.
#[derive(Clone)]
pub struct Metrics {
    /// Number of currently active jobs.
    pub active_jobs: Arc<std::sync::atomic::AtomicU64>,
    /// Number of completed jobs.
    pub completed_jobs: Arc<std::sync::atomic::AtomicU64>,
    /// Number of timeouts.
    pub timeouts: Arc<std::sync::atomic::AtomicU64>,
    /// Number of errors.
    pub errors: Arc<std::sync::atomic::AtomicU64>,
}

impl Metrics {
    /// Create a new metrics instance.
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
#[derive(Debug, Clone, Serialize)]
pub struct MetricsSnapshot {
    /// Number of currently active jobs.
    pub active_jobs: u64,
    /// Number of completed jobs.
    pub completed_jobs: u64,
    /// Number of timeouts.
    pub timeouts: u64,
    /// Number of errors.
    pub errors: u64,
}

/// Coordinator which matches agents and aggregates their responses.
#[derive(Clone)]
pub struct Coordinator {
    registry: AgentRegistry,
    router: Arc<dyn Router>,
    jobs: Arc<RwLock<HashMap<String, JobEntry>>>,
    metrics: Arc<Metrics>,
    /// Optional conflict-resolution policy applied by `start_job_sync`.
    policy: Option<Arc<policy::ConflictResolver>>,
}

impl Coordinator {
    /// Return a snapshot of the internal metrics counters.
    pub fn metrics_snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            active_jobs: self
                .metrics
                .active_jobs
                .load(std::sync::atomic::Ordering::Relaxed),
            completed_jobs: self
                .metrics
                .completed_jobs
                .load(std::sync::atomic::Ordering::Relaxed),
            timeouts: self
                .metrics
                .timeouts
                .load(std::sync::atomic::Ordering::Relaxed),
            errors: self
                .metrics
                .errors
                .load(std::sync::atomic::Ordering::Relaxed),
        }
    }
}

impl std::fmt::Debug for Coordinator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Coordinator")
            .field("jobs_count", &self.jobs.blocking_read().len())
            .finish()
    }
}

impl Coordinator {
    /// Default constructor using InProcessRouter.
    pub fn new(registry: AgentRegistry) -> Self {
        let router = Arc::new(super::router::InProcessRouter::new(registry.clone()));
        Self {
            registry,
            router,
            jobs: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(Metrics::new()),
            policy: None,
        }
    }

    /// Constructor that accepts a custom Router implementation.
    pub fn with_router(registry: AgentRegistry, router: Arc<dyn Router>) -> Self {
        Self {
            registry,
            router,
            jobs: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(Metrics::new()),
            policy: None,
        }
    }

    /// Constructor that sets a custom per-request timeout on the default InProcessRouter.
    pub fn with_request_timeout(registry: AgentRegistry, timeout: Duration) -> Self {
        let mut r = super::router::InProcessRouter::new(registry.clone());
        r.request_timeout = timeout;
        let router: Arc<dyn Router> = Arc::new(r);
        Self {
            registry,
            router,
            jobs: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(Metrics::new()),
            policy: None,
        }
    }

    /// Attach a [`policy::ConflictResolver`] to this coordinator.  When set,
    /// `start_job_sync` applies the policy to agent responses instead of
    /// concatenating them directly.
    pub fn with_policy(mut self, resolver: policy::ConflictResolver) -> Self {
        self.policy = Some(Arc::new(resolver));
        self
    }

    /// Start a job synchronously: match agents, send the payload to each matched
    /// agent, and aggregate responses. Returns concatenated results.
    pub async fn start_job_sync(&self, desc: JobDescriptor) -> Result<String> {
        tracing::info!(job_id = %desc.id, "start_job_sync");
        self.metrics
            .active_jobs
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let matches = self
            .registry
            .match_agents(&desc.required_capabilities)
            .await;
        if matches.is_empty() {
            self.metrics
                .errors
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            anyhow::bail!("no agents match the required capabilities")
        }

        let mut handles = Vec::new();
        for agent in matches.iter() {
            let router = self.router.clone();
            let agent_id = agent.id.clone();
            let msg = OrchestrationMessage {
                job_id: desc.id.clone(),
                payload: desc.payload.clone(),
            };
            let h = tokio::spawn(async move {
                match router.send(&agent_id, msg).await {
                    Ok(resp) => Ok((agent_id, resp)),
                    Err(e) => Err(e),
                }
            });
            handles.push(h);
        }

        // Collect responses
        let mut responses: Vec<(String, String)> = Vec::new();
        for h in handles {
            match h.await? {
                Ok((agent_id, resp)) => {
                    responses.push((agent_id, resp));
                }
                Err(e) => {
                    let err_str = e.to_string();
                    if err_str.contains("timed out") || err_str.contains("timeout") {
                        self.metrics
                            .timeouts
                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    } else {
                        // Defer counting errors until after collecting responses to
                        // avoid double-counting when all agents fail.
                        tracing::warn!(error = %err_str, "agent send error");
                    }
                }
            }
        }

        self.metrics
            .active_jobs
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        self.metrics
            .completed_jobs
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // If no agent produced a response (e.g., mailbox errors, timeouts), return an error.
        if responses.is_empty() {
            self.metrics
                .errors
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            anyhow::bail!("no successful responses from agents")
        }

        // Apply conflict-resolution policy if configured; otherwise concatenate.
        if let Some(resolver) = &self.policy {
            resolver.resolve(&desc.id, &responses).map_err(|e| {
                self.metrics
                    .errors
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                e
            })
        } else {
            let parts: Vec<String> = responses
                .into_iter()
                .map(|(id, resp)| format!("--- agent: {} ---\n{}", id, resp))
                .collect();
            Ok(parts.join("\n"))
        }
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
        self.metrics
            .active_jobs
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let matches = self
            .registry
            .match_agents(&desc.required_capabilities)
            .await;
        if matches.is_empty() {
            self.metrics
                .errors
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            anyhow::bail!("no agents match the required capabilities")
        }

        for agent in matches.iter() {
            let agent_id = agent.id.clone();
            let msg = OrchestrationMessage {
                job_id: desc.id.clone(),
                payload: desc.payload.clone(),
            };
            match self.router.send(&agent_id, msg).await {
                Ok(resp) => {
                    if !resp.trim_start().to_lowercase().starts_with("error:") {
                        self.metrics
                            .active_jobs
                            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                        self.metrics
                            .completed_jobs
                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        return Ok(format!("--- agent: {} ---\n{}", agent_id, resp));
                    }
                    continue;
                }
                Err(e) => {
                    let err_str = e.to_string();
                    if err_str.contains("timed out") || err_str.contains("timeout") {
                        self.metrics
                            .timeouts
                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    } else {
                        self.metrics
                            .errors
                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                    continue;
                }
            }
        }

        self.metrics
            .active_jobs
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        anyhow::bail!("no agent succeeded for job")
    }

    /// Start a job asynchronously: returns a job id. Events can be subscribed to
    /// via `subscribe_job_events`. The job runs in the background and updates
    /// its entry in the coordinator jobs map when complete.
    pub async fn start_job_async(&self, desc: JobDescriptor) -> Result<String> {
        let job_id = desc.id.clone();

        let (tx, _rx) = broadcast::channel::<JobEvent>(16);
        let entry = JobEntry {
            _id: job_id.clone(),
            status: "running".to_string(),
            result: None,
            events_tx: tx.clone(),
        };
        self.jobs.write().await.insert(job_id.clone(), entry);

        let registry = self.registry.clone();
        let router = self.router.clone();
        let jobs = self.jobs.clone();
        let desc_clone = desc.clone();
        let job_id_for_spawn = job_id.clone();
        let metrics = self.metrics.clone();
        metrics
            .active_jobs
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        tokio::spawn(async move {
            // publish JobStarted
            let _ = tx.send(JobEvent::JobStarted {
                job_id: job_id_for_spawn.clone(),
            });

            // match agents
            let matches = registry
                .match_agents(&desc_clone.required_capabilities)
                .await;
            if matches.is_empty() {
                let _ = tx.send(JobEvent::JobFailed {
                    job_id: job_id_for_spawn.clone(),
                    error: "no agents match".to_string(),
                });
                if let Some(j) = jobs.write().await.get_mut(&job_id_for_spawn) {
                    j.status = "failed".to_string();
                }
                metrics
                    .errors
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                metrics
                    .active_jobs
                    .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                return;
            }

            // assign subtasks in order; collect aggregated parts
            let mut parts = Vec::new();
            for agent in matches.iter() {
                let agent_id = agent.id.clone();
                let _ = tx.send(JobEvent::SubtaskAssigned {
                    job_id: job_id_for_spawn.clone(),
                    agent_id: agent_id.clone(),
                });
                let msg = OrchestrationMessage {
                    job_id: job_id_for_spawn.clone(),
                    payload: desc_clone.payload.clone(),
                };
                match router.send(&agent_id, msg).await {
                    Ok(resp) => {
                        let _ = tx.send(JobEvent::SubtaskCompleted {
                            job_id: job_id_for_spawn.clone(),
                            agent_id: agent_id.clone(),
                            success: true,
                        });
                        parts.push(format!("--- agent: {} ---\n{}", agent_id, resp));
                    }
                    Err(e) => {
                        let _ = tx.send(JobEvent::SubtaskCompleted {
                            job_id: job_id_for_spawn.clone(),
                            agent_id: agent_id.clone(),
                            success: false,
                        });
                        // record timeout vs error
                        let es = e.to_string();
                        if es.contains("timed out") || es.contains("timeout") {
                            metrics
                                .timeouts
                                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        } else {
                            metrics
                                .errors
                                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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
            let _ = tx.send(JobEvent::JobCompleted {
                job_id: job_id_for_spawn.clone(),
                success: true,
            });
            metrics
                .active_jobs
                .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            metrics
                .completed_jobs
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        });

        Ok(job_id)
    }

    /// Subscribe to job events. Returns a broadcast receiver which will receive
    /// subsequent events. Returns Err if the job id is unknown.
    pub async fn subscribe_job_events(
        &self,
        job_id: &str,
    ) -> Result<broadcast::Receiver<JobEvent>> {
        let jobs = self.jobs.read().await;
        let entry = jobs
            .get(job_id)
            .ok_or_else(|| anyhow::anyhow!("job not found"))?;
        Ok(entry.events_tx.subscribe())
    }

    /// Get job result/status if available.
    pub async fn get_job_result(&self, job_id: &str) -> Option<(String, Option<String>)> {
        self.jobs
            .read()
            .await
            .get(job_id)
            .map(|j| (j.status.clone(), j.result.clone()))
    }
}
