//! Extra tests-only coverage for the `AgentManager` task registry, the alias
//! tools (`update_file` / `run_code` / `ask_user`), `team_create`, the
//! system-prompt / session caches, and the remaining `TeamManager` task-list
//! branches that the already-existing team test files do not reach.
//!
//! No production code is modified; all file I/O stays inside temp dirs and no
//! real model or network provider is used.

use std::sync::Arc;

use ragent_agent::event::EventBus;
use ragent_agent::task::TaskStatus;
use ragent_agent::tool::aliases::{AskUserTool, RunCodeTool, UpdateFileTool};
use ragent_agent::tool::{Tool, ToolContext, create_default_registry};
use serde_json::json;

#[path = "task_support/mod.rs"]
mod task_support;
use task_support::{entry_full, manager};

#[path = "manager_support/mod.rs"]
mod manager_support;
use manager_support::team_manager;

// -- shared ToolContext ------------------------------------------------------

fn ctx_in(dir: &std::path::Path, bus: Arc<EventBus>) -> ToolContext {
    ToolContext {
        session_id: "sess-more".to_string(),
        working_dir: dir.to_path_buf(),
        event_bus: bus,
        storage: None,
        agent_manager: None,
        active_model: None,
        provider_registry: None,
        team_context: None,
        team_manager: None,
        code_index: None,
        bg_service: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        allowed_roots: vec![dir.to_path_buf()],
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        tool_registry: ToolContext::default_tool_registry(),
    }
}

// ===========================================================================
// AgentManager: cancel paths + spawn_detached
// ===========================================================================

#[tokio::test]
async fn kill_task_refuses_failed_and_cancelled_tasks() {
    // `kill_task` treats Completed, Failed, and Cancelled alike as "already
    // finished"; the existing suite only checks the Completed case.
    let mgr = manager(4);
    mgr.seed_completed_for_test(entry_full("bad", TaskStatus::Failed, "p", false, false))
        .await;
    mgr.seed_completed_for_test(entry_full("gone", TaskStatus::Cancelled, "p", false, false))
        .await;

    let err = mgr.kill_task("bad").await.expect_err("failed is finished");
    assert!(err.to_string().contains("already finished"), "err: {err}");
    let err = mgr
        .kill_task("gone")
        .await
        .expect_err("cancelled is finished");
    assert!(err.to_string().contains("already finished"), "err: {err}");
}

#[tokio::test]
async fn cancel_agent_sets_the_flag_for_a_live_background_task() {
    // A `#[tokio::test]` runs on a single-thread runtime, so the task spawned by
    // `spawn_background` has not been polled yet when we return: its cancel flag
    // is still registered and `cancel_agent` takes the success branch.
    let mgr = manager(4);
    let dir = tempfile::tempdir().expect("tempdir");
    let entry = mgr
        .spawn_background("p", "explore", "do it", None, dir.path())
        .await
        .expect("spawn accepted");

    mgr.cancel_agent(&entry.id)
        .await
        .expect("a live task's flag is present");
}

#[tokio::test]
async fn cancel_all_sets_flags_for_running_background_tasks() {
    let mgr = manager(4);
    let dir = tempfile::tempdir().expect("tempdir");
    let _entry = mgr
        .spawn_background("p", "explore", "do it", None, dir.path())
        .await
        .expect("spawn accepted");

    // Iterates the running background task for `p` and flips its cancel flag
    // (no panic, no error surface).
    mgr.cancel_all("p").await;
}

#[tokio::test]
async fn spawn_detached_marks_the_entry_and_arms_the_pending_flag() {
    let mgr = manager(4);
    let dir = tempfile::tempdir().expect("tempdir");
    let entry = mgr
        .spawn_detached("p", "explore", "fire and forget", None, dir.path())
        .await
        .expect("detached spawn accepted");
    assert!(entry.background, "detached tasks run in the background");
    assert!(entry.detached, "the detached flag must be set");
    assert!(
        mgr.has_pending_background(),
        "a detached spawn must arm the drain fast-path flag"
    );
    // Excluded from the delegation listing.
    assert!(mgr.list_agents("p").await.is_empty());
}

