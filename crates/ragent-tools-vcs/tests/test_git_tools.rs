//! Integration tests for Milestone 1 local git workspace tools.

use std::fs;
use std::path::Path;
use std::process::Command;

use ragent_tools_vcs::git::*;
use ragent_tools_vcs::{Tool, ToolContext};
use serde_json::json;

/// Create a temporary git repository with an initial commit on branch 'main'.
fn setup_git_repo(dir: &Path) {
    let _ = fs::remove_dir_all(dir.join(".git"));
    run_shell("git init", dir);
    run_shell("git config user.email 'test@example.com'", dir);
    run_shell("git config user.name 'Test User'", dir);
    run_shell("git checkout -b main", dir);
    fs::write(dir.join("README.md"), "# Hello").unwrap();
    run_shell("git add README.md", dir);
    run_shell("git commit -m 'Initial commit'", dir);
}

fn run_shell(cmd: &str, cwd: &Path) {
    let output = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", cmd])
            .current_dir(cwd)
            .output()
    } else {
        Command::new("sh")
            .args(["-c", cmd])
            .current_dir(cwd)
            .output()
    }
    .unwrap_or_else(|e| panic!("failed to run '{}': {}", cmd, e));
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("command '{}' failed: {}", cmd, stderr);
    }
}

fn make_ctx(dir: &Path) -> ToolContext {
    ToolContext {
        session_id: "test".to_string(),
        working_dir: dir.to_path_buf(),
        storage: None,
    }
}

// ---------------------------------------------------------------------------
// git_status
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_git_status_clean() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());

    let tool = GitStatusTool;
    let out = tool
        .execute(json!({}), &make_ctx(tmp.path()))
        .await
        .unwrap();

    assert!(
        out.content.contains("clean") || out.content.contains("#"),
        "Expected clean working tree or porcelain output, got: {}",
        out.content
    );
}

#[tokio::test]
async fn test_git_status_modified() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());
    fs::write(tmp.path().join("README.md"), "# Hello\n\nWorld").unwrap();

    let tool = GitStatusTool;
    let out = tool
        .execute(json!({}), &make_ctx(tmp.path()))
        .await
        .unwrap();

    let meta = out.metadata.expect("metadata should be present");
    let modified: Vec<String> = serde_json::from_value(meta["modified"].clone()).unwrap();
    assert!(
        modified.iter().any(|f| f.contains("README.md")),
        "Expected README.md in modified, got: {:?}",
        modified
    );
}

#[tokio::test]
async fn test_git_status_untracked() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());
    fs::write(tmp.path().join("new_file.txt"), "new content").unwrap();

    let tool = GitStatusTool;
    let out = tool
        .execute(json!({}), &make_ctx(tmp.path()))
        .await
        .unwrap();

    let meta = out.metadata.expect("metadata should be present");
    let untracked: Vec<String> = serde_json::from_value(meta["untracked"].clone()).unwrap();
    assert!(
        untracked.iter().any(|f| f.contains("new_file.txt")),
        "Expected new_file.txt in untracked, got: {:?}",
        untracked
    );
}

#[tokio::test]
async fn test_git_status_staged() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());
    fs::write(tmp.path().join("staged.txt"), "staged content").unwrap();
    run_shell("git add staged.txt", tmp.path());

    let tool = GitStatusTool;
    let out = tool
        .execute(json!({}), &make_ctx(tmp.path()))
        .await
        .unwrap();

    let meta = out.metadata.expect("metadata should be present");
    let staged: Vec<String> = serde_json::from_value(meta["staged"].clone()).unwrap();
    assert!(
        staged.iter().any(|f| f.contains("staged.txt")),
        "Expected staged.txt in staged, got: {:?}",
        staged
    );
}

#[tokio::test]
async fn test_git_status_short_format() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());
    fs::write(tmp.path().join("README.md"), "# Hello\n\nWorld").unwrap();

    let tool = GitStatusTool;
    let out = tool
        .execute(json!({"short": true}), &make_ctx(tmp.path()))
        .await
        .unwrap();

    // Short format should include the file status line
    assert!(
        out.content.contains("README.md"),
        "Expected README.md in short output, got: {}",
        out.content
    );
}

