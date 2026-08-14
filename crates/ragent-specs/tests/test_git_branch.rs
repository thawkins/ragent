//! Tests for the branch-per-spec git workflow (FR-009, T-018).

use ragent_specs::SpecCommand;
use ragent_specs::git::{BranchResult, create_spec_branch, spec_branch_name};
use std::fs;
use std::process::Command;
use tempfile::TempDir;

// ── spec_branch_name ────────────────────────────────────────────────────

#[test]
fn test_spec_branch_name_basic() {
    assert_eq!(spec_branch_name("my-feature"), "spec/my-feature");
}

#[test]
fn test_spec_branch_name_single_word() {
    assert_eq!(spec_branch_name("auth"), "spec/auth");
}

#[test]
fn test_spec_branch_name_with_underscores() {
    assert_eq!(spec_branch_name("user_auth_fix"), "spec/user_auth_fix");
}

// ── create_spec_branch: not a repo ──────────────────────────────────────

#[test]
fn test_create_spec_branch_not_a_repo() {
    let tmp = TempDir::new().expect("tempdir");
    // No git init — should return NotARepo.
    let result = create_spec_branch("test-spec", tmp.path());
    assert_eq!(result, BranchResult::NotARepo);
}

// ── create_spec_branch: success ─────────────────────────────────────────

#[test]
fn test_create_spec_branch_success() {
    let tmp = TempDir::new().expect("tempdir");
    init_git_repo(tmp.path());

    let result = create_spec_branch("my-feature", tmp.path());
    assert_eq!(
        result,
        BranchResult::Created {
            branch_name: "spec/my-feature".to_string(),
        }
    );

    // Verify the branch was actually created and checked out.
    let current = current_branch(tmp.path());
    assert_eq!(current, Some("spec/my-feature".to_string()));
}

// ── create_spec_branch: already exists ──────────────────────────────────

#[test]
fn test_create_spec_branch_already_exists() {
    let tmp = TempDir::new().expect("tempdir");
    init_git_repo(tmp.path());

    // Create the branch first.
    Command::new("git")
        .args(["branch", "spec/existing"])
        .current_dir(tmp.path())
        .output()
        .expect("git branch");

    let result = create_spec_branch("existing", tmp.path());
    assert_eq!(
        result,
        BranchResult::AlreadyExists {
            branch_name: "spec/existing".to_string(),
        }
    );
}

// ── SpecCommand helper functions ────────────────────────────────────────

#[test]
fn test_build_branch_message_created() {
    let result = BranchResult::Created {
        branch_name: "spec/my-feature".to_string(),
    };
    let msg = SpecCommand::build_branch_message(&result);
    assert!(msg.contains("spec/my-feature"));
    assert!(msg.contains("created"));
}

#[test]
fn test_build_branch_message_not_a_repo() {
    let result = BranchResult::NotARepo;
    let msg = SpecCommand::build_branch_message(&result);
    assert!(msg.contains("Not a git repository"));
}

#[test]
fn test_build_branch_message_already_exists() {
    let result = BranchResult::AlreadyExists {
        branch_name: "spec/auth".to_string(),
    };
    let msg = SpecCommand::build_branch_message(&result);
    assert!(msg.contains("spec/auth"));
    assert!(msg.contains("already exists"));
}

#[test]
fn test_build_branch_message_failed() {
    let result = BranchResult::Failed {
        msg: "some error".to_string(),
    };
    let msg = SpecCommand::build_branch_message(&result);
    assert!(msg.contains("some error"));
}

#[test]
fn test_build_branch_log_created() {
    let result = BranchResult::Created {
        branch_name: "spec/my-feature".to_string(),
    };
    let log = SpecCommand::build_branch_log("my-feature", &result);
    assert!(log.contains("spec/my-feature"));
    assert!(log.contains("Created"));
}

#[test]
fn test_build_branch_log_not_a_repo() {
    let result = BranchResult::NotARepo;
    let log = SpecCommand::build_branch_log("my-feature", &result);
    assert!(log.contains("No git repo"));
    assert!(log.contains("my-feature"));
}

#[test]
fn test_build_branch_log_already_exists() {
    let result = BranchResult::AlreadyExists {
        branch_name: "spec/auth".to_string(),
    };
    let log = SpecCommand::build_branch_log("auth", &result);
    assert!(log.contains("already exists"));
    assert!(log.contains("auth"));
}

#[test]
fn test_build_branch_log_failed() {
    let result = BranchResult::Failed {
        msg: "boom".to_string(),
    };
    let log = SpecCommand::build_branch_log("auth", &result);
    assert!(log.contains("failed"));
    assert!(log.contains("boom"));
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Initialise a minimal git repo in `dir` with one commit on `main`.
fn init_git_repo(dir: &std::path::Path) {
    Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(dir)
        .output()
        .expect("git init");

    // Set a local identity so commits succeed.
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(dir)
        .output()
        .expect("git config email");
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir)
        .output()
        .expect("git config name");

    // Create an initial commit so there is a HEAD to branch from.
    fs::write(dir.join("README.md"), "# test\n").expect("write readme");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(dir)
        .output()
        .expect("git add");
    Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(dir)
        .output()
        .expect("git commit");
}

/// Return the name of the currently checked-out branch, or `None`.
fn current_branch(dir: &std::path::Path) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(dir)
        .output()
        .expect("git rev-parse");
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}
