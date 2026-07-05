//! Shared test helpers for the memory subsystem.
//!
//! Provides [`setup_temp_dir`] — a temp-directory helper used across inline
//! `#[cfg(test)]` modules in `migrate.rs`, `storage.rs`, and
//! `cross_project.rs`.  Previously copy-pasted into each test module (see
//! `DUPPLAN.md` Milestone I, `cargo dupes` group 33).

use tempfile::TempDir;

/// Create a temporary directory for testing.
///
/// Returns a `TempDir` that is automatically cleaned up when dropped.
#[must_use]
pub fn setup_temp_dir() -> TempDir {
    TempDir::new().unwrap()
}