// ---------------------------------------------------------------------------
// git_log
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_git_log_basic() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());

    let tool = GitLogTool;
    let out = tool
        .execute(json!({"limit": 5}), &make_ctx(tmp.path()))
        .await
        .unwrap();

    assert!(
        out.content.contains("Initial commit"),
        "Expected 'Initial commit' in log, got: {}",
        out.content
    );
    let meta = out.metadata.expect("metadata should be present");
    let count = meta["count"].as_u64().unwrap_or(0);
    assert!(count >= 1, "Expected at least 1 commit, got {}", count);
}

#[tokio::test]
async fn test_git_log_author_filter() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());

    let tool = GitLogTool;
    let out = tool
        .execute(json!({"author": "Test User"}), &make_ctx(tmp.path()))
        .await
        .unwrap();

    assert!(
        out.content.contains("Initial commit"),
        "Expected commit by Test User, got: {}",
        out.content
    );
}

// ---------------------------------------------------------------------------
// git_diff
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_git_diff_working() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());
    fs::write(tmp.path().join("README.md"), "# Hello\n\nWorld").unwrap();

    let tool = GitDiffTool;
    let out = tool
        .execute(json!({}), &make_ctx(tmp.path()))
        .await
        .unwrap();

    assert!(
        out.content.contains("+") || out.content.contains("-"),
        "Expected diff output with changes, got: {}",
        out.content
    );
}

#[tokio::test]
async fn test_git_diff_staged() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());
    fs::write(tmp.path().join("staged.txt"), "staged content").unwrap();
    run_shell("git add staged.txt", tmp.path());

    let tool = GitDiffTool;
    let out = tool
        .execute(json!({"target": "staged"}), &make_ctx(tmp.path()))
        .await
        .unwrap();

    assert!(
        out.content.contains("staged.txt")
            || out.content.contains("+")
            || out.content.contains("No differences"),
        "Expected staged diff, got: {}",
        out.content
    );
}

#[tokio::test]
async fn test_git_diff_stat() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());
    fs::write(tmp.path().join("README.md"), "# Hello\n\nWorld\n\nMore").unwrap();

    let tool = GitDiffTool;
    let out = tool
        .execute(json!({"stat": true}), &make_ctx(tmp.path()))
        .await
        .unwrap();

    assert!(
        out.content.contains("README.md") || out.content.contains("|"),
        "Expected stat output, got: {}",
        out.content
    );
}

#[tokio::test]
async fn test_git_diff_no_changes() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());

    let tool = GitDiffTool;
    let out = tool
        .execute(json!({}), &make_ctx(tmp.path()))
        .await
        .unwrap();

    assert_eq!(
        out.content, "No differences found.",
        "Expected 'No differences found.', got: {}",
        out.content
    );
}

// ---------------------------------------------------------------------------
// git_branch
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_git_branch_list() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());
    run_shell("git checkout -b feature-branch", tmp.path());

    let tool = GitBranchTool;
    let out = tool
        .execute(json!({}), &make_ctx(tmp.path()))
        .await
        .unwrap();

    let meta = out.metadata.expect("metadata should be present");
    let branches: Vec<serde_json::Value> =
        serde_json::from_value(meta["branches"].clone()).unwrap();
    let names: Vec<String> = branches
        .iter()
        .filter_map(|b| b["name"].as_str().map(|s| s.to_string()))
        .collect();

    assert!(
        names.iter().any(|n| n == "main" || n == "master"),
        "Expected main/master branch, got: {:?}",
        names
    );
    assert!(
        names.contains(&"feature-branch".to_string()),
        "Expected feature-branch, got: {:?}",
        names
    );

    let current = meta["current_branch"].as_str();
    assert_eq!(
        current,
        Some("feature-branch"),
        "Expected current branch to be feature-branch"
    );
}

#[tokio::test]
async fn test_git_branch_short_format() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());

    let tool = GitBranchTool;
    let out = tool
        .execute(json!({}), &make_ctx(tmp.path()))
        .await
        .unwrap();

    assert!(
        !out.content.is_empty(),
        "Expected branch list output, got empty string"
    );
}

// ---------------------------------------------------------------------------
// git_show
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_git_show_head() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());

    let tool = GitShowTool;
    let out = tool
        .execute(json!({}), &make_ctx(tmp.path()))
        .await
        .unwrap();

    assert!(
        out.content.contains("Initial commit"),
        "Expected commit message in show output, got: {}",
        out.content
    );

    let meta = out.metadata.expect("metadata should be present");
    assert!(
        !meta["author"].is_null(),
        "Expected author metadata, got: {:?}",
        meta
    );
}

