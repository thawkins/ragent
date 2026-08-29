//! Policy-based conflict resolution and human-in-the-loop fallbacks (Task 5.3).
//!
//! Provides:
//! - [`ConflictPolicy`] — strategy used when multiple agents return responses.
//! - [`ConflictResolver`] — applies a [`ConflictPolicy`] to a list of
//!   `(agent_id, response)` pairs and returns a single aggregated result.
//! - [`HumanFallback`] — trait for human-review callbacks; see
//!   [`LoggingFallback`] for the default implementation.
//!
//! ## Wire into Coordinator
//!
//! Use [`crate::orchestrator::Coordinator::with_policy`] to attach a policy:
//!
//! ```rust,ignore
//! use ragent_agent::orchestrator::{Coordinator, AgentRegistry};
//! use ragent_agent::orchestrator::policy::{ConflictPolicy, ConflictResolver};
//!
//! let coord = Coordinator::new(registry)
//!     .with_policy(ConflictResolver::new(ConflictPolicy::Consensus { threshold: 2 }));
//! ```

use std::sync::Arc;

use anyhow::Result;

// ── ConflictPolicy ───────────────────────────────────────────────────────────

/// Strategy applied by [`ConflictResolver`] when aggregating multiple agent
/// responses into a single result.
#[derive(Debug, Clone)]
pub enum ConflictPolicy {
    /// Return the **concatenated** responses from all agents (default / MVP
    /// behaviour matching `start_job_sync`).
    Concat,

    /// Return the **first** response that does not begin with `"error:"`.
    /// If all responses are errors the last error is returned as `Err`.
    FirstSuccess,

    /// Return the **last** response received.
    LastResponse,

    /// Return a response only when at least `threshold` agents agree on the
    /// same result prefix (first 64 chars).  If the threshold is not met, all
    /// responses are concatenated and tagged with `[no consensus]`.
    Consensus {
        /// Minimum number of agreeing agents required.
        threshold: usize,
    },

    /// Escalate to the [`HumanFallback`] handler — useful when the coordinator
    /// cannot automatically resolve a conflict.
    HumanReview,
}

// ── HumanFallback ────────────────────────────────────────────────────────────

/// Called by [`ConflictResolver`] when [`ConflictPolicy::HumanReview`] is
/// active or when no automatic resolution is possible.
///
/// Implement this trait to integrate a real human approval flow (e.g. send a
/// Slack message, open a GitHub issue, prompt the TUI).
pub trait HumanFallback: Send + Sync {
    /// Invoked with the job id and the list of `(agent_id, response)` pairs.
    /// Must return a single resolved string (or an error explanation).
    fn on_conflict(&self, job_id: &str, responses: &[(String, String)]) -> String;
}

/// Default [`HumanFallback`] that logs the conflict to `tracing` and returns
/// all responses concatenated with a `[human-review]` header.
pub struct LoggingFallback;

impl HumanFallback for LoggingFallback {
    fn on_conflict(&self, job_id: &str, responses: &[(String, String)]) -> String {
        tracing::warn!(
            job_id = %job_id,
            agents = ?responses.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(),
            "ConflictPolicy::HumanReview triggered — concatenating all responses"
        );
        let parts: Vec<String> = responses
            .iter()
            .map(|(id, resp)| format!("--- agent: {id} ---\n{resp}"))
            .collect();
        format!("[human-review]\n{}", parts.join("\n"))
    }
}

// ── ConflictResolver ─────────────────────────────────────────────────────────

/// Applies a [`ConflictPolicy`] to a set of agent responses.
#[derive(Clone)]
pub struct ConflictResolver {
    policy: ConflictPolicy,
    fallback: Arc<dyn HumanFallback>,
}

impl ConflictResolver {
    /// Create a resolver with the given policy and the default [`LoggingFallback`].
    #[must_use]
    pub fn new(policy: ConflictPolicy) -> Self {
        Self {
            policy,
            fallback: Arc::new(LoggingFallback),
        }
    }

    /// Create a resolver with a custom [`HumanFallback`] handler.
    pub fn with_fallback(policy: ConflictPolicy, fallback: Arc<dyn HumanFallback>) -> Self {
        Self { policy, fallback }
    }

    /// Apply the policy to `responses` (list of `(agent_id, response)` pairs).
    ///
    /// Returns `Ok(result_string)` or `Err` if no valid resolution is found.
    pub fn resolve(&self, job_id: &str, responses: &[(String, String)]) -> Result<String> {
        if responses.is_empty() {
            anyhow::bail!("no responses to resolve");
        }

        match &self.policy {
            ConflictPolicy::Concat => {
                let parts: Vec<String> = responses
                    .iter()
                    .map(|(id, resp)| format!("--- agent: {id} ---\n{resp}"))
                    .collect();
                Ok(parts.join("\n"))
            }

            ConflictPolicy::FirstSuccess => {
                for (id, resp) in responses {
                    if !resp.trim_start().to_lowercase().starts_with("error:") {
                        return Ok(format!("--- agent: {id} ---\n{resp}"));
                    }
                }
                // All were errors — return last as Err (guaranteed non-empty above).
                let (_, last) = &responses[responses.len() - 1];
                Err(anyhow::anyhow!("all agents returned errors; last: {last}"))
            }

            ConflictPolicy::LastResponse => {
                // Guaranteed non-empty by the check above.
                let (id, resp) = &responses[responses.len() - 1];
                Ok(format!("--- agent: {id} ---\n{resp}"))
            }

            ConflictPolicy::Consensus { threshold } => {
                // Group by first 64 chars of the trimmed response.
                let mut groups: std::collections::HashMap<String, Vec<&str>> =
                    std::collections::HashMap::new();
                for (id, resp) in responses {
                    let key: String = resp.trim().chars().take(64).collect();
                    groups.entry(key).or_default().push(id.as_str());
                }
                // Find the group that meets the threshold.
                let winner = groups
                    .iter()
                    .filter(|(_, ids)| ids.len() >= *threshold)
                    .max_by_key(|(_, ids)| ids.len());

                if let Some((_, agreeing)) = winner {
                    // Return the response from the first agreeing agent.
                    let first_id = agreeing[0];
                    let resp = responses
                        .iter()
                        .find(|(id, _)| id == first_id)
                        .map_or("", |(_, r)| r.as_str());
                    Ok(format!("--- agent: {first_id} (consensus) ---\n{resp}"))
                } else {
                    // No consensus — concatenate all with a warning tag.
                    let parts: Vec<String> = responses
                        .iter()
                        .map(|(id, resp)| format!("--- agent: {id} ---\n{resp}"))
                        .collect();
                    Ok(format!("[no consensus]\n{}", parts.join("\n")))
                }
            }

            ConflictPolicy::HumanReview => Ok(self.fallback.on_conflict(job_id, responses)),
        }
    }
}

// ── Coordinator integration ──────────────────────────────────────────────────
// See `Coordinator::with_policy` in the parent module.
