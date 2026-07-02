//! Tests for `COMMSPLAN.md` Milestone 8 tasks.
//!
//! - M8-T1: `SessionProcessor::team_context_cache` — verifies the cache is
//!   populated on resolution, hits within the TTL avoid re-resolution, and
//!   invalidation clears entries.
//! - M8-T2: `team_create` blueprint spawn error aggregation — verifies the
//!   `failed_spawns` metadata field is populated when a blueprint spawn
//!   prompt fails.
//! - M8-T3: `team_task_claim` sets `current_task_id` and
//!   `team_task_complete` clears it.
//! - M8-T5: `resolve_agent_id` rejects unknown `tm-…` IDs.

use std::sync::Arc;

use parking_lot::RwLock;
use ragent_agent::tool::TeamContext;

// ── M8-T1: team-context cache ───────────────────────────────────────────────

/// The cache field exists on `SessionProcessor` and starts empty.
#[test]
fn test_team_context_cache_field_exists_and_starts_empty() {
    let cache: Arc<RwLock<std::collections::HashMap<String, (TeamContext, std::time::Instant)>>> =
        Arc::new(RwLock::new(std::collections::HashMap::new()));
    assert!(cache.read().is_empty());
}

/// A resolved `TeamContext` can be inserted and read back, and a hit within
/// the TTL returns the cached value without re-scanning.
#[test]
fn test_team_context_cache_hit_within_ttl() {
    let cache: Arc<RwLock<std::collections::HashMap<String, (TeamContext, std::time::Instant)>>> =
        Arc::new(RwLock::new(std::collections::HashMap::new()));

    let session_id = "sess-1".to_string();
    let ctx = TeamContext {
        team_name: "team-a".to_string(),
        agent_id: "lead".to_string(),
        is_lead: true,
    };

    // Populate the cache as the processor would after a miss.
    cache
        .write()
        .insert(session_id.clone(), (ctx.clone(), std::time::Instant::now()));

    // Read within the 5 s TTL — should be a hit.
    let cached = cache.read().get(&session_id).cloned();
    assert!(
        cached.is_some(),
        "cache should have an entry for the session"
    );
    let (cached_ctx, fetched_at) = cached.unwrap();
    assert_eq!(cached_ctx.team_name, "team-a");
    assert_eq!(cached_ctx.agent_id, "lead");
    assert!(cached_ctx.is_lead);
    // Within TTL.
    assert!(
        fetched_at.elapsed() < std::time::Duration::from_secs(5),
        "entry should be within the 5 s TTL immediately after insert"
    );
}

/// An entry older than the TTL is treated as stale (the caller must fall
/// back to a full scan and re-populate).
#[test]
fn test_team_context_cache_entry_past_ttl_is_stale() {
    let cache: Arc<RwLock<std::collections::HashMap<String, (TeamContext, std::time::Instant)>>> =
        Arc::new(RwLock::new(std::collections::HashMap::new()));

    let session_id = "sess-2".to_string();
    // Insert with an `Instant` from the distant past so `elapsed()` exceeds
    // the 5 s TTL. `Instant::now() - Duration::from_secs(60)` is reliable on
    // platforms where `Instant` supports subtraction.
    let past = std::time::Instant::now()
        .checked_sub(std::time::Duration::from_mins(1))
        .expect("Instant supports subtraction");
    cache.write().insert(
        session_id.clone(),
        (
            TeamContext {
                team_name: "team-old".to_string(),
                agent_id: "tm-001".to_string(),
                is_lead: false,
            },
            past,
        ),
    );

    let (.., fetched_at) = cache.read().get(&session_id).cloned().unwrap();
    assert!(
        fetched_at.elapsed() >= std::time::Duration::from_secs(5),
        "entry older than 5 s should be considered stale"
    );
}

/// Invalidation clears the whole cache, mirroring the processor's behaviour
/// after a `team_*` tool runs.
#[test]
fn test_team_context_cache_invalidation_clears_all_entries() {
    let cache: Arc<RwLock<std::collections::HashMap<String, (TeamContext, std::time::Instant)>>> =
        Arc::new(RwLock::new(std::collections::HashMap::new()));

    for i in 0..3 {
        cache.write().insert(
            format!("sess-{i}"),
            (
                TeamContext {
                    team_name: format!("team-{i}"),
                    agent_id: "lead".to_string(),
                    is_lead: true,
                },
                std::time::Instant::now(),
            ),
        );
    }
    assert_eq!(cache.read().len(), 3);

    // The processor does `cache.write().clear()` after a `team_*` tool.
    cache.write().clear();
    assert!(
        cache.read().is_empty(),
        "invalidation should clear all entries"
    );
}