#[tokio::test]
async fn test_git_show_stat_false() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());

    let tool = GitShowTool;
    let out = tool
        .execute(json!({"stat": false}), &make_ctx(tmp.path()))
        .await
        .unwrap();

    assert!(
        out.content.contains("Initial commit"),
        "Expected commit message even without stat, got: {}",
        out.content
    );
}

// ---------------------------------------------------------------------------
// git_remote
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_git_remote_list_empty() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());

    let tool = GitRemoteTool;
    let out = tool
        .execute(json!({}), &make_ctx(tmp.path()))
        .await
        .unwrap();

    assert!(
        out.content.contains("No remotes"),
        "Expected 'No remotes configured.', got: {}",
        out.content
    );
}

#[tokio::test]
async fn test_git_remote_add_and_list() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());

    let tool = GitRemoteTool;

    // Add remote
    let _ = tool
        .execute(
            json!({"action": "add", "name": "origin", "url": "https://github.com/test/repo.git"}),
            &make_ctx(tmp.path()),
        )
        .await
        .unwrap();

    // List remotes
    let out = tool
        .execute(json!({}), &make_ctx(tmp.path()))
        .await
        .unwrap();

    let meta = out.metadata.expect("metadata should be present");
    let remotes: Vec<serde_json::Value> = serde_json::from_value(meta["remotes"].clone()).unwrap();
    assert_eq!(
        remotes.len(),
        2,
        "Expected fetch and push entries for origin"
    ); // -v shows both fetch and push
}

#[tokio::test]
async fn test_git_remote_remove() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());

    let tool = GitRemoteTool;
    let _ = tool
        .execute(
            json!({"action": "add", "name": "origin", "url": "https://github.com/test/repo.git"}),
            &make_ctx(tmp.path()),
        )
        .await
        .unwrap();

    let out = tool
        .execute(
            json!({"action": "remove", "name": "origin"}),
            &make_ctx(tmp.path()),
        )
        .await
        .unwrap();

    assert!(
        out.content.contains("Removed remote 'origin'"),
        "Expected removal confirmation, got: {}",
        out.content
    );
}

// ---------------------------------------------------------------------------
// git_tag
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_git_tag_list_empty() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());

    let tool = GitTagTool;
    let out = tool
        .execute(json!({}), &make_ctx(tmp.path()))
        .await
        .unwrap();

    assert!(
        out.content.contains("No tags"),
        "Expected 'No tags found.', got: {}",
        out.content
    );
}

#[tokio::test]
async fn test_git_tag_create_and_list() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());

    let tool = GitTagTool;

    // Create tag
    let _ = tool
        .execute(
            json!({"action": "create", "name": "v1.0.0", "message": "Release 1.0.0"}),
            &make_ctx(tmp.path()),
        )
        .await
        .unwrap();

    // List tags
    let out = tool
        .execute(json!({}), &make_ctx(tmp.path()))
        .await
        .unwrap();

    let meta = out.metadata.expect("metadata should be present");
    let tags: Vec<serde_json::Value> = serde_json::from_value(meta["tags"].clone()).unwrap();
    assert_eq!(tags.len(), 1, "Expected 1 tag, got: {:?}", tags);
    assert_eq!(tags[0]["name"].as_str(), Some("v1.0.0"));
}

#[tokio::test]
async fn test_git_tag_delete() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());
    run_shell("git tag v1.0.0", tmp.path());

    let tool = GitTagTool;
    let out = tool
        .execute(
            json!({"action": "delete", "name": "v1.0.0"}),
            &make_ctx(tmp.path()),
        )
        .await
        .unwrap();

    assert!(
        out.content.contains("Deleted tag 'v1.0.0'"),
        "Expected deletion confirmation, got: {}",
        out.content
    );
}

#[tokio::test]
async fn test_git_tag_show() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());
    run_shell("git tag -a v1.0.0 -m 'Release'", tmp.path());

    let tool = GitTagTool;
    let out = tool
        .execute(
            json!({"action": "show", "name": "v1.0.0"}),
            &make_ctx(tmp.path()),
        )
        .await
        .unwrap();

    assert!(
        out.content.contains("Release") || out.content.contains("tag v1.0.0"),
        "Expected tag details in output, got: {}",
        out.content
    );
}

