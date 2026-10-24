//! Guard test for spec `uniformtools` T-034 / FR-027 / NFR-008 predicate (e):
//! fail when a retired `before` spelling survives as a quoted or backticked
//! literal in non-test source.
//!
//! The predicate is exercised directly by invoking the hermetic guard
//! ``scripts/check_tool_rename.py`` as a subprocess (the same implementation
//! the CI job runs, NFR-009). A second half writes a temporary ``before``
//! literal into a sibling source file the guard does scan, invokes the guard
//! again, and asserts the output names both ``(e)`` and the seeded retired
//! spelling -- the failure must be predicate-labelled and ASCII, and must name
//! the offending file and line rather than merely exiting non-zero.

// Tests in this module touch the filesystem (a seeded source file the guard
// scans) and may run concurrently with each other inside the single
// `ragent-agent` test binary (AGENTS.md "Test Layout" — one binary per crate).
// ``#[serial]`` plus the single ``global_env_lock`` would be heavyweight for a
// sibling file that no other test reads; these two tests order only with each
// other, so they share a dedicated lock. See field ``GUARD_SEED_LOCK`` below.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

static GUARD_SEED_LOCK: Mutex<()> = Mutex::new(());

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn run_guard(extra_args: &[&str]) -> (i32, String) {
    let root = repo_root();
    let script = root.join("scripts/check_tool_rename.py");
    let mut cmd = Command::new("python3");
    cmd.arg(&script);
    for a in extra_args {
        cmd.arg(a);
    }
    cmd.current_dir(&root);
    let out = cmd.output().expect("failed to invoke guard script");
    let code = out.status.code().unwrap_or(99);
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    (code, stdout + &stderr)
}

/// RAII guard for the seeded fixture file so it is removed even when the
/// assertion that follows panics.
struct SeedFile {
    path: PathBuf,
}

impl SeedFile {
    fn create(root: &Path) -> Self {
        let path = root.join("crates/ragent-tools-core/src/__tool_rename_guard_selftest.rs");
        std::fs::write(
            &path,
            "// Self-test fixture for test_tool_rename_guard -- not shipped.\n\
             pub fn fake_dispatch(tool: &str) {\n    \
             if tool == \"bash\" {\n        \
             let _ = tool;\n    \
             }\n}\n",
        )
        .expect("failed to write seeded fixture");
        Self { path }
    }
}

impl Drop for SeedFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[test]
fn test_guard_passes_on_the_checked_in_tree() {
    let _lock = GUARD_SEED_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (code, combined) = run_guard(&[]);
    assert_eq!(
        code, 0,
        "the guard must pass on the checked-in tree, but it failed:\n{combined}"
    );
    // Diagnostics shall be ASCII (NFR-009 / FR-027's "A guard shall fail...").
    assert!(
        combined.is_ascii(),
        "guard output must be ASCII, but it contained non-ASCII bytes: {combined:?}"
    );
    // And the success message must name the predicate letter, not just "OK".
    assert!(
        combined.contains("(e)"),
        "guard success message must name the predicate letter (e): {combined:?}"
    );
}

#[test]
fn test_guard_fails_on_a_seeded_retired_literal() {
    let _lock = GUARD_SEED_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let root = repo_root();
    let _seed = SeedFile::create(&root);

    let (code, combined) = run_guard(&[]);
    // _seed's Drop removes the file even if an assertion below panics.

    assert_ne!(
        code, 0,
        "guard should fail on a seeded stale literal tool == \"bash\", but it passed"
    );
    // Failure diagnostics shall be ASCII, predicate-labelled, and name the
    // offending file + line (NFR-008/NFR-009).
    assert!(
        combined.is_ascii(),
        "guard failure output must be ASCII, but it was not: {combined:?}"
    );
    assert!(
        combined.contains("(e)"),
        "guard failure must name the predicate letter (e): {combined:?}"
    );
    assert!(
        combined.contains("before"),
        "guard failure must name the retired before spelling: {combined:?}"
    );
    // The seeded file must appear in the report (guard cannot merely exit 1
    // without naming the offender).
    assert!(
        combined.contains("__tool_rename_guard_selftest"),
        "guard failure must name the offending file: {combined:?}"
    );
}
