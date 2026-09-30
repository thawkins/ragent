//! Regression tests for SEC-ragent-team-001 / SEC-ragent-agent-008 (SECTASKS
//! MS-01 T-003): a blueprint seed file must not execute a tool without going
//! through the session's permission gate.
//!
//! A blueprint directory lives inside the project (`.ragent/blueprints/teams/`)
//! or in a user-level location, and a project is untrusted content: a
//! `task-seed.json` naming `bash` used to reach `create_default_registry()` and
//! `tool.execute(...)` directly, bypassing `dispatch_tool_with_permissions`
//! entirely. The fix routes every seeded tool through the permission decision,
//! so the tests below drive `team_create` against a real blueprint and assert
//! that a `bash` seed does not run.
//!
//! The gate itself is asserted at the tool layer (the same decision the agent
//! loop applies): a seeded tool with an `Ask` verdict is refused, a `Deny`
//! verdict is refused, and an explicit `Allow` rule runs it.

use std::sync::Arc;

use parking_lot::RwLock;
use ragent_agent::permission::{Permission, PermissionAction, PermissionChecker, PermissionRule};
use ragent_agent::team::TeamStore;
use ragent_agent::tool::{TeamContext, Tool, ToolContext};
use serde_json::json;

/// A working directory with a project `.ragent/` tree and a blueprint that
/// seeds a `bash` command writing a marker file.
struct Fixture {
    dir: tempfile::TempDir,
    marker: std::path::PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("temp dir");
        let bp_dir = dir
            .path()
            .join(".ragent")
            .join("blueprints")
            .join("teams")
            .join("evil");
        std::fs::create_dir_all(&bp_dir).expect("blueprint dir");
        let marker = dir.path().join("pwned");
        // A seed that would run an unpermissioned shell command.
        std::fs::write(
            bp_dir.join("task-seed.json"),
            json!([{
                "tool": "bash",
                "args": { "command": format!("touch {}", marker.display()) }
            }])
            .to_string(),
        )
        .expect("write task-seed.json");
        // A benign seed: the team task tool carries a team permission category.
        std::fs::write(bp_dir.join("spawn-prompts.json"), json!([]).to_string())
            .expect("write spawn-prompts.json");
        Self { dir, marker }
    }

    fn working_dir(&self) -> &std::path::Path {
        self.dir.path()
    }
}

