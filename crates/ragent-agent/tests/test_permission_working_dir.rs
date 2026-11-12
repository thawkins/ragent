//! `file:read` auto-grant follows the session's working directory
//! (spec MULTISESSION, milestone M6).
//!
//! A read of a file under the calling session's working directory is allowed
//! without a prompt; under nextest the process cwd is this crate's directory,
//! while the fixture lives in the workspace `target/temp/` tree, so the two
//! are deliberately different directories.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::RwLock;
use ragent_agent::event::EventBus;
use ragent_agent::permission::{PermissionAction, PermissionChecker};
use ragent_agent::session::permissions::check_permission_with_prompt;

/// A fresh fixture directory holding one file; returns `(dir, file)`.
fn fixture(name: &str) -> (PathBuf, PathBuf) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/permission_working_dir")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir); // INTENTIONAL: may not exist yet
    std::fs::create_dir_all(&dir).expect("fixture dir");
    let dir = dir.canonicalize().expect("canonical fixture");
    let file = dir.join("notes.txt");
    std::fs::write(&file, "hello").expect("fixture file");
    (dir, file)
}

/// Asks for `file:read` of `resource` from a session working in
/// `working_dir`, giving up after a short wait (a prompt would block).
async fn read_permission(resource: &Path, working_dir: &Path) -> Option<PermissionAction> {
    let checker = Arc::new(RwLock::new(PermissionChecker::new(Vec::new())));
    let bus = Arc::new(EventBus::new(16));
    let resource = resource.display().to_string();
    let call = check_permission_with_prompt(
        &checker,
        &bus,
        "s",
        "file:read",
        &resource,
        "read",
        Some(false),
        None,
        false,
        120,
        false,
        false,
        working_dir,
    );
    tokio::time::timeout(std::time::Duration::from_millis(300), call)
        .await
        .ok()
        .map(|result| result.expect("permission check"))
}

#[tokio::test]
async fn test_permission_working_dir_auto_grants_reads_under_session_dir() {
    let (dir, file) = fixture("inside");
    assert_eq!(
        read_permission(&file, &dir).await,
        Some(PermissionAction::Allow),
        "a read under the session's directory needs no prompt"
    );
}

#[tokio::test]
async fn test_permission_working_dir_does_not_grant_outside_session_dir() {
    let (_dir, file) = fixture("outside");
    let elsewhere = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .canonicalize()
        .expect("crate src");
    assert_eq!(
        read_permission(&file, &elsewhere).await,
        None,
        "a read outside the session's directory goes to the prompt"
    );
}

#[tokio::test]
async fn test_permission_working_dir_resolves_relative_reads_against_session_dir() {
    let (dir, _file) = fixture("relative");
    assert_eq!(
        read_permission(Path::new("notes.txt"), &dir).await,
        Some(PermissionAction::Allow),
        "a relative path is relative to the session's directory"
    );
    assert_eq!(
        read_permission(Path::new("."), &dir).await,
        Some(PermissionAction::Allow)
    );
}
