//! Tests for T-008: local git init + initial commit (FR-008 local half,
//! FR-015 tolerance) and the FR-011 summary report.

use std::fs;
use std::path::Path;
use std::process::Command;

use ragent_tools_extended::project_scaffold::{
    AppType, EmitReport, FileStatus, GitFailure, GitInitReport, GitStep, Language, RemoteStatus,
    ScaffoldSummary, init_and_commit,
};

/// Fresh temp root for one test.
fn temp_root(name: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(name)
        .tempdir()
        .expect("tempdir")
}

/// Run a git command in `root`, returning combined stdout+stderr.
fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .expect("git spawn");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    format!("{stdout}{stderr}")
}

/// Commit-tree readback helper: subject, tracked file list, clean flag.
fn commit_state(root: &Path) -> (String, Vec<String>, bool) {
    let subject = git(root, &["log", "-1", "--pretty=%s"]);
    let files = git(root, &["ls-files"]);
    let status = git(root, &["status", "--porcelain"]);
    let tracked = files
        .lines()
        .map(str::to_owned)
        .filter(|s| !s.is_empty())
        .collect();
    (subject, tracked, status.trim().is_empty())
}

/// Deterministic identity for commit assertions (tests may run without any
/// global git identity; the scaffold fallback also satisfies git).
fn pin_identity(root: &Path) {
    git(root, &["config", "user.name", "test-runner"]);
    git(root, &["config", "user.email", "test@example.invalid"]);
}

/// Strip a possible `\[` escape some git versions print in log subjects.
fn normalise(subject: &str) -> String {
    subject.replace('\\', "")
}

// --------------------------------------------------------------- git init ---

#[test]
fn test_git_init_creates_repo_and_initial_commit() {
    let root = temp_root("git-init-basic");
    fs::write(root.path().join("main.rs"), b"fn main() {}\n").expect("write");
    fs::create_dir(root.path().join("log")).expect("mkdir");

    let report = init_and_commit(root.path(), "Initial scaffold").expect("init+commit");

    assert!(report.repo_created);
    assert!(report.committed);
    assert!(root.path().join(".git").is_dir());
    let (subject, tracked, clean) = commit_state(root.path());
    assert_eq!(normalise(&subject), "Initial scaffold\n");
    assert!(tracked.contains(&"main.rs".to_owned()));
    // log/ is a tracked-irrelevant empty dir; .gitignore content checks live
    // in the emitter tests. Clean tree after commit.
    assert!(clean);
}

#[test]
fn test_git_init_respects_gitignore_from_scaffold() {
    let root = temp_root("git-init-ignore");
    fs::write(root.path().join("Cargo.toml"), b"[package]\n").expect("write");
    // The scaffold-generated .gitignore excludes log/ and target/ (FR-004).
    fs::write(root.path().join(".gitignore"), b"log/\ntarget/\n.ragent/\n").expect("write");
    fs::create_dir_all(root.path().join("target/debug")).expect("mkdir");
    fs::write(root.path().join("target/debug/junk.bin"), b"x").expect("write");
    fs::create_dir(root.path().join("log")).expect("mkdir");

    init_and_commit(root.path(), "Initial scaffold").expect("init+commit");

    let (_, tracked, _) = commit_state(root.path());
    assert!(tracked.contains(&"Cargo.toml".to_owned()));
    assert!(tracked.contains(&".gitignore".to_owned()));
    assert!(!tracked.iter().any(|f| f.starts_with("target/")));
}

#[test]
fn test_git_init_reuses_existing_repo_without_reinit() {
    let root = temp_root("git-init-existing");
    git(root.path(), &["init"]);
    pin_identity(root.path());
    fs::write(root.path().join("seed.txt"), b"seed\n").expect("write");
    git(root.path(), &["add", "-A"]);
    git(root.path(), &["commit", "-m", "seed commit"]);

    fs::write(root.path().join("later.txt"), b"later\n").expect("write");
    let report = init_and_commit(root.path(), "Initial scaffold").expect("init+commit");

    assert!(!report.repo_created, "existing repo must be reused");
    assert!(report.committed);
    let log = git(root.path(), &["log", "--pretty=%s"]);
    // Seed commit history preserved (no re-init), new commit on top.
    assert!(log.contains("seed commit"));
    assert!(normalise(&log).contains("Initial scaffold"));
}

#[test]
fn test_git_init_retries_cleanly_on_partial_state() {
    // FR-015: previous run left a repo without a commit (partial state).
    let root = temp_root("git-init-partial");
    git(root.path(), &["init"]);
    fs::write(root.path().join("README.md"), b"hi\n").expect("write");

    let report = init_and_commit(root.path(), "Initial scaffold").expect("init+commit");

    assert!(!report.repo_created);
    assert!(report.committed);
    let (subject, _, clean) = commit_state(root.path());
    assert_eq!(normalise(&subject), "Initial scaffold\n");
    assert!(clean);
}

#[test]
fn test_git_init_clean_tree_reports_committed_false() {
    // FR-015 retry on a fully-committed tree: not an error.
    let root = temp_root("git-init-clean");
    git(root.path(), &["init"]);
    pin_identity(root.path());
    fs::write(root.path().join("a.txt"), b"a\n").expect("write");
    git(root.path(), &["add", "-A"]);
    git(root.path(), &["commit", "-m", "first"]);

    let report = init_and_commit(root.path(), "Initial scaffold").expect("clean-tree pass");

    assert!(!report.repo_created);
    assert!(!report.committed);
    let log = git(root.path(), &["log", "--oneline"]);
    assert_eq!(log.lines().count(), 1, "no extra commit");
}