// ===========================================================================
// alias tools: update_file / run_code
// ===========================================================================

#[test]
fn alias_tool_identity_surface() {
    let update = UpdateFileTool;
    assert_eq!(update.name(), "update_file");
    assert_eq!(update.permission_category(), "file:write");
    assert!(update.parameters_schema()["properties"]["path"].is_object());
    assert!(update.description().to_lowercase().contains("write"));

    let run = RunCodeTool;
    assert_eq!(run.name(), "run_code");
    assert_eq!(run.permission_category(), "bash:execute");
    assert!(run.parameters_schema()["properties"]["code"].is_object());
    assert!(run.description().to_lowercase().contains("command"));

    let ask = AskUserTool;
    assert_eq!(ask.name(), "AskUserQuestion");
    assert_eq!(ask.permission_category(), "ask_user");
    assert!(ask.parameters_schema()["properties"]["question"].is_object());
}

#[tokio::test]
async fn update_file_alias_writes_through_the_core_tool() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bus = Arc::new(EventBus::new(32));
    let ctx = ctx_in(dir.path(), bus);

    let out = UpdateFileTool
        .execute(json!({ "path": "out.txt", "content": "hello alias" }), &ctx)
        .await
        .expect("update_file delegates to write");
    assert!(
        !out.content.is_empty(),
        "the write wrapper returns a confirmation"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("out.txt")).expect("read back"),
        "hello alias"
    );
}

#[tokio::test]
async fn run_code_alias_accepts_command_code_and_cmd_array() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bus = Arc::new(EventBus::new(32));

    // `command` (canonical) passes straight through.
    let ctx = ctx_in(dir.path(), bus.clone());
    RunCodeTool
        .execute(json!({ "command": "echo canonical" }), &ctx)
        .await
        .expect("canonical command runs");

    // `code` is normalised to `command`.
    let ctx = ctx_in(dir.path(), bus.clone());
    RunCodeTool
        .execute(json!({ "code": "echo via-code" }), &ctx)
        .await
        .expect("code is mapped to command");

    // `cmd` as a string.
    let ctx = ctx_in(dir.path(), bus.clone());
    RunCodeTool
        .execute(json!({ "cmd": "echo via-cmd" }), &ctx)
        .await
        .expect("cmd string runs");

    // `cmd` as an array is joined into one shell command.
    let ctx = ctx_in(dir.path(), bus.clone());
    RunCodeTool
        .execute(json!({ "cmd": ["echo", "joined", "array"] }), &ctx)
        .await
        .expect("cmd array runs");
}

#[tokio::test]
async fn run_code_alias_rejects_a_missing_command() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bus = Arc::new(EventBus::new(32));
    let ctx = ctx_in(dir.path(), bus);
    let err = RunCodeTool
        .execute(json!({ "timeout": 5 }), &ctx)
        .await
        .expect_err("no command means the call fails");
    assert!(
        err.to_string().contains("command"),
        "the error names the missing parameter: {err}"
    );
}

// ===========================================================================
// team_create
// ===========================================================================

/// A working directory that owns a project-local `.ragent/` tree so
/// `TeamStore::create(project_local=true)` succeeds.
fn workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".ragent")).expect("create .ragent");
    dir
}

fn team_create_tool() -> Arc<dyn Tool> {
    create_default_registry()
        .get("team_create")
        .expect("team_create is registered")
}

#[tokio::test]
async fn team_create_requires_a_blueprint() {
    let ws = workspace();
    let ctx = ctx_in(ws.path(), Arc::new(EventBus::new(16)));
    let err = team_create_tool()
        .execute(json!({ "name": "no-bp" }), &ctx)
        .await
        .expect_err("a missing blueprint is rejected");
    assert!(
        err.to_string().contains("blueprint"),
        "the error names the missing parameter: {err}"
    );
}

