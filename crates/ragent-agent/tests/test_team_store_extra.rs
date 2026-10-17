//! Tests-only coverage for `TeamStore` lifecycle + naming helpers
//! (`ragent_agent::team::store`): slugify, list_teams, load/save (sync +
//! blocking), the `initialize_existing_without_config` recovery path, and the
//! `find_project_teams_dir` walk-up.

use ragent_agent::team::{TeamStore, find_project_teams_dir, global_teams_dir, slugify_team_name};

#[test]
fn slugify_team_name_normalises_arbitrary_input() {
    // Trailing punctuation becomes a trailing hyphen (interior symbols become
    // hyphens and are not trimmed).
    assert_eq!(slugify_team_name("My Team!"), "my-team-");
    assert_eq!(slugify_team_name("  padded  "), "padded");
    assert_eq!(slugify_team_name("a_b.c"), "a-b-c");
    // A leading non-alphanumeric is stripped; an all-symbol name falls back.
    assert_eq!(slugify_team_name("---"), "team");
    assert_eq!(slugify_team_name(""), "team");
    // Length is capped at 64 characters.
    assert_eq!(slugify_team_name(&"x".repeat(200)).len(), 64);
}

#[test]
fn find_project_teams_dir_walks_up_to_the_ragent_root() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    let nested = root.join("a").join("b").join("c");
    std::fs::create_dir_all(&nested).expect("nested");
    std::fs::create_dir_all(root.join(".ragent")).expect(".ragent");

    let found = find_project_teams_dir(&nested).expect("found from nested dir");
    assert_eq!(found, root.join(".ragent").join("teams"));

    // A tree without any `.ragent/` yields None.
    let bare = tmp.path().join("bare");
    std::fs::create_dir_all(&bare).expect("bare");
    assert!(find_project_teams_dir(&bare).is_none());
}

#[test]
fn list_teams_includes_project_teams_sorted() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let workspace = tmp.path().join("ws");
    std::fs::create_dir_all(workspace.join(".ragent")).expect(".ragent");

    TeamStore::create("beta-team", "lead", &workspace, true).expect("create beta");
    TeamStore::create("alpha-team", "lead", &workspace, true).expect("create alpha");

    let teams = TeamStore::list_teams(&workspace);
    let names: Vec<&str> = teams.iter().map(|(n, _, _)| n.as_str()).collect();
    // Project-local teams are present and the list is sorted alphabetically.
    assert!(names.contains(&"alpha-team"));
    assert!(names.contains(&"beta-team"));
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted, "list must be sorted");

    // The project-local entries are flagged `true`.
    let alpha = teams
        .iter()
        .find(|(n, _, _)| n == "alpha-team")
        .expect("alpha");
    assert!(alpha.2, "project teams are flagged project-local");
}

#[test]
fn load_and_save_round_trip_and_missing_dir_errors() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let workspace = tmp.path().join("ws");
    std::fs::create_dir_all(workspace.join(".ragent")).expect(".ragent");

    let mut store = TeamStore::create("round-trip", "lead", &workspace, true).expect("create");
    store
        .add_member(ragent_agent::team::TeamMember::new(
            "alice", "tm-001", "general",
        ))
        .expect("add member");

    let reloaded = TeamStore::load(&store.dir).expect("load");
    assert_eq!(reloaded.config.name, "round-trip");
    assert_eq!(reloaded.config.lead_session_id, "lead");
    assert_eq!(reloaded.config.members.len(), 1);

    // Loading a directory without a config.json fails.
    let empty = tmp.path().join("empty-team");
    std::fs::create_dir_all(&empty).expect("empty dir");
    assert!(TeamStore::load(&empty).is_err());
}

#[test]
fn load_by_name_resolves_a_project_team() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let workspace = tmp.path().join("ws");
    std::fs::create_dir_all(workspace.join(".ragent")).expect(".ragent");
    TeamStore::create("named-team", "lead", &workspace, true).expect("create");

    let loaded = TeamStore::load_by_name("named-team", &workspace).expect("load by name");
    assert_eq!(loaded.config.name, "named-team");

    assert!(
        TeamStore::load_by_name("ghost", &workspace).is_err(),
        "unknown name must error"
    );
}

#[test]
fn initialize_existing_without_config_recovers_a_partially_created_team() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let workspace = tmp.path().join("ws");
    let teams_dir = workspace.join(".ragent").join("teams");
    std::fs::create_dir_all(teams_dir.join("partial-team")).expect("partial dir");

    let store = TeamStore::initialize_existing_without_config("partial-team", "lead", &workspace)
        .expect("initialise");
    assert_eq!(store.config.name, "partial-team");
    assert!(store.dir.join("config.json").exists());
    assert!(store.dir.join("mailbox").is_dir());

    // Running it again fails because config.json now exists.
    assert!(
        TeamStore::initialize_existing_without_config("partial-team", "lead", &workspace).is_err()
    );
}

#[test]
fn next_agent_id_increments_from_existing_members() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let workspace = tmp.path().join("ws");
    std::fs::create_dir_all(workspace.join(".ragent")).expect(".ragent");
    let mut store = TeamStore::create("id-team", "lead", &workspace, true).expect("create");

    assert_eq!(store.next_agent_id(), "tm-001");
    store
        .add_member(ragent_agent::team::TeamMember::new(
            "a", "tm-003", "general",
        ))
        .expect("add");
    assert_eq!(store.next_agent_id(), "tm-004", "max numeric id + 1");
}

#[test]
fn global_teams_dir_is_under_the_config_home() {
    // Read-only: just assert the resolver returns a path ending in `teams`.
    if let Some(dir) = global_teams_dir() {
        assert_eq!(dir.file_name().and_then(|n| n.to_str()), Some("teams"));
    }
}

#[tokio::test]
async fn blocking_store_wrappers_round_trip() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let workspace = tmp.path().join("ws");
    std::fs::create_dir_all(workspace.join(".ragent")).expect(".ragent");
    let store = TeamStore::create("blocking-team", "lead", &workspace, true).expect("create");
    let dir = store.dir.clone();

    let loaded = TeamStore::load_blocking(dir.clone())
        .await
        .expect("load blocking");
    assert_eq!(loaded.config.name, "blocking-team");

    let by_name = TeamStore::load_by_name_blocking("blocking-team".to_string(), workspace.clone())
        .await
        .expect("load by name blocking");
    by_name.save_blocking().await.expect("save blocking");
}