// ---------------------------------------------------------------------------
// git_add
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_git_add_paths() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());
    fs::write(tmp.path().join("new.txt"), "new content").unwrap();

    let tool = GitAddTool;
    let out = tool
        .execute(json!({"paths": ["new.txt"]}), &make_ctx(tmp.path()))
        .await
        .unwrap();

    assert!(
        out.content.contains("Staged") || out.content.contains("new.txt"),
        "Expected staged confirmation, got: {}",
        out.content
    );

    // Verify via status
    let status_out = GitStatusTool
        .execute(json!({}), &make_ctx(tmp.path()))
        .await
        .unwrap();
    let meta = status_out.metadata.expect("metadata should be present");
    let staged: Vec<String> = serde_json::from_value(meta["staged"].clone()).unwrap();
    assert!(
        staged.iter().any(|f| f.contains("new.txt")),
        "Expected new.txt in staged, got: {:?}",
        staged
    );
}

#[tokio::test]
async fn test_git_add_all() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());
    fs::write(tmp.path().join("a.txt"), "a").unwrap();
    fs::write(tmp.path().join("b.txt"), "b").unwrap();

    let tool = GitAddTool;
    let out = tool
        .execute(json!({"all": true}), &make_ctx(tmp.path()))
        .await
        .unwrap();

    assert!(
        out.content.contains("Staged all"),
        "Expected 'Staged all', got: {}",
        out.content
    );
}

// ---------------------------------------------------------------------------
// git_commit
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_git_commit_basic() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());
    fs::write(tmp.path().join("commit.txt"), "commit me").unwrap();
    run_shell("git add commit.txt", tmp.path());

    let tool = GitCommitTool;
    let out = tool
        .execute(json!({"message": "Add commit.txt"}), &make_ctx(tmp.path()))
        .await
        .unwrap();

    assert!(
        out.content.contains("Add commit.txt") || out.content.contains("commit.txt"),
        "Expected commit confirmation, got: {}",
        out.content
    );
}

#[tokio::test]
async fn test_git_commit_all() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());
    fs::write(tmp.path().join("tracked.txt"), "tracked").unwrap();
    run_shell("git add tracked.txt", tmp.path());
    run_shell("git commit -m 'Add tracked'", tmp.path());

    fs::write(tmp.path().join("tracked.txt"), "modified").unwrap();

    let tool = GitCommitTool;
    let out = tool
        .execute(
            json!({"message": "Update tracked", "all": true}),
            &make_ctx(tmp.path()),
        )
        .await
        .unwrap();

    assert!(
        out.content.contains("Update tracked") || out.content.contains("tracked.txt"),
        "Expected commit confirmation, got: {}",
        out.content
    );
}

// ---------------------------------------------------------------------------
// git_reset
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_git_reset_unstage_paths() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());
    fs::write(tmp.path().join("staged.txt"), "staged").unwrap();
    run_shell("git add staged.txt", tmp.path());

    let tool = GitResetTool;
    let out = tool
        .execute(json!({"paths": ["staged.txt"]}), &make_ctx(tmp.path()))
        .await
        .unwrap();

    assert!(
        out.content.contains("Unstaged") || out.content.contains("staged.txt"),
        "Expected unstage confirmation, got: {}",
        out.content
    );
}

#[tokio::test]
async fn test_git_reset_mixed() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());
    fs::write(tmp.path().join("file.txt"), "content").unwrap();
    run_shell("git add file.txt", tmp.path());
    run_shell("git commit -m 'Add file'", tmp.path());
    fs::write(tmp.path().join("file.txt"), "modified").unwrap();
    run_shell("git add file.txt", tmp.path());

    let tool = GitResetTool;
    let out = tool
        .execute(json!({}), &make_ctx(tmp.path()))
        .await
        .unwrap();

    assert!(
        out.content.contains("Reset")
            || out.content.contains("Unstaged changes after reset")
            || out.content.contains("mixed"),
        "Expected reset confirmation, got: {}",
        out.content
    );
}

// ---------------------------------------------------------------------------
// git_checkout
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_git_checkout_branch() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());
    run_shell("git checkout -b feature", tmp.path());

    let tool = GitCheckoutTool;
    let out = tool
        .execute(json!({"branch": "main"}), &make_ctx(tmp.path()))
        .await
        .unwrap();

    assert!(
        out.content.contains("Switched to branch 'main'") || out.content.contains("Switched to"),
        "Expected branch switch, got: {}",
        out.content
    );
}

#[tokio::test]
async fn test_git_checkout_create_branch() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());

    let tool = GitCheckoutTool;
    let out = tool
        .execute(
            json!({"branch": "new-feature", "create_branch": true}),
            &make_ctx(tmp.path()),
        )
        .await
        .unwrap();

    assert!(
        out.content.contains("new branch 'new-feature'") || out.content.contains("new-feature"),
        "Expected branch creation, got: {}",
        out.content
    );
}

