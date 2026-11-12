//! Project records, session checkout data and the grouped listing (spec
//! `projects`, T-002).

use ragent_storage::storage::Storage;
use ragent_storage::{ProjectRemoteRow, SessionCheckout, SessionKind};

fn remote() -> ProjectRemoteRow {
    ProjectRemoteRow {
        kind: "github".to_string(),
        host: "github.com".to_string(),
        path: "acme/app".to_string(),
        url: "https://github.com/acme/app.git".to_string(),
        remote_name: "origin".to_string(),
        default_branch: Some("main".to_string()),
    }
}

#[test]
fn test_projects_crud_ensure_is_get_or_create() {
    let storage = Storage::open_in_memory().expect("storage");
    let first = storage.ensure_project("/work/app").expect("create");
    let second = storage.ensure_project("/work/app").expect("get");
    assert_eq!(first.id, second.id, "a worktree has exactly one project");
    assert_eq!(first.name, "app");
    assert_eq!(storage.list_projects().expect("list").len(), 1);
    assert_eq!(
        storage
            .get_project_by_worktree("/work/app")
            .expect("by worktree")
            .map(|p| p.id),
        Some(first.id)
    );
}

#[test]
fn test_projects_crud_round_trips_name_model_and_remote() {
    let storage = Storage::open_in_memory().expect("storage");
    let project = storage.ensure_project("/work/app").expect("create");

    storage
        .rename_project(&project.id, "  Storefront ")
        .expect("rename");
    storage
        .set_project_base_model(&project.id, Some(("anthropic", "claude-sonnet")))
        .expect("model");
    storage
        .set_project_remote(&project.id, Some(&remote()))
        .expect("remote");

    let stored = storage.get_project(&project.id).expect("get").expect("row");
    assert_eq!(stored.name, "Storefront");
    assert_eq!(stored.provider_id.as_deref(), Some("anthropic"));
    assert_eq!(stored.model_id.as_deref(), Some("claude-sonnet"));
    assert_eq!(stored.remote, Some(remote()));
    assert!(stored.remote_checked);

    storage
        .set_project_base_model(&project.id, None)
        .expect("clear model");
    storage
        .set_project_remote(&project.id, None)
        .expect("clear remote");
    let cleared = storage.get_project(&project.id).expect("get").expect("row");
    assert_eq!(cleared.provider_id, None);
    assert_eq!(cleared.model_id, None);
    assert_eq!(cleared.remote, None);
    assert!(
        cleared.remote_checked,
        "a project with no remote stays checked"
    );

    assert!(storage.rename_project(&project.id, "   ").is_err());
}

#[test]
fn test_projects_crud_find_by_name_id_and_prefix() {
    let storage = Storage::open_in_memory().expect("storage");
    let app = storage.ensure_project("/work/app").expect("app");
    let twin = storage.ensure_project("/other/app").expect("twin");
    let tools = storage.ensure_project("/work/tools").expect("tools");

    let by_name = storage.find_project("TOOLS").expect("find");
    assert_eq!(by_name.map(|p| p.id), Some(tools.id.clone()));
    let by_id = storage.find_project(&twin.id).expect("find");
    assert_eq!(by_id.map(|p| p.id), Some(twin.id));
    let by_prefix = storage.find_project(&app.id[..12]).expect("find");
    assert_eq!(by_prefix.map(|p| p.id), Some(app.id));
    assert!(
        storage.find_project("app").is_err(),
        "an ambiguous name is an error, not a guess"
    );
    assert_eq!(storage.find_project("nothing").expect("find"), None);
}

#[test]
fn test_projects_crud_sessions_attach_and_children_inherit() {
    let storage = Storage::open_in_memory().expect("storage");
    storage
        .create_session("lead", "/nonexistent/one")
        .expect("lead");
    let lead = storage.get_session("lead").expect("get").expect("row");
    let project = storage
        .get_project(&lead.project_id)
        .expect("get")
        .expect("project created with the session");
    assert_eq!(project.worktree, "/nonexistent/one");

    // A child working in another directory still belongs to its parent's
    // project.
    storage
        .create_session_with(
            "sub",
            "/nonexistent/two",
            Some("lead"),
            SessionKind::Subagent,
        )
        .expect("sub");
    let sub = storage.get_session("sub").expect("get").expect("row");
    assert_eq!(sub.project_id, project.id);
    assert_eq!(
        storage.list_projects().expect("list").len(),
        1,
        "the child did not create a project"
    );
}

