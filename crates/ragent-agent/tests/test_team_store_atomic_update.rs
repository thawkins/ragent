//! `config.json` read-modify-write must be atomic (review P0-3).
//!
//! `TeamStore::load` + `save` released the read lock before taking the write
//! lock, so two writers could read the same snapshot and the later save
//! discarded the earlier one. These tests race many writers against one
//! roster and require every write to survive.

use std::sync::Arc;

use ragent_agent::team::{TeamMember, TeamStore};

#[path = "manager_support/mod.rs"]
mod manager_support;
use manager_support::team_manager;

const ROUNDS: u32 = 40;

#[tokio::test]
async fn test_team_store_update_member_concurrent_writes_all_survive() {
    let (manager, _tmp, _bus) = team_manager("atomic-a", "lead-sess", true);
    let mut store = TeamStore::load(&manager.team_dir).expect("load");
    store
        .add_member(TeamMember::new("bob", "tm-002", "general"))
        .expect("add tm-002");
    drop(store);

    // Each writer bumps its own member's counter (kept in `spawn_prompt`).
    // A lost update shows up as a counter short of ROUNDS.
    let writers: Vec<_> = ["tm-001", "tm-002"]
        .into_iter()
        .map(|id| {
            let manager = Arc::clone(&manager);
            std::thread::spawn(move || {
                for _ in 0..ROUNDS {
                    manager
                        .update_member(id, |m| {
                            let n: u32 = m
                                .spawn_prompt
                                .as_deref()
                                .and_then(|s| s.parse().ok())
                                .unwrap_or(0);
                            m.spawn_prompt = Some((n + 1).to_string());
                        })
                        .expect("update_member");
                }
            })
        })
        .collect();
    for w in writers {
        w.join().expect("writer thread");
    }

    let config = TeamStore::load(&manager.team_dir).expect("load").config;
    for id in ["tm-001", "tm-002"] {
        let count = config
            .member_by_id(id)
            .and_then(|m| m.spawn_prompt.clone())
            .unwrap_or_default();
        assert_eq!(count, ROUNDS.to_string(), "{id} lost updates");
    }
}

#[tokio::test]
async fn test_team_store_update_skips_write_when_unchanged() {
    let (manager, _tmp, _bus) = team_manager("atomic-b", "lead-sess", true);
    let path = manager.team_dir.join("config.json");
    let before = std::fs::read_to_string(&path).expect("read");
    let name =
        TeamStore::update(&manager.team_dir, |config| Ok(config.name.clone())).expect("update");
    assert_eq!(name, "atomic-b");
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        before,
        "an inspecting closure must not rewrite the file"
    );
}

#[tokio::test]
async fn test_team_store_update_failed_closure_writes_nothing() {
    let (manager, _tmp, _bus) = team_manager("atomic-c", "lead-sess", true);
    let path = manager.team_dir.join("config.json");
    let before = std::fs::read_to_string(&path).expect("read");
    let err = TeamStore::update(&manager.team_dir, |config| -> anyhow::Result<()> {
        config.members.clear();
        anyhow::bail!("refused")
    })
    .expect_err("closure error propagates");
    assert!(err.to_string().contains("refused"));
    assert_eq!(std::fs::read_to_string(&path).expect("read"), before);
}