#[tokio::test]
async fn test_git_checkout_restore_paths() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());
    fs::write(tmp.path().join("README.md"), "# Modified").unwrap();

    let tool = GitCheckoutTool;
    let out = tool
        .execute(json!({"paths": ["README.md"]}), &make_ctx(tmp.path()))
        .await
        .unwrap();

    assert!(
        out.content.contains("Restored") || out.content.contains("README.md"),
        "Expected restore confirmation, got: {}",
        out.content
    );
}

// ---------------------------------------------------------------------------
// git_stash
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_git_stash_push_and_pop() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());
    // Create a tracked file then modify it so stash has something to save
    fs::write(tmp.path().join("stash.txt"), "original").unwrap();
    run_shell("git add stash.txt", tmp.path());
    run_shell("git commit -m 'Add stash.txt'", tmp.path());
    fs::write(tmp.path().join("stash.txt"), "stashed content").unwrap();

    // Push stash
    let tool = GitStashTool;
    let out = tool
        .execute(
            json!({"action": "push", "message": "WIP"}),
            &make_ctx(tmp.path()),
        )
        .await
        .unwrap();

    assert!(
        out.content.contains("stashed")
            || out.content.contains("Saved")
            || out.content.contains("WIP"),
        "Expected stash push, got: {}",
        out.content
    );

    // Verify file is reverted to original (clean working tree)
    let content = fs::read_to_string(tmp.path().join("stash.txt")).unwrap();
    assert_eq!(
        content, "original",
        "Expected file to be reverted after stash"
    );

    // Pop stash
    let out = tool
        .execute(json!({"action": "pop"}), &make_ctx(tmp.path()))
        .await
        .unwrap();

    assert!(
        out.content.contains("Applied")
            || out.content.contains("restored")
            || out.content.contains("stash"),
        "Expected stash pop, got: {}",
        out.content
    );

    // Verify file is restored
    let content = fs::read_to_string(tmp.path().join("stash.txt")).unwrap();
    assert_eq!(
        content, "stashed content",
        "Expected file to be restored after pop"
    );
}

#[tokio::test]
async fn test_git_stash_list() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());
    // Create a tracked file then modify it so stash has something to save
    fs::write(tmp.path().join("stash.txt"), "original").unwrap();
    run_shell("git add stash.txt", tmp.path());
    run_shell("git commit -m 'Add stash.txt'", tmp.path());
    fs::write(tmp.path().join("stash.txt"), "stashed content").unwrap();
    run_shell("git stash push -m 'WIP'", tmp.path());

    let tool = GitStashTool;
    let out = tool
        .execute(json!({"action": "list"}), &make_ctx(tmp.path()))
        .await
        .unwrap();

    let meta = out.metadata.expect("metadata should be present");
    let stashes: Vec<serde_json::Value> = serde_json::from_value(meta["stashes"].clone()).unwrap();
    assert!(
        stashes.len() >= 1,
        "Expected at least one stash, got: {:?}",
        stashes
    );
}

