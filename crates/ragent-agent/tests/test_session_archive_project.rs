//! Session archives carry the project the session belonged to (spec
//! `projects`, T-013).

#[path = "support/project_fixture.rs"]
mod project_fixture;

use std::path::PathBuf;
use std::process::Command;

use project_fixture::{fixture_dir, managers};
use ragent_agent::project::{ProjectRemote, RemoteKind};
use ragent_agent::session::archive::{
    ArchiveConfig, ArchiveManifest, ImportConfig, export_session_archive, import_session_archive,
};
use ragent_storage::SessionCheckout;
use ragent_types::message::Message;

/// Extracts `manifest.json` from an archive into a fixture directory.
fn read_manifest(archive: &std::path::Path, into: &std::path::Path) -> ArchiveManifest {
    std::fs::create_dir_all(into).expect("extract dir");
    let status = Command::new("tar")
        .arg("-xzf")
        .arg(archive)
        .arg("-C")
        .arg(into)
        .status()
        .expect("run tar");
    assert!(status.success(), "tar failed");
    let manifest = std::fs::read_to_string(into.join("manifest.json")).expect("manifest.json");
    serde_json::from_str(&manifest).expect("manifest parses")
}

#[tokio::test]
async fn test_session_archive_project_export_records_project_remote_and_branch() {
    let base = fixture_dir("archive_project");
    let m = managers();
    let project = m
        .projects
        .resolve_for_dir(std::path::Path::new("/nonexistent/archive/storefront"))
        .expect("project");
    m.projects
        .rename(&project.id, "Storefront")
        .expect("rename");
    let remote = ProjectRemote {
        kind: RemoteKind::GitLab,
        host: "gitlab.com".to_string(),
        path: "group/storefront".to_string(),
        url: "https://gitlab.com/group/storefront.git".to_string(),
        remote_name: "origin".to_string(),
        default_branch: Some("main".to_string()),
    };
    m.storage
        .set_project_remote(&project.id, Some(&remote.to_row()))
        .expect("remote");
    let session = m
        .sessions
        .create_session_in_project(
            &project.id,
            PathBuf::from("/nonexistent/archive/trees/feature-x"),
            &SessionCheckout {
                branch: Some("feature/x".to_string()),
                worktree: Some("/nonexistent/archive/trees/feature-x".to_string()),
                pr_count: Some(2),
            },
        )
        .expect("session");
    m.storage
        .create_message(&Message::user_text(&session.id, "hello"))
        .expect("message");
    let archive = base.join("session.tar.gz");

    export_session_archive(
        &m.storage,
        None,
        &session.id,
        &archive,
        &ArchiveConfig::default(),
    )
    .expect("export");

    let manifest = read_manifest(&archive, &base.join("extracted"));
    let archived = manifest.project.expect("the manifest records the project");
    assert_eq!(archived.name, "Storefront");
    assert_eq!(archived.remote, Some(remote));
    assert_eq!(archived.branch.as_deref(), Some("feature/x"));

    // Importing attaches the new session to the project of the directory it
    // lands in (the archived directory is outside the working directory, so
    // that is the working directory), not to the archived project.
    let result = import_session_archive(
        &m.storage,
        None,
        &archive,
        &ImportConfig {
            verify_checksums: true,
            ..Default::default()
        },
    )
    .await
    .expect("import");
    let imported = m
        .storage
        .get_session(&result.session_id)
        .expect("get")
        .expect("row");
    assert_eq!(result.project_id, imported.project_id);
    let landed = m
        .projects
        .get(&result.project_id)
        .expect("get")
        .expect("the imported session has a project record");
    assert_eq!(
        landed.worktree,
        PathBuf::from(ragent_storage::project_root(&imported.directory))
    );
    assert_ne!(result.project_id, project.id);
}

#[test]
fn test_session_archive_project_manifest_without_a_project_still_parses() {
    // A manifest written before projects were stored.
    let legacy = r#"{
        "manifest_version": 1,
        "session_id": "s",
        "session_title": "t",
        "session_directory": "/tmp/x",
        "created_at": "2026-01-01T00:00:00Z",
        "message_count": 0,
        "trigger_count": 0,
        "cron_job_count": 0,
        "loop_state_count": 0,
        "files": {},
        "sensitivity_warning": "w"
    }"#;
    let manifest: ArchiveManifest = serde_json::from_str(legacy).expect("legacy manifest parses");
    assert_eq!(manifest.project, None);
    let written = serde_json::to_string(&manifest).expect("serialize");
    assert!(
        !written.contains("\"project\""),
        "an absent project is not written: {written}"
    );
}
