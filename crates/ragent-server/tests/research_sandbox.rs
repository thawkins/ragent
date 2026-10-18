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
/// anything written here is invisible to `git status`. The directory is wiped
/// once per test process so a repeated run starts clean, then left for
/// inspection after the run (it lives under `target/temp/`, not the repo).
fn sandbox_once() -> &'static PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("target")
            .join("temp")
            .join("ragent-server-research");
        // The first caller wipes any stale tree from an earlier run.
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::create_dir_all(&root);
        root
    })
}

/// The research vault root tests hand to the router (`<sandbox>/research`).
///
/// The parent of this path is the project root for `from_files` containment
/// (SEC-002), so the sandbox also holds the `log/research` gather logs a run
/// writes.
#[must_use]
pub fn research_root() -> PathBuf {
    sandbox_once().join("research")
}