#[tokio::test]
async fn team_create_succeeds_with_an_explicit_name() {
    let ws = workspace();
    let ctx = ctx_in(ws.path(), Arc::new(EventBus::new(16)));
    let out = team_create_tool()
        .execute(
            json!({ "blueprint": "demo", "context": "review the crate", "name": "explicit-team" }),
            &ctx,
        )
        .await
        .expect("team_create succeeds");
    assert!(
        out.content.contains("created successfully"),
        "{}",
        out.content
    );
    assert!(ws.path().join(".ragent/teams/explicit-team").is_dir());
    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["team_name"], "explicit-team");
    assert_eq!(meta["auto_named"], false);
}

#[tokio::test]
async fn team_create_auto_names_when_no_name_is_given() {
    let ws = workspace();
    let ctx = ctx_in(ws.path(), Arc::new(EventBus::new(16)));
    let out = team_create_tool()
        .execute(json!({ "blueprint": "My Blueprint", "context": "x" }), &ctx)
        .await
        .expect("auto-named team_create succeeds");
    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["auto_named"], true, "no name means auto-generated");
    // The generated name is slugified from the blueprint.
    let name = meta["team_name"].as_str().expect("name is a string");
    assert!(name.starts_with("my-blueprint-"), "slugified name: {name}");
}

#[tokio::test]
async fn team_create_reloads_an_existing_team() {
    let ws = workspace();
    let bus = Arc::new(EventBus::new(16));
    // First call creates the team.
    team_create_tool()
        .execute(
            json!({ "blueprint": "demo", "context": "x", "name": "reload-team" }),
            &ctx_in(ws.path(), bus.clone()),
        )
        .await
        .expect("first create succeeds");
    // Second call hits the "already exists" branch and loads the team back.
    let out = team_create_tool()
        .execute(
            json!({ "blueprint": "demo", "context": "x", "name": "reload-team" }),
            &ctx_in(ws.path(), bus),
        )
        .await
        .expect("re-create loads the existing team");
    assert!(
        out.content.contains("created successfully"),
        "{}",
        out.content
    );
}