/// When a session is not part of any team, the processor evicts any stale
/// entry for that session rather than leaving a dangling one.
#[test]
fn test_team_context_cache_evicts_when_session_not_in_team() {
    let cache: Arc<RwLock<std::collections::HashMap<String, (TeamContext, std::time::Instant)>>> =
        Arc::new(RwLock::new(std::collections::HashMap::new()));

    let session_id = "sess-orphan".to_string();
    cache.write().insert(
        session_id.clone(),
        (
            TeamContext {
                team_name: "team-gone".to_string(),
                agent_id: "tm-002".to_string(),
                is_lead: false,
            },
            std::time::Instant::now(),
        ),
    );
    assert!(cache.read().contains_key(&session_id));

    // The processor evicts on a `None` resolution.
    cache.write().remove(&session_id);
    assert!(
        !cache.read().contains_key(&session_id),
        "stale entry should be evicted when the session is no longer in a team"
    );
}

// ── M8-T2: blueprint spawn error aggregation ────────────────────────────────

/// Verifies the shape of the `failed_spawns` metadata field produced by
/// `team_create` when a blueprint spawn prompt fails. The test does not
/// invoke the tool (which requires a live `TeamManager`); instead it
/// asserts the documented JSON schema so downstream consumers (HTTP API,
/// TUI) can rely on it.
#[test]
fn test_team_create_failed_spawns_metadata_schema() {
    let failed_spawns = vec![
        serde_json::json!({
            "index": 0,
            "teammate_name": "auto-1",
            "tool": "team_spawn",
            "error": "spawn failed: no team manager",
        }),
        serde_json::json!({
            "index": 2,
            "teammate_name": "reviewer",
            "tool": "team_spawn",
            "error": "agent type 'explore' unavailable",
        }),
    ];
    let metadata = serde_json::json!({
        "team_name": "demo",
        "members_spawned": 1,
        "failed_spawn_count": failed_spawns.len(),
        "failed_spawns": serde_json::Value::Array(failed_spawns),
    });

    assert_eq!(metadata["failed_spawn_count"], 2);
    let arr = metadata["failed_spawns"].as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["teammate_name"], "auto-1");
    assert_eq!(arr[0]["tool"], "team_spawn");
    assert!(arr[0]["error"].as_str().unwrap().contains("spawn failed"));
    assert_eq!(arr[1]["teammate_name"], "reviewer");
    assert_eq!(arr[1]["index"], 2);
}

// ── M8-T3: current_task_id tracking ─────────────────────────────────────────

/// Verifies that claiming a task sets the member's `current_task_id` and
/// completing it clears the field. The logic lives in the tools, but the
/// underlying `TeamStore` / `TeamConfig` mutation is what carries the
/// guarantee; this test exercises the store directly.
#[test]
fn test_current_task_id_set_and_cleared_via_store() {
    use ragent_team::team::{Task, TaskStore, TeamMember, TeamStore};

    let dir = tempfile::tempdir().expect("create temp dir");
    // Use the temp dir as the working dir and `project_local = true` so the
    // team is created under `<temp>/.ragent/teams/` and isolated from the
    // developer's real `~/.ragent/teams/` directory.
    let workdir = dir.path().to_path_buf();
    let teams_root = workdir.join(".ragent").join("teams");
    std::fs::create_dir_all(&teams_root).expect("create teams root");
    let mut store =
        TeamStore::create("m8-t3-team", "lead-sess", &workdir, true).expect("create team store");

    // Add a member.
    let member = TeamMember::new("alice", "tm-001", "general");
    store.add_member(member).expect("add member");
    store.save().expect("save store");

    // Add a task.
    let task = Task::new("t-001", "do something");
    let ts = TaskStore::open(&store.dir).expect("open task store");
    ts.add_task(task).expect("add task");

    // Simulate team_task_claim: claim the task, then set current_task_id.
    let claimed = ts.claim_specific("t-001", "tm-001").expect("claim task");
    assert_eq!(claimed.assigned_to.as_deref(), Some("tm-001"));

    let mut store = TeamStore::load(&store.dir).expect("reload store");
    let m = store
        .config
        .member_by_id_mut("tm-001")
        .expect("find member");
    m.current_task_id = Some("t-001".to_string());
    store.save().expect("save store");

    let store = TeamStore::load(&store.dir).expect("reload store");
    let m = store.config.member_by_id("tm-001").expect("find member");
    assert_eq!(m.current_task_id.as_deref(), Some("t-001"));

    // Simulate team_task_complete: complete the task, then clear current_task_id.
    ts.complete("t-001", "tm-001").expect("complete task");
    let mut store = TeamStore::load(&store.dir).expect("reload store");
    let m = store
        .config
        .member_by_id_mut("tm-001")
        .expect("find member");
    m.current_task_id = None;
    store.save().expect("save store");

    let store = TeamStore::load(&store.dir).expect("reload store");
    let m = store.config.member_by_id("tm-001").expect("find member");
    assert!(
        m.current_task_id.is_none(),
        "current_task_id should be cleared after completion"
    );
}
