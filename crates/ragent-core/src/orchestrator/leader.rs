//! In-process leader election and CoordinatorCluster (Task 5.2).
//!
//! Provides:
//! - [`LeaderEvent`] — broadcast events when leadership changes.
//! - [`LeaderElector`] — elects a leader among registered node ids using a
//!   simple in-process majority-vote mechanism.
//! - [`CoordinatorCluster`] — manages multiple [`Coordinator`]s and delegates
//!   job execution to the currently elected leader, falling back to any live
//!   coordinator if the leader is unavailable.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::{broadcast, RwLock};

use super::{Coordinator, JobDescriptor};

// ── Leader events ────────────────────────────────────────────────────────────

/// Events emitted by [`LeaderElector`] when leadership changes.
#[derive(Debug, Clone)]
pub enum LeaderEvent {
    /// A new leader has been elected.
    LeaderElected {
        /// Id of the newly elected leader.
        leader_id: String,
    },
    /// The current leader stepped down (e.g. unregistered or explicitly abdicated).
    LeaderStepped {
        /// Id of the coordinator that stepped down.
        former_id: String,
    },
}

// ── LeaderElector ────────────────────────────────────────────────────────────

/// In-process leader elector using a simple vote-based approach.
///
/// Each participant calls [`LeaderElector::nominate`] to cast a vote for a
/// candidate (typically itself).  The candidate with the most votes wins; ties
/// are broken by lexicographic order of the candidate id (deterministic).
///
/// A [`broadcast::Receiver<LeaderEvent>`] can be obtained via
/// [`LeaderElector::subscribe`] to monitor leadership changes.
#[derive(Clone)]
pub struct LeaderElector {
    votes: Arc<RwLock<HashMap<String, String>>>,  // voter_id → candidate_id
    leader: Arc<RwLock<Option<String>>>,
    tx: Arc<broadcast::Sender<LeaderEvent>>,
}

impl LeaderElector {
    /// Create a new elector with an internal broadcast channel of the given
    /// capacity.
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self {
            votes: Arc::new(RwLock::new(HashMap::new())),
            leader: Arc::new(RwLock::new(None)),
            tx: Arc::new(tx),
        }
    }

    /// Cast a vote from `voter_id` for `candidate_id`, then recount votes.
    /// Returns the id of the newly elected leader (which may be unchanged).
    pub async fn nominate(&self, voter_id: &str, candidate_id: &str) -> String {
        self.votes.write().await.insert(voter_id.to_string(), candidate_id.to_string());
        self.recount().await
    }

    /// Withdraw a voter's vote (e.g. when a node shuts down) and recount.
    /// Returns the new leader, or `None` if no votes remain.
    pub async fn withdraw(&self, voter_id: &str) -> Option<String> {
        self.votes.write().await.remove(voter_id);
        if self.votes.read().await.is_empty() {
            let mut l = self.leader.write().await;
            if let Some(former) = l.take() {
                let _ = self.tx.send(LeaderEvent::LeaderStepped { former_id: former });
            }
            return None;
        }
        Some(self.recount().await)
    }

    /// Return the current leader id, or `None` if no votes have been cast.
    pub async fn current_leader(&self) -> Option<String> {
        self.leader.read().await.clone()
    }

    /// Return `true` if `node_id` is currently the elected leader.
    pub async fn is_leader(&self, node_id: &str) -> bool {
        self.leader.read().await.as_deref() == Some(node_id)
    }

    /// Subscribe to leadership-change events.
    pub fn subscribe(&self) -> broadcast::Receiver<LeaderEvent> {
        self.tx.subscribe()
    }

    // Tally votes; the candidate with the most votes wins.  Ties broken by
    // lexicographic order (lowest id wins) for determinism.
    async fn recount(&self) -> String {
        let votes = self.votes.read().await;
        let mut tally: HashMap<&str, usize> = HashMap::new();
        for candidate in votes.values() {
            *tally.entry(candidate.as_str()).or_insert(0) += 1;
        }

        let winner = tally
            .iter()
            .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
            .map(|(id, _)| id.to_string())
            .unwrap_or_default();

        let mut current = self.leader.write().await;
        if current.as_deref() != Some(winner.as_str()) {
            *current = Some(winner.clone());
            let _ = self.tx.send(LeaderEvent::LeaderElected { leader_id: winner.clone() });
        }
        winner
    }
}

