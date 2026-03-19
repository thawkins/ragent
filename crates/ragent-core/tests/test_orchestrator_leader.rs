//! Tests for in-process leader election and CoordinatorCluster (Task 5.2).

use futures::future::FutureExt;
use std::sync::Arc;
use tokio::time::{Duration, sleep};

use ragent_core::orchestrator::leader::{CoordinatorCluster, LeaderElector, LeaderEvent};
use ragent_core::orchestrator::{AgentRegistry, Coordinator, JobDescriptor, Responder};

// ── LeaderElector ────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_elector_single_nominee_becomes_leader() {
    let elector = LeaderElector::new(16);
    let leader = elector.nominate("node-1", "node-1").await;
    assert_eq!(leader, "node-1");
    assert_eq!(elector.current_leader().await, Some("node-1".to_string()));
}

#[tokio::test]
async fn test_elector_majority_wins() {
    let elector = LeaderElector::new(16);
    // node-1 and node-2 vote for "alpha"; node-3 votes for "beta".
    elector.nominate("node-1", "alpha").await;
    elector.nominate("node-2", "alpha").await;
    let leader = elector.nominate("node-3", "beta").await;
    assert_eq!(leader, "alpha");
}

#[tokio::test]
async fn test_elector_is_leader() {
    let elector = LeaderElector::new(16);
    elector.nominate("n1", "n1").await;
    assert!(elector.is_leader("n1").await);
    assert!(!elector.is_leader("n2").await);
}

#[tokio::test]
async fn test_elector_withdraw_triggers_reelection() {
    let elector = LeaderElector::new(16);
    elector.nominate("n1", "n1").await;
    elector.nominate("n2", "n2").await;
    // n1 currently leads (tie broken by lexicographic order; "n1" < "n2").
    // Withdraw n1 → n2 should become leader.
    let new_leader = elector.withdraw("n1").await;
    assert_eq!(new_leader, Some("n2".to_string()));
}

#[tokio::test]
async fn test_elector_withdraw_last_voter_no_leader() {
    let elector = LeaderElector::new(16);
    elector.nominate("n1", "n1").await;
    let leader = elector.withdraw("n1").await;
    assert_eq!(leader, None);
    assert_eq!(elector.current_leader().await, None);
}

#[tokio::test]
async fn test_elector_subscribe_receives_leader_elected_event() {
    let elector = LeaderElector::new(16);
    let mut sub = elector.subscribe();
    elector.nominate("n1", "n1").await;

    let ev = tokio::time::timeout(Duration::from_secs(1), sub.recv()).await;
    assert!(ev.is_ok(), "expected event within timeout");
    let ev = ev.unwrap().unwrap();
    match ev {
        LeaderEvent::LeaderElected { leader_id } => assert_eq!(leader_id, "n1"),
        _ => panic!("expected LeaderElected event"),
    }
}

#[tokio::test]
async fn test_elector_no_duplicate_events_when_leader_unchanged() {
    let elector = LeaderElector::new(32);
    let mut sub = elector.subscribe();
    // First nomination elects "alpha".
    elector.nominate("v1", "alpha").await;
    // Second nomination for same candidate — no new event.
    elector.nominate("v2", "alpha").await;

    // Should receive exactly one LeaderElected event.
    let ev1 = tokio::time::timeout(Duration::from_millis(50), sub.recv()).await;
    assert!(ev1.is_ok());
    // Second recv should time out (no new event).
    let ev2 = tokio::time::timeout(Duration::from_millis(50), sub.recv()).await;
    assert!(ev2.is_err(), "expected no second LeaderElected event");
}

// ── CoordinatorCluster ───────────────────────────────────────────────────────

/// Helper: build a Coordinator with one agent tagged `"work"`.
async fn make_coord_with_agent(tag: &'static str) -> Coordinator {
    let registry = AgentRegistry::new();
    let tag_s = tag.to_string();
    let r: Responder = Arc::new(move |p: String| {
        let t = tag_s.clone();
        async move { format!("{}:{}", t, p) }.boxed()
    });
    registry
        .register(format!("{}-agent", tag), vec!["work".to_string()], Some(r))
        .await;
    Coordinator::new(registry)
}

#[tokio::test]
async fn test_cluster_add_and_len() {
    let elector = LeaderElector::new(16);
    let cluster = CoordinatorCluster::new(elector);
    assert_eq!(cluster.len().await, 0);
    assert!(cluster.is_empty().await);

    let registry = AgentRegistry::new();
    cluster.add("node-1", Coordinator::new(registry)).await;
    assert_eq!(cluster.len().await, 1);
    assert!(!cluster.is_empty().await);
}

