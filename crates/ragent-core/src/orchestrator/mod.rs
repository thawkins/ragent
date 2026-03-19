//! Multi-agent orchestration primitives (MVP)
//!
//! Provides an in-process AgentRegistry, an InProcessRouter (actor-style
//! inboxes), and a Coordinator that can start jobs synchronously and
//! asynchronously with basic negotiation and aggregation strategies.
//!
//! ## Submodules (Milestone 5 extensions)
//! - [`transport`] — pluggable transport adapters (HttpRouter, RouterComposite)
//! - [`leader`]    — in-process leader election and CoordinatorCluster
//! - [`policy`]    — conflict resolution policies and human-in-the-loop fallbacks

pub mod leader;
pub mod policy;
pub mod transport;
/// Agent registry for managing agent instances.
pub mod registry;
/// Routing infrastructure for message passing.
pub mod router;
/// Coordination layer for orchestrating multi-agent workflows.
pub mod coordinator;

// Re-export common orchestrator types for backwards compatibility.
pub use registry::{AgentEntry, AgentId, AgentRegistry, Responder};
pub use router::{InProcessRouter, Router};
pub use registry::OrchestrationRequest;
pub use coordinator::{Coordinator, JobDescriptor, JobEvent, MetricsSnapshot, OrchestrationMessage};

#[cfg(test)]
mod tests {
    use super::*;
    use futures::FutureExt;
    use std::sync::Arc;
    use tokio::time::{Duration, sleep};

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

        registry
            .register(
                "agent-a",
                vec!["search".to_string(), "analysis".to_string()],
                Some(responder_a),
            )
            .await;
        registry
            .register(
                "agent-b",
                vec!["search".to_string(), "compile".to_string()],
                Some(responder_b),
            )
            .await;

        let coord = Coordinator::new(registry.clone());
        let desc = JobDescriptor {
            id: "job-1".to_string(),
            required_capabilities: vec!["search".to_string()],
            payload: "find TODOs".to_string(),
        };

        let res = coord.start_job_sync(desc).await.unwrap();
        assert!(res.contains("agent-a received"));
        assert!(res.contains("agent-b processed"));
    }

    #[tokio::test]
    async fn test_start_job_async_and_subscribe() {
        let registry = AgentRegistry::new();

        let fast: Responder =
            Arc::new(|payload: String| async move { format!("fast: {}", payload) }.boxed());
        registry
            .register("fast-agent", vec!["echo".to_string()], Some(fast))
            .await;

        let coord = Coordinator::new(registry.clone());
        let desc = JobDescriptor {
            id: "job-async".to_string(),
            required_capabilities: vec!["echo".to_string()],
            payload: "ping".to_string(),
        };

        let job_id = coord.start_job_async(desc.clone()).await.unwrap();
        let mut sub = coord.subscribe_job_events(&job_id).await.unwrap();

        // Collect a few events
        let mut got_started = false;
        let mut got_completed = false;
        for _ in 0..4 {
            if let Ok(ev) = tokio::time::timeout(Duration::from_secs(1), sub.recv()).await {
                match ev {
                    Ok(JobEvent::JobStarted { job_id: jid }) => {
                        if jid == job_id {
                            got_started = true;
                        }
                    }
                    Ok(JobEvent::JobCompleted {
                        job_id: jid,
                        success: _,
                    }) => {
                        if jid == job_id {
                            got_completed = true;
                            break;
                        }
                    }
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
