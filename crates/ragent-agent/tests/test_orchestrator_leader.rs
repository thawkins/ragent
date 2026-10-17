//! Tests-only coverage for the in-process leader election and
//! `CoordinatorCluster` (`ragent_agent::orchestrator::leader`).
//!
//! Fully in-process: no mailboxes are consulted because the cluster is only
//! driven through its election / registration surface (jobs are exercised by
//! `test_orchestrator_coordinator.rs`).

use ragent_agent::orchestrator::leader::{CoordinatorCluster, LeaderElector, LeaderEvent};
use ragent_agent::orchestrator::{AgentRegistry, Coordinator};

#[tokio::test]
async fn elector_elects_and_updates_current_leader() {
    let elector = LeaderElector::new(16);
    assert!(elector.current_leader().await.is_none());

    let mut events = elector.subscribe();
    let leader = elector.nominate("node-1", "node-1").await;
    assert_eq!(leader, "node-1");
    assert!(elector.is_leader("node-1").await);
    assert!(!elector.is_leader("node-2").await);
    assert_eq!(elector.current_leader().await.as_deref(), Some("node-1"));

    match events.recv().await.expect("leader event") {
        LeaderEvent::LeaderElected { leader_id } => assert_eq!(leader_id, "node-1"),
        LeaderEvent::LeaderStepped { .. } => panic!("expected LeaderElected"),
    }
}

#[tokio::test]
async fn elector_breaks_ties_by_lowest_id() {
    let elector = LeaderElector::new(16);
    elector.nominate("voter-a", "candidate-b").await;
    let winner = elector.nominate("voter-b", "candidate-a").await;
    assert_eq!(
        winner, "candidate-a",
        "lexicographically lowest id wins a tie"
    );
}

#[tokio::test]
async fn elector_withdraw_last_vote_clears_leader_and_emits_stepped() {
    let elector = LeaderElector::new(16);
    let mut events = elector.subscribe();
    elector.nominate("only", "only").await;
    // Drain the LeaderElected event.
    let _ = events.recv().await;

    let remaining = elector.withdraw("only").await;
    assert!(remaining.is_none(), "no votes remain");
    assert!(elector.current_leader().await.is_none());
    match events.recv().await.expect("stepped event") {
        LeaderEvent::LeaderStepped { former_id } => assert_eq!(former_id, "only"),
        LeaderEvent::LeaderElected { .. } => panic!("expected LeaderStepped"),
    }
}

#[tokio::test]
async fn elector_withdraw_with_votes_remaining_recounts() {
    let elector = LeaderElector::new(16);
    elector.nominate("v1", "c1").await;
    elector.nominate("v2", "c1").await;
    let leader = elector.withdraw("v2").await;
    assert_eq!(leader.as_deref(), Some("c1"), "c1 still holds v1's vote");
}

#[tokio::test]
async fn cluster_registration_and_leader_routing() {
    let cluster = CoordinatorCluster::new(LeaderElector::new(16));
    assert!(cluster.is_empty().await);
    assert_eq!(cluster.len().await, 0);
    assert!(cluster.leader_coordinator().await.is_none());
    assert!(cluster.current_leader().await.is_none());

    cluster
        .add("node-1", Coordinator::new(AgentRegistry::new()))
        .await;
    assert_eq!(cluster.len().await, 1);
    assert!(!cluster.is_empty().await);

    // No vote yet -> the first registered coordinator is the fallback.
    assert!(
        cluster.leader_coordinator().await.is_some(),
        "fallback coordinator expected"
    );

    let elected = cluster.elect("node-1").await;
    assert_eq!(elected, "node-1");
    assert_eq!(cluster.current_leader().await.as_deref(), Some("node-1"));
    assert!(cluster.leader_coordinator().await.is_some());

    // Subscribe before removal to observe the withdraw-triggered recount.
    let mut events = cluster.subscribe_leader_events();
    cluster.remove("node-1").await;
    assert!(cluster.is_empty().await);
    assert!(cluster.current_leader().await.is_none());
    // The removal dropped the only vote; a LeaderStepped must have been emitted.
    let mut saw_stepped = false;
    while let Ok(event) = events.try_recv() {
        if matches!(event, LeaderEvent::LeaderStepped { .. }) {
            saw_stepped = true;
        }
    }
    assert!(
        saw_stepped,
        "expected LeaderStepped after removing the leader"
    );
}

#[tokio::test]
async fn cluster_job_on_empty_cluster_errors() {
    let cluster = CoordinatorCluster::new(LeaderElector::new(16));
    let desc = ragent_agent::orchestrator::JobDescriptor {
        id: "job-x".to_string(),
        required_capabilities: vec![],
        payload: "p".to_string(),
    };
    let err = cluster
        .start_job_sync(desc)
        .await
        .expect_err("no coordinators must error");
    assert!(err.to_string().contains("no coordinators registered"));
}