// ── CoordinatorCluster ───────────────────────────────────────────────────────

/// A cluster of named [`Coordinator`]s with leader-based job routing.
///
/// Jobs are routed to the elected leader coordinator.  If no leader has been
/// elected yet the first registered coordinator is used as a fallback.
///
/// ```rust,ignore
/// let cluster = CoordinatorCluster::new(LeaderElector::new(16));
/// cluster.add("node-1", coord_a).await;
/// cluster.add("node-2", coord_b).await;
/// cluster.elect("node-1").await;   // node-1 votes for itself
///
/// let result = cluster.start_job_sync(desc).await?;
/// ```
#[derive(Clone)]
pub struct CoordinatorCluster {
    nodes: Arc<RwLock<HashMap<String, Coordinator>>>,
    elector: LeaderElector,
}

impl CoordinatorCluster {
    /// Create a new cluster backed by the given [`LeaderElector`].
    pub fn new(elector: LeaderElector) -> Self {
        Self {
            nodes: Arc::new(RwLock::new(HashMap::new())),
            elector,
        }
    }

    /// Register a coordinator under `node_id`.
    pub async fn add(&self, node_id: impl Into<String>, coord: Coordinator) {
        self.nodes.write().await.insert(node_id.into(), coord);
    }

    /// Remove a coordinator from the cluster.  If it was the leader, withdraws
    /// its vote and triggers a new election.
    pub async fn remove(&self, node_id: &str) {
        self.nodes.write().await.remove(node_id);
        self.elector.withdraw(node_id).await;
    }

    /// Cast a self-nomination vote for `node_id` (i.e. it votes for itself).
    pub async fn elect(&self, node_id: &str) -> String {
        self.elector.nominate(node_id, node_id).await
    }

    /// Return the current leader id.
    pub async fn current_leader(&self) -> Option<String> {
        self.elector.current_leader().await
    }

    /// Return the coordinator for the elected leader, or the first registered
    /// coordinator if no leader has been elected.
    pub async fn leader_coordinator(&self) -> Option<Coordinator> {
        let nodes = self.nodes.read().await;
        if nodes.is_empty() {
            return None;
        }
        // Try elected leader first.
        if let Some(id) = self.elector.current_leader().await {
            if let Some(c) = nodes.get(&id) {
                return Some(c.clone());
            }
        }
        // Fallback: any registered coordinator.
        nodes.values().next().cloned()
    }

    /// Start a synchronous job on the leader coordinator.
    pub async fn start_job_sync(&self, desc: JobDescriptor) -> Result<String> {
        let coord = self
            .leader_coordinator()
            .await
            .ok_or_else(|| anyhow::anyhow!("no coordinators registered in cluster"))?;
        coord.start_job_sync(desc).await
    }

    /// Start an asynchronous job on the leader coordinator.
    pub async fn start_job_async(&self, desc: JobDescriptor) -> Result<String> {
        let coord = self
            .leader_coordinator()
            .await
            .ok_or_else(|| anyhow::anyhow!("no coordinators registered in cluster"))?;
        coord.start_job_async(desc).await
    }

    /// Subscribe to leader-change events.
    pub fn subscribe_leader_events(&self) -> broadcast::Receiver<LeaderEvent> {
        self.elector.subscribe()
    }

    /// Return the number of coordinators currently registered in the cluster.
    pub async fn len(&self) -> usize {
        self.nodes.read().await.len()
    }

    /// Returns `true` if the cluster has no registered coordinators.
    pub async fn is_empty(&self) -> bool {
        self.nodes.read().await.is_empty()
    }
}
