use ragent_core::orchestrator::*;
use std::sync::Arc;
use futures::future::FutureExt;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_first_success_with_retries() {
    let registry = AgentRegistry::new();

    // Agent 1: sleeps longer than timeout
    let slow: Responder = Arc::new(|payload: String| {
        async move {
            sleep(Duration::from_millis(200)).await;
            format!("slow handled: {}", payload)
        }
        .boxed()
    });

    // Agent 2: immediate error (simulate by returning an error-like string)
    let err: Responder = Arc::new(|payload: String| {
        async move { format!("error: failed to handle {}", payload) }.boxed()
    });

    // Agent 3: quick success
    let fast: Responder = Arc::new(|payload: String| {
        async move { format!("fast success: {}", payload) }.boxed()
    });

    registry.register("agent-slow", vec!["search".to_string()], Some(slow)).await;
    registry.register("agent-err", vec!["search".to_string()], Some(err)).await;
    registry.register("agent-fast", vec!["search".to_string()], Some(fast)).await;

    // Create coordinator with short per-agent timeout so the slow agent times out.
    let coord = Coordinator::with_request_timeout(registry.clone(), Duration::from_millis(50));

    let desc = JobDescriptor { id: "job-retry".to_string(), required_capabilities: vec!["search".to_string()], payload: "do work".to_string() };

    // Should eventually return the fast agent's response (first-success strategy)
    let res = coord.start_job_first_success(desc).await.unwrap();
    assert!(res.contains("agent-fast") || res.contains("fast success"));
}

#[tokio::test]
async fn test_first_success_all_fail() {
    let registry = AgentRegistry::new();

    // All agents sleep beyond timeout
    let slow1: Responder = Arc::new(|payload: String| {
        async move { sleep(Duration::from_millis(200)).await; format!("s1: {}", payload) }.boxed()
    });
    let slow2: Responder = Arc::new(|payload: String| {
        async move { sleep(Duration::from_millis(200)).await; format!("s2: {}", payload) }.boxed()
    });

    registry.register("agent-s1", vec!["analysis".to_string()], Some(slow1)).await;
    registry.register("agent-s2", vec!["analysis".to_string()], Some(slow2)).await;

    let coord = Coordinator::with_request_timeout(registry.clone(), Duration::from_millis(50));
    let desc = JobDescriptor { id: "job-fail".to_string(), required_capabilities: vec!["analysis".to_string()], payload: "work".to_string() };

    let res = coord.start_job_first_success(desc).await;
    assert!(res.is_err());
}
