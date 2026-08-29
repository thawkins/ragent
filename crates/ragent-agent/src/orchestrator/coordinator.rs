use anyhow::Result;
use dashmap::DashMap;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::broadcast;
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
    pub status: String,
    pub result: Option<String>,
    pub events_tx: broadcast::Sender<JobEvent>,
    /// R-20: Handle to the spawned job task so it can be aborted on
    /// shutdown. Wrapped in `Option` so the entry can be constructed
    /// before the handle is available.
    handle: Option<tokio::task::JoinHandle<()>>,
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
    #[must_use]
    pub fn new() -> Self {
        Self {
            active_jobs: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            completed_jobs: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            timeouts: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            errors: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    /// Classify a router send failure into the matching metric counter.
    ///
    /// Send failures currently carry the timeout marker only inside their
    /// error message, so the classification is string-based; keep it in one
    /// place so a change of wording cannot silently reclassify timeouts.
    pub fn record_send_error(&self, err: &anyhow::Error) {
        let err_str = err.to_string();
        if err_str.contains("timed out") || err_str.contains("timeout") {
            self.timeouts
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        } else {
            self.errors
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

/// R-20: Drop guard that decrements `active_jobs` when the job task finishes
/// (including on panic or cancellation), preventing stuck "running" jobs.
struct ActiveJobsGuard {
    active_jobs: Arc<std::sync::atomic::AtomicU64>,
}

impl Drop for ActiveJobsGuard {
    fn drop(&mut self) {
        self.active_jobs
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
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
    jobs: Arc<DashMap<String, JobEntry>>,
    metrics: Arc<Metrics>,
    /// Optional conflict-resolution policy applied by `start_job_sync`.
    policy: Option<Arc<policy::ConflictResolver>>,
}

impl Coordinator {
    /// Return a snapshot of the internal metrics counters.
    #[must_use]
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
            .field("jobs_count", &self.jobs.len())
            .finish()
    }
}

impl Drop for Coordinator {
    // R-20: abort any still-running job tasks when the coordinator is
    // dropped so spawned work does not outlive its owner.
    fn drop(&mut self) {
        for entry in self.jobs.iter() {
            if entry.status == "running" {
                if let Some(handle) = &entry.handle {
                    handle.abort();
                }
            }
        }
    }
}

impl Coordinator {
    /// Default constructor using `InProcessRouter`.
    #[must_use]
    pub fn new(registry: AgentRegistry) -> Self {
        let router = Arc::new(super::router::InProcessRouter::new(registry.clone()));
        Self {
            registry,
            router,
            jobs: Arc::new(DashMap::new()),
            metrics: Arc::new(Metrics::new()),
            policy: None,
        }
    }

    /// Constructor that accepts a custom Router implementation.
    pub fn with_router(registry: AgentRegistry, router: Arc<dyn Router>) -> Self {
        Self {
            registry,
            router,
            jobs: Arc::new(DashMap::new()),
            metrics: Arc::new(Metrics::new()),
            policy: None,
        }
    }

    /// Constructor that sets a custom per-request timeout on the default `InProcessRouter`.
    #[must_use]
    pub fn with_request_timeout(registry: AgentRegistry, timeout: Duration) -> Self {
        let mut r = super::router::InProcessRouter::new(registry.clone());
        r.request_timeout = timeout;
        let router: Arc<dyn Router> = Arc::new(r);
        Self {
            registry,
            router,
            jobs: Arc::new(DashMap::new()),
            metrics: Arc::new(Metrics::new()),
            policy: None,
        }
    }

    /// Attach a [`policy::ConflictResolver`] to this coordinator.  When set,
    /// `start_job_sync` applies the policy to agent responses instead of
    /// concatenating them directly.
    #[must_use]
    pub fn with_policy(mut self, resolver: policy::ConflictResolver) -> Self {
        self.policy = Some(Arc::new(resolver));
        self
    }

    /// Start a job synchronously: match agents, send the payload to each matched
    /// agent, and aggregate responses. Returns concatenated results.
    pub async fn start_job_sync(&self, desc: JobDescriptor) -> Result<String> {
        let span = tracing::info_span!("start_job_sync", job_id = %desc.id);
        let _enter = span.enter();
        tracing::info!(job_id = %desc.id, "start_job_sync");
        // R-24: Guard ensures `active_jobs` is decremented on every exit path,
        // including the empty-match early-return below.
        let _active_guard = ActiveJobsGuard {
            active_jobs: Arc::clone(&self.metrics.active_jobs),
        };
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
        for agent in &matches {
            let router = self.router.clone();
            let agent_id = agent.id.clone();
            let msg = OrchestrationMessage {
                job_id: desc.id.clone(),
                payload: desc.payload.clone(),
            };
            let h = tokio::spawn(async move {
                router
                    .send(&agent_id, msg)
                    .await
                    .map(|resp| (agent_id, resp))
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
                    // Note: failures are also counted via `record_send_error`.
                    self.metrics.record_send_error(&e);
                    tracing::warn!(error = %e, "agent send error");
                }
            }
        }

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
            resolver.resolve(&desc.id, &responses).inspect_err(|_e| {
                self.metrics
                    .errors
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            })
        } else {
            let parts: Vec<String> = responses
                .into_iter()
                .map(|(id, resp)| format!("--- agent: {id} ---\n{resp}"))
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
        let span = tracing::info_span!("start_job_first_success", job_id = %desc.id);
        let _enter = span.enter();
        tracing::info!(job_id = %desc.id, "start_job_first_success");
        // R-24: Guard ensures `active_jobs` is decremented on every exit path.
        let _active_guard = ActiveJobsGuard {
            active_jobs: Arc::clone(&self.metrics.active_jobs),
        };
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

        for agent in &matches {
            let agent_id = agent.id.clone();
            let msg = OrchestrationMessage {
                job_id: desc.id.clone(),
                payload: desc.payload.clone(),
            };
            match self.router.send(&agent_id, msg).await {
                Ok(resp) => {
                    if !resp.trim_start().to_lowercase().starts_with("error:") {
                        self.metrics
                            .completed_jobs
                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        return Ok(format!("--- agent: {agent_id} ---\n{resp}"));
                    }
                    continue;
                }
                Err(e) => {
                    self.metrics.record_send_error(&e);
                    continue;
                }
            }
        }

        anyhow::bail!("no agent succeeded for job")
    }

    /// Start a job asynchronously: returns a job id. Events can be subscribed to
    /// via `subscribe_job_events`. The job runs in the background and updates
    /// its entry in the coordinator jobs map when complete.
    pub async fn start_job_async(&self, desc: JobDescriptor) -> Result<String> {
        let job_id = desc.id.clone();

        let (tx, _rx) = broadcast::channel::<JobEvent>(16);
        let entry = JobEntry {
            status: "running".to_string(),
            result: None,
            events_tx: tx.clone(),
            handle: None,
        };
        self.jobs.insert(job_id.clone(), entry);

        let registry = self.registry.clone();
        let router = self.router.clone();
        let jobs = self.jobs.clone();
        let desc_clone = desc.clone();
        let job_id_for_spawn = job_id.clone();
        let metrics = self.metrics.clone();
        metrics
            .active_jobs
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // R-20: Store the JoinHandle in the JobEntry so it can be aborted on
        // shutdown. A Drop guard ensures metrics are updated even on panic.
        let metrics_guard = ActiveJobsGuard {
            active_jobs: Arc::clone(&metrics.active_jobs),
        };
        let handle = tokio::spawn(async move {
            let _guard = metrics_guard;
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
                if let Some(mut j) = jobs.get_mut(&job_id_for_spawn) {
                    j.status = "failed".to_string();
                }
                metrics
                    .errors
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                return;
            }

            // assign subtasks in order; collect aggregated parts
            let mut parts = Vec::new();
            for agent in &matches {
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
                        parts.push(format!("--- agent: {agent_id} ---\n{resp}"));
                    }
                    Err(e) => {
                        let _ = tx.send(JobEvent::SubtaskCompleted {
                            job_id: job_id_for_spawn.clone(),
                            agent_id: agent_id.clone(),
                            success: false,
                        });
                        metrics.record_send_error(&e);
                    }
                }
            }

            let result = parts.join("\n");
            if let Some(mut j) = jobs.get_mut(&job_id_for_spawn) {
                j.status = "completed".to_string();
                j.result = Some(result.clone());
            }
            let _ = tx.send(JobEvent::JobCompleted {
                job_id: job_id_for_spawn.clone(),
                success: true,
            });
            metrics
                .completed_jobs
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        });

        // Store the handle so it can be aborted on shutdown.
        if let Some(mut j) = self.jobs.get_mut(&job_id) {
            j.handle = Some(handle);
        }

        Ok(job_id)
    }

    /// Subscribe to job events. Returns a broadcast receiver which will receive
    /// subsequent events. Returns Err if the job id is unknown.
    pub async fn subscribe_job_events(
        &self,
        job_id: &str,
    ) -> Result<broadcast::Receiver<JobEvent>> {
        let entry = self
            .jobs
            .get(job_id)
            .ok_or_else(|| anyhow::anyhow!("job not found"))?;
        Ok(entry.events_tx.subscribe())
    }

    /// Get job result/status if available.
    pub async fn get_job_result(&self, job_id: &str) -> Option<(String, Option<String>)> {
        self.jobs
            .get(job_id)
            .map(|j| (j.status.clone(), j.result.clone()))
    }
}
