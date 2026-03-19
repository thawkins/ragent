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
//! use ragent_core::orchestrator::{Coordinator, AgentRegistry};
//! use ragent_core::orchestrator::policy::{ConflictPolicy, ConflictResolver};
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
            .map(|(id, resp)| format!("--- agent: {} ---\n{}", id, resp))
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
                    .map(|(id, resp)| format!("--- agent: {} ---\n{}", id, resp))
                    .collect();
                Ok(parts.join("\n"))
            }

            ConflictPolicy::FirstSuccess => {
                for (id, resp) in responses {
                    if !resp.trim_start().to_lowercase().starts_with("error:") {
                        return Ok(format!("--- agent: {} ---\n{}", id, resp));
                    }
                }
                // All were errors — return last as Err.
                let (_, last) = responses.last().unwrap();
                Err(anyhow::anyhow!(
                    "all agents returned errors; last: {}",
                    last
                ))
            }

            ConflictPolicy::LastResponse => {
                let (id, resp) = responses.last().unwrap();
                Ok(format!("--- agent: {} ---\n{}", id, resp))
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
                        .map(|(_, r)| r.as_str())
                        .unwrap_or("");
                    Ok(format!("--- agent: {} (consensus) ---\n{}", first_id, resp))
                } else {
                    // No consensus — concatenate all with a warning tag.
                    let parts: Vec<String> = responses
                        .iter()
                        .map(|(id, resp)| format!("--- agent: {} ---\n{}", id, resp))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn responses(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect()
    }

    #[test]
    fn test_concat_joins_all() {
        let r = ConflictResolver::new(ConflictPolicy::Concat);
        let res = r
            .resolve("j", &responses(&[("a", "hello"), ("b", "world")]))
            .unwrap();
        assert!(res.contains("hello"));
        assert!(res.contains("world"));
    }

    #[test]
    fn test_first_success_skips_errors() {
        let r = ConflictResolver::new(ConflictPolicy::FirstSuccess);
        let res = r
            .resolve("j", &responses(&[("a", "error: bad"), ("b", "ok result")]))
            .unwrap();
        assert!(res.contains("ok result"));
        assert!(!res.contains("error:"));
    }

    #[test]
    fn test_first_success_all_errors_returns_err() {
        let r = ConflictResolver::new(ConflictPolicy::FirstSuccess);
        let res = r.resolve("j", &responses(&[("a", "error: one"), ("b", "error: two")]));
        assert!(res.is_err());
    }

    #[test]
    fn test_last_response_returns_last() {
        let r = ConflictResolver::new(ConflictPolicy::LastResponse);
        let res = r
            .resolve(
                "j",
                &responses(&[("a", "first"), ("b", "second"), ("c", "third")]),
            )
            .unwrap();
        assert!(res.contains("third"));
        assert!(!res.contains("first"));
    }

    #[test]
    fn test_consensus_met() {
        let r = ConflictResolver::new(ConflictPolicy::Consensus { threshold: 2 });
        // a and b agree; c disagrees.
        let res = r
            .resolve(
                "j",
                &responses(&[
                    ("a", "the answer is 42"),
                    ("b", "the answer is 42"),
                    ("c", "different"),
                ]),
            )
            .unwrap();
        assert!(res.contains("consensus"));
        assert!(res.contains("the answer is 42"));
    }

    #[test]
    fn test_consensus_not_met_returns_all_tagged() {
        let r = ConflictResolver::new(ConflictPolicy::Consensus { threshold: 3 });
        let res = r
            .resolve("j", &responses(&[("a", "aaa"), ("b", "bbb"), ("c", "ccc")]))
            .unwrap();
        assert!(res.contains("[no consensus]"));
    }

    #[test]
    fn test_human_review_uses_fallback() {
        let r = ConflictResolver::new(ConflictPolicy::HumanReview);
        let res = r
            .resolve("j", &responses(&[("a", "one"), ("b", "two")]))
            .unwrap();
        assert!(res.contains("[human-review]"));
    }

    #[test]
    fn test_empty_responses_returns_err() {
        let r = ConflictResolver::new(ConflictPolicy::Concat);
        assert!(r.resolve("j", &[]).is_err());
    }
}
