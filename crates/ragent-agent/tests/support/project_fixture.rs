//! Git fixtures for the `projects` tests.
//!
//! Every repository is created under the git-ignored `target/temp/projects/`
//! tree in a directory unique to the calling test, so parallel test
//! processes never share one and nothing is written into the repository.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use ragent_agent::event::EventBus;
use ragent_agent::project::ProjectManager;
use ragent_agent::session::SessionManager;
use ragent_storage::Storage;

/// A fresh, empty directory unique to `name`.
pub fn fixture_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/projects")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir); // INTENTIONAL: may not exist yet
    std::fs::create_dir_all(&dir).expect("create fixture dir");
    dir.canonicalize().expect("canonical fixture dir")
}

/// Runs git in `dir` with a fixed identity and no user or system config,
/// panicking with git's stderr on failure. Returns trimmed stdout.
pub fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.invalid")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.invalid")
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Creates a repository at `dir` with one commit on `main`.
pub fn init_repo(dir: &Path) {
    std::fs::create_dir_all(dir).expect("repo dir");
    git(dir, &["init", "--quiet", "--initial-branch=main"]);
    std::fs::write(dir.join("README.md"), "fixture\n").expect("write file");
    git(dir, &["add", "README.md"]);
    git(dir, &["commit", "--quiet", "-m", "initial"]);
}

/// Storage, session manager and project manager over one in-memory database.
pub struct Managers {
    pub storage: Arc<Storage>,
    pub events: Arc<EventBus>,
    pub sessions: SessionManager,
    pub projects: ProjectManager,
}

pub fn managers() -> Managers {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let events = Arc::new(EventBus::new(64));
    Managers {
        sessions: SessionManager::new(Arc::clone(&storage), Arc::clone(&events)),
        projects: ProjectManager::new(Arc::clone(&storage), Arc::clone(&events)),
        storage,
        events,
    }
}