#[test]
fn test_projects_crud_session_checkout_round_trip() {
    let storage = Storage::open_in_memory().expect("storage");
    let project = storage.ensure_project("/nonexistent/app").expect("project");
    let checkout = SessionCheckout {
        branch: Some("feature/x".to_string()),
        worktree: Some("/nonexistent/trees/feature-x".to_string()),
        pr_count: None,
    };
    storage
        .create_session_in_project("s1", &project.id, "/nonexistent/trees/feature-x", &checkout)
        .expect("create");
    assert_eq!(storage.session_checkout("s1").expect("get"), Some(checkout));
    assert_eq!(
        storage
            .get_session("s1")
            .expect("get")
            .expect("row")
            .project_id,
        project.id
    );
    assert!(
        storage
            .create_session_in_project("s2", "no-such-project", "/x", &SessionCheckout::default())
            .is_err()
    );

    // A later session created inside the session's worktree belongs to the
    // owning project, not to a project of its own.
    storage
        .create_session("s3", "/nonexistent/trees/feature-x")
        .expect("s3");
    assert_eq!(
        storage
            .get_session("s3")
            .expect("get")
            .expect("row")
            .project_id,
        project.id
    );

    assert_eq!(
        storage
            .set_branch_pr_count(&project.id, "feature/x", Some(3))
            .expect("count"),
        1
    );
    assert_eq!(
        storage
            .set_branch_pr_count(&project.id, "feature/x", Some(3))
            .expect("count"),
        0,
        "an unchanged count reports no change"
    );
    assert_eq!(
        storage.session_checkouts().expect("all")["s1"].pr_count,
        Some(3)
    );
    assert_eq!(
        storage
            .project_session_branches(&project.id)
            .expect("branches"),
        vec!["feature/x".to_string()]
    );

    storage
        .set_session_checkout("s1", None, None)
        .expect("clear");
    assert_eq!(
        storage
            .session_checkout("s1")
            .expect("get")
            .expect("row")
            .branch,
        None
    );
    assert_eq!(storage.session_checkout("missing").expect("get"), None);
}

#[test]
fn test_projects_crud_grouped_list_includes_empty_projects() {
    let storage = Storage::open_in_memory().expect("storage");
    let empty = storage.ensure_project("/nonexistent/empty").expect("empty");
    storage.create_session("s", "/nonexistent/busy").expect("s");

    let groups = storage.list_sessions_grouped().expect("grouped");
    let shape: Vec<(&str, usize)> = groups
        .iter()
        .map(|g| (g.project.name.as_str(), g.sessions.len()))
        .collect();
    assert_eq!(
        shape,
        vec![("busy", 1), ("empty", 0)],
        "projects with sessions first, then projects with none"
    );
    assert_eq!(groups[1].project_id, empty.id);
}

#[test]
fn test_projects_crud_archive_refuses_live_sessions_and_ensure_restores() {
    let storage = Storage::open_in_memory().expect("storage");
    storage.create_session("s", "/nonexistent/app").expect("s");
    let project = storage
        .get_project_by_worktree("/nonexistent/app")
        .expect("get")
        .expect("project");

    assert!(
        storage.archive_project(&project.id).is_err(),
        "a project with a live session cannot be removed"
    );
    storage.archive_session("s").expect("archive session");
    storage
        .archive_project(&project.id)
        .expect("archive project");
    assert!(storage.list_projects().expect("list").is_empty());
    assert!(storage.list_sessions_grouped().expect("grouped").is_empty());
    assert_eq!(
        storage
            .list_project_sessions(&project.id, true)
            .expect("archived sessions")
            .len(),
        1
    );

    // Creating a session in the worktree again brings the same project back.
    storage
        .create_session("s2", "/nonexistent/app")
        .expect("s2");
    let listed = storage.list_projects().expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, project.id);
}

/// The grouped listing still uses the partial project index for a
/// per-project query (the plan the sidebar's per-project listing relies on).
#[test]
fn test_projects_crud_project_sessions_use_the_project_index() {
    let storage = Storage::open_in_memory().expect("storage");
    let guard = storage.conn_lock_for_test().expect("lock");
    let plan: Vec<String> = guard
        .prepare(
            "EXPLAIN QUERY PLAN SELECT id FROM sessions \
             WHERE project_id = 'p' AND archived_at IS NULL ORDER BY updated_at DESC",
        )
        .expect("prepare")
        .query_map([], |r| r.get::<_, String>(3))
        .expect("query")
        .collect::<Result<_, _>>()
        .expect("rows");
    assert!(
        plan.iter()
            .any(|l| l.contains("idx_sessions_project_updated")),
        "plan was: {plan:?}"
    );
}
