//! Tests-only coverage for `AgentRegistry` (`ragent_agent::orchestrator::registry`):
//! register/get/list/unregister, heartbeat, stale pruning, and capability
//! matching.

use std::sync::Arc;

use futures::future::FutureExt;
use ragent_agent::orchestrator::{AgentRegistry, Responder};

fn responder() -> Option<Responder> {
    // A no-op responder spawns a mailbox loop; used to cover the handle path.
    Some(Arc::new(|payload: String| async move { payload }.boxed()))
}

#[tokio::test]
async fn register_get_list_and_unregister_round_trip() {
    let registry = AgentRegistry::new();
    assert!(registry.list().await.is_empty());
    assert!(registry.get("a").await.is_none());

    registry
        .register("a", vec!["search".to_string()], responder())
        .await;
    registry
        .register("b", vec!["write".to_string()], None)
        .await;

    assert_eq!(registry.list().await.len(), 2);
    let a = registry.get("a").await.expect("agent a");
    assert_eq!(a.id, "a");
    assert!(a.capabilities.contains(&"search".to_string()));

    registry.unregister("a").await;
    assert!(registry.get("a").await.is_none());
    assert_eq!(registry.list().await.len(), 1);
}

#[tokio::test]
async fn heartbeat_refreshes_the_timestamp() {
    let registry = AgentRegistry::new();
    registry.register("a", vec![], None).await;
    let before = registry.get("a").await.expect("a").last_heartbeat;
    assert!(before.is_some());

    // Sleep a hair so the new heartbeat is strictly later.
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    registry.heartbeat("a").await;
    let after = registry.get("a").await.expect("a").last_heartbeat;
    assert!(after >= before, "heartbeat must not move backwards");

    // Heartbeating an unknown id is a harmless no-op.
    registry.heartbeat("ghost").await;
}

#[tokio::test]
async fn prune_stale_removes_old_entries() {
    let registry = AgentRegistry::new();
    registry.register("fresh", vec![], None).await;

    // A zero staleness window prunes everything registered before "now".
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    registry
        .prune_stale(std::time::Duration::from_millis(0))
        .await;
    assert!(
        registry.get("fresh").await.is_none(),
        "a zero window prunes the stale entry"
    );
}

#[tokio::test]
async fn match_agents_requires_all_capabilities() {
    let registry = AgentRegistry::new();
    registry
        .register(
            "both",
            vec!["search".to_string(), "write".to_string()],
            None,
        )
        .await;
    registry
        .register("search-only", vec!["search".to_string()], None)
        .await;

    let both = registry
        .match_agents(&["search".to_string(), "write".to_string()])
        .await;
    assert_eq!(both.len(), 1);
    assert_eq!(both[0].id, "both");

    let search = registry.match_agents(&["search".to_string()]).await;
    assert_eq!(search.len(), 2);

    let none = registry.match_agents(&["absent".to_string()]).await;
    assert!(none.is_empty());
}