#[test]
fn test_git_init_commit_works_without_global_identity() {
    // The scaffold-local identity fallback must make the commit succeed even
    // when no user.name/user.email resolves (sandboxed environments).
    let root = temp_root("git-init-noidentity");
    fs::write(root.path().join("x.txt"), b"x\n").expect("write");
    // Best-effort: strip any inherited identity for this repo only.
    git(root.path(), &["config", "--unset", "user.name"]);
    git(root.path(), &["config", "--unset", "user.email"]);
    // Note: global config may still resolve; the assertion below tolerates
    // both paths — the commit must not fail regardless.

    let report = init_and_commit(root.path(), "Initial scaffold").expect("init+commit");
    assert!(report.committed);
    assert!(root.path().join(".git").is_dir());
}

#[test]
fn test_git_failure_carries_step_and_message() {
    let failure = GitFailure {
        step: GitStep::Stage,
        message: "fatal: bad object\n".to_owned(),
    };
    let rendered = failure.to_string();
    assert!(rendered.contains("git add"));
    assert!(rendered.contains("fatal: bad object"));
}

#[test]
fn test_git_step_names() {
    assert_eq!(GitStep::Init.as_str(), "git init");
    assert_eq!(GitStep::Stage.as_str(), "git add");
    assert_eq!(GitStep::Commit.as_str(), "git commit");
}

#[test]
fn test_git_report_defaults() {
    let report = GitInitReport {
        repo_created: true,
        committed: true,
    };
    assert!(report.repo_created && report.committed);
}

// --------------------------------------------------------- summary (FR-011) --

#[test]
fn test_summary_renders_created_files_and_selection() {
    let mut emit = EmitReport::default();
    emit.record(FileStatus::Created, "Cargo.toml");
    emit.record(FileStatus::Created, "src/main.rs");
    let summary = ScaffoldSummary::from_reports(
        "myproj",
        std::path::PathBuf::from("/tmp/myproj"),
        Language::Rust,
        AppType::Cmdline,
        None,
        emit,
        Some(Ok(GitInitReport {
            repo_created: true,
            committed: true,
        })),
    );
    let text = summary.render();
    assert!(text.contains("Scaffolded myproj (rust, cmdline)"));
    assert!(text.contains("Created 2 file(s):"));
    assert!(text.contains("  + Cargo.toml"));
    assert!(text.contains("  + src/main.rs"));
    assert!(text.contains("Remote: none"));
    assert!(text.contains("initialised new repository"));
    assert!(text.contains("initial commit created"));
}

#[test]
fn test_summary_lists_skipped_existing_files() {
    let mut emit = EmitReport::default();
    emit.record(FileStatus::SkippedExisting, "AGENTS.md");
    let summary = ScaffoldSummary::from_reports(
        "myproj",
        std::path::PathBuf::from("/tmp/myproj"),
        Language::Python,
        AppType::Library,
        None,
        emit,
        None,
    );
    let text = summary.render();
    assert!(text.contains("Skipped existing (left untouched): AGENTS.md"));
    assert!(text.contains("Git: not run"));
}

#[test]
fn test_summary_reports_stack_and_remote_states() {
    let base = |remote: RemoteStatus| {
        let summary = ScaffoldSummary {
            project_name: "app".to_owned(),
            target: std::path::PathBuf::from("/tmp/app"),
            language: Language::Go,
            app_type: AppType::Tui,
            stack: Some("axum".to_owned()),
            emit: EmitReport::default(),
            git: Some(Err(GitFailure {
                step: GitStep::Commit,
                message: "nothing staged\n".to_owned(),
            })),
            remote,
            stopped_early: Some("remote push failed".to_owned()),
        };
        summary.render()
    };
    let failed = base(RemoteStatus::Failed {
        step: "push".to_owned(),
        message: "auth required".to_owned(),
    });
    assert!(failed.contains("stack: axum"));
    assert!(failed.contains("Remote: failed at push: auth required"));
    assert!(failed.contains("Stopped early: remote push failed"));
    let created = base(RemoteStatus::Created {
        url: "git@gitlab.com:me/app.git".to_owned(),
    });
    assert!(created.contains("Remote: git@gitlab.com:me/app.git"));
}

#[test]
fn test_summary_reports_partial_completion() {
    let mut emit = EmitReport::default();
    emit.record(FileStatus::Created, "Cargo.toml");
    emit.record(FileStatus::Created, "src/lib.rs");
    let mut summary = ScaffoldSummary::from_reports(
        "lib",
        std::path::PathBuf::from("/tmp/lib"),
        Language::Rust,
        AppType::Library,
        None,
        emit,
        Some(Ok(GitInitReport {
            repo_created: false,
            committed: false,
        })),
    );
    summary.stopped_early = Some("emission failed at src/lib.rs".to_owned());
    let text = summary.render();
    assert!(text.contains("Stopped early: emission failed at src/lib.rs"));
    assert!(text.contains("reused existing repository"));
    assert!(text.contains("nothing to commit (clean tree)"));
}
