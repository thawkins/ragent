//! Every session is created in a project and attached to it (spec
//! `projects`, T-005).

#[path = "support/project_fixture.rs"]
mod project_fixture;

use std::path::PathBuf;

use project_fixture::managers;
use ragent_storage::{SessionCheckout, SessionKind};

#[test]
fn test_session_project_attach_every_creation_path() {
    let m = managers();
    let dir = PathBuf::from("/nonexistent/attach/app");

    let interactive = m.sessions.create_session(dir.clone()).expect("interactive");
    let automation = m
        .sessions
        .create_session_of_kind(dir.clone(), SessionKind::Automation)
        .expect("automation");
    let project = m
        .projects
        .get(&interactive.project_id)
        .expect("get")
        .expect("the session reports the id of a stored project");
    assert_eq!(project.worktree, dir);
    assert_eq!(automation.project_id, project.id);

    // Children belong to the parent's project even from another directory.
    let elsewhere = PathBuf::from("/nonexistent/attach/elsewhere");
    for kind in [
        SessionKind::Subagent,
        SessionKind::Teammate,
        SessionKind::Skill,
    ] {
        let child = m
            .sessions
            .create_child_session(elsewhere.clone(), &interactive.id, kind)
            .expect("child");
        assert_eq!(child.project_id, project.id, "{kind:?}");
        let stored = m
            .sessions
            .get_session(&child.id)
            .expect("get")
            .expect("row");
        assert_eq!(stored.project_id, project.id, "{kind:?} as stored");
    }
    assert_eq!(
        m.projects.list().expect("list").len(),
        1,
        "no creation path made a second project"
    );
}

#[test]
fn test_session_project_attach_explicit_project_and_checkout() {
    let m = managers();
    let project = m
        .projects
        .resolve_for_dir(std::path::Path::new("/nonexistent/attach/explicit"))
        .expect("project");
    let checkout = SessionCheckout {
        branch: Some("feature/login".to_string()),
        worktree: Some("/nonexistent/trees/feature-login".to_string()),
        pr_count: None,
    };

    let session = m
        .sessions
        .create_session_in_project(
            &project.id,
            PathBuf::from("/nonexistent/trees/feature-login"),
            &checkout,
        )
        .expect("create");
    assert_eq!(session.project_id, project.id);
    assert_eq!(session.branch.as_deref(), Some("feature/login"));

    let loaded = m
        .sessions
        .get_session(&session.id)
        .expect("get")
        .expect("row");
    assert_eq!(loaded.branch.as_deref(), Some("feature/login"));
    assert_eq!(
        loaded.worktree,
        Some(PathBuf::from("/nonexistent/trees/feature-login"))
    );
    let listed = m.sessions.list_sessions().expect("list");
    let listed = listed.iter().find(|s| s.id == session.id).expect("listed");
    assert_eq!(listed.branch.as_deref(), Some("feature/login"));

    assert!(
        m.sessions
            .create_session_in_project("missing", PathBuf::from("/x"), &SessionCheckout::default())
            .is_err()
    );
}
