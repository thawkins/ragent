//! Regression tests for SEC-ragent-team-002 / SEC-ragent-agent-007 /
//! SEC-ragent-tui-004 (SECTASKS MS-01 T-004): a team name is interpolated into
//! filesystem paths, so it must be validated as a single safe component before
//! any `join`, `create_dir_all`, or `remove_dir_all`.
//!
//! Entry points covered:
//!
//! - `validate_team_name` itself (the shared guard);
//! - `TeamStore::create` (the create path);
//! - `find_team_dir` (the lookup used by every team tool);
//! - `find_team_dir_cached` (the hot path inside tool `execute()`);
//! - `team_cleanup` (the `remove_dir_all` sink).

use std::sync::Arc;

use ragent_agent::team::store::find_team_dir_cached;
use ragent_agent::team::{TeamStore, find_team_dir, validate_team_name};
use ragent_agent::tool::{TeamContext, ToolContext};
use serde_json::json;

/// Values that must never be accepted as a team name.
const MALICIOUS: &[&str] = &[
    "../..",
    "../../../../tmp/pwned",
    "/etc",
    "a/b",
    "a\\b",
    "..",
    ".",
    "",
    "   ",
    "-leading-hyphen",
    "UPPER",
    "with space",
    "with\0null",
];

fn temp_workdir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::create_dir_all(dir.path().join(".ragent")).expect("project .ragent");
    dir
}

fn ctx(working_dir: &std::path::Path) -> ToolContext {
    ToolContext {
        session_id: "team-name-session".to_string(),
        working_dir: working_dir.to_path_buf(),
        event_bus: Arc::new(ragent_types::event::EventBus::new(64)),
        storage: None,
        agent_manager: None,
        active_model: None,
        provider_registry: None,
        team_context: Some(Arc::new(TeamContext {
            team_name: "t".to_string(),
            agent_id: "lead".to_string(),
            is_lead: true,
        })),
        team_manager: None,
        code_index: None,
        bg_service: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        tool_registry: Arc::new(ragent_agent::tool::create_default_registry()),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        allowed_roots: vec![working_dir.to_path_buf()],
    }
}

// ── The guard itself ────────────────────────────────────────────────────────

#[test]
fn validate_team_name_rejects_traversal_and_absolute_names() {
    for name in MALICIOUS {
        assert!(
            validate_team_name(name).is_err(),
            "{name:?} must be rejected as a team name"
        );
    }
    // A name longer than the 64-character limit is refused too.
    assert!(validate_team_name(&"x".repeat(65)).is_err());
}

#[test]
fn validate_team_name_accepts_ordinary_names() {
    for name in [
        "t",
        "team-a",
        "m8-t3-team",
        "wd-team",
        // Slugified forms the store already produces/accepts.
        "my-team-3",
        &"a".repeat(64),
    ] {
        assert!(
            validate_team_name(name).is_ok(),
            "{name:?} should be an acceptable team name"
        );
    }
}

// ── Create path ─────────────────────────────────────────────────────────────

#[test]
fn team_store_create_rejects_a_traversal_name() {
    let workdir = temp_workdir();
    for name in MALICIOUS {
        let err = TeamStore::create(name, "lead-sess", workdir.path(), true)
            .err()
            .unwrap_or_else(|| panic!("create must refuse the name {name:?}"));
        assert!(
            err.to_string().contains("invalid team name"),
            "unexpected error for {name:?}: {err}"
        );
    }
    // Nothing escaped into a sibling/parent directory.
    assert!(
        !workdir
            .path()
            .parent()
            .expect("parent")
            .join("pwned")
            .exists(),
        "a traversal team name must not create a directory outside the project"
    );
}

// ── Lookup paths ────────────────────────────────────────────────────────────

#[test]
fn find_team_dir_rejects_invalid_names() {
    let workdir = temp_workdir();
    // A real directory named by a valid team so the positive case is meaningful.
    TeamStore::create("real-team", "lead-sess", workdir.path(), true).expect("create real team");
    assert!(
        find_team_dir(workdir.path(), "real-team").is_some(),
        "a valid team name still resolves"
    );
    for name in MALICIOUS {
        assert!(
            find_team_dir(workdir.path(), name).is_none(),
            "{name:?} must not resolve to a team directory"
        );
    }
}

#[test]
fn find_team_dir_cached_rejects_invalid_names() {
    let workdir = temp_workdir();
    let ctx = ctx(workdir.path());
    // Seed the cache with an invalid name so the cache-hit branch is exercised.
    {
        let mut guard = ctx.cached_team_dir.lock().expect("cache lock");
        *guard = Some(("../..".to_string(), workdir.path().to_path_buf()));
    }
    assert!(
        find_team_dir_cached(&ctx, "../..").is_none(),
        "an invalid cached entry must not be served"
    );

    TeamStore::create("cache-team", "lead-sess", workdir.path(), true).expect("create team");
    assert!(
        find_team_dir_cached(&ctx, "cache-team").is_some(),
        "a valid team still resolves through the cache path"
    );
}

// ── Cleanup sink ────────────────────────────────────────────────────────────

#[tokio::test]
async fn team_cleanup_refuses_a_traversal_name() {
    let workdir = temp_workdir();
    // A directory outside `.ragent/teams/` holding a team-ish config.json: if the
    // name were honoured, `remove_dir_all` would delete it.
    let victim = workdir.path().join("victim");
    std::fs::create_dir_all(&victim).expect("victim dir");
    std::fs::write(victim.join("config.json"), "{}").expect("victim config");

    let ctx = ctx(workdir.path());
    let tool = ctx
        .tool_registry
        .get("team_cleanup")
        .expect("team_cleanup registered");

    for name in ["..", "../victim", "victim/..", "/etc"] {
        let err = tool
            .execute(json!({ "team_name": name }), &ctx)
            .await
            .expect_err("cleanup must refuse an invalid team name");
        let msg = format!("{err}");
        assert!(
            msg.contains("not found") || msg.contains("invalid team name"),
            "unexpected refusal for {name:?}: {msg}"
        );
    }
    assert!(
        victim.is_dir(),
        "a traversal team name must not let cleanup delete an unrelated directory"
    );
}
