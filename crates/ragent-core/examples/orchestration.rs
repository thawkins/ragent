//! Example demonstrating the ragent-core orchestration API (Milestone 3).
//!
//! Registers two in-process agents, starts both a synchronous and an
//! asynchronous orchestration job, and prints the aggregated results and
//! metrics snapshot.
//!
//! Run with:
//!   cargo run -p ragent-core --example orchestration

use std::sync::Arc;

use futures::future::FutureExt;
use tokio::time::{Duration, sleep};

use ragent_core::orchestrator::{AgentRegistry, Coordinator, JobDescriptor, Responder};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let registry = AgentRegistry::new();

    // Register agent-a: handles "search" + "analysis" requests.
    let responder_a: Responder =
        Arc::new(|payload: String| async move { format!("agent-a received: {}", payload) }.boxed());

    // Register agent-b: handles "search" + "compile" requests (with a small delay).
    let responder_b: Responder = Arc::new(|payload: String| {
        async move {
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

    // --- Synchronous job: blocks until all matched agents respond ---
    let desc = JobDescriptor {
        id: "job-sync".to_string(),
        required_capabilities: vec!["search".to_string()],
        payload: "find TODOs".to_string(),
    };
    println!("Starting sync job...");
    let result = coord.start_job_sync(desc).await?;
    println!("Sync result:\n{}\n", result);

    // --- Asynchronous job: returns immediately with a job_id ---
    let desc2 = JobDescriptor {
        id: "job-async".to_string(),
        required_capabilities: vec!["search".to_string()],
        payload: "ping".to_string(),
    };
    let job_id = coord.start_job_async(desc2).await?;
    println!("Started async job id={}", job_id);

    // Wait briefly for the async job to complete.
    sleep(Duration::from_millis(50)).await;

    if let Some((status, res)) = coord.get_job_result(&job_id).await {
        println!("Async job status={} result={:?}", status, res);
    }

    // --- Metrics snapshot ---
    let m = coord.metrics_snapshot();
    println!(
        "\nMetrics: active={} completed={} timeouts={} errors={}",
        m.active_jobs, m.completed_jobs, m.timeouts, m.errors
    );

    Ok(())
}
