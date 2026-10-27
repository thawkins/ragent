//! Test-only sandbox paths for the research routes.
//!
//! The research HTTP routes operate on a vault root supplied through
//! [`ragent_server::routes::AppState::research_root`]. Tests point that root at
//! `target/temp/` so a test run never rewrites the checked-in
//! `crates/ragent-server/research/` tree (`INDEX.md`, `.index.json`) and never
//! leaves per-run item directories behind in the repository.
//!
//! This module is declared once in `tests/all.rs`, so every child test module
//! reaches it as `crate::research_sandbox::...`.

use std::path::PathBuf;
use std::sync::OnceLock;

/// Root of the research sandbox inside the workspace `target/` directory.
///
/// `target/` is git-ignored and already the documented scratch location, so
/// anything written here is invisible to `git status`. The directory is created
/// once per test process and then left for inspection after the run (it lives
/// under `target/temp/`, not the repo). It is never wiped: see the comment on
/// the `create_dir_all` below.
fn sandbox_once() -> &'static PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("target")
            .join("temp")
            .join("ragent-server-research");
        // The first caller creates the sandbox; it never wipes it. `target/temp`
        // is workspace-wide, so other test processes of this binary (and other
        // crates) share it: a `remove_dir_all` here deletes the `isolated/` trees
        // of every test already running in this process, and - because the
        // process-wide `OnceLock` makes the wipe "once per process" rather than
        // once per test - it lands in the middle of a run. Each caller's own
        // path is made unique by pid plus a counter instead.
        let _ = std::fs::create_dir_all(&root);
        root
    })
}

/// A private research vault root for one test.
///
/// `ResearchManager::refresh_index` rebuilds `INDEX.md` and `.index.json` from a
/// listing of the *whole* vault, so every mutation republishes the shared
/// root's index. Two tests that create items in one root therefore overwrite
/// each other's index - and once a test runs in its own process (`cargo
/// nextest` runs every test that way, and this crate's tests share one process
/// under `cargo test`) the two really do interleave. A per-test root means no
/// item and no index is ever shared, so nothing has to be serialised.
#[must_use]
pub fn isolated_research_root() -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);

    let root = sandbox_once().join("isolated").join(format!(
        "run-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    let research = root.join("research");
    let _ = std::fs::create_dir_all(&research);
    research
}