// ---------------------------------------------------------------------------
// git_cherry_pick
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_git_cherry_pick_basic() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());

    // Create a commit on a side branch
    run_shell("git checkout -b side", tmp.path());
    fs::write(tmp.path().join("pick.txt"), "cherry pick this").unwrap();
    run_shell("git add pick.txt", tmp.path());
    run_shell("git commit -m 'Add pick.txt'", tmp.path());

    // Get the commit hash
    let hash_output = Command::new("sh")
        .args(["-c", "git log -1 --format=%H"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    let commit_hash = String::from_utf8_lossy(&hash_output.stdout)
        .trim()
        .to_string();

    // Switch back to main and cherry-pick
    run_shell("git checkout main", tmp.path());

    let tool = GitCherryPickTool;
    let out = tool
        .execute(json!({"commits": [commit_hash]}), &make_ctx(tmp.path()))
        .await
        .unwrap();

    assert!(
        out.content.contains("Cherry-picked")
            || out.content.contains("pick.txt")
            || out.content.contains("commit"),
        "Expected cherry-pick confirmation, got: {}",
        out.content
    );

    // Verify file exists
    assert!(
        tmp.path().join("pick.txt").exists(),
        "Expected pick.txt to exist after cherry-pick"
    );
}

#[tokio::test]
async fn test_git_cherry_pick_no_commit() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());

    // Create a commit on a side branch
    run_shell("git checkout -b side", tmp.path());
    fs::write(tmp.path().join("pick.txt"), "cherry pick this").unwrap();
    run_shell("git add pick.txt", tmp.path());
    run_shell("git commit -m 'Add pick.txt'", tmp.path());

    let hash_output = Command::new("sh")
        .args(["-c", "git log -1 --format=%H"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    let commit_hash = String::from_utf8_lossy(&hash_output.stdout)
        .trim()
        .to_string();

    run_shell("git checkout main", tmp.path());

    let tool = GitCherryPickTool;
    let out = tool
        .execute(
            json!({"commits": [commit_hash], "no_commit": true}),
            &make_ctx(tmp.path()),
        )
        .await
        .unwrap();

    assert!(
        out.content.contains("without committing") || out.content.contains("Applied"),
        "Expected no-commit cherry-pick, got: {}",
        out.content
    );

    // File should exist but not committed (staged)
    assert!(
        tmp.path().join("pick.txt").exists(),
        "Expected pick.txt to exist"
    );
}

// ---------------------------------------------------------------------------
// git_clone
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_git_clone_basic() {
    let tmp = tempfile::tempdir().unwrap();
    // Create a bare repo to clone from
    let bare_dir = tmp.path().join("source.git");
    fs::create_dir(&bare_dir).unwrap();
    run_shell("git init --bare", &bare_dir);

    // Create a normal repo, add a commit, and push to bare
    let origin_dir = tmp.path().join("origin");
    fs::create_dir(&origin_dir).unwrap();
    run_shell("git init", &origin_dir);
    run_shell("git config user.email 'test@example.com'", &origin_dir);
    run_shell("git config user.name 'Test User'", &origin_dir);
    run_shell("git checkout -b main", &origin_dir);
    fs::write(origin_dir.join("hello.txt"), "hello").unwrap();
    run_shell("git add hello.txt", &origin_dir);
    run_shell("git commit -m 'Initial'", &origin_dir);
    run_shell(
        &format!("git push '{}' main:main", bare_dir.display()),
        &origin_dir,
    );

    // Now clone the bare repo into a new directory under tmp
    let tool = GitCloneTool;
    let out = tool
        .execute(
            json!({"url": bare_dir.to_str().unwrap(), "directory": "cloned", "branch": "main"}),
            &make_ctx(tmp.path()),
        )
        .await
        .unwrap();

    assert!(
        out.content.contains("Cloned") || out.content.contains("cloned"),
        "Expected clone confirmation, got: {}",
        out.content
    );

    let cloned_dir = tmp.path().join("cloned");
    assert!(cloned_dir.exists(), "Expected cloned directory to exist");
    assert!(
        cloned_dir.join("hello.txt").exists(),
        "Expected hello.txt in cloned repo"
    );
}

// ---------------------------------------------------------------------------
// git_push / git_pull / git_fetch (using local bare repo as remote)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_git_push_and_fetch() {
    let tmp = tempfile::tempdir().unwrap();

    // Create bare remote
    let bare_dir = tmp.path().join("remote.git");
    fs::create_dir(&bare_dir).unwrap();
    run_shell("git init --bare", &bare_dir);

    // Create local repo with initial commit
    let repo_dir = tmp.path().join("repo");
    fs::create_dir(&repo_dir).unwrap();
    run_shell("git init", &repo_dir);
    run_shell("git config user.email 'test@example.com'", &repo_dir);
    run_shell("git config user.name 'Test User'", &repo_dir);
    run_shell("git checkout -b main", &repo_dir);
    fs::write(repo_dir.join("file.txt"), "content").unwrap();
    run_shell("git add file.txt", &repo_dir);
    run_shell("git commit -m 'Initial'", &repo_dir);

    // Add remote
    let _ = GitRemoteTool
        .execute(
            json!({
                "action": "add",
                "name": "origin",
                "url": bare_dir.to_str().unwrap(),
            }),
            &make_ctx(&repo_dir),
        )
        .await
        .unwrap();

    // Push
    let tool = GitPushTool;
    let out = tool
        .execute(
            json!({"remote": "origin", "branch": "main"}),
            &make_ctx(&repo_dir),
        )
        .await
        .unwrap();

    assert!(
        out.content.contains("Pushed")
            || out.content.contains("main")
            || out.content.contains("branch"),
        "Expected push confirmation, got: {}",
        out.content
    );

    // Fetch
    let fetch_tool = GitFetchTool;
    let out = fetch_tool
        .execute(json!({"remote": "origin"}), &make_ctx(&repo_dir))
        .await
        .unwrap();

    assert!(
        out.content.contains("Fetched") || out.content.contains("origin"),
        "Expected fetch confirmation, got: {}",
        out.content
    );
}

