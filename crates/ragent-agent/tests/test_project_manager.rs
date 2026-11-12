//! Project records and their manager (spec `projects`, T-003).

#[path = "support/project_fixture.rs"]
mod project_fixture;

use project_fixture::{fixture_dir, git, init_repo, managers};
use ragent_agent::agent::ModelRef;
use ragent_agent::event::Event;
use ragent_agent::project::{RemoteKind, parse_model_ref};

#[test]
fn test_project_manager_resolve_is_stable_from_a_subdirectory() {
    let repo = fixture_dir("manager_resolve").join("storefront");
    init_repo(&repo);
    let sub = repo.join("src/deep");
    std::fs::create_dir_all(&sub).expect("subdir");
    let m = managers();

    let from_root = m.projects.resolve_for_dir(&repo).expect("root");
    let from_sub = m.projects.resolve_for_dir(&sub).expect("sub");

    assert_eq!(from_root.id, from_sub.id);
    assert_eq!(from_root.worktree, repo);
    assert_eq!(from_root.name, "storefront");
    assert_eq!(from_root.base_model, None);
    assert_eq!(from_root.remote, None);
    assert_eq!(m.projects.list().expect("list").len(), 1);
}

#[test]
fn test_project_manager_detects_the_remote_once_and_on_refresh() {
    let repo = fixture_dir("manager_remote").join("app");
    init_repo(&repo);
    git(
        &repo,
        &["remote", "add", "origin", "git@github.com:acme/app.git"],
    );
    let m = managers();

    let project = m.projects.resolve_for_dir(&repo).expect("resolve");
    let remote = project.remote.expect("remote detected on first use");
    assert_eq!(remote.kind, RemoteKind::GitHub);
    assert_eq!(remote.path, "acme/app");

    // The remote is not re-read on every resolve...
    git(
        &repo,
        &[
            "remote",
            "set-url",
            "origin",
            "git@gitlab.com:group/app.git",
        ],
    );
    let again = m.projects.resolve_for_dir(&repo).expect("resolve");
    assert_eq!(again.remote.expect("remote").kind, RemoteKind::GitHub);
    // ...only when asked.
    let refreshed = m.projects.refresh_remote(&project.id).expect("refresh");
    let remote = refreshed.remote.expect("remote");
    assert_eq!(remote.kind, RemoteKind::GitLab);
    assert_eq!(remote.path, "group/app");

    git(&repo, &["remote", "remove", "origin"]);
    let cleared = m.projects.refresh_remote(&project.id).expect("refresh");
    assert_eq!(cleared.remote, None);
}

#[test]
fn test_project_manager_edits_publish_project_updated() {
    let m = managers();
    let project = m
        .projects
        .resolve_for_dir(std::path::Path::new("/nonexistent/manager_edits"))
        .expect("resolve");
    let mut rx = m.events.subscribe();
    let mut updates = |rx: &mut tokio::sync::broadcast::Receiver<Event>| {
        let mut n = 0;
        while let Ok(event) = rx.try_recv() {
            if matches!(&event, Event::ProjectUpdated { project_id } if *project_id == project.id) {
                n += 1;
            }
        }
        n
    };

    m.projects.rename(&project.id, "Renamed").expect("rename");
    assert_eq!(updates(&mut rx), 1);
    let model = ModelRef {
        provider_id: "anthropic".to_string(),
        model_id: "claude-sonnet".to_string(),
    };
    m.projects
        .set_base_model(&project.id, Some(&model))
        .expect("model");
    assert_eq!(updates(&mut rx), 1);

    let stored = m.projects.get(&project.id).expect("get").expect("project");
    assert_eq!(stored.name, "Renamed");
    assert_eq!(stored.base_model, Some(model));
    assert_eq!(
        stored.base_model_label().as_deref(),
        Some("anthropic/claude-sonnet")
    );
    assert_eq!(
        m.projects.find("renamed").expect("find").map(|p| p.id),
        Some(project.id.clone())
    );

    m.projects.set_base_model(&project.id, None).expect("clear");
    assert_eq!(
        m.projects
            .get(&project.id)
            .expect("get")
            .expect("p")
            .base_model,
        None
    );
    m.projects.remove(&project.id).expect("remove");
    assert!(m.projects.list().expect("list").is_empty());
}

#[test]
fn test_project_manager_parse_model_ref() {
    let model = parse_model_ref(" openrouter/meta/llama-3 ").expect("parse");
    assert_eq!(model.provider_id, "openrouter");
    assert_eq!(model.model_id, "meta/llama-3");
    for bad in ["", "nomodel", "/model", "provider/"] {
        assert!(parse_model_ref(bad).is_err(), "{bad:?} must be rejected");
    }
}
