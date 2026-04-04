//! Example demonstrating orchestrator usage for coordinating multiple agent tasks

use futures::future::FutureExt;
use ragent_core::orchestrator::{AgentRegistry, Coordinator, JobDescriptor, Responder};
use std::sync::Arc;
use tokio::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let registry = AgentRegistry::new();

    let responder_a: Responder =
        Arc::new(|payload: String| async move { format!("agent-a: {payload}") }.boxed());
    let responder_b: Responder = Arc::new(|payload: String| {
        async move {
            sleep(Duration::from_millis(50)).await;
            format!("agent-b: {payload}")
        }
        .boxed()
    });

    registry
        .register("agent-a", vec!["echo".to_string()], Some(responder_a))
        .await;
    registry
        .register("agent-b", vec!["echo".to_string()], Some(responder_b))
        .await;

    let coord = Coordinator::new(registry.clone());

    // synchronous example
    let desc = JobDescriptor {
        id: "ex-sync".to_string(),
        required_capabilities: vec!["echo".to_string()],
        payload: "hello sync".to_string(),
    };
    let res = coord.start_job_sync(desc).await?;
    println!("Sync result:\n{res}");

    // asynchronous example
    let desc2 = JobDescriptor {
        id: "ex-async".to_string(),
        required_capabilities: vec!["echo".to_string()],
        payload: "hello async".to_string(),
    };
    let job_id = coord.start_job_async(desc2.clone()).await?;
    println!("Started async job {job_id}");

    let mut sub = coord.subscribe_job_events(&job_id).await?;
    // listen for a few events
    for _ in 0..5 {
        match tokio::time::timeout(Duration::from_secs(2), sub.recv()).await {
            Ok(Ok(evt)) => println!("Event: {evt:?}"),
            Ok(Err(e)) => println!("Event error: {e}"),
            Err(_) => break,
        }
    }

    if let Some((status, result)) = coord.get_job_result(&job_id).await {
        println!("Job {job_id} status={status} result=\n{result:?}");
    }

    Ok(())
}