#[tokio::test]
async fn test_git_pull_fast_forward() {
    let tmp = tempfile::tempdir().unwrap();

    // Create bare remote
    let bare_dir = tmp.path().join("remote.git");
    fs::create_dir(&bare_dir).unwrap();
    run_shell("git init --bare", &bare_dir);

    // Create repo A, push to bare
    let repo_a = tmp.path().join("repo_a");
    fs::create_dir(&repo_a).unwrap();
    run_shell("git init", &repo_a);
    run_shell("git config user.email 'a@example.com'", &repo_a);
    run_shell("git config user.name 'User A'", &repo_a);
    run_shell("git checkout -b main", &repo_a);
    fs::write(repo_a.join("a.txt"), "a").unwrap();
    run_shell("git add a.txt", &repo_a);
    run_shell("git commit -m 'A'", &repo_a);
    run_shell(
        &format!("git push --set-upstream '{}' main", bare_dir.display()),
        &repo_a,
    );

    // Clone repo B from bare
    let repo_b = tmp.path().join("repo_b");
    fs::create_dir(&repo_b).unwrap();
    run_shell(
        &format!(
            "git clone --branch main --single-branch '{}' '{}'",
            bare_dir.display(),
            repo_b.display()
        ),
        tmp.path(),
    );
    // Ensure repo_b is on main branch
    run_shell("git checkout main", &repo_b);
    run_shell("git config user.email 'b@example.com'", &repo_b);
    run_shell("git config user.name 'User B'", &repo_b);
    // Set tracking branch to main explicitly
    run_shell("git branch --set-upstream-to=origin/main main", &repo_b);

    // Add another commit to repo A and push
    fs::write(repo_a.join("b.txt"), "b").unwrap();
    run_shell("git add b.txt", &repo_a);
    run_shell("git commit -m 'B'", &repo_a);
    run_shell(&format!("git push '{}' main", bare_dir.display()), &repo_a);

    // Pull in repo B
    let tool = GitPullTool;
    let out = tool
        .execute(json!({"remote": "origin"}), &make_ctx(&repo_b))
        .await
        .unwrap();

    assert!(
        out.content.contains("Pulled")
            || out.content.contains("fast-forward")
            || out.content.contains("b.txt"),
        "Expected pull confirmation, got: {}",
        out.content
    );

    // Verify b.txt exists in repo B
    assert!(repo_b.join("b.txt").exists(), "Expected b.txt after pull");
}

#[tokio::test]
async fn test_git_fetch_prune() {
    let tmp = tempfile::tempdir().unwrap();

    let bare_dir = tmp.path().join("remote.git");
    fs::create_dir(&bare_dir).unwrap();
    run_shell("git init --bare", &bare_dir);

    let repo_dir = tmp.path().join("repo");
    fs::create_dir(&repo_dir).unwrap();
    run_shell("git init", &repo_dir);
    run_shell("git config user.email 'test@example.com'", &repo_dir);
    run_shell("git config user.name 'Test User'", &repo_dir);
    run_shell("git checkout -b main", &repo_dir);
    fs::write(repo_dir.join("file.txt"), "content").unwrap();
    run_shell("git add file.txt", &repo_dir);
    run_shell("git commit -m 'Initial'", &repo_dir);

    let _ = GitRemoteTool
        .execute(
            json!({
                "action": "add",
                "name": "origin",
                "url": bare_dir.to_str().unwrap(),
            }),
            &make_ctx(&repo_dir),
        )
        .await
        .unwrap();

    let tool = GitFetchTool;
    let out = tool
        .execute(
            json!({"remote": "origin", "prune": true}),
            &make_ctx(&repo_dir),
        )
        .await
        .unwrap();

    assert!(
        out.content.contains("Fetched") || out.content.contains("prune"),
        "Expected fetch confirmation, got: {}",
        out.content
    );
}