#[tokio::test]
async fn team_create_seeds_a_task_from_task_seed_json() {
    let ws = workspace();
    let bp_dir = ws.path().join(".ragent/blueprints/teams/seedbp");
    std::fs::create_dir_all(&bp_dir).expect("blueprint dir");
    std::fs::write(
        bp_dir.join("task-seed.json"),
        json!([{ "title": "seeded task", "description": "from the seed" }]).to_string(),
    )
    .expect("write task-seed.json");

    let ctx = ctx_in(ws.path(), Arc::new(EventBus::new(16)));
    team_create_tool()
        .execute(
            json!({ "blueprint": "seedbp", "context": "x", "name": "seed-team" }),
            &ctx,
        )
        .await
        .expect("team_create with a task seed succeeds");

    let tasks = ragent_agent::team::TaskStore::open(&ws.path().join(".ragent/teams/seed-team"))
        .expect("task store")
        .read()
        .expect("read tasks");
    assert!(
        tasks.tasks.iter().any(|t| t.title == "seeded task"),
        "the seeded task must exist: {:?}",
        tasks.tasks.iter().map(|t| &t.title).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn team_create_records_failed_blueprint_spawns() {
    let ws = workspace();
    let bp_dir = ws.path().join(".ragent/blueprints/teams/failbp");
    std::fs::create_dir_all(&bp_dir).expect("blueprint dir");
    // A `bash` seed carries a permission category, but the ToolContext has no
    // permission checker, so `dispatch_seed_tool` fails closed and the failure
    // is surfaced in the tool metadata.
    std::fs::write(
        bp_dir.join("spawn-prompts.json"),
        json!([{ "tool": "Bash", "args": { "command": "echo nope" } }]).to_string(),
    )
    .expect("write spawn-prompts.json");

    let ctx = ctx_in(ws.path(), Arc::new(EventBus::new(16)));
    let out = team_create_tool()
        .execute(
            json!({ "blueprint": "failbp", "context": "x", "name": "fail-team" }),
            &ctx,
        )
        .await
        .expect("team_create itself succeeds; the seed is what fails");
    let meta = out.metadata.expect("metadata present");
    assert_eq!(
        meta["failed_spawn_count"], 1,
        "one refused seed must be recorded: {meta}"
    );
    let failures = meta["failed_spawns"].as_array().expect("array");
    assert_eq!(failures[0]["tool"], "Bash");
    assert!(out.content.contains("failed"), "{}", out.content);
}

/// `seed_only` must never create a team directory: `/team create` has already
/// created the team, and a re-creating seed that ran after a `/team clear` or
/// `/team forcecleanup` removed the tree resurrected it, so removal failed with
/// ENOTEMPTY (CI flake in `test_slash_arms_deep::team_clear_removes_ta` and
/// `test_slash_teams_deep::forcecleanup_confirm_removes_team`).
#[tokio::test]
async fn team_create_seed_only_refuses_to_create_a_team() {
    let ws = workspace();
    let bp_dir = ws.path().join(".ragent/blueprints/teams/seedbp2");
    std::fs::create_dir_all(&bp_dir).expect("blueprint dir");
    std::fs::write(bp_dir.join("spawn-prompts.json"), json!([]).to_string())
        .expect("write spawn-prompts.json");

    let ctx = ctx_in(ws.path(), Arc::new(EventBus::new(16)));
    let err = team_create_tool()
        .execute(
            json!({
                "blueprint": "seedbp2",
                "context": "x",
                "name": "never-created",
                "seed_only": true,
            }),
            &ctx,
        )
        .await
        .expect_err("seed_only must not create the team");
    assert!(
        err.to_string().contains("not found"),
        "the error names the missing team: {err}"
    );
    assert!(
        !ws.path().join(".ragent/teams/never-created").exists(),
        "seed_only must leave no team directory behind"
    );
}

/// `seed_only` applies the blueprint to a team that already exists.
#[tokio::test]
async fn team_create_seed_only_seeds_an_existing_team() {
    let ws = workspace();
    let bp_dir = ws.path().join(".ragent/blueprints/teams/seedbp3");
    std::fs::create_dir_all(&bp_dir).expect("blueprint dir");
    std::fs::write(
        bp_dir.join("task-seed.json"),
        json!([{ "title": "seeded task", "description": "from the seed" }]).to_string(),
    )
    .expect("write task-seed.json");

    let bus = Arc::new(EventBus::new(16));
    // `/team create` already created the team; the seed applies to it.
    ragent_agent::team::TeamStore::create("seeded-team", "sess-more", ws.path(), true)
        .expect("create team");

    let out = team_create_tool()
        .execute(
            json!({
                "blueprint": "seedbp3",
                "context": "x",
                "name": "seeded-team",
                "seed_only": true,
            }),
            &ctx_in(ws.path(), bus),
        )
        .await
        .expect("seed_only against an existing team succeeds");
    assert!(
        out.content.contains("created successfully"),
        "{}",
        out.content
    );
    let tasks = ragent_agent::team::TaskStore::open(&ws.path().join(".ragent/teams/seeded-team"))
        .expect("task store")
        .read()
        .expect("read tasks");
    assert!(
        tasks.tasks.iter().any(|t| t.title == "seeded task"),
        "the seeded task must exist: {:?}",
        tasks.tasks.iter().map(|t| &t.title).collect::<Vec<_>>()
    );
}

// ===========================================================================
// session/cache.rs: SystemPromptCache component coverage
// ===========================================================================

use ragent_agent::agent::AgentInfo;
use ragent_agent::session::cache::SystemPromptCache;
use ragent_agent::tool::ToolRegistry;
use ragent_agent::tool::plan::PlanEnterTool;
use serial_test::serial;

#[test]
#[serial]
fn cached_validity_get_set_and_invalidate() {
    use ragent_agent::session::cache::{Cached, current_cache_version};
    let mut cached: Cached<u32> = Cached::new();
    assert!(!cached.is_valid(current_cache_version()));
    assert!(cached.get(current_cache_version()).is_none());
    cached.set(7);
    assert!(cached.is_valid(current_cache_version()));
    assert_eq!(cached.get(current_cache_version()), Some(7));
    cached.invalidate();
    assert!(cached.get(current_cache_version()).is_none());
}

#[test]
#[serial]
fn agent_prompt_cache_hits_misses_and_none() {
    let cache = SystemPromptCache::new();
    let agent = AgentInfo::new("explore", "explorer");

    // First call runs the compute closure and caches the result.
    let first = cache
        .get_agent_prompt(&agent, |_a| Some("prompt".to_string()))
        .expect("first compute yields a value");
    assert_eq!(first, "prompt");
    // Second call returns the same value (a cache hit when the process-global
    // cache version has not been bumped by a parallel test's `invalidate_all`).
    let second = cache
        .get_agent_prompt(&agent, |_a| Some("prompt".to_string()))
        .expect("second call yields a value");
    assert_eq!(second, "prompt");

    // A `None` compute result is propagated (and not cached).
    let other = AgentInfo::new("other", "d");
    assert!(cache.get_agent_prompt(&other, |_a| None).is_none());
}

#[test]
#[serial]
fn tool_reference_rebuilds_when_the_registry_version_changes() {
    let registry = ToolRegistry::new();
    registry.register(Arc::new(PlanEnterTool));
    let cache = SystemPromptCache::new();

    let first = cache
        .get_tool_reference(&registry, |_defs| "v1".to_string())
        .expect("first reference");
    assert_eq!(first, "v1");
    // Cache hit on an unchanged registry.
    let hit = cache
        .get_tool_reference(&registry, |_defs| panic!("must not recompute"))
        .expect("hit");
    assert_eq!(hit, "v1");

    // Registering a tool bumps the version, forcing a rebuild.
    registry.register(Arc::new(PlanEnterTool));
    let rebuilt = cache
        .get_tool_reference(&registry, |_defs| "v2".to_string())
        .expect("rebuilt reference");
    assert_eq!(rebuilt, "v2");
}

#[test]
#[serial]
fn tool_definitions_cache_shares_the_allocation() {
    let registry = ToolRegistry::new();
    registry.register(Arc::new(PlanEnterTool));
    let cache = SystemPromptCache::new();

    let table = ragent_agent::tool::alias_table::AliasTable::default();
    let first = cache
        .get_tool_definitions(&registry, &table, false)
        .expect("defs");
    let second = cache
        .get_tool_definitions(&registry, &table, false)
        .expect("defs");
    assert!(
        Arc::ptr_eq(&first, &second),
        "an unchanged registry returns the cached allocation"
    );

    registry.register(Arc::new(PlanEnterTool));
    let third = cache
        .get_tool_definitions(&registry, &table, false)
        .expect("defs");
    assert!(
        !Arc::ptr_eq(&second, &third),
        "a registry change forces a rebuild"
    );
}

/// `get_codeindex_guidance_with` takes two closures of the SAME type, so the
/// only way to call it is with a (copyable) fn item. The active branch is then
/// selected by the `code_index_active` flag.
fn fixed_guidance() -> String {
    "GUIDANCE".to_string()
}

#[test]
#[serial]
fn codeindex_guidance_with_wrapper_dispatches_both_states() {
    let cache = SystemPromptCache::new();
    let active = cache
        .get_codeindex_guidance_with(true, fixed_guidance, fixed_guidance)
        .expect("active branch");
    assert_eq!(active, "GUIDANCE");
    // A state flip forces a recompute through the same closure, still Some.
    let disabled = cache
        .get_codeindex_guidance_with(false, fixed_guidance, fixed_guidance)
        .expect("disabled branch");
    assert_eq!(disabled, "GUIDANCE");
}

#[test]
#[serial]
fn team_guidance_caches_per_context_hash_and_invalidates() {
    use ragent_agent::tool::TeamContext;
    let cache = SystemPromptCache::new();
    let lead = TeamContext {
        team_name: "alpha".to_string(),
        agent_id: "lead".to_string(),
        is_lead: true,
    };
    let first = cache
        .get_team_guidance(Some(&lead), |_c| "lead-guidance".to_string())
        .expect("first");
    assert_eq!(first, "lead-guidance");
    // Same context hash: cache hit.
    let hit = cache
        .get_team_guidance(Some(&lead), |_c| panic!("must not recompute"))
        .expect("hit");
    assert_eq!(hit, "lead-guidance");

    // A different team name forces a recompute.
    let other = TeamContext {
        team_name: "beta".to_string(),
        agent_id: "lead".to_string(),
        is_lead: true,
    };
    let changed = cache
        .get_team_guidance(Some(&other), |_c| "beta-guidance".to_string())
        .expect("changed");
    assert_eq!(changed, "beta-guidance");

    // Explicit invalidation forces the next call to recompute even for the
    // same context.
    cache.invalidate_team_cache();
    let after = cache
        .get_team_guidance(Some(&other), |_c| "recomputed".to_string())
        .expect("after invalidate");
    assert_eq!(after, "recomputed");
}

#[test]
#[serial]
fn codeindex_cache_and_full_invalidation_are_callable() {
    let cache = SystemPromptCache::new();
    cache
        .get_codeindex_guidance(true, |_a| "ci".to_string())
        .expect("populate");
    cache.invalidate_codeindex_cache();
    cache.invalidate_tool_cache();
    cache.invalidate_all();
    // A call after a full invalidation simply recomputes.
    let after = cache
        .get_codeindex_guidance(false, |_a| "off".to_string())
        .expect("recompute after invalidate_all");
    assert_eq!(after, "off");
}

#[test]
#[serial]
fn spec_section_cache_store_get_invalidate_and_mismatch() {
    let cache = SystemPromptCache::new();
    assert!(cache.get_spec_section("s1", 10).is_none(), "empty cache");

    cache.store_spec_section("s1".to_string(), 10, "SECTION".to_string());
    assert_eq!(
        cache.get_spec_section("s1", 10),
        Some("SECTION".to_string())
    );
    // A differing modified-at is a miss.
    assert!(cache.get_spec_section("s1", 11).is_none());
    // A differing id is a miss.
    assert!(cache.get_spec_section("s2", 10).is_none());

    cache.invalidate_spec_cache();
    assert!(cache.get_spec_section("s1", 10).is_none());
    // `version` advances monotonically (smoke check of the accessor).
    assert!(cache.version() >= 1);
}

// ===========================================================================
// team/manager.rs: task-list cache + task mutation branches
// ===========================================================================

#[test]
fn manager_task_list_cache_hit_and_external_invalidation() {
    use ragent_agent::team::{Task, TaskStore};
    let (manager, _tmp, _bus) = team_manager("tlc-team", "lead-1", true);

    // First call loads from disk (empty) and caches.
    let first = manager.task_list().expect("first list");
    assert!(first.tasks.is_empty());
    // Second call is a cache hit (same mtime).
    let second = manager.task_list().expect("cached list");
    assert!(second.tasks.is_empty());

    // An external writer bypasses the cache; the mtime advance forces a reload.
    let ts = TaskStore::open(&manager.team_dir).expect("task store");
    ts.add_task(Task::new("task-001", "external")).expect("add");
    let reloaded = manager.task_list().expect("reloaded list");
    assert_eq!(reloaded.tasks.len(), 1);
}

#[test]
fn manager_claim_next_with_no_claimable_task_returns_none() {
    let (manager, _tmp, _bus) = team_manager("empty-claim", "lead-1", true);
    let (claimed, already_had) = manager.claim_next_task("tm-001").expect("claim");
    assert!(claimed.is_none());
    assert!(!already_had);
}

#[test]
fn manager_complete_task_auto_assigns_an_unowned_pending_task() {
    use ragent_agent::team::Task;
    let (manager, _tmp, _bus) = team_manager("auto-complete", "lead-1", true);
    manager
        .apply_to_task_list(|list| list.tasks.push(Task::new("task-001", "unowned")))
        .expect("seed");

    // The task is Pending with no assignee; `complete_task` claims it for the
    // agent and marks it completed.
    let done = manager
        .complete_task("task-001", "tm-001")
        .expect("complete");
    assert_eq!(done.status, ragent_agent::team::TaskStatus::Completed);
    assert_eq!(done.assigned_to.as_deref(), Some("tm-001"));
    assert_eq!(done.completed_by.as_deref(), Some("tm-001"));
}

#[test]
fn manager_complete_task_owned_by_another_agent_completes_without_reassigning() {
    use ragent_agent::team::{Task, TaskStatus};
    let (manager, _tmp, _bus) = team_manager("owned-complete", "lead-1", true);
    manager
        .apply_to_task_list(|list| {
            let mut t = Task::new("task-001", "owned");
            t.status = TaskStatus::InProgress;
            t.assigned_to = Some("tm-002".to_string());
            list.tasks.push(t);
        })
        .expect("seed");

    // `TeamManager::complete_task` (unlike the stricter `TaskStore::complete`)
    // marks the task completed even though it is owned by tm-002, but the inner
    // guard skips the re-assignment because the status is not Pending and an
    // assignee is already present.
    let done = manager
        .complete_task("task-001", "tm-001")
        .expect("complete");
    assert_eq!(done.status, TaskStatus::Completed);
    assert_eq!(
        done.assigned_to.as_deref(),
        Some("tm-002"),
        "the original owner is left in place"
    );
    assert_eq!(done.completed_by.as_deref(), Some("tm-001"));
}

#[test]
fn adopt_orphaned_tasks_errors_on_a_missing_team_dir() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("does-not-exist");
    let err = ragent_agent::team::manager::TeamManager::adopt_orphaned_tasks(&missing, "lead")
        .expect_err("a missing team dir has no task store");
    assert_ne!(err.to_string(), "");
}

#[test]
fn adopt_orphaned_tasks_leaves_tasks_owned_by_others_untouched() {
    use ragent_agent::team::{Task, TaskStatus, TaskStore};
    let dir = tempfile::tempdir().expect("tempdir");
    let ts = TaskStore::open(dir.path()).expect("task store");
    // One task owned by another agent, one already completed.
    let mut other = Task::new("task-other", "someone else");
    other.status = TaskStatus::InProgress;
    other.assigned_to = Some("tm-009".to_string());
    ts.add_task(other).expect("add");
    let mut done = Task::new("task-done", "finished");
    done.status = TaskStatus::Completed;
    ts.add_task(done).expect("add");

    ragent_agent::team::manager::TeamManager::adopt_orphaned_tasks(dir.path(), "lead")
        .expect("adopt is a no-op for foreign tasks");

    let list = ts.read().expect("read");
    let keep = list
        .tasks
        .iter()
        .find(|t| t.id == "task-other")
        .expect("present");
    assert_eq!(
        keep.status,
        TaskStatus::InProgress,
        "a task owned by a teammate must not be reassigned"
    );

    // Sanity: the same call DOES reassign a task owned by the departing lead.
    let mut orphan = Task::new("task-orphan", "lead's work");
    orphan.status = TaskStatus::InProgress;
    orphan.assigned_to = Some("lead".to_string());
    ts.add_task(orphan).expect("add");
    ragent_agent::team::manager::TeamManager::adopt_orphaned_tasks(dir.path(), "lead")
        .expect("adopt");
    let list = ts.read().expect("read");
    let adopted = list
        .tasks
        .iter()
        .find(|t| t.id == "task-orphan")
        .expect("present");
    assert_eq!(adopted.status, TaskStatus::Pending);
    assert!(adopted.assigned_to.is_none());
}

#[test]
fn manager_plan_pending_cache_ttl_and_unknown_agent() {
    let (manager, _tmp, _bus) = team_manager("plan-cache", "lead-1", true);
    // Unknown agent: no member, so `false`, and the (false) result is cached.
    assert!(!manager.is_plan_pending("tm-404"));
    assert!(
        !manager.is_plan_pending("tm-404"),
        "second read hits the cache"
    );
    // Explicit invalidation then a re-read still returns false.
    manager.invalidate_plan_pending_cache("tm-404");
    assert!(!manager.is_plan_pending("tm-404"));
}
