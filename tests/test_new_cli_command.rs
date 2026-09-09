//! Integration tests for the FR-013 CLI parity surface (spec `newproj`
//! T-012): `ragent new …` and the `ragent run "/new …"` interception must
//! behave identically to the TUI `/new` slash command.
//!
//! Each test spawns the built `ragent` binary inside a fresh empty tempdir
//! (via [`std::env::set_current_dir`] in the test process) and asserts on
//! exit codes, stdout/stderr, and the resulting file tree. Hosting flags are
//! never exercised here with network credentials: the child environment has
//! GitHub/GitLab tokens stripped so a hosting run would fail contained
//! (FR-010) rather than touch the network.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Mutex, MutexGuard};

/// Serialise tests that change the process working directory. A poisoned
/// lock (a panicking sibling) must not cascade into unrelated failures.
static CWD_LOCK: Mutex<()> = Mutex::new(());

fn cwd_lock() -> MutexGuard<'static, ()> {
    CWD_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Restores the previous cwd on drop so a panicking closure cannot leave the
/// process cwd inside a deleted tempdir.
struct CwdGuard {
    prev: PathBuf,
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.prev);
    }
}

/// Run `ragent` with `args` inside a fresh empty tempdir.
fn ragent_in_empty_cwd(args: &[&str]) -> (PathBuf, Output) {
    let _lock = cwd_lock();
    let temp = tempfile::tempdir().expect("tempdir");
    let prev = std::env::current_dir().expect("cwd");
    std::env::set_current_dir(temp.path()).expect("set cwd");
    let _guard = CwdGuard { prev };

    // Strip hosting credentials so no test can reach the network; a hosting
    // invocation fails contained at the auth step instead (FR-010).
    let output = Command::new(env!("CARGO_BIN_EXE_ragent"))
        .args(args)
        .env_remove("GITHUB_TOKEN")
        .env_remove("GITLAB_TOKEN")
        .env_remove("GITLAB_URL")
        .output()
        .expect("spawn ragent");
    let dir = temp.path().to_path_buf();
    // Keep the tempdir alive until after assertions by leaking it; the test
    // process is short-lived and the tree is small.
    std::mem::forget(temp);
    (dir, output)
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn assert_exists(dir: &Path, rel: &str) {
    assert!(
        dir.join(rel).exists(),
        "expected {rel} to exist after scaffold"
    );
}

#[test]
fn test_new_cli_bare_invocation_prints_usage_and_registry_values() {
    let (_dir, out) = ragent_in_empty_cwd(&["new"]);
    assert!(
        out.status.success(),
        "bare `ragent new` shows usage, stderr: {}",
        stderr(&out)
    );
    let text = stdout(&out);
    assert!(text.contains("Usage:"));
    assert!(text.contains("--language"));
    assert!(text.contains("--type"));
    // NFR-001: registry-derived value lists, not hardcoded prose.
    assert!(text.contains("rust, python, go, typescript"));
    assert!(text.contains("library, cmdline, tui, gui"));
}

#[test]
fn test_new_cli_help_word_prints_detailed_help_page() {
    // FR-018: `ragent new help` prints the dedicated help page — purpose,
    // per-argument docs with optionality/values/omitted defaults, and two
    // worked examples (one minimal, one with a hosting flag).
    let (_dir, out) = ragent_in_empty_cwd(&["new", "help"]);
    assert!(
        out.status.success(),
        "help exits 0, stderr: {}",
        stderr(&out)
    );
    let text = stdout(&out);

    assert!(text.contains("Purpose:"), "FR-018 purpose: {text}");
    assert!(text.contains("Scaffold a brand-new project"));
    assert!(text.contains("Required. The computer language to scaffold"));
    assert!(text.contains("Optional flag. Creates a private GitHub repository"));
    assert!(text.contains("Omitted: no remote is created and nothing is pushed"));
    // NFR-001 registry-derived values and known stacks.
    assert!(text.contains("Accepted values: rust, python, go, typescript"));
    assert!(text.contains("rust: axum, warp, raylib, gtk4, ratatui"));
    // Worked examples in the binary surface spelling.
    assert!(text.contains("Worked examples:"), "FR-018 examples: {text}");
    assert!(text.contains("ragent new --language rust --type cmdline"));
    assert!(text.contains("ragent new --language python --type library --gitlab"));
    // Zero file mutations: help never scaffolds.
    assert!(!_dir.join("Cargo.toml").exists());
}

#[test]
fn test_new_cli_missing_required_flags_fails_with_usage() {
    let (_dir, out) = ragent_in_empty_cwd(&["new", "--language", "rust"]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "missing --type must exit 2 (FR-003)"
    );
    assert!(stderr(&out).contains("missing required flag --type"));
    assert!(stderr(&out).contains("Usage:"));
}

#[test]
fn test_new_cli_unknown_language_value_fails_before_any_writes() {
    let (dir, out) = ragent_in_empty_cwd(&["new", "--language", "cobol", "--type", "cmdline"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("unknown --language value 'cobol'"));
    // FR-003: validation aborts with zero file mutations (only the ragent
    // config bootstrap may exist).
    assert!(!dir.join("Cargo.toml").exists());
    assert!(!dir.join("src").exists());
}

#[test]
fn test_new_cli_hosting_conflict_aborts_without_scaffolding() {
    let (dir, out) = ragent_in_empty_cwd(&[
        "new",
        "--language",
        "rust",
        "--type",
        "cmdline",
        "--github",
        "--gitlab",
    ]);
    assert_ne!(out.status.code(), Some(0), "FR-009 conflict must fail");
    assert!(
        !dir.join("Cargo.toml").exists(),
        "conflict abort must not scaffold"
    );
}

#[test]
fn test_new_cli_non_empty_directory_refused() {
    let _lock = cwd_lock();
    let temp = tempfile::tempdir().expect("tempdir");
    let prev = std::env::current_dir().expect("cwd");
    std::env::set_current_dir(temp.path()).expect("set cwd");
    let _guard = CwdGuard { prev };
    std::fs::write(temp.path().join("BLOCKER.txt"), "keep out").expect("write blocker");

    let out = Command::new(env!("CARGO_BIN_EXE_ragent"))
        .args(["new", "--language", "rust", "--type", "cmdline"])
        .env_remove("GITHUB_TOKEN")
        .env_remove("GITLAB_TOKEN")
        .env_remove("GITLAB_URL")
        .output()
        .expect("spawn ragent");

    assert_eq!(
        out.status.code(),
        Some(1),
        "FR-002 guard refusal must exit 1"
    );
    assert!(stderr(&out).contains("not empty"));
    assert!(stderr(&out).contains("BLOCKER.txt"));
    assert!(
        !temp.path().join("Cargo.toml").exists(),
        "guard refusal must not scaffold"
    );
}

#[test]
fn test_new_cli_scaffolds_go_library_per_tc013() {
    // TESTPLAN TC-013: `ragent run "/new --language go --type library"`
    // parity via the dedicated CLI surface.
    let (dir, out) = ragent_in_empty_cwd(&["new", "--language", "go", "--type", "library"]);
    assert!(
        out.status.success(),
        "scaffold should succeed, stderr: {}",
        stderr(&out)
    );
    let text = stdout(&out);
    assert!(text.contains("Scaffolded"), "summary on stdout: {text}");
    assert!(text.contains("Remote: none"), "no hosting flags: {text}");

    // Go library layout: go.mod present, no binary entrypoint.
    assert_exists(&dir, "go.mod");
    assert!(
        !dir.join("main.go").exists(),
        "library type must not emit a main.go binary entry"
    );

    // FR-004 ragent workspace layout.
    assert_exists(&dir, ".ragent/agents/README.md");
    assert_exists(&dir, "specs/README.md");
    assert_exists(&dir, ".gitignore");
    assert_exists(&dir, "AGENTS.md");

    // FR-019 documentation layer.
    assert_exists(&dir, "README.md");
    assert_exists(&dir, "QUICKSTART.md");
    assert_exists(&dir, "STATS.md");
    assert_exists(&dir, "docs/README.md");

    // FR-008 local half: git repo initialised with an initial commit.
    let head = Command::new("git")
        .args(["-C"])
        .arg(&dir)
        .arg("rev-parse")
        .arg("--verify")
        .arg("HEAD")
        .output()
        .expect("git rev-parse");
    assert!(head.status.success(), "git repo must have a commit");
}

#[test]
fn test_run_prompt_new_interception_matches_cli_surface() {
    // FR-013: `ragent run "/new …"` reaches the same behaviour.
    let (dir, out) =
        ragent_in_empty_cwd(&["run", "/new --language rust --type cmdline", "--no-tui"]);
    assert!(
        out.status.success(),
        "run-intercepted scaffold should succeed, stderr: {}",
        stderr(&out)
    );
    let text = stdout(&out);
    assert!(text.contains("Scaffolded"), "summary on stdout: {text}");
    assert_exists(&dir, "Cargo.toml");
    assert_exists(&dir, "src/main.rs");
    assert_exists(&dir, "AGENTS.md");
    // The prompt must NOT have been sent to the LLM: no session transcript
    // echo, just the scaffold summary.
    assert!(!text.contains("assistant"));
}

#[test]
fn test_run_prompt_new_validation_error_is_contained() {
    let (dir, out) = ragent_in_empty_cwd(&["run", "/new --language rust", "--no-tui"]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "validation failure exits 2 through the run interception too"
    );
    assert!(stderr(&out).contains("missing required flag --type"));
    assert!(!dir.join("Cargo.toml").exists());
}