#[tokio::test]
async fn test_cluster_elect_sets_leader() {
    let elector = LeaderElector::new(16);
    let cluster = CoordinatorCluster::new(elector);

    let r = AgentRegistry::new();
    cluster.add("node-a", Coordinator::new(r.clone())).await;
    cluster.add("node-b", Coordinator::new(r)).await;

    let leader = cluster.elect("node-b").await;
    assert_eq!(leader, "node-b");
    assert_eq!(cluster.current_leader().await, Some("node-b".to_string()));
}

#[tokio::test]
async fn test_cluster_leader_coordinator_returns_leader() {
    let elector = LeaderElector::new(16);
    let cluster = CoordinatorCluster::new(elector);

    let r = AgentRegistry::new();
    cluster.add("n1", Coordinator::new(r.clone())).await;
    cluster.add("n2", Coordinator::new(r)).await;
    cluster.elect("n1").await;

    assert!(cluster.leader_coordinator().await.is_some());
}

#[tokio::test]
async fn test_cluster_start_job_routes_to_leader() {
    let elector = LeaderElector::new(16);
    let cluster = CoordinatorCluster::new(elector);

    // Leader coordinator has an agent registered.
    let registry = AgentRegistry::new();
    let r: Responder = Arc::new(|p: String| async move { format!("leader-resp:{}", p) }.boxed());
    registry
        .register("leader-ag", vec!["w".to_string()], Some(r))
        .await;
    let leader_coord = Coordinator::new(registry);

    // Follower coordinator has no agents.
    let follower_coord = Coordinator::new(AgentRegistry::new());

    cluster.add("leader", leader_coord).await;
    cluster.add("follower", follower_coord).await;
    cluster.elect("leader").await;

    let desc = JobDescriptor {
        id: "cluster-job".to_string(),
        required_capabilities: vec!["w".to_string()],
        payload: "task".to_string(),
    };
    let result = cluster.start_job_sync(desc).await.unwrap();
    assert!(result.contains("leader-resp:task"));
}

#[tokio::test]
async fn test_cluster_remove_and_fallback() {
    let elector = LeaderElector::new(16);
    let cluster = CoordinatorCluster::new(elector);

    let r = AgentRegistry::new();
    cluster.add("n1", Coordinator::new(r.clone())).await;
    cluster.add("n2", Coordinator::new(r)).await;
    cluster.elect("n1").await;

    // Remove the leader.
    cluster.remove("n1").await;

    assert_eq!(cluster.len().await, 1);
    // leader_coordinator should fall back to n2.
    assert!(cluster.leader_coordinator().await.is_some());
}

#[tokio::test]
async fn test_cluster_no_coordinators_returns_err() {
    let elector = LeaderElector::new(16);
    let cluster = CoordinatorCluster::new(elector);
    let desc = JobDescriptor {
        id: "empty".to_string(),
        required_capabilities: vec![],
        payload: "x".to_string(),
    };
    assert!(cluster.start_job_sync(desc).await.is_err());
}

#[tokio::test]
async fn test_cluster_subscribe_leader_events() {
    let elector = LeaderElector::new(16);
    let cluster = CoordinatorCluster::new(elector);
    let mut sub = cluster.subscribe_leader_events();

    let r = AgentRegistry::new();
    cluster.add("evt-node", Coordinator::new(r)).await;
    cluster.elect("evt-node").await;

    let ev = tokio::time::timeout(Duration::from_secs(1), sub.recv()).await;
    assert!(ev.is_ok(), "expected LeaderElected event");
    match ev.unwrap().unwrap() {
        LeaderEvent::LeaderElected { leader_id } => assert_eq!(leader_id, "evt-node"),
        _ => panic!("expected LeaderElected"),
    }
}

/// Cluster async job delegates to leader coordinator.
#[tokio::test]
async fn test_cluster_start_job_async() {
    let elector = LeaderElector::new(16);
    let cluster = CoordinatorCluster::new(elector);

    let registry = AgentRegistry::new();
    let r: Responder = Arc::new(|p: String| async move { format!("async:{}", p) }.boxed());
    registry
        .register("async-ag", vec!["x".to_string()], Some(r))
        .await;
    cluster.add("only", Coordinator::new(registry)).await;
    cluster.elect("only").await;

    let desc = JobDescriptor {
        id: "async-cluster".to_string(),
        required_capabilities: vec!["x".to_string()],
        payload: "go".to_string(),
    };
    let job_id = cluster.start_job_async(desc).await.unwrap();
    assert!(!job_id.is_empty());
    sleep(Duration::from_millis(100)).await;
}