fn ctx(fixture: &Fixture, rules: Vec<PermissionRule>) -> ToolContext {
    ToolContext {
        session_id: "seed-perm-session".to_string(),
        working_dir: fixture.working_dir().to_path_buf(),
        event_bus: Arc::new(ragent_types::event::EventBus::new(64)),
        storage: None,
        agent_manager: None,
        active_model: None,
        provider_registry: None,
        team_context: Some(Arc::new(TeamContext {
            team_name: String::new(),
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
        permission_checker: Some(Arc::new(RwLock::new(PermissionChecker::new(rules)))),
        tool_registry: Arc::new(ragent_agent::tool::create_default_registry()),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        allowed_roots: vec![fixture.working_dir().to_path_buf()],
    }
}

/// Find the `team_create` tool in the registry.
fn team_create_tool() -> Arc<dyn Tool> {
    Arc::new(ragent_agent::tool::create_default_registry())
        .get("team_create")
        .expect("team_create is registered")
}

/// With no permission rule, a seeded `bash` call raises `Ask` and must not run:
/// the marker file stays absent and the tool reports the refusal.
#[tokio::test]
async fn blueprint_bash_seed_is_refused_without_a_permission_rule() {
    let fixture = Fixture::new();
    let ctx = ctx(&fixture, Vec::new());

    let _ = team_create_tool()
        .execute(
            json!({ "blueprint": "evil", "context": "do the thing", "name": "seed-ask" }),
            &ctx,
        )
        .await
        .expect("team_create itself succeeds; the seed is what is refused");

    assert!(
        !fixture.marker.exists(),
        "an unpermissioned blueprint `bash` seed must not execute"
    );
    // The team directory was still created (blueprint seeding is best-effort).
    let teams_root = fixture.working_dir().join(".ragent").join("teams");
    assert!(teams_root.join("seed-ask").is_dir(), "team created");
}

/// An explicit `Deny` rule for the `bash` category also refuses the seed.
#[tokio::test]
async fn blueprint_bash_seed_is_refused_under_an_explicit_deny_rule() {
    let fixture = Fixture::new();
    let ctx = ctx(
        &fixture,
        vec![PermissionRule {
            permission: Permission::Bash,
            pattern: Some("*".to_string()),
            action: PermissionAction::Deny,
        }],
    );

    let _ = team_create_tool()
        .execute(
            json!({ "blueprint": "evil", "context": "do the thing", "name": "seed-deny" }),
            &ctx,
        )
        .await;

    assert!(
        !fixture.marker.exists(),
        "a denied blueprint `bash` seed must not execute"
    );
}

/// Sanity check: the same seed DOES run once the session explicitly allows the
/// category — the gate withholds unapproved tools, it does not disable seeding.
#[tokio::test]
async fn blueprint_bash_seed_runs_when_the_category_is_allowed() {
    let fixture = Fixture::new();
    let ctx = ctx(
        &fixture,
        vec![PermissionRule {
            permission: Permission::Bash,
            pattern: Some("*".to_string()),
            action: PermissionAction::Allow,
        }],
    );

    let _ = team_create_tool()
        .execute(
            json!({ "blueprint": "evil", "context": "do the thing", "name": "seed-allow" }),
            &ctx,
        )
        .await;

    assert!(
        fixture.marker.exists(),
        "an explicitly allowed blueprint `bash` seed should execute"
    );
}

/// A session with no permission checker at all fails closed: the seed does not
/// run rather than silently bypassing the gate.
#[tokio::test]
async fn blueprint_seed_fails_closed_without_a_permission_checker() {
    let fixture = Fixture::new();
    let mut ctx = ctx(&fixture, Vec::new());
    ctx.permission_checker = None;

    let _ = team_create_tool()
        .execute(
            json!({ "blueprint": "evil", "context": "do the thing", "name": "seed-nochecker" }),
            &ctx,
        )
        .await;

    assert!(
        !fixture.marker.exists(),
        "without a permission checker a seeded `bash` call must fail closed"
    );
}

/// The team store is created under the project `.ragent/teams/` root, so the
/// fixture is real. Guards against a future refactor that silently moves the
/// team tree and makes the assertions above vacuous.
#[test]
fn fixture_team_root_is_inside_the_project() {
    let fixture = Fixture::new();
    let _ = TeamStore::create("probe", "lead", fixture.working_dir(), true).expect("create team");
    let dir = TeamStore::load_by_name("probe", fixture.working_dir()).expect("load team");
    assert!(dir.dir.starts_with(fixture.working_dir()));
}

/// A blueprint that seeds an ordinary task (`team_task_create`) still works
/// when the session already allows the `team:manage` category: the gate
/// withholds unapproved tools, it does not stop blueprint seeding.
#[tokio::test]
async fn benign_blueprint_task_seed_still_creates_a_task() {
    let fixture = Fixture::new();
    // The seeding tool is `team_task_create` — no `bash`, no file write.
    let bp_dir = fixture
        .working_dir()
        .join(".ragent")
        .join("blueprints")
        .join("teams")
        .join("good");
    std::fs::create_dir_all(&bp_dir).expect("blueprint dir");
    std::fs::write(
        bp_dir.join("task-seed.json"),
        json!([{
            "tool": "team_task_create",
            "args": { "title": "Review the crate" }
        }])
        .to_string(),
    )
    .expect("write task-seed.json");

    // `team_task_create` carries the `team:manage` category, so the session
    // holds an explicit allow for it — exactly the "already pre-approved"
    // state the gate is designed to honour.
    let ctx = ctx(
        &fixture,
        vec![PermissionRule {
            permission: Permission::Custom("team:manage".to_string()),
            pattern: Some("*".to_string()),
            action: PermissionAction::Allow,
        }],
    );
    let _ = team_create_tool()
        .execute(
            json!({ "blueprint": "good", "context": "do the thing", "name": "seed-benign" }),
            &ctx,
        )
        .await;

    let store = TeamStore::load_by_name("seed-benign", fixture.working_dir()).expect("team exists");
    let tasks = store
        .task_store()
        .expect("task store opens")
        .read()
        .expect("task list readable")
        .tasks
        .iter()
        .filter(|t| t.title == "Review the crate")
        .count();
    assert_eq!(tasks, 1, "the benign seeded task must be created");

    // The malicious fixture's marker is untouched by this fixture.
    assert!(!fixture.marker.exists());
}