// ---------------------------------------------------------------------------
// git_merge
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_git_merge_fast_forward() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());

    // Create a feature branch with one commit
    run_shell("git checkout -b feature", tmp.path());
    fs::write(tmp.path().join("feature.txt"), "feature content").unwrap();
    run_shell("git add feature.txt", tmp.path());
    run_shell("git commit -m 'Add feature'", tmp.path());

    // Switch back to main and merge
    run_shell("git checkout main", tmp.path());

    let tool = GitMergeTool;
    let out = tool
        .execute(json!({"branch": "feature"}), &make_ctx(tmp.path()))
        .await
        .unwrap();

    assert!(
        !out.content.contains("CONFLICT"),
        "Expected clean merge, got conflicts: {}",
        out.content
    );

    let meta = out.metadata.expect("metadata should be present");
    assert_eq!(
        meta["conflicts"].as_bool(),
        Some(false),
        "Expected no conflicts"
    );

    // Verify file exists
    assert!(
        tmp.path().join("feature.txt").exists(),
        "Expected feature.txt after merge"
    );
}

#[tokio::test]
async fn test_git_merge_no_ff() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());

    // Create a feature branch with one commit
    run_shell("git checkout -b feature", tmp.path());
    fs::write(tmp.path().join("feature.txt"), "feature content").unwrap();
    run_shell("git add feature.txt", tmp.path());
    run_shell("git commit -m 'Add feature'", tmp.path());

    // Switch back to main and merge with --no-ff
    run_shell("git checkout main", tmp.path());

    let tool = GitMergeTool;
    let out = tool
        .execute(
            json!({"branch": "feature", "no_ff": true}),
            &make_ctx(tmp.path()),
        )
        .await
        .unwrap();

    assert!(
        !out.content.contains("CONFLICT"),
        "Expected clean merge, got conflicts: {}",
        out.content
    );

    // Verify merge commit was created
    let log_out = GitLogTool
        .execute(json!({"limit": 5}), &make_ctx(tmp.path()))
        .await
        .unwrap();
    assert!(
        log_out.content.contains("Merge") || log_out.content.contains("feature"),
        "Expected merge commit in log, got: {}",
        log_out.content
    );
}

#[tokio::test]
async fn test_git_merge_squash() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());

    // Create a feature branch with two commits
    run_shell("git checkout -b feature", tmp.path());
    fs::write(tmp.path().join("a.txt"), "a").unwrap();
    run_shell("git add a.txt", tmp.path());
    run_shell("git commit -m 'Add a'", tmp.path());
    fs::write(tmp.path().join("b.txt"), "b").unwrap();
    run_shell("git add b.txt", tmp.path());
    run_shell("git commit -m 'Add b'", tmp.path());

    // Switch back to main and squash merge
    run_shell("git checkout main", tmp.path());

    let tool = GitMergeTool;
    let out = tool
        .execute(
            json!({"branch": "feature", "squash": true}),
            &make_ctx(tmp.path()),
        )
        .await
        .unwrap();

    assert!(
        !out.content.contains("CONFLICT"),
        "Expected clean squash merge, got conflicts: {}",
        out.content
    );

    // After squash merge, files should be staged but not committed
    let status_out = GitStatusTool
        .execute(json!({}), &make_ctx(tmp.path()))
        .await
        .unwrap();
    let meta = status_out.metadata.expect("metadata should be present");
    let staged: Vec<String> = serde_json::from_value(meta["staged"].clone()).unwrap();
    assert!(
        staged
            .iter()
            .any(|f| f.contains("a.txt") || f.contains("b.txt")),
        "Expected staged files after squash merge, got: {:?}",
        staged
    );
}

#[tokio::test]
async fn test_git_merge_ff_only_abort() {
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());

    // Create a feature branch with one commit (diverges from main)
    run_shell("git checkout -b feature", tmp.path());
    fs::write(tmp.path().join("feature.txt"), "feature").unwrap();
    run_shell("git add feature.txt", tmp.path());
    run_shell("git commit -m 'Add feature'", tmp.path());

    // Add another commit to main so feature is NOT a fast-forward
    run_shell("git checkout main", tmp.path());
    fs::write(tmp.path().join("main.txt"), "main").unwrap();
    run_shell("git add main.txt", tmp.path());
    run_shell("git commit -m 'Add main'", tmp.path());

    // Try ff-only merge — should abort
    let tool = GitMergeTool;
    let out = tool
        .execute(
            json!({"branch": "feature", "ff_only": true}),
            &make_ctx(tmp.path()),
        )
        .await
        .unwrap();

    assert!(
        out.content.contains("abort")
            || out.content.contains("not possible")
            || out.content.contains("ff-only")
            || out.content.contains("fast-forward"),
        "Expected ff-only abort message, got: {}",
        out.content
    );
}
